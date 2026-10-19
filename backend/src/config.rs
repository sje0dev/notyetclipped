use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub s3_endpoint: Option<String>,
    pub s3_public_endpoint: Option<String>,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub data_dir: PathBuf,
    pub ffmpeg: String,
    pub streamlink: String,
    pub bind: String,
    pub chatgpt_callback: String,
    pub whisper: String,
    pub whisper_model: PathBuf,
}

fn value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.into())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let endpoint = value("S3_ENDPOINT_URL", "http://localhost:9000");
        let public = value("S3_PUBLIC_ENDPOINT_URL", &endpoint);
        let data_dir = PathBuf::from(value("DATA_DIR", "./data"));
        std::fs::create_dir_all(&data_dir)?;
        Ok(Self {
            database_url: value(
                "DATABASE_URL",
                "postgres://notyetclipped:local-postgres-change-me@localhost:5432/notyetclipped",
            ),
            s3_endpoint: (!endpoint.is_empty()).then_some(endpoint),
            s3_public_endpoint: (!public.is_empty()).then_some(public),
            s3_access_key: value("S3_ACCESS_KEY_ID", "local-access-key"),
            s3_secret_key: value("S3_SECRET_ACCESS_KEY", "local-secret-change-me"),
            s3_bucket: value("S3_BUCKET", "notyetclipped"),
            s3_region: value("S3_REGION", "us-east-1"),
            data_dir: data_dir.canonicalize()?,
            ffmpeg: value("FFMPEG_BIN", "ffmpeg"),
            streamlink: value("STREAMLINK_BIN", "streamlink"),
            bind: value("API_BIND", "127.0.0.1:8000"),
            chatgpt_callback: value(
                "CHATGPT_CALLBACK_URL",
                "http://127.0.0.1:3000/api/ai/callback",
            ),
            whisper: value("WHISPER_BIN", "whisper-cli"),
            whisper_model: value("WHISPER_MODEL", "/opt/whisper/ggml-base.bin").into(),
        })
    }
}
