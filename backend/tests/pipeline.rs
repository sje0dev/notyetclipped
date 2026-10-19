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
        sqlx::query_as("INSERT INTO streamers(id,login,pre_roll,post_roll) VALUES ($1,'test_streamer',5,5) RETURNING *")
            .bind(Uuid::new_v4()).fetch_one(&self.state.pool).await.unwrap()
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
    let id = record_fixture(&mut fixture).await;
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
