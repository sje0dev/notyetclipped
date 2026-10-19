use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader},
    process::{Child, Command},
    sync::{Mutex, Semaphore, watch},
};
use uuid::Uuid;

use crate::{
    events,
    media::{AudioDetector, decode_audio, tail},
    models::{Recording, Streamer},
    services,
    state::State,
};

struct ProcessGroup {
    child: Child,
    group: Option<Pid>,
}

impl ProcessGroup {
    fn new(child: Child) -> Self {
        Self {
            group: child.id().map(|id| Pid::from_raw(id as i32)),
            child,
        }
    }

    async fn stop(&mut self, signal: Signal) {
        if let Some(group) = self.group {
            let _ = killpg(group, signal);
            if tokio::time::timeout(Duration::from_secs(8), self.child.wait())
                .await
                .is_err()
            {
                let _ = killpg(group, Signal::SIGKILL);
                let _ = self.child.wait().await;
            }
            // The parent may have exited while browser subprocesses are still alive.
            let _ = killpg(group, Signal::SIGKILL);
            self.group = None;
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if let Some(group) = self.group {
            let _ = killpg(group, Signal::SIGKILL);
        }
    }
}

struct Capture {
    stream: ProcessGroup,
    ffmpeg: ProcessGroup,
    pipe: tokio::task::JoinHandle<()>,
    messages: Arc<Mutex<VecDeque<String>>>,
}

impl Capture {
    fn start(state: &State, streamer: &Streamer, directory: &Path) -> Result<Self> {
        let messages = Arc::new(Mutex::new(VecDeque::new()));
        let mut stream = ProcessGroup::new(
            Command::new(&state.config.streamlink)
                .args([
                    "--stdout",
                    "--loglevel",
                    "warning",
                    "--stream-timeout",
                    "60",
                    "--retry-streams",
                    "0",
                    "--twitch-supported-codecs",
                    "h264",
                    "--webbrowser-headless",
                    "yes",
                    "--ffmpeg-ffmpeg",
                    &state.config.ffmpeg,
                    &format!("https://www.twitch.tv/{}", streamer.login),
                    &streamer.quality,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .process_group(0)
                .kill_on_drop(true)
                .spawn()
                .context("Streamlink konnte nicht gestartet werden.")?,
        );
        let mut ffmpeg = ProcessGroup::new(
            Command::new(&state.config.ffmpeg)
                .args([
                    "-nostdin",
                    "-hide_banner",
                    "-loglevel",
                    "warning",
                    "-fflags",
                    "+genpts",
                    "-i",
                    "pipe:0",
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a:0?",
                    "-c",
                    "copy",
                    "-f",
                    "segment",
                    "-segment_time",
                    "4",
                    "-segment_format",
                    "mpegts",
                    "-reset_timestamps",
                    "1",
                    "-segment_list",
                    &directory.join("segments.csv").to_string_lossy(),
                    "-segment_list_type",
                    "csv",
                    &directory.join("segment-%09d.ts").to_string_lossy(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .process_group(0)
                .kill_on_drop(true)
                .spawn()
                .context("FFmpeg konnte nicht gestartet werden.")?,
        );
        for stderr in [
            stream.child.stderr.take().unwrap(),
            ffmpeg.child.stderr.take().unwrap(),
        ] {
            let messages = messages.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut messages = messages.lock().await;
                    messages.push_back(tail(&line, 400));
                    if messages.len() > 12 {
                        messages.pop_front();
                    }
                }
            });
        }
        let mut source = stream.child.stdout.take().unwrap();
        let mut target = ffmpeg.child.stdin.take().unwrap();
        let pipe = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut source, &mut target).await;
        });
        Ok(Self {
            stream,
            ffmpeg,
            pipe,
            messages,
        })
    }

    async fn stop(&mut self) {
        tokio::join!(
            self.ffmpeg.stop(Signal::SIGINT),
            self.stream.stop(Signal::SIGTERM)
        );
        self.pipe.abort();
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.pipe.abort();
    }
}

pub struct IndexReader {
    pub offset: u64,
}
pub struct ClosedSegment {
    pub path: PathBuf,
    pub start: i64,
    pub end: i64,
    pub next_offset: u64,
}

impl IndexReader {
    pub async fn read(&self, directory: &Path) -> Result<Vec<ClosedSegment>> {
        let index = directory.join("segments.csv");
        if !index.exists() {
            return Ok(vec![]);
        }
        let mut file = tokio::fs::File::open(index).await?;
        file.seek(std::io::SeekFrom::Start(self.offset)).await?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        let mut offset = self.offset;
        let mut segments = Vec::new();
        for line in bytes.split_inclusive(|&byte| byte == b'\n') {
            if line.last() != Some(&b'\n') {
                break;
            }
            offset += line.len() as u64;
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(line);
            if let Some(Ok(row)) = reader.records().next() {
                if row.len() != 3 {
                    continue;
                }
                let name = Path::new(&row[0])
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("Ungültiger Segmentname.")?;
                if !name.starts_with("segment-")
                    || !name.ends_with(".ts")
                    || !name[8..name.len() - 3].bytes().all(|b| b.is_ascii_digit())
                {
                    continue;
                }
                let path = directory.join(name);
                if path.is_file() {
                    segments.push(ClosedSegment {
                        path,
                        start: (row[1].parse::<f64>()? * 1000.0).round() as i64,
                        end: (row[2].parse::<f64>()? * 1000.0).round() as i64,
                        next_offset: offset,
                    });
                }
            }
        }
        Ok(segments)
    }
}

struct Ingest {
    id: Uuid,
    streamer_id: Uuid,
    directory: PathBuf,
    reader: IndexReader,
    detector: AudioDetector,
    got_segment: bool,
    last_segment: Instant,
}

impl Ingest {
    async fn run(&mut self, state: &State, analyze: bool, semaphore: &Semaphore) -> Result<()> {
        for segment in self.reader.read(&self.directory).await? {
            let relative = segment
                .path
                .strip_prefix(&state.config.data_dir)?
                .to_string_lossy();
            let mut tx = state.pool.begin().await?;
            let _: Recording = sqlx::query_as("SELECT * FROM recordings WHERE id=$1 FOR UPDATE")
                .bind(self.id)
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO segments(recording_id,path,start_ms,end_ms,size_bytes) VALUES ($1,$2,$3,$4,$5) ON CONFLICT(path) DO NOTHING")
                .bind(self.id).bind(relative.as_ref()).bind(segment.start).bind(segment.end).bind(tokio::fs::metadata(&segment.path).await?.len() as i64).execute(&mut *tx).await?;
            let recording: Recording =
                sqlx::query_as("UPDATE recordings SET latest_ms=$2 WHERE id=$1 RETURNING *")
                    .bind(self.id)
                    .bind(segment.end)
                    .fetch_one(&mut *tx)
                    .await?;
            let streamer: Streamer = sqlx::query_as(
                "UPDATE streamers SET status='recording',last_error=NULL WHERE id=$1 RETURNING *",
            )
            .bind(self.streamer_id)
            .fetch_one(&mut *tx)
            .await?;
            if !self.got_segment {
                services::activity(
                    &mut tx,
                    "recording",
                    &format!("@{} ist live. Videopuffer läuft.", streamer.login),
                    Some(streamer.id),
                )
                .await?;
            }
            services::finalize_pending(&mut tx, &recording).await?;
            tx.commit().await?;
            self.reader.offset = segment.next_offset;
            self.got_segment = true;
            self.last_segment = Instant::now();
            self.detector.threshold_db = streamer.threshold_db;
            if analyze {
                let _permit = semaphore.acquire().await?;
                match decode_audio(&state.config, &segment.path).await {
                    Ok(pcm) => {
                        let (hit, level) = self.detector.feed(&pcm, segment.start);
                        let mut tx = state.pool.begin().await?;
                        let _: Recording =
                            sqlx::query_as("SELECT * FROM recordings WHERE id=$1 FOR UPDATE")
                                .bind(self.id)
                                .fetch_one(&mut *tx)
                                .await?;
                        sqlx::query("UPDATE recordings SET audio_db=$2 WHERE id=$1")
                            .bind(self.id)
                            .bind(level)
                            .execute(&mut *tx)
                            .await?;
                        if let Some(hit) = hit
                            && streamer.auto_clip
                            && streamer.enabled
                            && streamer.deleted_at.is_none()
                        {
                            services::create_clip(
                                &mut tx,
                                &streamer,
                                &recording,
                                hit.time_ms,
                                "audio",
                                hit.score,
                                &format!(
                                    "Audio-Spitze: {:.1} dB über dem Kanal-Normalpegel.",
                                    hit.level_db - hit.baseline_db
                                ),
                            )
                            .await?;
                            services::finalize_pending(&mut tx, &recording).await?;
                        }
                        tx.commit().await?;
                    }
                    Err(error) => {
                        tracing::warn!(%error, streamer=%streamer.login, "Audioanalyse fehlgeschlagen")
                    }
                }
            }
            events::publish(&state.pool, "stream.updated", Some(self.streamer_id)).await;
        }
        Ok(())
    }
}

pub async fn run(
    state: State,
    streamer: Streamer,
    mut stop: watch::Receiver<bool>,
    semaphore: Arc<Semaphore>,
) -> Result<()> {
    let id = Uuid::new_v4();
    let directory = state
        .config
        .data_dir
        .join("recordings")
        .join(id.to_string());
    tokio::fs::create_dir_all(&directory).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO recordings(id,streamer_id) VALUES ($1,$2)")
        .bind(id)
        .bind(streamer.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE streamers SET status='connecting' WHERE id=$1")
        .bind(streamer.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    events::publish(&state.pool, "stream.updated", Some(streamer.id)).await;
    let mut ingest = Ingest {
        id,
        streamer_id: streamer.id,
        directory: directory.clone(),
        reader: IndexReader { offset: 0 },
        detector: AudioDetector::new(streamer.threshold_db),
        got_segment: false,
        last_segment: Instant::now(),
    };
    let mut capture = None;
    let result: Result<()> = async {
        capture = Some(Capture::start(&state, &streamer, &directory)?);
        let capture = capture.as_mut().unwrap();
        loop {
            if *stop.borrow() { break; }
            ingest.run(&state, true, &semaphore).await?;
            if capture.ffmpeg.child.try_wait()?.is_some() { break; }
            anyhow::ensure!(ingest.last_segment.elapsed() < Duration::from_secs(120), "Seit 120 Sekunden keine neuen Videosegmente empfangen.");
            tokio::select! { _ = tokio::time::sleep(Duration::from_secs(1)) => {}, _ = stop.changed() => break }
        }
        Ok(())
    }.await;
    let mut error = result.err().map(|error| tail(&error.to_string(), 800));
    if let Some(capture) = &mut capture {
        capture.stop().await;
        if error.is_none() && !ingest.got_segment && !*stop.borrow() {
            let messages = capture
                .messages
                .lock()
                .await
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            error = Some(if messages.is_empty() {
                "Stream ist offline oder aktuell nicht erreichbar.".into()
            } else {
                tail(&messages, 800)
            });
        }
    }
    if let Err(error) = ingest.run(&state, false, &semaphore).await {
        tracing::warn!(%error, "Letztes Segment konnte nicht eingelesen werden");
    }
    let settings = services::settings(&state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let recording: Recording =
        sqlx::query_as("UPDATE recordings SET ended_at=now(),error=$2 WHERE id=$1 RETURNING *")
            .bind(id)
            .bind(&error)
            .fetch_one(&mut *tx)
            .await?;
    services::finalize_pending(&mut tx, &recording).await?;
    let retry_seconds = settings.offline_poll_seconds + rand::random_range(0..15);
    sqlx::query("UPDATE streamers SET status=CASE WHEN enabled AND deleted_at IS NULL THEN 'offline' ELSE 'paused' END,last_error=$2,next_check_at=now()+($3 * interval '1 second') WHERE id=$1")
        .bind(streamer.id).bind(error).bind(retry_seconds as f64).execute(&mut *tx).await?;
    if ingest.got_segment {
        services::activity(
            &mut tx,
            "system",
            &format!("Aufnahme von @{} beendet.", streamer.login),
            Some(streamer.id),
        )
        .await?;
    }
    tx.commit().await?;
    events::publish(&state.pool, "stream.updated", Some(streamer.id)).await;
    Ok(())
}

pub fn stats(
    active: usize,
    exports: usize,
    used: u64,
    free: u64,
    blocked: bool,
) -> serde_json::Value {
    json!({"active_recordings":active,"active_exports":exports,"scratch_bytes":used,"disk_free_bytes":free,
        "paused_reason":if blocked {Some("Lokales Speicherlimit erreicht. Aufnahmen pausieren.")} else {None}})
}
