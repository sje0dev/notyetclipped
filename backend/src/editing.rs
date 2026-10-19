//! Streamer-specific reframing. Jobs snapshot the layout and retain the original clip.
use std::{path::Path, time::Duration};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{events, jobs, media, services, state::State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    fn validate(&self) -> Result<()> {
        ensure!(
            (0.0..=1.0).contains(&self.x)
                && (0.0..=1.0).contains(&self.y)
                && (0.02..=1.0).contains(&self.width)
                && (0.02..=1.0).contains(&self.height)
                && self.x + self.width <= 1.000001
                && self.y + self.height <= 1.000001,
            "Ausschnitt muss vollständig im Quellbild liegen (mindestens 2% Breite/Höhe)."
        );
        Ok(())
    }
    fn crop(&self) -> String {
        format!(
            "crop=w='max(2,trunc(iw*{:.6}/2)*2)':h='max(2,trunc(ih*{:.6}/2)*2)':x='trunc(iw*{:.6}/2)*2':y='trunc(ih*{:.6}/2)*2'",
            self.width, self.height, self.x, self.y
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub auto_queue: bool,
    pub framing: String,
    pub gameplay: Rect,
    pub facecam_enabled: bool,
    pub facecam: Rect,
    pub facecam_position: String,
    pub facecam_percent: u32,
    pub chat_mode: String,
    pub chat: Rect,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            auto_queue: false,
            framing: "fit".into(),
            gameplay: Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            facecam_enabled: false,
            facecam: Rect {
                x: 0.,
                y: 0.,
                width: 0.25,
                height: 0.25,
            },
            facecam_position: "top".into(),
            facecam_percent: 25,
            chat_mode: "keep".into(),
            chat: Rect {
                x: 0.75,
                y: 0.,
                width: 0.25,
                height: 0.5,
            },
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        self.gameplay.validate()?;
        self.facecam.validate()?;
        self.chat.validate()?;
        ensure!(
            ["fit", "fill"].contains(&self.framing.as_str())
                && ["top", "bottom"].contains(&self.facecam_position.as_str())
                && ["keep", "hide", "panel"].contains(&self.chat_mode.as_str())
                && (15..=40).contains(&self.facecam_percent),
            "Ungültiges Hochkant-Layout."
        );
        Ok(())
    }

    pub fn filter(&self) -> Result<String> {
        self.validate()?;
        let cam_height = if self.facecam_enabled {
            (1280 * self.facecam_percent / 100) / 2 * 2
        } else {
            0
        };
        let chat_height = if self.chat_mode == "panel" { 256 } else { 0 };
        let game_height = 1280 - cam_height - chat_height;
        let mut branches = vec!["[game0]"];
        if self.facecam_enabled {
            branches.push("[cam0]");
        }
        if chat_height > 0 {
            branches.push("[chat0]");
        }
        let mut graph = if branches.len() > 1 {
            format!("[0:v]split={}{};", branches.len(), branches.join(""))
        } else {
            "[0:v]null[game0];".into()
        };
        graph.push_str("[game0]");
        if self.chat_mode != "keep" {
            graph.push_str(&format!(
                "drawbox=x=iw*{:.6}:y=ih*{:.6}:w=iw*{:.6}:h=ih*{:.6}:color=black:t=fill,",
                self.chat.x, self.chat.y, self.chat.width, self.chat.height
            ));
        }
        let resize = |height: u32, fill: bool| -> String {
            if fill {
                format!(
                    "scale=720:{height}:force_original_aspect_ratio=increase,crop=720:{height},setsar=1"
                )
            } else {
                format!(
                    "scale=720:{height}:force_original_aspect_ratio=decrease,pad=720:{height}:(ow-iw)/2:(oh-ih)/2:black,setsar=1"
                )
            }
        };
        graph.push_str(&format!(
            "{},{},fps=30[game];",
            self.gameplay.crop(),
            resize(game_height, self.framing == "fill")
        ));
        if self.facecam_enabled {
            graph.push_str(&format!(
                "[cam0]{},{},fps=30[cam];",
                self.facecam.crop(),
                resize(cam_height, false)
            ));
        }
        if chat_height > 0 {
            graph.push_str(&format!(
                "[chat0]{},{},fps=30[chat];",
                self.chat.crop(),
                resize(chat_height, false)
            ));
        }
        let mut outputs = vec!["[game]"];
        if self.facecam_enabled {
            if self.facecam_position == "top" {
                outputs.insert(0, "[cam]");
            } else {
                outputs.push("[cam]");
            }
        }
        if chat_height > 0 {
            outputs.push("[chat]");
        }
        graph.push_str(&if outputs.len() > 1 {
            format!(
                "{}vstack=inputs={}:shortest=1[out]",
                outputs.join(""),
                outputs.len()
            )
        } else {
            "[game]null[out]".into()
        });
        Ok(graph)
    }
}

#[derive(FromRow, Serialize)]
pub struct Edit {
    pub id: Uuid,
    pub clip_id: Uuid,
    pub profile: sqlx::types::Json<Profile>,
    pub status: String,
    pub video_key: String,
    pub size_bytes: i64,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn enqueue(db: &mut PgConnection, clip_id: Uuid, profile: &Profile) -> Result<Uuid> {
    profile.validate()?;
    let id = Uuid::new_v4();
    // Caller locks the clip, making double-clicks and auto/manual admission idempotent.
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM edits WHERE clip_id=$1 AND status IN ('queued','rendering')",
    )
    .bind(clip_id)
    .fetch_optional(&mut *db)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    sqlx::query("INSERT INTO edits(id,clip_id,profile,video_key) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(clip_id)
        .bind(json!(profile))
        .bind(format!("edits/{id}/vertical.mp4"))
        .execute(&mut *db)
        .await?;
    sqlx::query("INSERT INTO job_logs(job_id,level,message) VALUES ($1,'info','Auftrag angelegt. Layout gespeichert; wartet auf freien Exportplatz.')").bind(id).execute(db).await?;
    Ok(id)
}

pub async fn auto_enqueue(db: &mut PgConnection, clip_id: Uuid, streamer_id: Uuid) -> Result<()> {
    let value: Option<Value> =
        sqlx::query_scalar("SELECT profile FROM editing_profiles WHERE streamer_id=$1")
            .bind(streamer_id)
            .fetch_optional(&mut *db)
            .await?;
    if let Some(value) = value {
        let profile: Profile = serde_json::from_value(value)?;
        if profile.auto_queue {
            enqueue(db, clip_id, &profile).await?;
        }
    }
    Ok(())
}

pub async fn reserve(state: &State) -> Result<Option<Uuid>> {
    let mut tx = state.pool.begin().await?;
    let id: Option<Uuid> = sqlx::query_scalar("SELECT e.id FROM edits e JOIN clips c ON c.id=e.clip_id WHERE e.status='queued' AND c.status='ready' ORDER BY e.created_at LIMIT 1 FOR UPDATE OF c,e SKIP LOCKED")
        .fetch_optional(&mut *tx).await?;
    if let Some(id) = id {
        sqlx::query("UPDATE edits SET status='rendering',error=NULL WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    if let Some(id) = id {
        events::publish(&state.pool, "edit.updated", Some(id)).await;
    }
    Ok(id)
}

pub async fn render(state: &State, source: &Path, output: &Path, profile: &Profile) -> Result<()> {
    media::ffmpeg(
        &state.config,
        &[
            "-i".into(),
            source.to_string_lossy().into(),
            "-filter_complex".into(),
            profile.filter()?,
            "-map".into(),
            "[out]".into(),
            "-map".into(),
            "0:a:0?".into(),
            "-c:v".into(),
            "libx264".into(),
            "-threads".into(),
            "2".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "20".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "160k".into(),
            "-movflags".into(),
            "+faststart".into(),
            output.to_string_lossy().into(),
        ],
        Duration::from_secs(600),
    )
    .await?;
    Ok(())
}

pub async fn process(state: &State, id: Uuid) -> Result<()> {
    jobs::log(
        &state.pool,
        id,
        "info",
        "Hochkant-Bearbeitung gestartet. Originalclip aus privatem S3 laden.",
    )
    .await;
    let directory = state.config.data_dir.join("edits").join(id.to_string());
    let result: Result<()> = async {
        let edit: Edit=sqlx::query_as("SELECT * FROM edits WHERE id=$1").bind(id).fetch_one(&state.pool).await?;
        let (source_key,streamer_id,title): (String,Uuid,String)=sqlx::query_as("SELECT video_key,streamer_id,title FROM clips WHERE id=$1")
            .bind(edit.clip_id).fetch_one(&state.pool).await?;
        tokio::fs::create_dir_all(&directory).await?;
        let source=directory.join("source.mp4");
        let output=directory.join("vertical.mp4");
        let response=state.storage.client.get_object().bucket(&state.storage.bucket).key(source_key).send().await?;
        let mut body=response.body.into_async_read();
        let mut file=tokio::fs::File::create(&source).await?;
        tokio::io::copy(&mut body,&mut file).await?;
        drop(file);
        jobs::log(&state.pool,id,"info","Quelle geladen. Streamer-Layout rendern: 720×1280, Facecam/Spiel/Chat gemäß gespeicherter Vorlage.").await;
        render(state,&source,&output,&edit.profile).await?;
        jobs::log(&state.pool,id,"info","Hochkant-MP4 fertig. Upload in privaten S3-Speicher.").await;
        state.storage.upload(&output,&edit.video_key,"video/mp4").await?;
        let size=tokio::fs::metadata(&output).await?.len() as i64;
        let mut tx=state.pool.begin().await?;
        sqlx::query("UPDATE edits SET status='ready',size_bytes=$2,finished_at=now() WHERE id=$1").bind(id).bind(size).execute(&mut *tx).await?;
        services::activity(&mut tx,"edit",&format!("Hochkant-Export fertig: {title}"),Some(streamer_id)).await?;
        tx.commit().await?;
        Ok(())
    }.await;
    if let Err(error) = result {
        jobs::log(&state.pool, id, "error", &error.to_string()).await;
        sqlx::query("UPDATE edits SET status='failed',error=$2,finished_at=now() WHERE id=$1")
            .bind(id)
            .bind(media::tail(&error.to_string(), 700))
            .execute(&state.pool)
            .await?;
    } else {
        jobs::log(
            &state.pool,
            id,
            "info",
            "Hochkant-Export fertig und herunterladbar.",
        )
        .await;
    }
    if directory.exists() {
        tokio::fs::remove_dir_all(directory).await?;
    }
    events::publish(&state.pool, "edit.updated", Some(id)).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_outside_or_injected_layouts() {
        let mut p = Profile::default();
        assert!(p.filter().is_ok());
        p.facecam.x = 0.9;
        assert!(p.validate().is_err());
        p.facecam.x = 0.;
        p.chat.width = f64::NAN;
        assert!(p.validate().is_err());
        p.chat.width = 0.25;
        p.framing = "fill;movie=/secret".into();
        assert!(p.validate().is_err());
    }
}
