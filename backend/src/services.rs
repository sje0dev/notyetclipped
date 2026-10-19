use anyhow::{Context, Result};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    models::{Clip, ManifestEntry, Recording, Segment, Settings, Streamer},
    state::State,
};

pub async fn settings(pool: &PgPool) -> Result<Settings> {
    let value: Option<Value> = sqlx::query_scalar("SELECT values FROM app_settings WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(match value {
        Some(value) => serde_json::from_value(value)?,
        None => Settings::default(),
    })
}

pub async fn activity(
    db: &mut PgConnection,
    kind: &str,
    message: &str,
    streamer_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query("INSERT INTO activities(kind,message,streamer_id) VALUES ($1,$2,$3)")
        .bind(kind)
        .bind(message)
        .bind(streamer_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn create_clip(
    db: &mut PgConnection,
    streamer: &Streamer,
    recording: &Recording,
    event_ms: i64,
    trigger: &str,
    score: f64,
    reason: &str,
) -> Result<Clip> {
    // Caller holds the recording row lock; reservation and garbage collection serialize on it.
    let earliest: Option<i64> =
        sqlx::query_scalar("SELECT min(start_ms) FROM segments WHERE recording_id=$1")
            .bind(recording.id)
            .fetch_one(&mut *db)
            .await?;
    let start = earliest
        .context("Der Videopuffer ist noch leer. Bitte kurz warten.")?
        .max(event_ms - i64::from(streamer.pre_roll) * 1000);
    let end = event_ms + i64::from(streamer.post_roll) * 1000;
    if trigger != "manual" {
        let existing: Option<Clip> = sqlx::query_as("SELECT * FROM clips WHERE recording_id=$1 AND trigger != 'manual' AND status='collecting' AND end_ms >= $2 ORDER BY created_at DESC LIMIT 1 FOR UPDATE")
            .bind(recording.id).bind(event_ms - 3000).fetch_optional(&mut *db).await?;
        if let Some(existing) = existing
            && end - existing.start_ms <= 120_000
        {
            return Ok(sqlx::query_as("UPDATE clips SET end_ms=GREATEST(end_ms,$2), score=GREATEST(score,$3) WHERE id=$1 RETURNING *")
                    .bind(existing.id).bind(end).bind(score).fetch_one(&mut *db).await?);
        }
        let recent = sqlx::query_as::<_, Clip>("SELECT * FROM clips WHERE recording_id=$1 AND trigger != 'manual' AND status NOT IN ('failed','deleting') AND end_ms >= $2 ORDER BY created_at DESC LIMIT 1")
            .bind(recording.id).bind(event_ms).fetch_optional(&mut *db).await?;
        if let Some(recent) = recent {
            return Ok(recent);
        }
    }
    let title = format!(
        "{} · {}",
        if trigger != "manual" {
            "Trigger-Clip"
        } else {
            "Manueller Clip"
        },
        streamer.login
    );
    let clip = sqlx::query_as("INSERT INTO clips(id,streamer_id,recording_id,title,start_ms,end_ms,trigger,score,reason) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING *")
        .bind(Uuid::new_v4()).bind(streamer.id).bind(recording.id).bind(title).bind(start).bind(end).bind(trigger).bind(score).bind(reason).fetch_one(&mut *db).await?;
    activity(
        db,
        "highlight",
        &format!("@{}: {reason}", streamer.login),
        Some(streamer.id),
    )
    .await?;
    Ok(clip)
}

pub async fn finalize_pending(db: &mut PgConnection, recording: &Recording) -> Result<()> {
    let clips = sqlx::query_as::<_, Clip>(
        "SELECT * FROM clips WHERE recording_id=$1 AND status='collecting' FOR UPDATE",
    )
    .bind(recording.id)
    .fetch_all(&mut *db)
    .await?;
    for clip in clips {
        if recording.ended_at.is_none() && recording.latest_ms < clip.end_ms {
            continue;
        }
        let end = clip.end_ms.min(recording.latest_ms);
        let segments = sqlx::query_as::<_, Segment>("SELECT * FROM segments WHERE recording_id=$1 AND end_ms > $2 AND start_ms < $3 ORDER BY start_ms")
            .bind(recording.id).bind(clip.start_ms).bind(end).fetch_all(&mut *db).await?;
        if segments.is_empty() || end - clip.start_ms < 2000 {
            sqlx::query("UPDATE clips SET status='failed', error='Der Stream endete, bevor genügend Videodaten vorlagen.' WHERE id=$1").bind(clip.id).execute(&mut *db).await?;
            continue;
        }
        let start = clip.start_ms.max(segments[0].start_ms);
        let manifest: Vec<ManifestEntry> = segments
            .into_iter()
            .map(|s| ManifestEntry {
                path: s.path,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
            })
            .collect();
        sqlx::query(
            "UPDATE clips SET status='queued', start_ms=$2, end_ms=$3, manifest=$4 WHERE id=$1",
        )
        .bind(clip.id)
        .bind(start)
        .bind(end)
        .bind(serde_json::to_value(manifest)?)
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn streamer_views(pool: &PgPool) -> Result<Vec<Value>> {
    let streamers = sqlx::query_as::<_, Streamer>(
        "SELECT * FROM streamers WHERE deleted_at IS NULL ORDER BY priority DESC, created_at",
    )
    .fetch_all(pool)
    .await?;
    let recordings =
        sqlx::query_as::<_, Recording>("SELECT * FROM recordings WHERE ended_at IS NULL")
            .fetch_all(pool)
            .await?;
    let spans: Vec<(Uuid, i64)> = sqlx::query_as("SELECT s.recording_id, (MAX(s.end_ms)-MIN(s.start_ms))/1000 FROM segments s JOIN recordings r ON r.id=s.recording_id WHERE r.ended_at IS NULL GROUP BY s.recording_id").fetch_all(pool).await?;
    Ok(streamers
        .into_iter()
        .map(|streamer| {
            let recording = recordings.iter().find(|r| r.streamer_id == streamer.id);
            let mut value = serde_json::to_value(&streamer).unwrap();
            let object = value.as_object_mut().unwrap();
            object.remove("deleted_at");
            object.insert("recording_id".into(), json!(recording.map(|r| r.id)));
            object.insert("started_at".into(), json!(recording.map(|r| r.started_at)));
            object.insert("audio_db".into(), json!(recording.and_then(|r| r.audio_db)));
            object.insert(
                "trigger_state".into(),
                json!(recording.map(|r| &r.trigger_state)),
            );
            object.insert(
                "buffer_seconds".into(),
                json!(
                    recording
                        .and_then(|r| spans.iter().find(|span| span.0 == r.id))
                        .map_or(0, |span| span.1)
                ),
            );
            value
        })
        .collect())
}

pub async fn clip_view(state: &State, clip: &Clip, login: &str, detail: bool) -> Result<Value> {
    let mut value = serde_json::to_value(clip)?;
    let object = value.as_object_mut().unwrap();
    for key in [
        "manifest",
        "video_key",
        "thumbnail_key",
        "start_ms",
        "end_ms",
    ] {
        object.remove(key);
    }
    object.insert("streamer_login".into(), json!(login));
    object.insert(
        "duration".into(),
        json!((clip.end_ms - clip.start_ms) as f64 / 1000.0),
    );
    let thumb = if clip.status == "ready" {
        match &clip.thumbnail_key {
            Some(key) => Some(state.storage.url(key, None).await?),
            None => None,
        }
    } else {
        None
    };
    object.insert("thumbnail_url".into(), json!(thumb));
    if detail {
        let (video, download) = if clip.status == "ready" {
            if let Some(key) = &clip.video_key {
                (
                    Some(state.storage.url(key, None).await?),
                    Some(
                        state
                            .storage
                            .url(key, Some(&format!("{login}-{}.mp4", clip.id)))
                            .await?,
                    ),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        object.insert("video_url".into(), json!(video));
        object.insert("download_url".into(), json!(download));
    }
    Ok(value)
}
