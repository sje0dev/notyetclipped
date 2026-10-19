use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn log(pool: &PgPool, id: Uuid, level: &str, message: &str) {
    if let Err(error) = sqlx::query("INSERT INTO job_logs(job_id,level,message) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(level)
        .bind(message.chars().take(2000).collect::<String>())
        .execute(pool)
        .await
    {
        tracing::warn!(%error,"Jobprotokoll konnte nicht geschrieben werden");
    }
    crate::events::publish(pool, "job.updated", Some(id)).await;
}

pub async fn list(pool: &PgPool) -> anyhow::Result<Value> {
    let rows: Vec<Value> = sqlx::query_scalar(r#"
        WITH jobs AS (
            SELECT r.id,'recording' AS kind,'Aufnahme @'||s.login AS label,s.id AS streamer_id,
                CASE WHEN r.ended_at IS NULL THEN s.status WHEN r.error IS NULL THEN 'finished' ELSE 'failed' END AS status,
                r.started_at AS created_at,r.ended_at AS finished_at,r.error AS message,
                r.ended_at IS NULL AS active
            FROM recordings r JOIN streamers s ON s.id=r.streamer_id WHERE r.ended_at IS NULL OR r.ended_at>now()-interval '1 hour'
            UNION ALL
            SELECT c.id,'clip',c.title,c.streamer_id,c.status,c.created_at,c.ready_at,c.error,
                c.status IN ('collecting','queued','cutting','uploading','retry','deleting')
            FROM clips c WHERE c.status IN ('collecting','queued','cutting','uploading','retry','deleting') OR c.created_at>now()-interval '1 hour'
            UNION ALL
            SELECT a.id,'ai','KI-Analyse @'||s.login,a.streamer_id,a.status,a.created_at,a.finished_at,a.message,
                a.status IN ('preparing','analyzing')
            FROM ai_runs a JOIN streamers s ON s.id=a.streamer_id WHERE a.status IN ('preparing','analyzing') OR a.created_at>now()-interval '1 hour'
            UNION ALL
            SELECT e.id,'edit','Hochkant: '||c.title,c.streamer_id,e.status,e.created_at,e.finished_at,e.error,
                e.status IN ('queued','rendering')
            FROM edits e JOIN clips c ON c.id=e.clip_id WHERE e.status IN ('queued','rendering') OR e.created_at>now()-interval '1 hour'
        ) SELECT to_jsonb(j) FROM (SELECT * FROM jobs ORDER BY active DESC,created_at DESC LIMIT 100) j
    "#).fetch_all(pool).await?;
    Ok(json!(rows))
}
