//! Integration checks use a dedicated PostgreSQL database and an S3-compatible test endpoint.
//! Run: cargo test --test pipeline -- --ignored --test-threads=1
use std::{os::unix::fs::PermissionsExt, time::Duration};

use actix_web::{App, HttpServer, web};
use futures_util::StreamExt;
use notyetclipped::{
    api,
    config::Config,
    events, media,
    models::{Clip, ManifestEntry, Recording, Streamer},
    recording, services,
    state::State,
    storage::Storage,
    worker,
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tempfile::TempDir;
use tokio::sync::{Semaphore, watch};
use uuid::Uuid;

struct Fixture {
    state: State,
    _directory: TempDir,
    admin: PgPool,
    schema: String,
}

impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
        assert!(
            url.split('?').next().unwrap().ends_with("_test"),
            "Use a dedicated database ending in _test"
        );
        let endpoint = std::env::var("TEST_S3_ENDPOINT").expect("TEST_S3_ENDPOINT is required");
        let directory = tempfile::tempdir().unwrap();
        let mut config = Config::from_env().unwrap();
        config.database_url = url.clone();
        config.data_dir = directory.path().canonicalize().unwrap();
        config.s3_endpoint = Some(endpoint.clone());
        config.s3_public_endpoint = Some(endpoint);
        config.s3_access_key = "testing".into();
        config.s3_secret_key = "testing".into();
        config.s3_bucket = format!("clip-test-{}", Uuid::new_v4());
        let admin = PgPool::connect(&url).await.unwrap();
        let schema = format!("test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let search_path = schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .after_connect(move |connection, _| {
                let sql = format!("SET search_path TO {search_path}");
                Box::pin(async move {
                    sqlx::query(&sql).execute(connection).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = State::new(pool, config);
        state
            .storage
            .client
            .create_bucket()
            .bucket(&state.storage.bucket)
            .send()
            .await
            .unwrap();
        Self {
            state,
            _directory: directory,
            admin,
            schema,
        }
    }
    async fn clean(self) {
        let objects = self
            .state
            .storage
            .client
            .list_objects_v2()
            .bucket(&self.state.storage.bucket)
            .send()
            .await
            .unwrap();
        let keys = objects
            .contents()
            .iter()
            .filter_map(|object| object.key().map(str::to_owned))
            .collect::<Vec<_>>();
        self.state.storage.delete(&keys).await.unwrap();
        self.state
            .storage
            .client
            .delete_bucket()
            .bucket(&self.state.storage.bucket)
            .send()
            .await
            .unwrap();
        self.state.pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
    async fn streamer(&self) -> Streamer {
        let mut profile = notyetclipped::triggers::Profile {
            warmup_seconds: 15,
            ..Default::default()
        };
        profile.rules[0].name = "Audio fixture".into();
        profile.rules[0].conditions.retain(|c| {
            matches!(
                c.metric,
                notyetclipped::triggers::Metric::AudioRise
                    | notyetclipped::triggers::Metric::AudioLevel
            )
        });
        sqlx::query_as("INSERT INTO streamers(id,login,pre_roll,post_roll,trigger_profile) VALUES ($1,'test_streamer',5,5,$2) RETURNING *")
            .bind(Uuid::new_v4()).bind(json!(profile)).fetch_one(&self.state.pool).await.unwrap()
    }
}

async fn generate_source(state: &State) -> std::path::PathBuf {
    let path = state.config.data_dir.join("source.ts");
    let args = [
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x90:rate=10",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc=if(lt(t\\,16)\\,0.02\\,0.5)*sin(2*PI*440*t):s=16000",
        "-t",
        "24",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-g",
        "40",
        "-sc_threshold",
        "0",
        "-c:a",
        "aac",
        "-f",
        "mpegts",
    ];
    let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    args.push(path.to_string_lossy().into());
    media::ffmpeg(&state.config, &args, Duration::from_secs(30))
        .await
        .unwrap();
    path
}

async fn record_fixture(fixture: &mut Fixture) -> Uuid {
    let source = generate_source(&fixture.state).await;
    let script = fixture.state.config.data_dir.join("streamlink-fixture.sh");
    tokio::fs::write(
        &script,
        format!("#!/bin/sh\nexec cat '{}'\n", source.to_string_lossy()),
    )
    .await
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = (*fixture.state.config).clone();
    config.streamlink = script.to_string_lossy().into();
    fixture.state.config = std::sync::Arc::new(config);
    let streamer = fixture.streamer().await;
    let (_stop, receiver) = watch::channel(false);
    recording::run(
        fixture.state.clone(),
        streamer,
        receiver,
        std::sync::Arc::new(Semaphore::new(2)),
    )
    .await
    .unwrap();
    let clips: Vec<Clip> = sqlx::query_as("SELECT * FROM clips ORDER BY created_at")
        .fetch_all(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(
        clips.len(),
        1,
        "The sustained audio spike should create exactly one highlight"
    );
    assert_eq!(clips[0].status, "queued");
    assert_eq!(clips[0].end_ms - clips[0].start_ms, 10_000);
    clips[0].id
}

#[tokio::test]
#[ignore = "Requires TEST_DATABASE_URL, TEST_S3_ENDPOINT and FFmpeg"]
async fn capture_detect_render_upload_download_delete() {
    let mut fixture = Fixture::new().await;
    let mut live = sqlx::postgres::PgListener::connect_with(&fixture.state.pool)
        .await
        .unwrap();
    live.listen("studio_events").await.unwrap();
    let id = record_fixture(&mut fixture).await;
    let stored: Vec<Value> =
        sqlx::query_scalar("SELECT frames FROM trigger_samples ORDER BY start_ms")
            .fetch_all(&fixture.state.pool)
            .await
            .unwrap();
    let expected: Vec<Value> = stored
        .into_iter()
        .flat_map(|value| value.as_array().unwrap().clone())
        .collect();
    let delivered = tokio::time::timeout(Duration::from_secs(5), async {
        let mut frames = Vec::new();
        while frames.len() < expected.len() {
            let notification = live.recv().await.unwrap();
            assert!(notification.payload().len() < 8000);
            let event: Value = serde_json::from_str(notification.payload()).unwrap();
            if event["type"] == "signals.updated" {
                let batch = event["frames"].as_array().unwrap();
                assert!(batch.len() <= 8);
                assert!(event["recording"]["trigger_state"]["updated_at"].is_string());
                frames.extend(batch.iter().cloned());
            }
        }
        frames
    })
    .await
    .unwrap();
    assert!(!expected.is_empty());
    assert_eq!(
        delivered, expected,
        "All stored measurements must also be pushed, in order"
    );
    drop(live);
    sqlx::query("UPDATE clips SET attempts=1,status='cutting' WHERE id=$1")
        .bind(id)
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    worker::export_clip(&fixture.state, id).await.unwrap();
    let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1")
        .bind(id)
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(clip.status, "ready", "{:?}", clip.error);
    assert!(clip.size_bytes > 1000);
    let key = clip.video_key.as_ref().unwrap();
    let url = fixture
        .state
        .storage
        .url(key, Some("clip.mp4"))
        .await
        .unwrap();
    assert!(url.contains("X-Amz-Signature="));
    let body = reqwest::get(url)
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let downloaded = fixture.state.config.data_dir.join("download.mp4");
    tokio::fs::write(&downloaded, body).await.unwrap();
    let pcm = media::decode_audio(&fixture.state.config, &downloaded)
        .await
        .unwrap();
    assert!(
        (158000..=163000).contains(&pcm.len()),
        "Expected a ten-second cut, got {} samples",
        pcm.len()
    );
    let acl = fixture
        .state
        .storage
        .client
        .get_object_acl()
        .bucket(&fixture.state.storage.bucket)
        .key(key)
        .send()
        .await
        .unwrap();
    assert!(
        acl.grants()
            .iter()
            .all(|grant| grant.grantee().and_then(|grantee| grantee.uri()).is_none())
    );
    assert!(
        !fixture
            .state
            .config
            .data_dir
            .join("exports")
            .join(id.to_string())
            .exists()
    );
    sqlx::query("UPDATE clips SET status='deleting' WHERE id=$1")
        .bind(id)
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    worker::delete_clip(&fixture.state, id).await.unwrap();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM clips WHERE id=$1)")
        .bind(id)
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert!(!exists);
    assert!(
        fixture
            .state
            .storage
            .client
            .head_object()
            .bucket(&fixture.state.storage.bucket)
            .key(key)
            .send()
            .await
            .is_err()
    );
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "Requires TEST_DATABASE_URL, TEST_S3_ENDPOINT and FFmpeg"]
async fn upload_retry_keeps_render_and_gc_protects_reserved_segments() {
    let mut fixture = Fixture::new().await;
    let id = record_fixture(&mut fixture).await;
    worker::clean_buffers(&fixture.state, 180).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM segments")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert!(
        count > 0,
        "Queued clip must pin its source segments even after the stream ended"
    );
    let working = fixture.state.storage.clone();
    let mut failing = Storage::new(&fixture.state.config);
    failing.bucket = "nonexistent-test-bucket".into();
    fixture.state.storage = failing;
    sqlx::query("UPDATE clips SET attempts=1 WHERE id=$1")
        .bind(id)
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    worker::export_clip(&fixture.state, id).await.unwrap();
    let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1")
        .bind(id)
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(clip.status, "retry");
    assert!(clip.ready_at.is_none());
    let manifest: Vec<ManifestEntry> = serde_json::from_value(clip.manifest).unwrap();
    for entry in manifest {
        tokio::fs::remove_file(
            media::safe_path(&fixture.state.config.data_dir, &entry.path).unwrap(),
        )
        .await
        .unwrap();
    }
    fixture.state.storage = working;
    worker::export_clip(&fixture.state, id).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM clips WHERE id=$1")
        .bind(id)
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(
        status, "ready",
        "Retry should reuse the completed MP4 without the original segments"
    );
    worker::clean_buffers(&fixture.state, 180).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM segments")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "Requires TEST_DATABASE_URL, TEST_S3_ENDPOINT and FFmpeg"]
async fn restart_finalizes_partial_windows_and_requeues_inflight_exports() {
    let mut fixture = Fixture::new().await;
    let original = record_fixture(&mut fixture).await;
    let streamer: Streamer = sqlx::query_as("SELECT * FROM streamers LIMIT 1")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    let mut tx = fixture.state.pool.begin().await.unwrap();
    let recording: Recording = sqlx::query_as("UPDATE recordings SET ended_at=NULL RETURNING *")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let partial = services::create_clip(
        &mut tx,
        &streamer,
        &recording,
        recording.latest_ms,
        "manual",
        0.0,
        "Test",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE clips SET status='uploading' WHERE id=$1")
        .bind(original)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    worker::recover(&fixture.state).await.unwrap();
    let clips: Vec<Clip> = sqlx::query_as("SELECT * FROM clips")
        .fetch_all(&fixture.state.pool)
        .await
        .unwrap();
    assert!(clips.iter().all(|clip| clip.status == "queued"));
    assert_eq!(
        clips
            .iter()
            .find(|clip| clip.id == partial.id)
            .unwrap()
            .end_ms,
        recording.latest_ms
    );
    let empty_id = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings(id,streamer_id,ended_at) VALUES ($1,$2,now())")
        .bind(empty_id)
        .bind(streamer.id)
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    let empty_directory = fixture
        .state
        .config
        .data_dir
        .join("recordings")
        .join(empty_id.to_string());
    tokio::fs::create_dir_all(&empty_directory).await.unwrap();
    tokio::fs::write(empty_directory.join("segments.csv"), b"")
        .await
        .unwrap();
    worker::clean_buffers(&fixture.state, 300).await.unwrap();
    assert!(
        !empty_directory.exists(),
        "Offline checks must not leak empty recording directories"
    );
    fixture.clean().await;
}

#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL and TEST_S3_ENDPOINT"]
async fn api_mutations_publish_postgres_events_over_websocket() {
    let fixture = Fixture::new().await;
    let state = fixture.state.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(api::routes)
    })
    .bind(("127.0.0.1", 0))
    .unwrap();
    let address = server.addrs()[0];
    let server = server.run();
    let handle = server.handle();
    let server_task = tokio::spawn(server);
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/api/ws"))
        .await
        .unwrap();
    let hello = socket.next().await.unwrap().unwrap().into_text().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&hello).unwrap()["type"],
        "hello"
    );
    let listener = tokio::spawn(events::listen(fixture.state.clone()));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if message.is_text()
                && serde_json::from_str::<Value>(message.to_text().unwrap()).unwrap()["type"]
                    == "resync"
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://{address}/api");
    assert_eq!(
        client
            .post(format!("{url}/streamers"))
            .json(&json!({"login":"example"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = client
        .post(format!("{url}/streamers"))
        .header("x-requested-with", "notyetclipped")
        .json(&json!({"login":"https://twitch.tv/Example/","enabled":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let channel: Value = response.json().await.unwrap();
    assert_eq!(channel["login"], "example");
    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if message.is_text() {
                let value: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                if value["type"] == "stream.updated" {
                    break value;
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(event["id"], channel["id"]);
    let id = channel["id"].as_str().unwrap();
    for body in [
        json!({"quality":"$(evil)"}),
        json!({"threshold_db":-10}),
        json!({"enabled":null}),
    ] {
        assert_eq!(
            client
                .patch(format!("{url}/streamers/{id}"))
                .header("x-requested-with", "notyetclipped")
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            422
        );
    }
    let settings: Value = client
        .get(format!("{url}/settings"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["max_recordings"], 4);
    let dashboard: Value = client
        .get(format!("{url}/dashboard"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dashboard["stats"]["clips_today"], 0);
    assert_eq!(dashboard["streamers"].as_array().unwrap().len(), 1);
    assert_eq!(dashboard["trend"].as_array().unwrap().len(), 7);
    assert!(
        dashboard["trend"]
            .as_array()
            .unwrap()
            .iter()
            .all(|day| day["automatic"] == 0 && day["manual"] == 0)
    );
    // Roundtrip a real builder profile, then atomically copy only trigger-related fields.
    let mut profile = notyetclipped::triggers::Profile::default();
    profile.rules[0].name = "Test reaction".into();
    let result = client
        .patch(format!("{url}/streamers/{id}"))
        .header("x-requested-with", "notyetclipped")
        .json(&json!({"trigger_profile":profile,"pre_roll":15,"post_roll":10}))
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), 200);
    let target: Value = client
        .post(format!("{url}/streamers"))
        .header("x-requested-with", "notyetclipped")
        .json(&json!({"login":"target_channel","enabled":false,"auto_clip":false,"quality":"best"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let target_id = target["id"].as_str().unwrap();
    let copy = client
        .post(format!("{url}/streamers/{id}/triggers/copy"))
        .header("x-requested-with", "notyetclipped");
    assert_eq!(
        copy.try_clone()
            .unwrap()
            .json(&json!({"target_ids":[target_id, Uuid::new_v4().to_string()]}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let unchanged: Streamer = sqlx::query_as("SELECT * FROM streamers WHERE id=$1")
        .bind(Uuid::parse_str(target_id).unwrap())
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(
        unchanged.pre_roll, 30,
        "An invalid target must roll back the entire copy"
    );
    assert_eq!(
        copy.json(&json!({"target_ids":[target_id]}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let copied: Streamer = sqlx::query_as("SELECT * FROM streamers WHERE id=$1")
        .bind(Uuid::parse_str(target_id).unwrap())
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(copied.trigger_profile.0, profile);
    assert_eq!((copied.pre_roll, copied.post_roll), (15, 10));
    assert!(!copied.auto_clip && !copied.enabled);
    assert_eq!(copied.quality, "best");
    let recording_id = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings(id,streamer_id) VALUES ($1,$2)")
        .bind(recording_id)
        .bind(Uuid::parse_str(id).unwrap())
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    let profile_url = format!("{url}/streamers/{id}/editing");
    let layout: Value = client
        .get(&profile_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(layout["profile"]["auto_queue"], false);
    assert!(layout["preview_url"].is_null());
    let mut layout = layout["profile"].clone();
    layout["facecam"]["x"] = json!(0.99);
    assert_eq!(
        client
            .put(&profile_url)
            .header("x-requested-with", "notyetclipped")
            .json(&layout)
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    layout["facecam"]["x"] = json!(0.0);
    layout["auto_queue"] = json!(true);
    assert!(
        client
            .put(&profile_url)
            .header("x-requested-with", "notyetclipped")
            .json(&layout)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let saved: Value = client
        .get(&profile_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["profile"], layout);
    notyetclipped::jobs::log(
        &fixture.state.pool,
        recording_id,
        "info",
        "Segmentpuffer gestartet",
    )
    .await;
    let jobs: Value = client
        .get(format!("{url}/jobs"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        jobs.as_array()
            .unwrap()
            .iter()
            .any(|job| job["id"] == recording_id.to_string())
    );
    let log_response = client
        .get(format!("{url}/jobs/{recording_id}/logs?download=true"))
        .send()
        .await
        .unwrap();
    assert!(
        log_response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    assert!(
        log_response
            .text()
            .await
            .unwrap()
            .contains("[info] Segmentpuffer gestartet")
    );
    let frames: Vec<_> = (0..4000)
        .step_by(250)
        .map(|time_ms| notyetclipped::triggers::Signal {
            time_ms,
            audio_level: -12.0,
            audio_rise: 15.0,
            calibrated_seconds: 65.0,
            chat_messages: Some(10.0),
            chat_users: Some(5.0),
            ..Default::default()
        })
        .collect();
    sqlx::query("INSERT INTO trigger_samples(recording_id,start_ms,frames) VALUES ($1,0,$2)")
        .bind(recording_id)
        .bind(json!(frames))
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    let history: Value = client
        .get(format!("{url}/streamers/{id}/signals"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history["recording"]["id"], recording_id.to_string());
    assert_eq!(
        history["frames"],
        json!(frames),
        "History must retain full-resolution reference samples"
    );
    let pushed = json!({"type":"signals.updated","id":id,"recording":history["recording"],"frames":&frames[..8],"hourly_count":0,"last_trigger_at":null});
    events::publish_data(&fixture.state.pool, pushed.clone()).await;
    let received = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if message.is_text() {
                let event: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                if event["type"] == "signals.updated" {
                    break event;
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        received, pushed,
        "WebSockets deliver measurement data without HTTP invalidation"
    );
    let result: Value = client
        .post(format!("{url}/streamers/{id}/triggers/test"))
        .header("x-requested-with", "notyetclipped")
        .json(&profile)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result["hits"].as_array().unwrap().len(), 1);
    assert_eq!(result["hits"][0]["rule"], "Test reaction");
    let clips: i64 = sqlx::query_scalar("SELECT count(*) FROM clips")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(clips, 0, "Dry runs must never reserve clips");
    profile.rules[0].conditions.clear();
    assert_eq!(
        client
            .patch(format!("{url}/streamers/{id}"))
            .header("x-requested-with", "notyetclipped")
            .json(&json!({"trigger_profile":profile}))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    socket.close(None).await.unwrap();
    listener.abort();
    handle.stop(false).await;
    server_task.await.unwrap().unwrap();
    fixture.clean().await;
}

#[tokio::test]
async fn csv_reader_does_not_consume_partial_lines() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("segments.csv");
    tokio::fs::write(directory.path().join("segment-000000000.ts"), b"segment")
        .await
        .unwrap();
    tokio::fs::write(&file, "segment-000000000.ts,0.0,4")
        .await
        .unwrap();
    let reader = recording::IndexReader { offset: 0 };
    assert!(reader.read(directory.path()).await.unwrap().is_empty());
    tokio::fs::write(&file, "segment-000000000.ts,0.0,4.0\n")
        .await
        .unwrap();
    let rows = reader.read(directory.path()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].end, 4000);
    let reader = recording::IndexReader {
        offset: rows[0].next_offset,
    };
    assert!(reader.read(directory.path()).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL, FFmpeg and S3 test services"]
async fn semantic_clips_keep_context_pins_budgets_and_deleted_clip_deduplication() {
    use notyetclipped::ai::{self, Decision, Run};
    let fixture = Fixture::new().await;
    let state = &fixture.state;
    let mut streamer = fixture.streamer().await;
    streamer.trigger_profile.semantic.enabled = true;
    sqlx::query("UPDATE streamers SET trigger_profile=$2 WHERE id=$1")
        .bind(streamer.id)
        .bind(json!(streamer.trigger_profile))
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_control SET settings=jsonb_set(settings,'{enabled}','true') WHERE id=1")
        .execute(&state.pool)
        .await
        .unwrap();
    let source = generate_source(state).await;
    let recording: Recording = sqlx::query_as("INSERT INTO recordings(id,streamer_id,latest_ms,ended_at) VALUES ($1,$2,24000,now()) RETURNING *").bind(Uuid::new_v4()).bind(streamer.id).fetch_one(&state.pool).await.unwrap();
    let relative = source
        .strip_prefix(&state.config.data_dir)
        .unwrap()
        .to_str()
        .unwrap();
    sqlx::query("INSERT INTO segments(recording_id,path,start_ms,end_ms,size_bytes) VALUES ($1,$2,0,24000,1000)").bind(recording.id).bind(relative).execute(&state.pool).await.unwrap();
    let run: Run = sqlx::query_as("INSERT INTO ai_runs(id,recording_id,streamer_id,start_ms,end_ms,sent_at) VALUES ($1,$2,$3,0,24000,now()) RETURNING *").bind(Uuid::new_v4()).bind(recording.id).bind(streamer.id).fetch_one(&state.pool).await.unwrap();
    worker::clean_buffers(state, 180).await.unwrap();
    assert!(
        source.is_file(),
        "In-flight analysis must pin even an ended recording"
    );
    let input = ai::prepare(
        state,
        &run,
        &state.config.data_dir.join("analysis-test"),
        &notyetclipped::triggers::Semantic {
            stories: false,
            ..Default::default()
        },
        &ai::Settings::default(),
    )
    .await
    .unwrap();
    assert!(input.iter().filter(|v| v["type"] == "input_image").count() >= 2);
    let mut decision = Decision {
        highlight: true,
        kind: "story".into(),
        start_seconds: 2.,
        end_seconds: 18.,
        confidence: 0.9,
        title: "Testgeschichte".into(),
        reason: "Anfang und Pointe".into(),
        trigger_id: "story".into(),
    };
    let profile = &streamer.trigger_profile.semantic;
    let mut edited = profile.clone();
    edited.guidance = "changed".into();
    assert!(
        ai::apply_decision(state, &run, &decision, &edited)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query(
        "UPDATE ai_control SET settings=jsonb_set(settings,'{enabled}','false') WHERE id=1",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(
        ai::apply_decision(state, &run, &decision, profile)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("UPDATE ai_control SET settings=jsonb_set(settings,'{enabled}','true') WHERE id=1")
        .execute(&state.pool)
        .await
        .unwrap();
    decision.confidence = 0.6;
    assert!(
        ai::apply_decision(state, &run, &decision, profile)
            .await
            .unwrap()
            .is_none()
    );
    decision.confidence = 0.9;
    let id = ai::apply_decision(state, &run, &decision, profile)
        .await
        .unwrap()
        .unwrap();
    let clip: Clip = sqlx::query_as("SELECT * FROM clips WHERE id=$1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        (clip.start_ms, clip.end_ms),
        (2000, 18000),
        "Use semantic bounds, not generic pre/post-roll"
    );
    assert_eq!(clip.status, "queued");
    assert_eq!(clip.trigger, "story");
    assert!(
        ai::apply_decision(state, &run, &decision, profile)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("DELETE FROM clips WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM trigger_events WHERE streamer_id=$1")
        .bind(streamer.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "Deleting a clip must not refund its trigger budget"
    );
    sqlx::query("UPDATE trigger_events SET created_at=now()-interval '10 minutes'")
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        ai::apply_decision(state, &run, &decision, profile)
            .await
            .unwrap()
            .is_none(),
        "Completed moments stay deduplicated after clip deletion and cooldown"
    );
    assert_eq!(ai::status(state).await.unwrap()["requests_day"], 1);
    worker::clean_buffers(state, 180).await.unwrap();
    assert!(
        !source.exists(),
        "Completed analysis must release its source pin"
    );
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL and S3 test services"]
async fn semantic_scheduler_respects_pause_request_budgets_and_new_footage() {
    use notyetclipped::ai;
    let fixture = Fixture::new().await;
    let state = &fixture.state;
    let mut streamer = fixture.streamer().await;
    streamer.trigger_profile.semantic.enabled = true;
    sqlx::query("UPDATE streamers SET trigger_profile=$2 WHERE id=$1")
        .bind(streamer.id)
        .bind(json!(streamer.trigger_profile))
        .execute(&state.pool)
        .await
        .unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings(id,streamer_id,latest_ms) VALUES ($1,$2,40000)")
        .bind(id)
        .bind(streamer.id)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO segments(recording_id,path,start_ms,end_ms,size_bytes) VALUES ($1,'sample.ts',0,40000,1000)").bind(id).execute(&state.pool).await.unwrap();
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Global analysis starts disabled"
    );
    sqlx::query("UPDATE ai_control SET settings=$1 WHERE id=1")
        .bind(json!({"enabled":true,"model":"test","hourly_limit":1,"daily_limit":2}))
        .execute(&state.pool)
        .await
        .unwrap();
    let (run, _) = ai::reserve(state).await.unwrap().unwrap();
    assert_eq!((run.start_ms, run.end_ms), (0, 40000));
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Respect per-channel intervals"
    );
    sqlx::query(
        "UPDATE ai_runs SET created_at=now()-interval '10 minutes',status='skipped' WHERE id=$1",
    )
    .bind(run.id)
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "A stalled stream must not spend requests on the same footage"
    );
    sqlx::query("UPDATE recordings SET latest_ms=60000 WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_runs SET sent_at=now() WHERE id=$1")
        .bind(run.id)
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Request admission counts persisted sends"
    );
    sqlx::query("UPDATE ai_runs SET sent_at=now()-interval '2 hours' WHERE id=$1")
        .bind(run.id)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_control SET paused_reason='quota' WHERE id=1")
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Quota pause prevents preparing more work"
    );
    sqlx::query("UPDATE ai_control SET paused_reason=NULL WHERE id=1")
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO trigger_events(streamer_id) SELECT $1 FROM generate_series(1,6)")
        .bind(streamer.id)
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Do not transcribe when no clip budget is available"
    );
    sqlx::query("DELETE FROM trigger_events")
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(ai::reserve(state).await.unwrap().is_some());
    assert!(
        ai::reserve(state).await.unwrap().is_none(),
        "Never prepare concurrently or pin an unbounded backlog"
    );
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL, FFmpeg and S3 test services"]
async fn vertical_edit_queue_snapshots_layout_renders_and_deletes_with_source() {
    use notyetclipped::{editing, jobs};
    let fixture = Fixture::new().await;
    let state = &fixture.state;
    let streamer = fixture.streamer().await;
    let source = generate_source(state).await;
    let recording = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO recordings(id,streamer_id,ended_at,latest_ms) VALUES ($1,$2,now(),24000)",
    )
    .bind(recording)
    .bind(streamer.id)
    .execute(&state.pool)
    .await
    .unwrap();
    let clip_id = Uuid::new_v4();
    let source_key = format!("fixtures/{clip_id}.ts");
    state
        .storage
        .upload(&source, &source_key, "video/mp2t")
        .await
        .unwrap();
    sqlx::query("INSERT INTO clips(id,streamer_id,recording_id,title,status,start_ms,end_ms,video_key) VALUES ($1,$2,$3,'Editing fixture','ready',0,24000,$4)").bind(clip_id).bind(streamer.id).bind(recording).bind(&source_key).execute(&state.pool).await.unwrap();
    let profile = editing::Profile {
        auto_queue: true,
        facecam_enabled: true,
        chat_mode: "panel".into(),
        framing: "fill".into(),
        ..Default::default()
    };
    let mut tx = state.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO editing_profiles(streamer_id,profile) VALUES ($1,$2)")
        .bind(streamer.id)
        .bind(json!(profile))
        .execute(&mut *tx)
        .await
        .unwrap();
    editing::auto_enqueue(&mut tx, clip_id, streamer.id)
        .await
        .unwrap();
    let id = editing::enqueue(&mut tx, clip_id, &profile).await.unwrap();
    assert_eq!(
        editing::enqueue(&mut tx, clip_id, &editing::Profile::default())
            .await
            .unwrap(),
        id,
        "Double-click must not create duplicate active work or replace the snapshot"
    );
    tx.commit().await.unwrap();
    let saved: Value = sqlx::query_scalar("SELECT profile FROM edits WHERE id=$1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(saved, json!(profile));
    assert_eq!(editing::reserve(state).await.unwrap(), Some(id));
    assert!(editing::reserve(state).await.unwrap().is_none());
    worker::recover(state).await.unwrap();
    assert_eq!(
        editing::reserve(state).await.unwrap(),
        Some(id),
        "Interrupted edits resume with the same snapshot"
    );
    assert!(
        jobs::list(&state.pool)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|j| j["id"] == id.to_string() && j["active"] == true)
    );
    editing::process(state, id).await.unwrap();
    let edit: editing::Edit = sqlx::query_as("SELECT * FROM edits WHERE id=$1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(edit.status, "ready", "{:?}", edit.error);
    assert!(edit.size_bytes > 1000);
    let output = state.config.data_dir.join("downloaded-edit.mp4");
    let object = state
        .storage
        .client
        .get_object()
        .bucket(&state.storage.bucket)
        .key(&edit.video_key)
        .send()
        .await
        .unwrap();
    tokio::fs::write(&output, object.body.collect().await.unwrap().into_bytes())
        .await
        .unwrap();
    let bmp = media::ffmpeg(
        &state.config,
        &[
            "-i".into(),
            output.to_string_lossy().into(),
            "-frames:v".into(),
            "1".into(),
            "-f".into(),
            "image2pipe".into(),
            "-c:v".into(),
            "bmp".into(),
            "pipe:1".into(),
        ],
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 720);
    assert_eq!(
        i32::from_le_bytes(bmp[22..26].try_into().unwrap()).abs(),
        1280
    );
    assert!(
        media::decode_audio(&state.config, &output)
            .await
            .unwrap()
            .len()
            > 16000
    );
    let logs: i64 = sqlx::query_scalar("SELECT count(*) FROM job_logs WHERE job_id=$1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(logs >= 4);
    sqlx::query("UPDATE clips SET status='deleting' WHERE id=$1")
        .bind(clip_id)
        .execute(&state.pool)
        .await
        .unwrap();
    worker::delete_clip(state, clip_id).await.unwrap();
    assert!(
        state
            .storage
            .client
            .head_object()
            .bucket(&state.storage.bucket)
            .key(&edit.video_key)
            .send()
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM edits WHERE clip_id=$1")
        .bind(clip_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL and S3 test services"]
async fn chatgpt_login_uses_pkce_stable_host_and_single_use_state_without_exposing_secrets() {
    use notyetclipped::chatgpt;
    let fixture = Fixture::new().await;
    let state = &fixture.state;
    let url = url::Url::parse(&chatgpt::login(state, None).await.unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.host_str(), Some("auth.openai.com"));
    assert_eq!(query["client_id"], "dynamic_agent_client");
    assert_eq!(query["code_challenge_method"], "S256");
    assert!(query["scope"].contains("chatgpt.tokens.use.direct"));
    let path = state.config.data_dir.join("auth/chatgpt.json");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let invalid = serde_json::from_value(json!({"state":"wrong","error":"access_denied"})).unwrap();
    assert!(chatgpt::callback(state, invalid).await.is_err());
    let denied =
        serde_json::from_value(json!({"state":query["state"],"error":"access_denied"})).unwrap();
    assert!(chatgpt::callback(state, denied).await.is_err());
    let vault: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(vault["pending"].is_null(), "A denied attempt is single-use");
    let next = url::Url::parse(&chatgpt::login(state, None).await.unwrap()).unwrap();
    let next: std::collections::HashMap<_, _> = next.query_pairs().into_owned().collect();
    assert_eq!(query["ext_agent_host_id"], next["ext_agent_host_id"]);
    assert_ne!(query["state"], next["state"]);
    assert_ne!(query["code_challenge"], next["code_challenge"]);
    let status = chatgpt::status(state).await.unwrap().to_string();
    assert!(
        !status.contains("verifier")
            && !status.contains(&next["state"])
            && !status.contains("access_token")
    );
    assert!(chatgpt::access_token(state).await.is_err());
    fixture.clean().await;
}
