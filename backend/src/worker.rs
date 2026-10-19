use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use sqlx::{Connection, PgConnection};
use tokio::{
    sync::{Semaphore, watch},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    events,
    media::{overlaps, render, safe_path, tail},
    models::{ACTIVE, Clip, ManifestEntry, Recording, Segment, Streamer},
    recording, services,
    state::State,
};

pub async fn recover(state: &State) -> Result<()> {
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE clips SET status='queued' WHERE status IN ('cutting','uploading')")
        .execute(&mut *tx)
        .await?;
    let recordings: Vec<Recording> = sqlx::query_as("UPDATE recordings SET ended_at=now(),error='Aufnahmesitzung nach Worker-Neustart abgeschlossen.' WHERE ended_at IS NULL RETURNING *").fetch_all(&mut *tx).await?;
    for recording in recordings {
        services::finalize_pending(&mut tx, &recording).await?;
    }
    sqlx::query("UPDATE streamers SET status=CASE WHEN enabled THEN 'waiting' ELSE 'paused' END WHERE deleted_at IS NULL").execute(&mut *tx).await?;
    services::activity(
        &mut tx,
        "system",
        "Rust-Worker gestartet. Aufnahme- und Exportaufträge werden fortgesetzt.",
        None,
    )
    .await?;
    tx.commit().await?;
    events::publish(&state.pool, "system.updated", None).await;
    Ok(())
}

pub async fn export_clip(state: &State, id: Uuid) -> Result<()> {
    let directory = state.config.data_dir.join("exports").join(id.to_string());
    let result: Result<()> = async {
        let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1").bind(id).fetch_one(&state.pool).await?;
        if clip.status == "deleting" { return Ok(()); }
        let login: String = sqlx::query_scalar("SELECT login FROM streamers WHERE id=$1").bind(clip.streamer_id).fetch_one(&state.pool).await?;
        let prefix = format!("clips/{}/{}/{id}", login, clip.created_at.format("%Y/%m/%d"));
        let video_key = format!("{prefix}/video.mp4");
        let thumbnail_key = format!("{prefix}/thumbnail.jpg");
        sqlx::query("UPDATE clips SET video_key=$2,thumbnail_key=$3 WHERE id=$1").bind(id).bind(&video_key).bind(&thumbnail_key).execute(&state.pool).await?;
        let manifest: Vec<ManifestEntry> = serde_json::from_value(clip.manifest)?;
        let (video, thumbnail) = render(&state.config, &manifest, clip.start_ms, clip.end_ms, &directory).await?;
        let changed = sqlx::query("UPDATE clips SET status='uploading' WHERE id=$1 AND status != 'deleting'").bind(id).execute(&state.pool).await?;
        if changed.rows_affected() == 0 { return Ok(()); }
        events::publish(&state.pool, "clip.updated", Some(id)).await;
        state.storage.upload(&video, &video_key, "video/mp4").await?;
        state.storage.upload(&thumbnail, &thumbnail_key, "image/jpeg").await?;
        let size = tokio::fs::metadata(&video).await?.len() + tokio::fs::metadata(&thumbnail).await?.len();
        let mut tx = state.pool.begin().await?;
        let changed = sqlx::query("UPDATE clips SET status='ready',error=NULL,ready_at=now(),size_bytes=$2 WHERE id=$1 AND status != 'deleting'").bind(id).bind(size as i64).execute(&mut *tx).await?;
        if changed.rows_affected() > 0 {
            services::activity(&mut tx, "clip", &format!("Clip von @{login} in S3 gespeichert."), Some(clip.streamer_id)).await?;
        }
        tx.commit().await?;
        if changed.rows_affected() > 0 { tokio::fs::remove_dir_all(&directory).await?; }
        Ok(())
    }.await;
    if let Err(error) = result {
        tracing::warn!(%error,%id,"Clip export failed");
        sqlx::query("UPDATE clips SET error=$2,status=CASE WHEN attempts<5 THEN 'retry' ELSE 'failed' END,retry_at=now()+(LEAST(600,15*power(2,LEAST(attempts,5)))*interval '1 second') WHERE id=$1 AND status NOT IN ('deleting','ready')")
            .bind(id).bind(tail(&error.to_string(),800)).execute(&state.pool).await?;
    }
    events::publish(&state.pool, "clip.updated", Some(id)).await;
    Ok(())
}

pub async fn delete_clip(state: &State, id: Uuid) -> Result<()> {
    let result: Result<()> = async {
        let clip: Option<Clip> = sqlx::query_as("SELECT * FROM clips WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
        let Some(clip) = clip else {
            return Ok(());
        };
        let keys: Vec<String> = [clip.video_key, clip.thumbnail_key]
            .into_iter()
            .flatten()
            .collect();
        state.storage.delete(&keys).await?;
        let directory = state.config.data_dir.join("exports").join(id.to_string());
        if directory.exists() {
            tokio::fs::remove_dir_all(directory).await?;
        }
        sqlx::query("DELETE FROM clips WHERE id=$1 AND status='deleting'")
            .bind(id)
            .execute(&state.pool)
            .await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        sqlx::query("UPDATE clips SET error=$2,attempts=attempts+1,retry_at=now()+(LEAST(600,15*power(2,LEAST(attempts+1,5)))*interval '1 second') WHERE id=$1")
            .bind(id).bind(tail(&error.to_string(),800)).execute(&state.pool).await?;
    }
    events::publish(&state.pool, "clip.updated", Some(id)).await;
    Ok(())
}

pub async fn clean_buffers(state: &State, buffer_seconds: i64) -> Result<()> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM recordings WHERE ended_at IS NULL OR buffer_cleaned_at IS NULL",
    )
    .fetch_all(&state.pool)
    .await?;
    for id in ids {
        let mut tx = state.pool.begin().await?;
        let recording: Recording =
            sqlx::query_as("SELECT * FROM recordings WHERE id=$1 FOR UPDATE")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let protected: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT start_ms,end_ms FROM clips WHERE recording_id=$1 AND status=ANY($2)",
        )
        .bind(id)
        .bind(ACTIVE)
        .fetch_all(&mut *tx)
        .await?;
        let segments: Vec<Segment> =
            sqlx::query_as("SELECT * FROM segments WHERE recording_id=$1 AND ($2 OR end_ms < $3)")
                .bind(id)
                .bind(recording.ended_at.is_some())
                .bind(recording.latest_ms - buffer_seconds * 1000)
                .fetch_all(&mut *tx)
                .await?;
        for segment in segments {
            if protected
                .iter()
                .any(|(start, end)| overlaps(segment.start_ms, segment.end_ms, *start, *end))
            {
                continue;
            }
            let path = safe_path(&state.config.data_dir, &segment.path)?;
            match tokio::fs::remove_file(path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(%error,"Could not remove segment");
                    continue;
                }
            }
            sqlx::query("DELETE FROM segments WHERE id=$1")
                .bind(segment.id)
                .execute(&mut *tx)
                .await?;
        }
        let remains: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM segments WHERE recording_id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if recording.ended_at.is_some() && !remains {
            let directory = state
                .config
                .data_dir
                .join("recordings")
                .join(id.to_string());
            if directory.exists() {
                tokio::fs::remove_dir_all(directory).await?;
            }
            sqlx::query("UPDATE recordings SET buffer_cleaned_at=now() WHERE id=$1")
                .bind(id)
                .execute(&state.pool)
                .await?;
        }
    }
    let settings = services::settings(&state.pool).await?;
    if settings.retention_days > 0 {
        sqlx::query("UPDATE clips SET status='deleting',retry_at=NULL,attempts=0 WHERE status='ready' AND NOT favorite AND created_at < now()-($1*interval '1 day')")
            .bind(settings.retention_days as f64).execute(&state.pool).await?;
    }
    sqlx::query("DELETE FROM activities WHERE created_at < now()-interval '30 days'")
        .execute(&state.pool)
        .await?;
    // Keep session metadata referenced by clips; discard empty offline checks after a week.
    sqlx::query("DELETE FROM recordings r WHERE ended_at < now()-interval '7 days' AND NOT EXISTS(SELECT 1 FROM clips WHERE recording_id=r.id) AND NOT EXISTS(SELECT 1 FROM segments WHERE recording_id=r.id)").execute(&state.pool).await?;
    Ok(())
}

async fn scratch_size(root: &std::path::Path) -> Result<u64> {
    let mut directories = vec![root.to_path_buf()];
    let mut total = 0;
    while let Some(directory) = directories.pop() {
        let mut reader = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = reader.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    directories.push(entry.path());
                } else if metadata.is_file() {
                    total += metadata.len();
                }
            }
        }
    }
    Ok(total)
}

struct RecorderHandle {
    stop: watch::Sender<bool>,
    task: JoinHandle<Result<()>>,
    quality: String,
}

async fn finish_abandoned_recording(state: &State, streamer_id: Uuid) -> Result<()> {
    let mut tx = state.pool.begin().await?;
    let rows: Vec<Recording> = sqlx::query_as(
        "UPDATE recordings SET ended_at=now(),error='Aufnahmeprozess wurde unterbrochen.' WHERE streamer_id=$1 AND ended_at IS NULL RETURNING *"
    ).bind(streamer_id).fetch_all(&mut *tx).await?;
    for recording in rows {
        services::finalize_pending(&mut tx, &recording).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn serve(state: State) -> Result<()> {
    // Dedicated connection: losing this session ends leadership, rather than silently reacquiring it.
    let mut leader = PgConnection::connect(&state.config.database_url).await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(71430921)")
        .fetch_one(&mut leader)
        .await?;
    anyhow::ensure!(
        acquired,
        "Es läuft bereits ein Aufnahme-Worker für diese Datenbank."
    );
    recover(&state).await?;
    let mut shutdown = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let analysis = Arc::new(Semaphore::new(2));
    let mut recorders: HashMap<Uuid, RecorderHandle> = HashMap::new();
    let mut exports: HashMap<Uuid, JoinHandle<Result<()>>> = HashMap::new();
    let mut last_cleanup = Instant::now() - Duration::from_secs(60);
    let mut last_heartbeat_event = Instant::now() - Duration::from_secs(60);
    let mut used = 0;
    let mut fatal = None;
    loop {
        tokio::select! { _=shutdown.recv()=>break, _=&mut interrupt=>break, _=tokio::time::sleep(Duration::from_secs(2))=>{} }
        if let Err(error) = sqlx::query("SELECT 1").execute(&mut leader).await {
            fatal = Some(error);
            break;
        }
        let tick:Result<()>=async {
            let finished:Vec<_>=recorders.iter().filter(|(_,r)|r.task.is_finished()).map(|(id,_)|*id).collect();
            for id in finished {
                if let Some(recorder)=recorders.remove(&id) {
                    match recorder.task.await { Ok(Ok(()))=>{}, result=>tracing::warn!(?result,%id,"Recorder ended with an error") }
                }
            }
            let finished:Vec<_>=exports.iter().filter(|(_,t)|t.is_finished()).map(|(id,_)|*id).collect();
            for id in finished {
                if let Some(task)=exports.remove(&id)
                    && let Err(error)=task.await.context("Export task panicked").and_then(|v|v) {
                    tracing::warn!(%error,%id,"Export job failed");
                }
            }
            let settings=services::settings(&state.pool).await?;
            let running: Vec<Uuid> = exports.keys().copied().collect();
            // A failed DB connection can prevent a task from persisting its retry status.
            sqlx::query("UPDATE clips SET status='queued' WHERE status IN ('cutting','uploading') AND NOT(id=ANY($1))")
                .bind(running).execute(&state.pool).await?;
            let channels:Vec<Streamer>=sqlx::query_as("SELECT * FROM streamers WHERE enabled AND deleted_at IS NULL ORDER BY priority DESC,next_check_at ASC NULLS FIRST,created_at").fetch_all(&state.pool).await?;
            if last_cleanup.elapsed()>Duration::from_secs(15) {
                clean_buffers(&state,settings.buffer_seconds).await?;
                used=scratch_size(&state.config.data_dir).await?;
                last_cleanup=Instant::now();
            }
            let disk=nix::sys::statvfs::statvfs(&state.config.data_dir)?;
            let free=u64::from(disk.blocks_available())*disk.fragment_size();
            let blocked=used>=settings.scratch_budget_gb*1024_u64.pow(3)||free<settings.min_free_disk_gb*1024_u64.pow(3);
            let active_sorted:Vec<_>=channels.iter().filter(|channel|recorders.contains_key(&channel.id)).map(|channel|channel.id).take(settings.max_recordings).collect();
            let allowed:HashSet<_>=active_sorted.into_iter().collect();
            for (id,recorder) in &recorders {
                if blocked || !allowed.contains(id) || channels.iter().find(|c|c.id==*id).is_some_and(|c|c.quality!=recorder.quality) { let _=recorder.stop.send(true); }
            }
            if !blocked {
                for channel in channels {
                    if recorders.len()>=settings.max_recordings { break; }
                    if recorders.contains_key(&channel.id)||channel.next_check_at.is_some_and(|at|at>Utc::now()) { continue; }
                    finish_abandoned_recording(&state, channel.id).await?;
                    let (stop,rx)=watch::channel(false);
                    let id=channel.id;
                    let quality=channel.quality.clone();
                    let task=tokio::spawn(recording::run(state.clone(),channel,rx,analysis.clone()));
                    recorders.insert(id,RecorderHandle{stop,task,quality});
                }
            }
            let capacity=settings.max_exports.saturating_sub(exports.len());
            if capacity>0 {
                let mut tx=state.pool.begin().await?;
                let ids:Vec<Uuid>=exports.keys().copied().collect();
                let clips:Vec<Clip>=sqlx::query_as("SELECT * FROM clips WHERE status IN ('queued','retry','deleting') AND (retry_at IS NULL OR retry_at<=now()) AND NOT(id=ANY($1)) ORDER BY created_at LIMIT $2 FOR UPDATE SKIP LOCKED")
                    .bind(ids).bind(capacity as i64).fetch_all(&mut *tx).await?;
                for clip in &clips {
                    if clip.status!="deleting" { sqlx::query("UPDATE clips SET status='cutting',attempts=attempts+1 WHERE id=$1").bind(clip.id).execute(&mut *tx).await?; }
                }
                tx.commit().await?;
                for clip in clips {
                    let state=state.clone();
                    let id=clip.id;
                    exports.insert(id,tokio::spawn(async move { if clip.status=="deleting" { delete_clip(&state,id).await } else { export_clip(&state,id).await } }));
                }
            }
            let details=recording::stats(recorders.len(),exports.len(),used,free,blocked);
            sqlx::query("INSERT INTO worker_state(id,heartbeat_at,details) VALUES (1,now(),$1) ON CONFLICT(id) DO UPDATE SET heartbeat_at=now(),details=EXCLUDED.details").bind(details).execute(&state.pool).await?;
            if last_heartbeat_event.elapsed()>Duration::from_secs(10) {
                events::publish(&state.pool,"system.updated",None).await;
                last_heartbeat_event=Instant::now();
            }
            Ok(())
        }.await;
        if let Err(error) = tick {
            tracing::error!(%error,"Worker cycle failed; retrying");
        }
    }
    if let Some(error) = fatal {
        // Leadership has been lost. Stop all work before another supervisor can take over.
        for recorder in recorders.values() {
            recorder.task.abort();
        }
        for task in exports.values() {
            task.abort();
        }
        for (_, recorder) in recorders {
            let _ = recorder.task.await;
        }
        for (_, task) in exports {
            let _ = task.await;
        }
        return Err(error.into());
    }
    for recorder in recorders.values() {
        let _ = recorder.stop.send(true);
    }
    for (_, recorder) in recorders {
        let _ = recorder.task.await;
    }
    for (_, task) in exports {
        let _ = task.await;
    }
    sqlx::query(
        "UPDATE worker_state SET heartbeat_at=now()-interval '1 hour',details=$1 WHERE id=1",
    )
    .bind(json!({"paused_reason":"Worker beendet."}))
    .execute(&state.pool)
    .await
    .ok();
    Ok(())
}
