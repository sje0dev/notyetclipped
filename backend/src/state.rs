use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::broadcast;

use crate::{config::Config, storage::Storage};

#[derive(Clone)]
pub struct State {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub storage: Storage,
    pub events: broadcast::Sender<String>,
}

impl State {
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self {
            pool,
            storage: Storage::new(&config),
            config: Arc::new(config),
            events: broadcast::channel(256).0,
        }
    }
}
