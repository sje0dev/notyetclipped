use std::time::{Duration, Instant};

use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt;
use serde_json::json;
use sqlx::{PgPool, postgres::PgListener};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::state::State;

pub async fn publish(pool: &PgPool, kind: &str, id: Option<Uuid>) {
    publish_data(
        pool,
        json!({"type": kind, "id": id, "at": chrono::Utc::now()}),
    )
    .await;
}

pub async fn publish_data(pool: &PgPool, payload: serde_json::Value) {
    let payload = payload.to_string();
    // PostgreSQL NOTIFY payloads must stay below 8000 bytes.
    if payload.len() >= 8000 {
        tracing::warn!(bytes = payload.len(), "Event exceeds NOTIFY payload limit");
        return;
    }
    if let Err(error) = sqlx::query("SELECT pg_notify('studio_events', $1)")
        .bind(payload)
        .execute(pool)
        .await
    {
        tracing::warn!(%error, "Could not publish update");
    }
}

pub async fn listen(state: State) {
    loop {
        let result: anyhow::Result<()> = async {
            let mut listener = PgListener::connect_with(&state.pool).await?;
            listener.listen("studio_events").await?;
            let _ = state.events.send(json!({"type":"resync"}).to_string());
            loop {
                let message = listener.recv().await?;
                let _ = state.events.send(message.payload().to_owned());
            }
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%error, "PostgreSQL event listener reconnecting");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub async fn websocket(
    request: HttpRequest,
    body: web::Payload,
    state: web::Data<State>,
) -> Result<HttpResponse, Error> {
    if let Some(origin) = request.headers().get("origin") {
        let same_origin = origin
            .to_str()
            .ok()
            .and_then(|origin| url::Url::parse(origin).ok())
            .is_some_and(|origin| {
                origin[url::Position::BeforeHost..url::Position::AfterPort]
                    == *request.connection_info().host()
            });
        if !same_origin {
            return Ok(HttpResponse::Forbidden().finish());
        }
    }
    let (response, mut session, mut incoming) = actix_ws::handle(&request, body)?;
    let mut events = state.events.subscribe();
    actix_web::rt::spawn(async move {
        if session
            .text(json!({"type":"hello", "at":chrono::Utc::now()}).to_string())
            .await
            .is_err()
        {
            return;
        }
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
        let mut last_pong = Instant::now();
        loop {
            tokio::select! {
                message = incoming.next() => match message {
                    Some(Ok(Message::Ping(bytes))) => { if session.pong(&bytes).await.is_err() { break; } }
                    Some(Ok(Message::Pong(_))) => last_pong = Instant::now(),
                    Some(Ok(Message::Close(reason))) => { let _ = session.close(reason).await; return; }
                    Some(Ok(Message::Text(text))) if text == "ping" => { let _ = session.text("{\"type\":\"pong\"}").await; }
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Continuation(_))) | Some(Err(_)) | None => break,
                    _ => {}
                },
                event = events.recv() => {
                    let message = match event {
                        Ok(message) => message,
                        Err(broadcast::error::RecvError::Lagged(_)) => "{\"type\":\"resync\"}".into(),
                        Err(_) => break,
                    };
                    if session.text(message).await.is_err() { break; }
                }
                _ = heartbeat.tick() => {
                    if last_pong.elapsed() > Duration::from_secs(45) || session.ping(b"studio").await.is_err() { break; }
                }
            }
        }
        let _ = session.close(None).await;
    });
    Ok(response)
}
