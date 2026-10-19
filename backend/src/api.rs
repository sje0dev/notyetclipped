use std::future::{Ready, ready};

use actix_web::{
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, dev::Payload, http::StatusCode,
    middleware, web,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    ai, chatgpt, editing,
    error::{ApiError, conflict, invalid, missing},
    events, jobs,
    media::safe_path,
    models::{ACTIVE, Clip, ManifestEntry, Recording, Settings, Streamer, StreamerInput},
    services,
    state::State,
    triggers::{Engine, Profile, Signal},
};

type Response = Result<web::Json<Value>, ApiError>;

pub struct ApplicationRequest;
impl FromRequest for ApplicationRequest {
    type Error = ApiError;
    type Future = Ready<Result<Self, Self::Error>>;
    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(
            if request
                .headers()
                .get("x-requested-with")
                .is_some_and(|v| v == "notyetclipped")
            {
                Ok(Self)
            } else {
                Err(ApiError(
                    StatusCode::FORBIDDEN,
                    "Anwendungs-Header fehlt.".into(),
                ))
            },
        )
    }
}

pub async fn serve(state: State) -> anyhow::Result<()> {
    let bind = state.config.bind.clone();
    tokio::spawn(events::listen(state.clone()));
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(
                web::JsonConfig::default()
                    .limit(65536)
                    .error_handler(|error, _| {
                        invalid(format!("Ungültige JSON-Daten: {error}")).into()
                    }),
            )
            .wrap(middleware::DefaultHeaders::new().add(("Cache-Control", "no-store")))
            .wrap(middleware::Logger::default().exclude("/api/ai/callback"))
            .configure(routes)
    })
    .bind(bind)?
    .run()
    .await?;
    Ok(())
}

pub fn routes(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api")
            .route("/ping", web::get().to(ping))
            .route("/health", web::get().to(health))
            .route("/ai", web::get().to(ai_status))
            .route("/ai/settings", web::put().to(ai_settings))
            .route("/ai/login", web::post().to(ai_login))
            .route("/ai/logout", web::post().to(ai_logout))
            .route("/ai/callback", web::get().to(ai_callback))
            .route("/ai/models", web::get().to(ai_models))
            .route("/ws", web::get().to(events::websocket))
            .route("/dashboard", web::get().to(dashboard))
            .route("/activities", web::get().to(activities))
            .route("/jobs", web::get().to(list_jobs))
            .route("/jobs/{id}/logs", web::get().to(job_logs))
            .route("/edits", web::get().to(list_edits))
            .route("/edits/{id}/retry", web::post().to(retry_edit))
            .route("/streamers", web::get().to(list_streamers))
            .route("/streamers", web::post().to(add_streamer))
            .route("/streamers/{id}", web::patch().to(update_streamer))
            .route("/streamers/{id}", web::delete().to(remove_streamer))
            .route("/streamers/{id}/clip", web::post().to(manual_clip))
            .route("/streamers/{id}/signals", web::get().to(trigger_signals))
            .route("/streamers/{id}/analyses", web::get().to(streamer_analyses))
            .route("/streamers/{id}/editing", web::get().to(editing_profile))
            .route(
                "/streamers/{id}/editing",
                web::put().to(save_editing_profile),
            )
            .route(
                "/streamers/{id}/triggers/test",
                web::post().to(test_triggers),
            )
            .route(
                "/streamers/{id}/triggers/copy",
                web::post().to(copy_triggers),
            )
            .route("/clips", web::get().to(list_clips))
            .route("/clips/{id}", web::get().to(clip_detail))
            .route("/clips/{id}", web::patch().to(update_clip))
            .route("/clips/{id}", web::delete().to(delete_clip))
            .route("/clips/{id}/retry", web::post().to(retry_clip))
            .route("/clips/{id}/edits", web::post().to(queue_edit))
            .route("/settings", web::get().to(get_settings))
            .route("/settings", web::put().to(update_settings))
            .route("/storage", web::get().to(storage_info)),
    );
}

async fn ai_status(state: web::Data<State>) -> Response {
    Ok(web::Json(ai::status(&state).await?))
}

async fn streamer_analyses(state: web::Data<State>, id: web::Path<Uuid>) -> Response {
    let runs: Vec<ai::Run> = sqlx::query_as(
        "SELECT * FROM ai_runs WHERE streamer_id=$1 ORDER BY created_at DESC LIMIT 10",
    )
    .bind(*id)
    .fetch_all(&state.pool)
    .await?;
    Ok(web::Json(json!(runs)))
}

async fn activities(state: web::Data<State>) -> Response {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(a) FROM (SELECT id,streamer_id,kind,message,created_at FROM activities ORDER BY created_at DESC,id DESC LIMIT 30) a")
        .fetch_all(&state.pool).await?;
    Ok(web::Json(json!(rows)))
}

async fn list_jobs(state: web::Data<State>) -> Response {
    Ok(web::Json(jobs::list(&state.pool).await?))
}

#[derive(Deserialize)]
struct LogQuery {
    download: Option<bool>,
}
async fn job_logs(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    query: web::Query<LogQuery>,
) -> Result<HttpResponse, ApiError> {
    let rows: Vec<(i64,DateTime<Utc>,String,String)> = sqlx::query_as("SELECT id,created_at,level,message FROM (SELECT * FROM job_logs WHERE job_id=$1 ORDER BY id DESC LIMIT 500) recent ORDER BY id")
        .bind(*id).fetch_all(&state.pool).await?;
    if query.download == Some(true) {
        let text = rows
            .iter()
            .map(|(_, at, level, message)| format!("{} [{level}] {message}\n", at.to_rfc3339()))
            .collect::<String>();
        return Ok(HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .insert_header((
                "Content-Disposition",
                format!("attachment; filename=\"job-{id}.log\""),
            ))
            .body(text));
    }
    Ok(HttpResponse::Ok().json(rows.into_iter().map(|(id,at,level,message)|json!({"id":id,"created_at":at,"level":level,"message":message})).collect::<Vec<_>>()))
}

async fn editing_profile(state: web::Data<State>, id: web::Path<Uuid>) -> Response {
    get_streamer(&state, *id).await?;
    let value: Option<Value> =
        sqlx::query_scalar("SELECT profile FROM editing_profiles WHERE streamer_id=$1")
            .bind(*id)
            .fetch_optional(&state.pool)
            .await?;
    let profile: editing::Profile = value
        .map(serde_json::from_value)
        .transpose()
        .map_err(anyhow::Error::from)?
        .unwrap_or_default();
    let key: Option<String> = sqlx::query_scalar("SELECT thumbnail_key FROM clips WHERE streamer_id=$1 AND status='ready' AND thumbnail_key IS NOT NULL ORDER BY created_at DESC LIMIT 1").bind(*id).fetch_optional(&state.pool).await?;
    let preview = match key {
        Some(key) => Some(state.storage.url(&key, None).await?),
        None => None,
    };
    Ok(web::Json(json!({"profile":profile,"preview_url":preview})))
}

async fn save_editing_profile(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    input: web::Json<editing::Profile>,
    _: ApplicationRequest,
) -> Response {
    get_streamer(&state, *id).await?;
    input.validate().map_err(|e| invalid(e.to_string()))?;
    sqlx::query("INSERT INTO editing_profiles(streamer_id,profile) VALUES ($1,$2) ON CONFLICT(streamer_id) DO UPDATE SET profile=EXCLUDED.profile")
        .bind(*id).bind(json!(&*input)).execute(&state.pool).await?;
    Ok(web::Json(json!(&*input)))
}

async fn queue_edit(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Response {
    let mut tx = state.pool.begin().await?;
    let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1 FOR UPDATE")
        .bind(*id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    if clip.status != "ready" {
        return Err(conflict(
            "Nur fertige Clips können zur Bearbeitung geschickt werden.",
        ));
    }
    let value: Option<Value> =
        sqlx::query_scalar("SELECT profile FROM editing_profiles WHERE streamer_id=$1")
            .bind(clip.streamer_id)
            .fetch_optional(&mut *tx)
            .await?;
    let profile: editing::Profile = value
        .map(serde_json::from_value)
        .transpose()
        .map_err(anyhow::Error::from)?
        .unwrap_or_default();
    let edit_id = editing::enqueue(&mut tx, clip.id, &profile).await?;
    tx.commit().await?;
    events::publish(&state.pool, "edit.updated", Some(edit_id)).await;
    Ok(web::Json(json!({"id":edit_id})))
}

async fn list_edits(state: web::Data<State>) -> Response {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(e)||jsonb_build_object('title',c.title,'streamer_id',c.streamer_id,'streamer_login',s.login) FROM edits e JOIN clips c ON c.id=e.clip_id JOIN streamers s ON s.id=c.streamer_id ORDER BY e.created_at DESC LIMIT 100")
        .fetch_all(&state.pool).await?;
    let mut result = Vec::new();
    for mut row in rows {
        let key = row.as_object_mut().unwrap().remove("video_key").unwrap();
        if row["status"] == "ready" {
            let key = key.as_str().unwrap();
            row["video_url"] = json!(state.storage.url(key, None).await?);
            row["download_url"] = json!(state.storage.url(key, Some("hochkant.mp4")).await?);
        }
        result.push(row);
    }
    Ok(web::Json(json!(result)))
}

async fn retry_edit(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Response {
    let mut tx = state.pool.begin().await?;
    let clip_id: Uuid = sqlx::query_scalar("SELECT clip_id FROM edits WHERE id=$1")
        .bind(*id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1 FOR UPDATE")
        .bind(clip_id)
        .fetch_one(&mut *tx)
        .await?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM edits WHERE clip_id=$1 AND status IN ('queued','rendering'))",
    )
    .bind(clip_id)
    .fetch_one(&mut *tx)
    .await?;
    if clip.status != "ready" || active {
        return Err(conflict(
            "Quelle nicht bereit oder Bearbeitung bereits aktiv.",
        ));
    }
    let changed=sqlx::query("UPDATE edits SET status='queued',error=NULL,finished_at=NULL WHERE id=$1 AND status='failed'").bind(*id).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 {
        return Err(conflict(
            "Nur fehlgeschlagene Bearbeitungen können wiederholt werden.",
        ));
    }
    tx.commit().await?;
    events::publish(&state.pool, "edit.updated", Some(*id)).await;
    Ok(web::Json(json!({"ok":true})))
}

async fn ai_models(state: web::Data<State>) -> Response {
    Ok(web::Json(
        chatgpt::models(&state)
            .await
            .map_err(|e| invalid(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    account_id: Option<String>,
}

async fn ai_login(
    state: web::Data<State>,
    input: web::Json<LoginRequest>,
    _: ApplicationRequest,
) -> Response {
    let url = chatgpt::login(&state, input.account_id.as_deref())
        .await
        .map_err(|e| invalid(e.to_string()))?;
    Ok(web::Json(json!({"url":url})))
}

async fn ai_logout(state: web::Data<State>, _: ApplicationRequest) -> Response {
    Ok(web::Json(json!({"revoked":chatgpt::logout(&state).await?})))
}

async fn ai_callback(
    state: web::Data<State>,
    query: web::Query<chatgpt::Callback>,
) -> HttpResponse {
    match chatgpt::callback(&state, query.into_inner()).await {
        Ok(()) => HttpResponse::SeeOther().insert_header(("Location", "/settings?chatgpt=connected")).finish(),
        Err(error) => HttpResponse::BadRequest().content_type("text/plain; charset=utf-8").body(format!("ChatGPT-Anmeldung fehlgeschlagen: {error}\n\nBitte zu /settings zurückkehren und die Anmeldung erneut starten.")),
    }
}

async fn ai_settings(
    state: web::Data<State>,
    input: web::Json<ai::Settings>,
    _: ApplicationRequest,
) -> Response {
    input.validate().map_err(|e| invalid(e.to_string()))?;
    if input.enabled {
        let models = chatgpt::models(&state)
            .await
            .map_err(|e| invalid(e.to_string()))?;
        if !models
            .as_array()
            .is_some_and(|models| models.iter().any(|m| m["id"] == input.model))
        {
            return Err(invalid(
                "Dieses Modell ist für dein ChatGPT-Konto nicht verfügbar.",
            ));
        }
    }
    sqlx::query("UPDATE ai_control SET settings=$1,paused_reason=NULL WHERE id=1")
        .bind(json!(&*input))
        .execute(&state.pool)
        .await?;
    events::publish(&state.pool, "ai.updated", None).await;
    Ok(web::Json(json!(&*input)))
}

async fn ping(state: web::Data<State>) -> Response {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(web::Json(json!({"ok":true})))
}

async fn health(state: web::Data<State>) -> web::Json<Value> {
    let (database, storage) = tokio::join!(
        sqlx::query("SELECT 1").execute(&state.pool),
        state.storage.check()
    );
    let worker: Option<(DateTime<Utc>, Value)> =
        sqlx::query_as("SELECT heartbeat_at,details FROM worker_state WHERE id=1")
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
    web::Json(json!({
        "database":database.is_ok(), "storage":storage.is_ok(),
        "worker":worker.as_ref().is_some_and(|w| w.0 > Utc::now() - chrono::Duration::seconds(30)),
        "worker_details":worker.map_or(json!({}), |w| w.1), "bucket":state.config.s3_bucket,
    }))
}

#[derive(FromRow, Serialize)]
struct ActivityView {
    id: i64,
    kind: String,
    message: String,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct JoinedClip {
    #[sqlx(flatten)]
    clip: Clip,
    streamer_login: String,
}

async fn dashboard(state: web::Data<State>) -> Response {
    let streamers = services::streamer_views(&state.pool).await?;
    let counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT status,count(*) FROM clips GROUP BY status")
            .fetch_all(&state.pool)
            .await?;
    let count = |status: &str| counts.iter().find(|c| c.0 == status).map_or(0, |c| c.1);
    let today: i64 = sqlx::query_scalar("SELECT count(*) FROM clips WHERE created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' AND status != 'deleting'").fetch_one(&state.pool).await?;
    let stored: i64 = sqlx::query_scalar(
        "SELECT (SELECT coalesce(sum(size_bytes),0) FROM clips WHERE status='ready')::bigint+(SELECT coalesce(sum(size_bytes),0) FROM edits WHERE status='ready')::bigint",
    )
    .fetch_one(&state.pool)
    .await?;
    let latest: Vec<JoinedClip> = sqlx::query_as("SELECT c.*, s.login AS streamer_login FROM clips c JOIN streamers s ON s.id=c.streamer_id WHERE c.status != 'deleting' ORDER BY c.created_at DESC LIMIT 6").fetch_all(&state.pool).await?;
    let mut clips = Vec::new();
    for row in latest {
        clips.push(services::clip_view(&state, &row.clip, &row.streamer_login, false).await?);
    }
    let activity: Vec<ActivityView> = sqlx::query_as(
        "SELECT id,kind,message,created_at FROM activities ORDER BY created_at DESC LIMIT 8",
    )
    .fetch_all(&state.pool)
    .await?;
    let trend: Vec<(String, i64, i64)> = sqlx::query_as("WITH days AS (SELECT generate_series((now() AT TIME ZONE 'UTC')::date - 6, (now() AT TIME ZONE 'UTC')::date, interval '1 day') AS day) SELECT to_char(d.day,'YYYY-MM-DD'),count(c.id) FILTER (WHERE c.trigger != 'manual'),count(c.id) FILTER (WHERE c.trigger = 'manual') FROM days d LEFT JOIN clips c ON c.created_at >= d.day AT TIME ZONE 'UTC' AND c.created_at < (d.day + interval '1 day') AT TIME ZONE 'UTC' AND c.status != 'deleting' GROUP BY d.day ORDER BY d.day")
        .fetch_all(&state.pool).await?;
    let trend: Vec<_> = trend
        .into_iter()
        .map(|(day, automatic, manual)| json!({"day":day,"automatic":automatic,"manual":manual}))
        .collect();
    Ok(web::Json(json!({"stats": {
        "recording":streamers.iter().filter(|s| s["status"]=="recording").count(), "streamers":streamers.len(),
        "clips_today":today, "clips_ready":count("ready"), "stored_bytes":stored,
        "pending":ACTIVE.iter().map(|status| count(status)).sum::<i64>(), "failed":count("failed"),
    }, "trend":trend, "streamers":streamers, "clips":clips, "activity":activity, "settings":services::settings(&state.pool).await?})))
}

async fn list_streamers(state: web::Data<State>) -> Response {
    Ok(web::Json(json!(
        services::streamer_views(&state.pool).await?
    )))
}

async fn get_streamer(state: &State, id: Uuid) -> Result<Streamer, ApiError> {
    sqlx::query_as("SELECT * FROM streamers WHERE id=$1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(missing)
}

async fn add_streamer(
    state: web::Data<State>,
    payload: web::Json<StreamerInput>,
    _: ApplicationRequest,
) -> Result<HttpResponse, ApiError> {
    let mut input = payload.into_inner();
    input.validate().map_err(invalid)?;
    let mut tx = state.pool.begin().await?;
    let existing: Option<Streamer> =
        sqlx::query_as("SELECT * FROM streamers WHERE login=$1 FOR UPDATE")
            .bind(&input.login)
            .fetch_optional(&mut *tx)
            .await?;
    if existing.as_ref().is_some_and(|s| s.deleted_at.is_none()) {
        return Err(conflict("Dieser Streamer ist bereits in deiner Liste."));
    }
    let id = existing.map_or_else(Uuid::new_v4, |s| s.id);
    let streamer: Streamer = sqlx::query_as("INSERT INTO streamers(id,login,enabled,quality,priority,pre_roll,post_roll,threshold_db,auto_clip,status,trigger_profile) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(id) DO UPDATE SET enabled=EXCLUDED.enabled,quality=EXCLUDED.quality,priority=EXCLUDED.priority,pre_roll=EXCLUDED.pre_roll,post_roll=EXCLUDED.post_roll,threshold_db=EXCLUDED.threshold_db,auto_clip=EXCLUDED.auto_clip,status=EXCLUDED.status,trigger_profile=EXCLUDED.trigger_profile,deleted_at=NULL,last_error=NULL,next_check_at=NULL RETURNING *")
        .bind(id).bind(&input.login).bind(input.enabled).bind(input.quality).bind(input.priority).bind(input.pre_roll).bind(input.post_roll).bind(input.threshold_db).bind(input.auto_clip).bind(if input.enabled {"waiting"} else {"paused"}).bind(json!(input.trigger_profile)).fetch_one(&mut *tx).await?;
    services::activity(
        &mut tx,
        "system",
        &format!("@{} zur Watchlist hinzugefügt.", input.login),
        Some(id),
    )
    .await?;
    tx.commit().await?;
    events::publish(&state.pool, "stream.updated", Some(id)).await;
    Ok(HttpResponse::Created().json(streamer))
}

async fn update_streamer(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    payload: web::Json<Value>,
    _: ApplicationRequest,
) -> Response {
    let mut tx = state.pool.begin().await?;
    let streamer: Streamer =
        sqlx::query_as("SELECT * FROM streamers WHERE id=$1 AND deleted_at IS NULL FOR UPDATE")
            .bind(*id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(missing)?;
    let mut input = json!({"login":streamer.login,"enabled":streamer.enabled,"quality":streamer.quality,
        "priority":streamer.priority,"pre_roll":streamer.pre_roll,"post_roll":streamer.post_roll,
        "threshold_db":streamer.threshold_db,"auto_clip":streamer.auto_clip,"trigger_profile":streamer.trigger_profile});
    let patch = payload
        .as_object()
        .ok_or_else(|| invalid("JSON-Objekt erwartet."))?;
    for (key, value) in patch {
        if key == "login" || !input.as_object().unwrap().contains_key(key) {
            return Err(invalid("Unbekanntes Einstellungsfeld."));
        }
        input[key] = value.clone();
    }
    let mut input: StreamerInput =
        serde_json::from_value(input).map_err(|_| invalid("Ungültige Streamer-Einstellungen."))?;
    input.validate().map_err(invalid)?;
    let result: Streamer = sqlx::query_as("UPDATE streamers SET enabled=$2,quality=$3,priority=$4,pre_roll=$5,post_roll=$6,threshold_db=$7,auto_clip=$8,status=CASE WHEN $9 THEN $10 ELSE status END,next_check_at=CASE WHEN $9 THEN NULL ELSE next_check_at END,last_error=CASE WHEN $9 THEN NULL ELSE last_error END,trigger_profile=$11 WHERE id=$1 RETURNING *")
        .bind(*id).bind(input.enabled).bind(input.quality).bind(input.priority).bind(input.pre_roll).bind(input.post_roll).bind(input.threshold_db).bind(input.auto_clip).bind(patch.contains_key("enabled"))
        .bind(if input.enabled {"waiting"} else {"paused"}).bind(json!(input.trigger_profile)).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    events::publish(&state.pool, "stream.updated", Some(*id)).await;
    Ok(web::Json(json!(result)))
}

async fn signal_history(
    state: &State,
    id: Uuid,
) -> Result<(Option<Recording>, Vec<Signal>), ApiError> {
    get_streamer(state, id).await?;
    let recording: Option<Recording> = sqlx::query_as(
        "SELECT * FROM recordings WHERE streamer_id=$1 ORDER BY started_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let mut frames = Vec::new();
    if let Some(recording) = &recording {
        let batches: Vec<Value> = sqlx::query_scalar("SELECT frames FROM (SELECT start_ms,frames FROM trigger_samples WHERE recording_id=$1 AND created_at > now()-interval '15 minutes' ORDER BY start_ms DESC LIMIT 900) recent ORDER BY start_ms")
            .bind(recording.id).fetch_all(&state.pool).await?;
        for batch in batches {
            frames
                .extend(serde_json::from_value::<Vec<Signal>>(batch).map_err(anyhow::Error::from)?);
        }
        if let Some(last) = frames.last() {
            let cutoff = last.time_ms - 900_000;
            frames.retain(|frame| frame.time_ms > cutoff);
        }
    }
    Ok((recording, frames))
}

async fn trigger_signals(state: web::Data<State>, id: web::Path<Uuid>) -> Response {
    let (recording, frames) = signal_history(&state, *id).await?;
    let cutoff = frames.last().map_or(0, |frame| frame.time_ms - 300_000);
    let recent: Vec<_> = frames
        .iter()
        .filter(|frame| frame.time_ms > cutoff)
        .collect();
    let (count, last): (i64, Option<DateTime<Utc>>) = sqlx::query_as("SELECT count(*),max(created_at) FROM trigger_events WHERE streamer_id=$1 AND created_at > now()-interval '1 hour'")
        .bind(*id).fetch_one(&state.pool).await?;
    Ok(web::Json(
        json!({"recording":recording,"frames":recent,"hourly_count":count,"last_trigger_at":last}),
    ))
}

async fn test_triggers(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    profile: web::Json<Profile>,
    _: ApplicationRequest,
) -> Response {
    profile.validate().map_err(invalid)?;
    let (recording, frames) = signal_history(&state, *id).await?;
    let mut engine = Engine::default();
    let hits: Vec<_> = frames
        .iter()
        .filter_map(|signal| engine.evaluate(&profile, signal))
        .collect();
    let seconds = frames.len() as f64 / 4.0;
    Ok(web::Json(
        json!({"hits":hits,"seconds":seconds,"recording_started_at":recording.map(|r| r.started_at),
        "chat_available":frames.iter().any(|s| s.chat_users.is_some()),"sample_count":frames.len()}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyTriggers {
    target_ids: Vec<Uuid>,
}

async fn copy_triggers(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    payload: web::Json<CopyTriggers>,
    _: ApplicationRequest,
) -> Response {
    let mut ids = payload.target_ids.clone();
    ids.sort();
    ids.dedup();
    ids.retain(|target| target != &*id);
    if ids.is_empty() || ids.len() > 50 {
        return Err(invalid("Bitte 1–50 andere Streamer auswählen."));
    }
    let mut locks = ids.clone();
    locks.push(*id);
    locks.sort();
    let mut tx = state.pool.begin().await?;
    let rows: Vec<Streamer> = sqlx::query_as(
        "SELECT * FROM streamers WHERE id=ANY($1) AND deleted_at IS NULL ORDER BY id FOR UPDATE",
    )
    .bind(&locks)
    .fetch_all(&mut *tx)
    .await?;
    if rows.len() != locks.len() {
        return Err(missing());
    }
    let source = rows.iter().find(|row| row.id == *id).ok_or_else(missing)?;
    sqlx::query(
        "UPDATE streamers SET trigger_profile=$2,pre_roll=$3,post_roll=$4 WHERE id=ANY($1)",
    )
    .bind(&ids)
    .bind(&source.trigger_profile)
    .bind(source.pre_roll)
    .bind(source.post_roll)
    .execute(&mut *tx)
    .await?;
    services::activity(
        &mut tx,
        "system",
        &format!(
            "Trigger von @{} auf {} Kanäle kopiert.",
            source.login,
            ids.len()
        ),
        Some(*id),
    )
    .await?;
    tx.commit().await?;
    for id in &ids {
        events::publish(&state.pool, "stream.updated", Some(*id)).await;
    }
    Ok(web::Json(json!({"copied":ids.len()})))
}

async fn remove_streamer(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Response {
    get_streamer(&state, *id).await?;
    sqlx::query("UPDATE streamers SET enabled=false,deleted_at=now(),status='paused' WHERE id=$1")
        .bind(*id)
        .execute(&state.pool)
        .await?;
    events::publish(&state.pool, "stream.updated", Some(*id)).await;
    Ok(web::Json(json!({"ok":true})))
}

async fn manual_clip(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Result<HttpResponse, ApiError> {
    let streamer = get_streamer(&state, *id).await?;
    let mut tx = state.pool.begin().await?;
    let recording: Recording = sqlx::query_as(
        "SELECT * FROM recordings WHERE streamer_id=$1 AND ended_at IS NULL FOR UPDATE",
    )
    .bind(*id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| conflict("Für diesen Kanal läuft gerade keine Aufnahme."))?;
    let clip = services::create_clip(
        &mut tx,
        &streamer,
        &recording,
        recording.latest_ms,
        "manual",
        0.0,
        "Manuell aus dem Live-Puffer gesichert.",
    )
    .await
    .map_err(|e| conflict(e.to_string()))?;
    tx.commit().await?;
    events::publish(&state.pool, "clip.updated", Some(clip.id)).await;
    Ok(HttpResponse::Accepted()
        .json(services::clip_view(&state, &clip, &streamer.login, false).await?))
}

#[derive(Deserialize)]
#[serde(default)]
struct ClipQuery {
    search: String,
    streamer_id: Option<Uuid>,
    status: String,
    favorite: bool,
    sort: String,
    offset: i64,
    limit: i64,
}
impl Default for ClipQuery {
    fn default() -> Self {
        Self {
            search: String::new(),
            streamer_id: None,
            status: "all".into(),
            favorite: false,
            sort: "newest".into(),
            offset: 0,
            limit: 24,
        }
    }
}
async fn list_clips(state: web::Data<State>, query: web::Query<ClipQuery>) -> Response {
    if !(1..=100).contains(&query.limit) || query.offset < 0 || query.search.len() > 400 {
        return Err(invalid("Ungültiger Clip-Filter."));
    }
    let search = if query.search.is_empty() {
        String::new()
    } else {
        format!(
            "%{}%",
            query
                .search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    };
    let clause = "FROM clips c JOIN streamers s ON s.id=c.streamer_id WHERE c.status != 'deleting' AND ($1='' OR c.title ILIKE $1 OR s.login ILIKE $1) AND ($2::uuid IS NULL OR c.streamer_id=$2) AND ($3='all' OR c.status=$3) AND (NOT $4 OR c.favorite)";
    let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) {clause}"))
        .bind(&search)
        .bind(query.streamer_id)
        .bind(&query.status)
        .bind(query.favorite)
        .fetch_one(&state.pool)
        .await?;
    let rows: Vec<JoinedClip> = sqlx::query_as(&format!("SELECT c.*,s.login AS streamer_login {clause} ORDER BY CASE WHEN $5='score' THEN c.score END DESC,c.created_at DESC,c.id LIMIT $6 OFFSET $7"))
        .bind(search).bind(query.streamer_id).bind(&query.status).bind(query.favorite).bind(&query.sort).bind(query.limit).bind(query.offset).fetch_all(&state.pool).await?;
    let mut items = Vec::new();
    for row in rows {
        items.push(services::clip_view(&state, &row.clip, &row.streamer_login, false).await?);
    }
    Ok(web::Json(json!({"items":items,"total":count})))
}

async fn get_clip(state: &State, id: Uuid) -> Result<JoinedClip, ApiError> {
    sqlx::query_as("SELECT c.*,s.login AS streamer_login FROM clips c JOIN streamers s ON s.id=c.streamer_id WHERE c.id=$1").bind(id).fetch_optional(&state.pool).await?.ok_or_else(missing)
}
async fn clip_detail(state: web::Data<State>, id: web::Path<Uuid>) -> Response {
    let row = get_clip(&state, *id).await?;
    Ok(web::Json(
        services::clip_view(&state, &row.clip, &row.streamer_login, true).await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipPatch {
    title: Option<String>,
    favorite: Option<bool>,
    rating: Option<i32>,
}
async fn update_clip(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    payload: web::Json<ClipPatch>,
    _: ApplicationRequest,
) -> Response {
    let row = get_clip(&state, *id).await?;
    if payload
        .title
        .as_ref()
        .is_some_and(|title| title.trim().is_empty() || title.chars().count() > 200)
        || payload
            .rating
            .is_some_and(|rating| !(-1..=1).contains(&rating))
    {
        return Err(invalid("Ungültiger Titel oder Bewertung."));
    }
    let clip: Clip = sqlx::query_as("UPDATE clips SET title=coalesce($2,title),favorite=coalesce($3,favorite),rating=coalesce($4,rating) WHERE id=$1 RETURNING *")
        .bind(*id).bind(&payload.title).bind(payload.favorite).bind(payload.rating).fetch_one(&state.pool).await?;
    events::publish(&state.pool, "clip.updated", Some(*id)).await;
    Ok(web::Json(
        services::clip_view(&state, &clip, &row.streamer_login, true).await?,
    ))
}
async fn delete_clip(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Result<HttpResponse, ApiError> {
    get_clip(&state, *id).await?;
    sqlx::query("UPDATE clips SET status='deleting',retry_at=NULL,attempts=0 WHERE id=$1")
        .bind(*id)
        .execute(&state.pool)
        .await?;
    events::publish(&state.pool, "clip.updated", Some(*id)).await;
    Ok(HttpResponse::Accepted().json(json!({"ok":true})))
}
async fn retry_clip(
    state: web::Data<State>,
    id: web::Path<Uuid>,
    _: ApplicationRequest,
) -> Result<HttpResponse, ApiError> {
    let row = get_clip(&state, *id).await?;
    if row.clip.status != "failed" {
        return Err(conflict(
            "Nur fehlgeschlagene Clips können erneut verarbeitet werden.",
        ));
    }
    let mut tx = state.pool.begin().await?;
    // Match the reservation/GC lock order before checking and pinning source files.
    sqlx::query("SELECT id FROM recordings WHERE id=$1 FOR UPDATE")
        .bind(row.clip.recording_id)
        .execute(&mut *tx)
        .await?;
    let manifest: Vec<ManifestEntry> =
        serde_json::from_value(row.clip.manifest.clone()).map_err(|e| invalid(e.to_string()))?;
    let rendered = state
        .config
        .data_dir
        .join("exports")
        .join(id.to_string())
        .join("video.mp4");
    let available = !manifest.is_empty()
        && manifest.iter().all(|entry| {
            safe_path(&state.config.data_dir, &entry.path).is_ok_and(|path| path.is_file())
        });
    if !rendered.is_file() && !available {
        return Err(conflict(
            "Die Quelldateien sind nicht mehr im Videopuffer vorhanden.",
        ));
    }
    let clip: Clip = sqlx::query_as("UPDATE clips SET status='queued',error=NULL,attempts=0,retry_at=NULL WHERE id=$1 AND status='failed' RETURNING *").bind(*id).fetch_optional(&mut *tx).await?.ok_or_else(|| conflict("Der Clip wurde bereits geändert."))?;
    tx.commit().await?;
    events::publish(&state.pool, "clip.updated", Some(*id)).await;
    Ok(HttpResponse::Accepted()
        .json(services::clip_view(&state, &clip, &row.streamer_login, false).await?))
}
async fn get_settings(state: web::Data<State>) -> Response {
    Ok(web::Json(json!(services::settings(&state.pool).await?)))
}
async fn update_settings(
    state: web::Data<State>,
    payload: web::Json<Settings>,
    _: ApplicationRequest,
) -> Response {
    payload.validate().map_err(invalid)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO app_settings(id,values) VALUES (1,$1) ON CONFLICT(id) DO UPDATE SET values=EXCLUDED.values").bind(json!(&*payload)).execute(&mut *tx).await?;
    services::activity(
        &mut tx,
        "system",
        "Aufnahme- und Speichereinstellungen aktualisiert.",
        None,
    )
    .await?;
    tx.commit().await?;
    events::publish(&state.pool, "settings.updated", None).await;
    Ok(web::Json(json!(&*payload)))
}
async fn storage_info(state: web::Data<State>) -> Response {
    let disk = nix::sys::statvfs::statvfs(&state.config.data_dir).map_err(anyhow::Error::from)?;
    let stored: i64 = sqlx::query_scalar(
        "SELECT (SELECT coalesce(sum(size_bytes),0) FROM clips WHERE status='ready')::bigint+(SELECT coalesce(sum(size_bytes),0) FROM edits WHERE status='ready')::bigint",
    )
    .fetch_one(&state.pool)
    .await?;
    let scratch: i64 =
        sqlx::query_scalar("SELECT coalesce(sum(size_bytes),0)::bigint FROM segments")
            .fetch_one(&state.pool)
            .await?;
    Ok(web::Json(
        json!({"bucket":state.config.s3_bucket,"stored_bytes":stored,"buffer_bytes":scratch,
        "disk_free_bytes":u64::from(disk.blocks_available())*disk.fragment_size(),"disk_total_bytes":u64::from(disk.blocks())*disk.fragment_size()}),
    ))
}
