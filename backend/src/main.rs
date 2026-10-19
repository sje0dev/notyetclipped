use std::time::Duration;

use notyetclipped::{api, config::Config, state::State, storage::Storage, worker};
use sqlx::postgres::PgPoolOptions;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "notyetclipped=info,actix_web=info".into()),
        )
        .init();
    let command = std::env::args().nth(1).unwrap_or_else(|| "api".into());
    if command == "init-storage" {
        return Storage::new(&config).initialize_local(&config).await;
    }
    if command == "healthcheck" {
        reqwest::get("http://127.0.0.1:8000/api/ping")
            .await?
            .error_for_status()?;
        return Ok(());
    }
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;
    if command == "migrate" {
        sqlx::migrate!().run(&pool).await?;
        return Ok(());
    }
    let state = State::new(pool, config);
    match command.as_str() {
        "api" => api::serve(state).await,
        "worker" => worker::serve(state).await,
        _ => anyhow::bail!("Usage: notyetclipped [api|worker|migrate|init-storage|healthcheck]"),
    }
}
