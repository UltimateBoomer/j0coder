use anyhow::{Context, Result, ensure};
use j0coder::catalog::{self, ValidatedRelease};
use j0coder::contract::Problem;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;
use uuid::Uuid;

const LOCK_ID: i64 = 0x434154414c4f47;
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Settings {
    repository_url: String,
    strategy: String,
    revision: String,
    poll_interval_seconds: i32,
    enabled: bool,
    generation: i64,
}

fn validate_settings(s: &Settings) -> Result<()> {
    catalog::validate_repository_url(&s.repository_url)?;
    ensure!(
        matches!(s.strategy.as_str(), "track_branch" | "pinned_commit"),
        "invalid strategy"
    );
    ensure!(
        (10..=86400).contains(&s.poll_interval_seconds),
        "poll interval must be 10-86400 seconds"
    );
    ensure!(
        !s.revision.is_empty()
            && s.revision.len() <= 200
            && !s.revision.starts_with('-')
            && !s.revision.chars().any(char::is_whitespace),
        "invalid revision"
    );
    if s.strategy == "pinned_commit" {
        ensure!(
            s.revision.len() == 40 && s.revision.bytes().all(|c| c.is_ascii_hexdigit()),
            "pinned revision must be a full 40-character commit hash"
        );
    }
    Ok(())
}
async fn seed(db: &PgPool) -> Result<()> {
    let Some(url) = std::env::var("CATALOG_REPOSITORY_URL")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    let s = Settings {
        repository_url: url,
        strategy: j0coder::env("CATALOG_STRATEGY", "track_branch"),
        revision: j0coder::env("CATALOG_REVISION", "main"),
        poll_interval_seconds: j0coder::env("CATALOG_POLL_INTERVAL_SECONDS", "300").parse()?,
        enabled: j0coder::env("CATALOG_ENABLED", "true") == "true",
        generation: 1,
    };
    validate_settings(&s)?;
    sqlx::query("INSERT INTO catalog_settings(repository_url,strategy,revision,poll_interval_seconds,enabled) VALUES($1,$2::catalog_strategy,$3,$4,$5) ON CONFLICT(singleton) DO NOTHING").bind(s.repository_url).bind(s.strategy).bind(s.revision).bind(s.poll_interval_seconds).bind(s.enabled).execute(db).await?;
    Ok(())
}
async fn settings(db: &PgPool) -> Result<Option<Settings>> {
    Ok(sqlx::query_as::<_,(String,String,String,i32,bool,i64)>("SELECT repository_url,strategy::text,revision,poll_interval_seconds,enabled,generation FROM catalog_settings WHERE singleton").fetch_optional(db).await?.map(|x|Settings{repository_url:x.0,strategy:x.1,revision:x.2,poll_interval_seconds:x.3,enabled:x.4,generation:x.5}))
}

async fn git(args: &[&str], git_dir: &Path, key: &Path, known_hosts: &Path) -> Result<String> {
    let ssh = format!(
        "ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile={}",
        key.display(),
        known_hosts.display()
    );
    let output = Command::new("git")
        .args(["--git-dir", git_dir.to_str().unwrap()])
        .args(args)
        .env("GIT_SSH_COMMAND", ssh)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .await
        .context("run git")?;
    ensure!(
        output.status.success(),
        "git operation failed: {}",
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
async fn fetch(
    s: &Settings,
    key: &Path,
    known_hosts: &Path,
) -> Result<(String, PathBuf, ValidatedRelease)> {
    ensure!(key.is_file(), "SSH private key is unavailable");
    ensure!(known_hosts.is_file(), "known_hosts is unavailable");
    let base = std::env::temp_dir().join(format!("catalog-{}", Uuid::new_v4()));
    let git_dir = base.join("repo.git");
    let tree = base.join("tree");
    tokio::fs::create_dir_all(&git_dir).await?;
    tokio::fs::create_dir_all(&tree).await?;
    let init = Command::new("git")
        .args(["init", "--bare", git_dir.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .await?;
    ensure!(init.status.success(), "git initialization failed");
    git(
        &["remote", "add", "origin", &s.repository_url],
        &git_dir,
        key,
        known_hosts,
    )
    .await?;
    git(
        &["fetch", "--no-tags", "--depth=1", "origin", &s.revision],
        &git_dir,
        key,
        known_hosts,
    )
    .await?;
    let commit = git(
        &["rev-parse", "--verify", "FETCH_HEAD^{commit}"],
        &git_dir,
        key,
        known_hosts,
    )
    .await?;
    if s.strategy == "pinned_commit" {
        ensure!(
            commit.eq_ignore_ascii_case(&s.revision),
            "fetched revision did not resolve to pinned commit"
        );
    }
    let work = tree.to_str().unwrap();
    git(
        &[
            "--work-tree",
            work,
            "checkout",
            "--force",
            &commit,
            "--",
            ".",
        ],
        &git_dir,
        key,
        known_hosts,
    )
    .await?;
    let release = catalog::validate_release(&tree)?;
    Ok((commit, base, release))
}

async fn apply(
    db: &PgPool,
    s: &Settings,
    commit: &str,
    release: &ValidatedRelease,
) -> Result<(String, i32, i32, i32, i32)> {
    let mut tx = db.begin().await?;
    let keys: Vec<&str> = release.problems.iter().map(|p| p.key.as_str()).collect();
    let existing=sqlx::query("SELECT i.catalog_key,i.problem_id,i.artifact_hash,p.current_version,p.draft FROM problem_imports i JOIN problems p ON p.id=i.problem_id FOR UPDATE OF i,p").fetch_all(&mut *tx).await?;
    let mut created = 0;
    let mut changed = 0;
    let mut unchanged = 0;
    let mut reference_changes = 0;
    for item in &release.problems {
        let old = existing
            .iter()
            .find(|r| r.get::<String, _>("catalog_key") == item.key);
        let problem_id = old
            .map(|r| r.get::<Uuid, _>("problem_id"))
            .unwrap_or_else(Uuid::new_v4);
        let semantic_hash = catalog::problem_hash(&item.problem)?;
        let same = old.is_some_and(|r| {
            r.get::<Option<Uuid>, _>("current_version").is_some()
                && serde_json::from_value::<Problem>(r.get("draft"))
                    .ok()
                    .and_then(|p| catalog::problem_hash(&p).ok())
                    .is_some_and(|hash| hash == semantic_hash)
        });
        let mut version_id = old.and_then(|r| r.get::<Option<Uuid>, _>("current_version"));
        if same {
            unchanged += 1;
        } else {
            if old.is_none() {
                created += 1;
                sqlx::query("INSERT INTO problems(id,draft) VALUES($1,$2)")
                    .bind(problem_id)
                    .bind(json!(item.problem))
                    .execute(&mut *tx)
                    .await?;
            } else {
                changed += 1;
                sqlx::query("UPDATE problems SET draft=$2 WHERE id=$1")
                    .bind(problem_id)
                    .bind(json!(item.problem))
                    .execute(&mut *tx)
                    .await?;
            }
            let version = Uuid::new_v4();
            version_id = Some(version);
            let mut public = item.problem.clone();
            let tests = json!(public.tests);
            public.tests.retain(|t| !t.hidden);
            let provenance = json!({"catalog_key":item.key,"repository_url":s.repository_url,"resolved_commit":commit,"source_checksum":release.source_checksum,"artifact_hash":item.hash});
            sqlx::query("INSERT INTO versions(id,problem_id,public,tests,catalog_provenance) VALUES($1,$2,$3,$4,$5)").bind(version).bind(problem_id).bind(json!(public)).bind(tests).bind(provenance).execute(&mut *tx).await?;
            sqlx::query("UPDATE problems SET current_version=$2 WHERE id=$1")
                .bind(problem_id)
                .bind(version)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(version_id) = version_id {
            let previous: Option<Uuid> = sqlx::query_scalar(
                "SELECT revision_id FROM catalog_reference_bindings WHERE version_id=$1",
            )
            .bind(version_id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(reference) = &item.reference {
                let language = match reference.language {
                    j0coder::contract::Language::Python => "python",
                    j0coder::contract::Language::Cpp => "cpp",
                    j0coder::contract::Language::Java => "java",
                    j0coder::contract::Language::Kotlin => "kotlin",
                };
                sqlx::query("INSERT INTO catalog_reference_revisions(id,problem_id,language,source,source_hash,repository_url,resolved_commit) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(problem_id,language,source_hash) DO NOTHING")
                    .bind(Uuid::new_v4()).bind(problem_id).bind(language).bind(&reference.source).bind(&reference.hash).bind(&s.repository_url).bind(commit).execute(&mut *tx).await?;
                let revision_id: Uuid = sqlx::query_scalar("SELECT id FROM catalog_reference_revisions WHERE problem_id=$1 AND language=$2 AND source_hash=$3")
                    .bind(problem_id).bind(language).bind(&reference.hash).fetch_one(&mut *tx).await?;
                if previous != Some(revision_id) {
                    reference_changes += 1;
                }
                sqlx::query("INSERT INTO catalog_reference_bindings(version_id,revision_id) VALUES($1,$2) ON CONFLICT(version_id) DO UPDATE SET revision_id=excluded.revision_id,updated_at=now()")
                    .bind(version_id).bind(revision_id).execute(&mut *tx).await?;
            } else {
                if previous.is_some() {
                    reference_changes += 1;
                }
                sqlx::query("DELETE FROM catalog_reference_bindings WHERE version_id=$1")
                    .bind(version_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        sqlx::query("INSERT INTO problem_imports(catalog_key,problem_id,artifact_path,artifact_hash,repository_url,resolved_commit,source_checksum) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(catalog_key) DO UPDATE SET artifact_path=excluded.artifact_path,artifact_hash=excluded.artifact_hash,repository_url=excluded.repository_url,resolved_commit=excluded.resolved_commit,source_checksum=excluded.source_checksum,updated_at=now()").bind(&item.key).bind(problem_id).bind(&item.path).bind(&item.hash).bind(&s.repository_url).bind(commit).bind(&release.source_checksum).execute(&mut *tx).await?;
    }
    let removed=sqlx::query("UPDATE problems SET current_version=NULL WHERE id IN (SELECT problem_id FROM problem_imports WHERE NOT(catalog_key=ANY($1))) AND current_version IS NOT NULL").bind(&keys).execute(&mut *tx).await?.rows_affected() as i32;
    let result = if created + changed + removed + reference_changes == 0 {
        "unchanged"
    } else {
        "applied"
    };
    tx.commit().await?;
    Ok((result.into(), created, changed, unchanged, removed))
}

async fn reconcile(db: &PgPool, s: &Settings, key: &Path, hosts: &Path) -> Result<()> {
    let mut lock = db.acquire().await?;
    if !sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
        .bind(LOCK_ID)
        .fetch_one(&mut *lock)
        .await?
    {
        sqlx::query("INSERT INTO catalog_runs(id,settings_generation,repository_url,requested_revision,result,diagnostics,finished_at) VALUES($1,$2,$3,$4,'locked','another controller owns the reconciliation lock',now())")
            .bind(Uuid::new_v4()).bind(s.generation).bind(&s.repository_url).bind(&s.revision).execute(db).await?;
        return Ok(());
    }
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog_runs(id,settings_generation,repository_url,requested_revision,result) VALUES($1,$2,$3,$4,'running')").bind(run).bind(s.generation).bind(&s.repository_url).bind(&s.revision).execute(&mut *lock).await?;
    let outcome = async {
        let (commit, temp, release) = fetch(s, key, hosts).await?;
        let applied = apply(db, s, &commit, &release).await?;
        let _ = tokio::fs::remove_dir_all(temp).await;
        Ok::<_, anyhow::Error>((commit, release.source_checksum, applied))
    }
    .await;
    match outcome {
        Ok((commit, sum, (result, a, b, c, d))) => {
            sqlx::query("UPDATE catalog_runs SET resolved_commit=$2,source_checksum=$3,result=$4,created_count=$5,changed_count=$6,unchanged_count=$7,removed_count=$8,finished_at=now() WHERE id=$1").bind(run).bind(commit).bind(sum).bind(result).bind(a).bind(b).bind(c).bind(d).execute(&mut *lock).await?;
        }
        Err(e) => {
            let diagnostic = format!("{e:#}");
            sqlx::query("UPDATE catalog_runs SET result='failed',diagnostics=$2,finished_at=now() WHERE id=$1").bind(run).bind(diagnostic.chars().take(4000).collect::<String>()).execute(&mut *lock).await?;
        }
    }
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_ID)
        .execute(&mut *lock)
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    j0coder::logging();
    let database_url = std::env::var("DATABASE_URL")?;
    let db = PgPool::connect(&database_url).await?;
    sqlx::migrate!("../../migrations").run(&db).await?;
    seed(&db).await?;
    let key = PathBuf::from(std::env::var("CATALOG_SSH_KEY_PATH")?);
    let hosts = PathBuf::from(std::env::var("CATALOG_KNOWN_HOSTS_PATH")?);
    let mut listener = sqlx::postgres::PgListener::connect(&database_url).await?;
    listener.listen("catalog_settings_changed").await?;
    loop {
        let s = settings(&db).await?;
        let delay = if let Some(s) = s {
            if s.enabled
                && let Err(e) = reconcile(&db, &s, &key, &hosts).await
            {
                tracing::error!(error=%e,"catalog reconciliation failed")
            }
            Duration::from_secs(s.poll_interval_seconds as u64)
        } else {
            Duration::from_secs(30)
        };
        tokio::select! {_=tokio::time::sleep(delay)=>{}, notice=listener.recv()=>{notice?;}}
    }
}
