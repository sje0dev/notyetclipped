use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

pub const ACTIVE: &[&str] = &["collecting", "queued", "cutting", "uploading", "retry"];

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Streamer {
    pub id: Uuid,
    pub login: String,
    pub enabled: bool,
    pub quality: String,
    pub priority: i32,
    pub pre_roll: i32,
    pub post_roll: i32,
    pub threshold_db: f64,
    pub auto_clip: bool,
    pub status: String,
    pub last_error: Option<String>,
    pub next_check_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Recording {
    pub id: Uuid,
    pub streamer_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub latest_ms: i64,
    pub audio_db: Option<f64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct Segment {
    pub id: i64,
    pub recording_id: Uuid,
    pub path: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub size_bytes: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Clip {
    pub id: Uuid,
    pub streamer_id: Uuid,
    pub recording_id: Uuid,
    pub title: String,
    pub status: String,
    pub trigger: String,
    pub score: f64,
    pub reason: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub manifest: Value,
    pub video_key: Option<String>,
    pub thumbnail_key: Option<String>,
    pub size_bytes: i64,
    pub favorite: bool,
    pub rating: i32,
    pub attempts: i32,
    pub error: Option<String>,
    pub retry_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub ready_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StreamerInput {
    pub login: String,
    pub enabled: bool,
    pub quality: String,
    pub priority: i32,
    pub pre_roll: i32,
    pub post_roll: i32,
    pub threshold_db: f64,
    pub auto_clip: bool,
}

impl Default for StreamerInput {
    fn default() -> Self {
        Self {
            login: String::new(),
            enabled: true,
            quality: "720p,best".into(),
            priority: 0,
            pre_roll: 30,
            post_roll: 20,
            threshold_db: 9.0,
            auto_clip: true,
        }
    }
}

impl StreamerInput {
    pub fn validate(&mut self) -> Result<(), String> {
        self.login = self.login.trim().to_lowercase();
        for prefix in [
            "https://www.twitch.tv/",
            "https://twitch.tv/",
            "http://www.twitch.tv/",
            "http://twitch.tv/",
            "www.twitch.tv/",
            "twitch.tv/",
        ] {
            if let Some(login) = self.login.strip_prefix(prefix) {
                self.login = login.to_owned();
                break;
            }
        }
        self.login = self.login.trim_end_matches('/').to_owned();
        if !(3..=25).contains(&self.login.len())
            || !self
                .login
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err("Bitte einen gültigen Twitch-Kanalnamen eingeben.".into());
        }
        if ![
            "best",
            "1080p60,1080p,best",
            "720p60,720p,best",
            "720p,best",
            "480p,worst",
        ]
        .contains(&self.quality.as_str())
        {
            return Err("Ungültige Aufnahmequalität.".into());
        }
        if !(0..=100).contains(&self.priority)
            || !(5..=90).contains(&self.pre_roll)
            || !(5..=60).contains(&self.post_roll)
            || self.pre_roll + self.post_roll > 120
            || !(3.0..=24.0).contains(&self.threshold_db)
        {
            return Err("Ungültige Einstellungen: Priorität 0–100, Vorlauf 5–90s, Nachlauf 5–60s, zusammen höchstens 120s, Audio-Schwelle 3–24 dB.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub max_recordings: usize,
    pub max_exports: usize,
    pub buffer_seconds: i64,
    pub scratch_budget_gb: u64,
    pub min_free_disk_gb: u64,
    pub offline_poll_seconds: i64,
    pub retention_days: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_recordings: 4,
            max_exports: 2,
            buffer_seconds: 300,
            scratch_budget_gb: 20,
            min_free_disk_gb: 2,
            offline_poll_seconds: 90,
            retention_days: 0,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=50).contains(&self.max_recordings)
            || !(1..=8).contains(&self.max_exports)
            || !(180..=1800).contains(&self.buffer_seconds)
            || !(2..=2000).contains(&self.scratch_budget_gb)
            || !(1..=500).contains(&self.min_free_disk_gb)
            || !(30..=1800).contains(&self.offline_poll_seconds)
            || !(0..=365).contains(&self.retention_days)
        {
            return Err(
                "Mindestens eine Einstellung liegt außerhalb des erlaubten Bereichs.".into(),
            );
        }
        Ok(())
    }
}
