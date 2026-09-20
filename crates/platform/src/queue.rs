use crate::contract::*;
use anyhow::Result;
use redis::{
    AsyncCommands,
    streams::{StreamAutoClaimReply, StreamReadReply},
};
use sqlx::{PgPool, Row};
use uuid::Uuid;
pub const JOBS: &str = "practice:jobs:v1";
pub const EVENTS: &str = "practice:events:v1";
pub async fn connection() -> Result<redis::aio::ConnectionManager> {
    Ok(
        redis::Client::open(crate::env("REDIS_URL", "redis://redis:6379"))?
            .get_connection_manager()
            .await?,
    )
}
pub async fn group(c: &mut redis::aio::ConnectionManager, stream: &str, g: &str) -> Result<()> {
    let r: redis::RedisResult<String> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(g)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(c)
        .await;
    match r {
        Ok(_) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e.into()),
    }
}
pub async fn dispatch_forever(db: PgPool, shutdown: crate::shutdown::Shutdown) {
    while !shutdown.is_cancelled() {
        if let Err(e) = dispatch(&db).await {
            tracing::error!(error=%e,"dispatcher unavailable")
        }
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
        }
    }
}
async fn dispatch(db: &PgPool) -> Result<()> {
    let mut c = connection().await?;
    let mut tx = db.begin().await?;
    let rows=sqlx::query("SELECT s.id,s.job,s.attempt,o.last_sent FROM outbox o JOIN submissions s ON s.id=o.submission_id WHERE s.status!='completed' AND (o.last_sent IS NULL OR o.last_sent<now()-interval '1 hour') ORDER BY s.created_at LIMIT 50 FOR UPDATE OF o SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in rows {
        let mut job: Job = serde_json::from_value(row.get("job"))?;
        if row
            .get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_sent")
            .is_some()
        {
            job.generation = Uuid::new_v4();
            job.attempt_base = row.get::<i32, _>("attempt") as u32;
            sqlx::query("UPDATE submissions SET job=$2,token=NULL,status='queued',updated_at=now() WHERE id=$1 AND status!='completed'").bind(job.id).bind(serde_json::to_value(&job)?).execute(&mut *tx).await?;
        }
        let _: String = c
            .xadd(JOBS, "*", &[("payload", serde_json::to_string(&job)?)])
            .await?;
        sqlx::query("UPDATE outbox SET last_sent=now() WHERE submission_id=$1")
            .bind(job.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn events_forever(db: PgPool, shutdown: crate::shutdown::Shutdown) {
    let consumer = Uuid::new_v4().to_string();
    while !shutdown.is_cancelled() {
        if let Err(e) = events(&db, &consumer, &shutdown).await {
            tracing::error!(error=%e,"event consumer unavailable");
            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }
        }
    }
}
async fn events(db: &PgPool, consumer: &str, shutdown: &crate::shutdown::Shutdown) -> Result<()> {
    let mut c = connection().await?;
    group(&mut c, EVENTS, "api").await?;
    loop {
        if shutdown.is_cancelled() {
            return Ok(());
        }
        let claim: StreamAutoClaimReply = redis::cmd("XAUTOCLAIM")
            .arg(EVENTS)
            .arg("api")
            .arg(consumer)
            .arg(30000)
            .arg("0-0")
            .arg("COUNT")
            .arg(20)
            .query_async(&mut c)
            .await?;
        let mut messages = claim.claimed;
        if messages.is_empty() {
            let r: StreamReadReply = c
                .xread_options(
                    &[EVENTS],
                    &[">"],
                    &redis::streams::StreamReadOptions::default()
                        .group("api", consumer)
                        .count(20)
                        .block(1000),
                )
                .await?;
            messages = r.keys.into_iter().flat_map(|k| k.ids).collect();
        }
        for m in messages {
            let Some(payload) = m.get::<String>("payload") else {
                continue;
            };
            let e: Event = serde_json::from_str(&payload)?;
            anyhow::ensure!(e.schema == 1, "unsupported event");
            let mut tx = db.begin().await?;
            if let Some(outcome) = e.outcome {
                sqlx::query("UPDATE submissions SET status='completed',result=$4,updated_at=now() WHERE id=$1 AND (attempt<$3 OR (attempt=$3 AND token=$2)) AND status!='completed' AND job->>'generation'=$5").bind(e.id).bind(&e.token).bind(e.attempt as i32).bind(serde_json::to_value(outcome)?).bind(e.generation.to_string()).execute(&mut *tx).await?;
            } else {
                sqlx::query("UPDATE submissions SET status='running',token=$2,attempt=$3,updated_at=now() WHERE id=$1 AND status!='completed' AND attempt<$3 AND job->>'generation'=$4").bind(e.id).bind(&e.token).bind(e.attempt as i32).bind(e.generation.to_string()).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            let _: () = redis::pipe()
                .atomic()
                .cmd("XACK")
                .arg(EVENTS)
                .arg("api")
                .arg(&m.id)
                .ignore()
                .cmd("XDEL")
                .arg(EVENTS)
                .arg(&m.id)
                .ignore()
                .query_async(&mut c)
                .await?;
        }
    }
}
const ACQUIRE: &str = r#"
if redis.call('EXISTS',KEYS[2])==1 then redis.call('XACK',KEYS[4],'workers',ARGV[2]); redis.call('XDEL',KEYS[4],ARGV[2]); return 0 end
if not redis.call('SET',KEYS[1],ARGV[1],'NX','PX',30000) then return 0 end
local attempt=redis.call('INCR',KEYS[3]); redis.call('EXPIRE',KEYS[3],604800); return attempt
"#;
const RENEW: &str = r#"if redis.call('GET',KEYS[1])==ARGV[1] then return redis.call('PEXPIRE',KEYS[1],30000) else return 0 end"#;
const COMPLETE: &str = r#"
if redis.call('GET',KEYS[1])~=ARGV[1] then return 0 end
redis.call('XADD',KEYS[3],'*','payload',ARGV[2]);redis.call('SET',KEYS[2],'1','EX',604800)
redis.call('XACK',KEYS[4],'workers',ARGV[3]);redis.call('XDEL',KEYS[4],ARGV[3]);redis.call('DEL',KEYS[1]);return 1
"#;
const RELEASE: &str = r#"if redis.call('GET',KEYS[1])==ARGV[1] then return redis.call('DEL',KEYS[1]) else return 0 end"#;
pub async fn worker(shutdown: crate::shutdown::Shutdown) -> Result<()> {
    crate::sandbox::Podman::new().preflight().await?;
    let n: usize = crate::env("WORKER_CONCURRENCY", "1").parse()?;
    anyhow::ensure!((1..=32).contains(&n), "invalid concurrency");
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(n as u32)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..n {
        let db = db.clone();
        let shutdown = shutdown.clone();
        tasks.spawn(async move {
            let name = Uuid::new_v4().to_string();
            while !shutdown.is_cancelled() {
                if let Err(e) = work_loop(&name, &db, &shutdown).await {
                    tracing::error!(error=%e,"worker infrastructure failure");
                    tokio::select! {
                        _ = shutdown.cancelled() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                    }
                }
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    Ok(())
}
async fn work_loop(name: &str, db: &PgPool, shutdown: &crate::shutdown::Shutdown) -> Result<()> {
    let mut c = connection().await?;
    group(&mut c, JOBS, "workers").await?;
    loop {
        if shutdown.is_cancelled() {
            return Ok(());
        }
        let claim: StreamAutoClaimReply = redis::cmd("XAUTOCLAIM")
            .arg(JOBS)
            .arg("workers")
            .arg(name)
            .arg(35000)
            .arg("0-0")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut c)
            .await?;
        let mut messages = claim.claimed;
        if messages.is_empty() {
            let r: StreamReadReply = c
                .xread_options(
                    &[JOBS],
                    &[">"],
                    &redis::streams::StreamReadOptions::default()
                        .group("workers", name)
                        .count(1)
                        .block(1000),
                )
                .await?;
            messages = r.keys.into_iter().flat_map(|k| k.ids).collect()
        }
        for m in messages {
            let Some(payload) = m.get::<String>("payload") else {
                continue;
            };
            let job: Job = serde_json::from_str(&payload)?;
            anyhow::ensure!(job.schema == 1, "unsupported job");
            let prefix = format!("practice:{}:{}", job.id, job.generation);
            let lease = format!("{prefix}:lease");
            let done = format!("{prefix}:done");
            let attempts = format!("{prefix}:attempts");
            let token = Uuid::new_v4().to_string();
            let attempt: u32 = redis::Script::new(ACQUIRE)
                .key(&lease)
                .key(&done)
                .key(&attempts)
                .key(JOBS)
                .arg(&token)
                .arg(&m.id)
                .invoke_async(&mut c)
                .await?;
            if attempt == 0 {
                continue;
            }
            let attempt = attempt + job.attempt_base;
            let execution_start = std::time::Instant::now();
            let started = Event {
                schema: 1,
                generation: job.generation,
                id: job.id,
                token: token.clone(),
                attempt,
                outcome: None,
            };
            let _: String = c
                .xadd(
                    EVENTS,
                    "*",
                    &[("payload", serde_json::to_string(&started)?)],
                )
                .await?;
            let mut renew_c = c.clone();
            let renew_key = lease.clone();
            let renew_token = token.clone();
            let (lost_tx, mut lost_rx) = tokio::sync::oneshot::channel();
            let renew = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                    let r: redis::RedisResult<i32> = redis::Script::new(RENEW)
                        .key(&renew_key)
                        .arg(&renew_token)
                        .invoke_async(&mut renew_c)
                        .await;
                    if !matches!(r, Ok(1)) {
                        let _ = lost_tx.send(());
                        break;
                    }
                }
            });
            let execution = async {
                anyhow::ensure!(attempt <= 3, "retry budget exhausted");
                let (source, tests) = load_payload(db, &job).await?;
                crate::sandbox::judge_with_shutdown(&job, &source, &tests, shutdown).await
            };
            let result = tokio::select! {
                r=execution => r,
                _=&mut lost_rx => Err(anyhow::anyhow!("lease lost")),
            };
            renew.abort();
            let _ = renew.await;
            if shutdown.is_cancelled() {
                let _: i32 = redis::Script::new(RELEASE)
                    .key(&lease)
                    .arg(&token)
                    .invoke_async(&mut c)
                    .await?;
                return Ok(());
            }
            let mut outcome = match result {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!(submission=%job.id,attempt,error=%e,"execution infrastructure failure");
                    if attempt < 3 {
                        let _: i32 = redis::Script::new(RELEASE)
                            .key(&lease)
                            .arg(&token)
                            .invoke_async(&mut c)
                            .await?;
                        continue;
                    }
                    let tests = load_payload(db, &job)
                        .await
                        .map(|(_, tests)| tests)
                        .unwrap_or_default();
                    Outcome {
                        elapsed_ms: 0,
                        verdict: Verdict::InfrastructureFailure,
                        passed: 0,
                        total: tests.len(),
                        cases: crate::sandbox::not_run(&tests),
                        diagnostic: None,
                    }
                }
            };
            outcome.elapsed_ms = execution_start.elapsed().as_millis() as u64;
            let event = Event {
                outcome: Some(outcome),
                ..started
            };
            let _: i32 = redis::Script::new(COMPLETE)
                .key(&lease)
                .key(&done)
                .key(EVENTS)
                .key(JOBS)
                .arg(&token)
                .arg(serde_json::to_string(&event)?)
                .arg(&m.id)
                .invoke_async(&mut c)
                .await?;
        }
    }
}
async fn load_payload(db: &PgPool, job: &Job) -> Result<(String, Vec<Case>)> {
    let row = sqlx::query("SELECT s.source,coalesce(s.custom_cases,v.tests) AS tests FROM submissions s JOIN versions v ON v.id=s.version_id WHERE s.id=$1 AND s.version_id=$2")
        .bind(job.id)
        .bind(job.version)
        .fetch_one(db)
        .await?;
    Ok((row.get("source"), serde_json::from_value(row.get("tests"))?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_is_fenced() {
        assert!(COMPLETE.find("~=ARGV[1]").unwrap() < COMPLETE.find("XADD").unwrap());
        assert!(COMPLETE.contains("XACK"));
    }

    #[test]
    fn shutdown_release_keeps_job_recoverable() {
        assert!(RELEASE.contains("DEL"));
        assert!(!RELEASE.contains("XACK"));
        assert!(!RELEASE.contains("XDEL"));
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    #[ignore = "requires isolated Redis"]
    async fn redis_fencing_and_atomic_ack() -> Result<()> {
        let mut c = connection().await?;
        let p = format!("test:{}", Uuid::new_v4());
        let jobs = format!("{p}:jobs");
        let events = format!("{p}:events");
        let lease = format!("{p}:lease");
        let done = format!("{p}:done");
        let attempts = format!("{p}:attempts");
        group(&mut c, &jobs, "workers").await?;
        let id: String = c.xadd(&jobs, "*", &[("payload", "test")]).await?;
        let _: StreamReadReply = c
            .xread_options(
                &[&jobs],
                &[">"],
                &redis::streams::StreamReadOptions::default().group("workers", "one"),
            )
            .await?;
        let first: u32 = redis::Script::new(ACQUIRE)
            .key(&lease)
            .key(&done)
            .key(&attempts)
            .key(&jobs)
            .arg("first")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(first, 1);
        let duplicate: u32 = redis::Script::new(ACQUIRE)
            .key(&lease)
            .key(&done)
            .key(&attempts)
            .key(&jobs)
            .arg("second")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(duplicate, 0);
        let _: usize = c.del(&lease).await?;
        let next: u32 = redis::Script::new(ACQUIRE)
            .key(&lease)
            .key(&done)
            .key(&attempts)
            .key(&jobs)
            .arg("second")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(next, 2);
        let stale: i32 = redis::Script::new(COMPLETE)
            .key(&lease)
            .key(&done)
            .key(&events)
            .key(&jobs)
            .arg("first")
            .arg("stale")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(stale, 0);
        let accepted: i32 = redis::Script::new(COMPLETE)
            .key(&lease)
            .key(&done)
            .key(&events)
            .key(&jobs)
            .arg("second")
            .arg("accepted")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(accepted, 1);
        let length: usize = c.xlen(&events).await?;
        assert_eq!(length, 1);
        let duplicate: u32 = redis::Script::new(ACQUIRE)
            .key(&lease)
            .key(&done)
            .key(&attempts)
            .key(&jobs)
            .arg("third")
            .arg(&id)
            .invoke_async(&mut c)
            .await?;
        assert_eq!(duplicate, 0);
        let _: usize = c.del(&[jobs, events, lease, done, attempts]).await?;
        Ok(())
    }
    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL and Redis"]
    async fn database_event_reordering_and_reconciliation() -> Result<()> {
        let db = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
        let u = Uuid::new_v4();
        let p = Uuid::new_v4();
        let version = Uuid::new_v4();
        let id = Uuid::new_v4();
        let generation = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password) VALUES($1,$2,'unused')")
            .bind(u)
            .bind(u.to_string())
            .execute(&db)
            .await?;
        sqlx::query("INSERT INTO problems(id,draft) VALUES($1,'{}')")
            .bind(p)
            .execute(&db)
            .await?;
        sqlx::query("INSERT INTO versions(id,problem_id,public,tests) VALUES($1,$2,'{}',$3)")
            .bind(version)
            .bind(p)
            .bind(json!([{"args":[],"expected":1,"hidden":true}]))
            .execute(&db)
            .await?;
        let job = Job {
            attempt_base: 0,
            generation,
            schema: 1,
            id,
            version,
            language: Language::Python,
            signature: Signature {
                method: "solve".into(),
                params: vec![],
                returns: Type::Int,
            },
            limits: Limits::default(),
            mode: "submit".into(),
        };
        sqlx::query("INSERT INTO submissions(id,user_id,version_id,idempotency_key,request_hash,source,job) VALUES($1,$2,$3,'test','test','test',$4)").bind(id).bind(u).bind(version).bind(serde_json::to_value(&job)?).execute(&db).await?;
        let (source, tests) = load_payload(&db, &job).await?;
        assert_eq!(source, "test");
        assert_eq!(tests.len(), 1);
        assert!(tests[0].hidden);
        sqlx::query("UPDATE submissions SET custom_cases=$2 WHERE id=$1")
            .bind(id)
            .bind(json!([{"args":[],"expected":2,"hidden":false}]))
            .execute(&db)
            .await?;
        let (_, tests) = load_payload(&db, &job).await?;
        assert_eq!(tests.len(), 1);
        assert!(!tests[0].hidden);
        assert_eq!(tests[0].expected, Some(json!(2)));
        sqlx::query(
            "INSERT INTO outbox(submission_id,last_sent) VALUES($1,now()-interval '2 hours')",
        )
        .bind(id)
        .execute(&db)
        .await?;
        dispatch(&db).await?;
        let new_job: serde_json::Value =
            sqlx::query_scalar("SELECT job FROM submissions WHERE id=$1")
                .bind(id)
                .fetch_one(&db)
                .await?;
        let new_generation = serde_json::from_value::<Job>(new_job)?.generation;
        assert_ne!(new_generation, generation);
        let mut c = connection().await?;
        group(&mut c, EVENTS, "api").await?;
        let outcome = Outcome {
            elapsed_ms: 0,
            verdict: Verdict::Accepted,
            passed: 1,
            total: 1,
            cases: vec![],
            diagnostic: None,
        };
        let event = Event {
            generation: new_generation,
            schema: 1,
            id,
            token: "current".into(),
            attempt: 2,
            outcome: Some(outcome),
        };
        let _: String = c
            .xadd(EVENTS, "*", &[("payload", serde_json::to_string(&event)?)])
            .await?;
        let db2 = db.clone();
        let (_trigger, shutdown) = crate::shutdown::channel();
        let consumer = tokio::spawn(async move { events(&db2, "integration", &shutdown).await });
        let mut accepted = false;
        for _ in 0..50 {
            let status: String = sqlx::query_scalar("SELECT status FROM submissions WHERE id=$1")
                .bind(id)
                .fetch_one(&db)
                .await?;
            if status == "completed" {
                accepted = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(accepted, "completion-before-start must commit");
        let stale = Event {
            generation,
            token: "stale".into(),
            attempt: 99,
            outcome: None,
            ..event
        };
        let _: String = c
            .xadd(EVENTS, "*", &[("payload", serde_json::to_string(&stale)?)])
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let status: String = sqlx::query_scalar("SELECT status FROM submissions WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await?;
        assert_eq!(status, "completed");
        consumer.abort();
        Ok(())
    }
}
