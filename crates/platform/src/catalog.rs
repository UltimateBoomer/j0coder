use crate::contract::Problem;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

pub const MANIFEST_FILE: &str = "catalog.json";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema: u8,
    pub problems: Vec<ManifestProblem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProblem {
    pub key: String,
    pub path: String,
    pub sha256: String,
}
#[derive(Debug)]
pub struct ValidatedProblem {
    pub key: String,
    pub path: String,
    pub hash: String,
    pub problem: Problem,
}
#[derive(Debug)]
pub struct ValidatedRelease {
    pub manifest_checksum: String,
    pub problems: Vec<ValidatedProblem>,
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub fn validate_problem(bytes: &[u8]) -> Result<(Problem, String)> {
    let problem: Problem = serde_json::from_slice(bytes).context("parse problem JSON")?;
    problem.validate().context("validate problem")?;
    let canonical = serde_json::to_vec(&problem)?;
    Ok((problem, sha256(&canonical)))
}
fn safe_relative(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty() && !path.is_absolute(),
        "artifact path must be relative"
    );
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "artifact path contains traversal"
    );
    Ok(path.to_owned())
}
fn files(root: &Path, dir: &Path, out: &mut HashSet<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "symlinks are not allowed: {}",
            entry.path().display()
        );
        if kind.is_dir() {
            files(root, &entry.path(), out)?;
        } else if kind.is_file() {
            out.insert(entry.path().strip_prefix(root)?.to_owned());
        } else {
            anyhow::bail!("unsupported filesystem entry: {}", entry.path().display());
        }
    }
    Ok(())
}
pub fn validate_release(root: &Path) -> Result<ValidatedRelease> {
    let manifest_path = root.join(MANIFEST_FILE);
    ensure!(
        !fs::symlink_metadata(&manifest_path)?
            .file_type()
            .is_symlink(),
        "manifest may not be a symlink"
    );
    let bytes = fs::read(&manifest_path).context("read catalog.json")?;
    let manifest: ReleaseManifest = serde_json::from_slice(&bytes).context("parse catalog.json")?;
    ensure!(
        manifest.schema == 1,
        "unsupported catalog schema {}",
        manifest.schema
    );
    ensure!(
        !manifest.problems.is_empty(),
        "catalog contains no problems"
    );
    let mut keys = HashSet::new();
    let mut declared = HashSet::from([PathBuf::from(MANIFEST_FILE)]);
    let mut validated = Vec::new();
    for item in manifest.problems {
        ensure!(
            item.key.len() >= 3
                && item.key.len() <= 200
                && item
                    .key
                    .bytes()
                    .enumerate()
                    .all(|(i, c)| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || (i > 0 && b"._:/-".contains(&c))),
            "invalid catalog key {}",
            item.key
        );
        ensure!(
            keys.insert(item.key.clone()),
            "duplicate catalog key {}",
            item.key
        );
        ensure!(
            item.sha256.len() == 64 && item.sha256.bytes().all(|c| c.is_ascii_hexdigit()),
            "invalid checksum for {}",
            item.key
        );
        let relative = safe_relative(&item.path)?;
        ensure!(
            declared.insert(relative.clone()),
            "duplicate artifact path {}",
            item.path
        );
        let path = root.join(&relative);
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("inspect {}", item.path))?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "artifact is not a regular file: {}",
            item.path
        );
        let artifact = fs::read(path)?;
        let actual = sha256(&artifact);
        ensure!(
            actual.eq_ignore_ascii_case(&item.sha256),
            "checksum mismatch for {}",
            item.path
        );
        let (problem, _) = validate_problem(&artifact)
            .with_context(|| format!("invalid artifact {}", item.path))?;
        validated.push(ValidatedProblem {
            key: item.key,
            path: item.path,
            hash: actual,
            problem,
        });
    }
    let mut actual = HashSet::new();
    files(root, root, &mut actual)?;
    ensure!(
        actual == declared,
        "release contains files not declared by manifest"
    );
    Ok(ValidatedRelease {
        manifest_checksum: sha256(&bytes),
        problems: validated,
    })
}
pub fn validate_repository_url(url: &str) -> Result<()> {
    let ssh = url
        .strip_prefix("ssh://")
        .is_some_and(|r| r.contains('@') && r.contains('/'));
    let scp = !url.contains("://")
        && url
            .split_once(':')
            .is_some_and(|(h, p)| h.contains('@') && !p.is_empty());
    ensure!(ssh || scp, "repository URL must be an SSH URL");
    ensure!(
        !url.chars().any(char::is_whitespace),
        "repository URL contains whitespace"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_only() {
        assert!(validate_repository_url("git@example.com:a/b.git").is_ok());
        assert!(validate_repository_url("https://example.com/a").is_err());
    }
    #[test]
    fn no_traversal() {
        assert!(safe_relative("../x").is_err());
    }
}
