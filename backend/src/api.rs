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
    error::{ApiError, conflict, invalid, missing},
    events,
    media::safe_path,
    models::{ACTIVE, Clip, ManifestEntry, Recording, Settings, Streamer, StreamerInput},
    services,
    state::State,
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
            .wrap(middleware::Logger::default())
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
            .route("/ws", web::get().to(events::websocket))
            .route("/dashboard", web::get().to(dashboard))
            .route("/streamers", web::get().to(list_streamers))
            .route("/streamers", web::post().to(add_streamer))
            .route("/streamers/{id}", web::patch().to(update_streamer))
            .route("/streamers/{id}", web::delete().to(remove_streamer))
            .route("/streamers/{id}/clip", web::post().to(manual_clip))
            .route("/clips", web::get().to(list_clips))
            .route("/clips/{id}", web::get().to(clip_detail))
            .route("/clips/{id}", web::patch().to(update_clip))
            .route("/clips/{id}", web::delete().to(delete_clip))
            .route("/clips/{id}/retry", web::post().to(retry_clip))
            .route("/settings", web::get().to(get_settings))
            .route("/settings", web::put().to(update_settings))
            .route("/storage", web::get().to(storage_info)),
    );
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
        "SELECT coalesce(sum(size_bytes),0)::bigint FROM clips WHERE status='ready'",
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
    Ok(web::Json(json!({"stats": {
        "recording":streamers.iter().filter(|s| s["status"]=="recording").count(), "streamers":streamers.len(),
        "clips_today":today, "clips_ready":count("ready"), "stored_bytes":stored,
        "pending":ACTIVE.iter().map(|status| count(status)).sum::<i64>(), "failed":count("failed"),
    }, "streamers":streamers, "clips":clips, "activity":activity, "settings":services::settings(&state.pool).await?})))
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
    let streamer: Streamer = sqlx::query_as("INSERT INTO streamers(id,login,enabled,quality,priority,pre_roll,post_roll,threshold_db,auto_clip,status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(id) DO UPDATE SET enabled=EXCLUDED.enabled,quality=EXCLUDED.quality,priority=EXCLUDED.priority,pre_roll=EXCLUDED.pre_roll,post_roll=EXCLUDED.post_roll,threshold_db=EXCLUDED.threshold_db,auto_clip=EXCLUDED.auto_clip,status=EXCLUDED.status,deleted_at=NULL,last_error=NULL,next_check_at=NULL RETURNING *")
        .bind(id).bind(&input.login).bind(input.enabled).bind(input.quality).bind(input.priority).bind(input.pre_roll).bind(input.post_roll).bind(input.threshold_db).bind(input.auto_clip).bind(if input.enabled {"waiting"} else {"paused"}).fetch_one(&mut *tx).await?;
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
    let streamer = get_streamer(&state, *id).await?;
    let mut input = json!({"login":streamer.login,"enabled":streamer.enabled,"quality":streamer.quality,
        "priority":streamer.priority,"pre_roll":streamer.pre_roll,"post_roll":streamer.post_roll,
        "threshold_db":streamer.threshold_db,"auto_clip":streamer.auto_clip});
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
    let result: Streamer = sqlx::query_as("UPDATE streamers SET enabled=$2,quality=$3,priority=$4,pre_roll=$5,post_roll=$6,threshold_db=$7,auto_clip=$8,status=CASE WHEN $9 THEN $10 ELSE status END,next_check_at=CASE WHEN $9 THEN NULL ELSE next_check_at END,last_error=CASE WHEN $9 THEN NULL ELSE last_error END WHERE id=$1 RETURNING *")
        .bind(*id).bind(input.enabled).bind(input.quality).bind(input.priority).bind(input.pre_roll).bind(input.post_roll).bind(input.threshold_db).bind(input.auto_clip).bind(patch.contains_key("enabled"))
        .bind(if input.enabled {"waiting"} else {"paused"}).fetch_one(&state.pool).await?;
    events::publish(&state.pool, "stream.updated", Some(*id)).await;
    Ok(web::Json(json!(result)))
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
        "SELECT coalesce(sum(size_bytes),0)::bigint FROM clips WHERE status='ready'",
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
