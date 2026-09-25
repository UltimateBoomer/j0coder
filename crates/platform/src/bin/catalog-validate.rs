use anyhow::{Result, ensure};
use locoder::catalog::{validate_problem, validate_release};
use std::{collections::HashSet, env, fs, path::Path};

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
        Scratch(env::temp_dir().join(format!("locoder-catalog-check-{}", uuid::Uuid::new_v4())));
    copy_release(Path::new(&root), &scratch.0, true)?;
    let mut problems = fs::read_dir(scratch.0.join("problems"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    problems.sort();
    let mut errors = Vec::new();
    for path in problems {
        let bytes = fs::read(&path)?;
        if let Err(error) = validate_problem(&bytes) {
            errors.push(format!("{}: {error:#}", path.display()));
        }
    }
    ensure!(errors.is_empty(), "{}", errors.join("\n"));
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
            p.difficulty == locoder::contract::Problem::difficulty_band(score),
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
    Ok(())
}
