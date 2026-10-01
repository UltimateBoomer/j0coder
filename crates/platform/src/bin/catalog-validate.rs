use anyhow::{Result, ensure};
use j0coder::catalog::validate_release;
use j0coder::contract::Job;
use std::{collections::HashSet, env, fs, path::Path};
use uuid::Uuid;

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_release(source: &Path, target: &Path, top_level: bool) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if top_level && entry.file_name() == ".git" {
            continue;
        }
        let dest = target.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_release(&entry.path(), &dest, false)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), dest)?;
        } else {
            anyhow::bail!("unsupported filesystem entry: {}", entry.path().display());
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let root = env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: catalog-validate <catalog-directory>"))?;
    let scratch =
        Scratch(env::temp_dir().join(format!("j0coder-catalog-check-{}", uuid::Uuid::new_v4())));
    copy_release(Path::new(&root), &scratch.0, true)?;
    let release = validate_release(&scratch.0)?;
    let mut titles = HashSet::new();
    let mut bands = [0usize; 3];
    let mut scores = [0usize; 5];
    for item in &release.problems {
        let p = &item.problem;
        ensure!(
            titles.insert(p.title.trim().to_lowercase()),
            "duplicate title: {}",
            p.title
        );
        let score = p.effective_score();
        bands[match p.difficulty.as_str() {
            "easy" => 0,
            "medium" => 1,
            _ => 2,
        }] += 1;
        scores[usize::from(score - 1)] += 1;
        ensure!(
            p.difficulty == j0coder::contract::Problem::difficulty_band(score),
            "difficulty and score disagree for {}",
            item.key
        );
    }
    println!(
        "valid: {} problems; easy={}, medium={}, hard={}; scores 1-5={scores:?}",
        release.problems.len(),
        bands[0],
        bands[1],
        bands[2]
    );
    if env::args().any(|arg| arg == "--check-references") {
        let count = release
            .problems
            .iter()
            .filter(|item| item.reference.is_some())
            .count();
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            for item in &release.problems {
                let Some(reference) = &item.reference else {
                    continue;
                };
                let p = &item.problem;
                let job = Job {
                    attempt_base: 0,
                    generation: Uuid::new_v4(),
                    schema: p.schema,
                    id: Uuid::new_v4(),
                    language: reference.language,
                    version: Uuid::new_v4(),
                    interface: p.interface.clone(),
                    limits: p.limits.clone(),
                    mode: "run".into(),
                    type_definitions: p.type_definitions.clone(),
                    comparison: p.comparison.clone(),
                };
                j0coder::sandbox::reference_outputs(&job, &reference.source, &p.tests)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}: reference failed: {e:#}", item.path))?;
            }
            Ok::<_, anyhow::Error>(())
        })?;
        println!("validated {count} reference solutions against their catalog cases");
    }
    Ok(())
}
