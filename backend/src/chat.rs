//! Read-only anonymous Twitch IRC. No Helix polling, chat text is never persisted.
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tokio_tungstenite::{Connector, connect_async_tls_with_config, tungstenite::Message};

#[derive(Default)]
pub struct Metrics {
    connected: bool,
    joined_at: Option<Instant>,
    messages: VecDeque<(Instant, String)>,
}

impl Metrics {
    fn prune(&mut self, now: Instant) {
        while self
            .messages
            .front()
            .is_some_and(|m| now.duration_since(m.0).as_secs() >= 150)
        {
            self.messages.pop_front();
        }
    }

    pub fn snapshot_at(&mut self, now: Instant) -> (Option<f64>, Option<f64>, Option<f64>) {
        self.prune(Instant::now());
        // A full observation window is required after reconnecting.
        if !self.connected
            || self
                .joined_at
                .is_none_or(|at| now.saturating_duration_since(at).as_secs() < 10)
        {
            return (None, None, None);
        }
        let recent: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.0 <= now && now.duration_since(m.0).as_secs() < 10)
            .collect();
        let count = recent.len() as f64;
        let users = recent.iter().map(|m| &m.1).collect::<HashSet<_>>().len() as f64;
        let observed = now.duration_since(self.joined_at.unwrap()).as_secs_f64();
        let rate = (observed >= 70.0).then(|| {
            let prior = self
                .messages
                .iter()
                .filter(|m| m.0 <= now && (10..130).contains(&now.duration_since(m.0).as_secs()))
                .count();
            let baseline = prior as f64 / (observed - 10.0).min(120.0) * 10.0;
            count / baseline.max(1.0)
        });
        (Some(count), Some(users), rate)
    }
}

pub struct Chat {
    pub metrics: Arc<Mutex<Metrics>>,
    task: tokio::task::JoinHandle<()>,
}

impl Chat {
    pub fn start(login: String) -> Self {
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let shared = metrics.clone();
        let task = tokio::spawn(async move {
            loop {
                if let Err(error) = listen(&login, &shared).await {
                    tracing::debug!(%error, %login, "Chat-Verbindung wird erneut aufgebaut");
                }
                *shared.lock().await = Metrics::default();
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
        });
        Self { metrics, task }
    }
}

impl Drop for Chat {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn author<'a>(line: &'a str, login: &str) -> Option<&'a str> {
    let (prefix, rest) = line.strip_prefix(':')?.split_once(' ')?;
    let (command, _) = rest.split_once(" :")?;
    if command != format!("PRIVMSG #{login}") {
        return None;
    }
    prefix.split_once('!').map(|(user, _)| user)
}

fn tls_connector() -> anyhow::Result<Connector> {
    // S3 enables aws-lc while reqwest enables ring. Pick explicitly for this client
    // instead of relying on rustls' ambiguous process-wide auto-detection.
    let tls = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_safe_default_protocol_versions()?
    .with_root_certificates(rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    })
    .with_no_client_auth();
    Ok(Connector::Rustls(Arc::new(tls)))
}

async fn listen(login: &str, metrics: &Mutex<Metrics>) -> anyhow::Result<()> {
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(20),
        connect_async_tls_with_config(
            "wss://irc-ws.chat.twitch.tv:443",
            None,
            false,
            Some(tls_connector()?),
        ),
    )
    .await??;
    socket
        .send(Message::Text(
            format!("NICK justinfan{}", rand::random_range(10000..999999)).into(),
        ))
        .await?;
    // Twitch expects one IRC command per WebSocket message.
    socket
        .send(Message::Text(format!("JOIN #{login}").into()))
        .await?;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
    let mut received = Instant::now();
    loop {
        let message = tokio::select! {
            message = socket.next() => match message { Some(message) => message, None => break },
            _ = heartbeat.tick() => {
                anyhow::ensure!(received.elapsed() < Duration::from_secs(90), "Chat heartbeat timeout");
                socket.send(Message::Ping(Vec::new().into())).await?;
                continue;
            }
        };
        received = Instant::now();
        match message? {
            Message::Text(text) => {
                for line in text.split("\r\n") {
                    if let Some(ping) = line.strip_prefix("PING ") {
                        socket
                            .send(Message::Text(format!("PONG {ping}").into()))
                            .await?;
                    }
                    if line.contains(" 366 ") && line.contains(&format!(" #{login} ")) {
                        let mut metrics = metrics.lock().await;
                        metrics.connected = true;
                        metrics.joined_at = Some(Instant::now());
                    }
                    if line.contains(" RECONNECT") {
                        return Ok(());
                    }
                    if let Some(user) = author(line, login) {
                        let mut metrics = metrics.lock().await;
                        let now = Instant::now();
                        metrics.prune(now);
                        metrics.messages.push_back((now, user.to_owned()));
                        // Bounded memory on very large channels.
                        if metrics.messages.len() > 100_000 {
                            metrics.messages.pop_front();
                        }
                    }
                }
            }
            Message::Ping(data) => socket.send(Message::Pong(data)).await?,
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_tls_supports_both_crypto_providers_in_the_dependency_graph() {
        assert!(matches!(tls_connector().unwrap(), Connector::Rustls(_)));
    }
    #[test]
    fn counts_distinct_chatters_and_never_treats_missing_chat_as_zero() {
        assert_eq!(
            author(":alice!alice@host PRIVMSG #test :hello", "test"),
            Some("alice")
        );
        assert_eq!(
            author(":alice!alice@host PRIVMSG #other :hello", "test"),
            None
        );
        let now = Instant::now();
        let mut metrics = Metrics {
            connected: true,
            joined_at: Some(now - Duration::from_secs(90)),
            messages: VecDeque::from([
                (now, "alice".into()),
                (now, "alice".into()),
                (now, "bob".into()),
            ]),
        };
        assert_eq!(metrics.snapshot_at(now), (Some(3.0), Some(2.0), Some(3.0)));
        assert_eq!(
            metrics.snapshot_at(now - Duration::from_secs(1)),
            (Some(0.0), Some(0.0), Some(0.0))
        );
        metrics.connected = false;
        assert_eq!(metrics.snapshot_at(now), (None, None, None));
    }
}
