use crate::contract::{Language, Problem};
use anyhow::{Context, Result, ensure};
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fmt, fs,
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
    pub reference: Option<ReferenceSource>,
}
#[derive(Debug, Clone)]
pub struct ReferenceSource {
    pub language: Language,
    pub source: String,
    pub hash: String,
}
#[derive(Debug)]
pub struct ValidatedRelease {
    pub source_checksum: String,
    pub problems: Vec<ValidatedProblem>,
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub fn problem_hash(problem: &Problem) -> Result<String> {
    Ok(sha256(&serde_json::to_vec(problem)?))
}
pub fn validate_problem(bytes: &[u8]) -> Result<(Problem, String)> {
    let problem: Problem = serde_json::from_slice(bytes).context("parse problem JSON")?;
    problem.validate().context("validate problem")?;
    let hash = problem_hash(&problem)?;
    Ok((problem, hash))
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
    if root.join(MANIFEST_FILE).exists() {
        ensure!(
            !root
                .join("problems")
                .read_dir()?
                .any(|e| e.is_ok_and(|e| e.path().is_dir())),
            "mixed legacy and directory catalog"
        );
        validate_legacy_release(root)
    } else {
        validate_directory_release(root)
    }
}
fn validate_legacy_release(root: &Path) -> Result<ValidatedRelease> {
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
            reference: None,
        });
    }
    let mut actual = HashSet::new();
    files(root, root, &mut actual)?;
    ensure!(
        actual == declared,
        "release contains files not declared by manifest"
    );
    Ok(ValidatedRelease {
        source_checksum: sha256(&bytes),
        problems: validated,
    })
}

fn yaml_json(bytes: &[u8], path: &Path) -> Result<Value> {
    let source =
        std::str::from_utf8(bytes).with_context(|| format!("{}: UTF-8", path.display()))?;
    // YAML aliases and custom tags hide the values that an author reviews in a case file.
    // Reject their syntax before serde_yaml resolves it.
    let mut block_indent = None;
    for line in source.lines() {
        let indent = line.bytes().take_while(|c| *c == b' ').count();
        if let Some(base) = block_indent {
            if line.trim().is_empty() || indent > base {
                continue;
            }
            block_indent = None;
        }
        let mut quote = None;
        let mut escaped = false;
        let mut prefix = true;
        for c in line.chars() {
            if let Some(q) = quote {
                if q == '"' && c == '\\' && !escaped {
                    escaped = true;
                    continue;
                }
                if c == q && !escaped {
                    quote = None;
                }
                escaped = false;
                continue;
            }
            if c == '#' && prefix {
                break;
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
                prefix = false;
                continue;
            }
            if prefix && matches!(c, '&' | '*' | '!') {
                anyhow::bail!("{}: YAML tags and aliases are unsupported", path.display());
            }
            prefix = c.is_whitespace() || matches!(c, ':' | '-' | ',' | '[' | '{');
        }
        let declaration = line.trim_end();
        if (declaration.contains(':') || declaration.trim_start().starts_with("- "))
            && ["|", "|-", "|+", ">", ">-", ">+"]
                .iter()
                .any(|marker| declaration.ends_with(marker))
        {
            block_indent = Some(indent);
        }
    }
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(source).with_context(|| format!("parse {}", path.display()))?;
    fn convert(value: serde_yaml::Value) -> Result<Value> {
        use serde_yaml::Value as Y;
        Ok(match value {
            Y::Null => Value::Null,
            Y::Bool(v) => Value::Bool(v),
            Y::Number(v) => {
                ensure!(
                    v.as_f64().is_none_or(f64::is_finite),
                    "non-finite YAML number"
                );
                serde_json::to_value(v).context("JSON-compatible YAML number")?
            }
            Y::String(v) => Value::String(v),
            Y::Sequence(xs) => Value::Array(xs.into_iter().map(convert).collect::<Result<_>>()?),
            Y::Mapping(xs) => {
                let mut out = serde_json::Map::new();
                for (k, v) in xs {
                    let Y::String(k) = k else {
                        anyhow::bail!("YAML mapping keys must be strings")
                    };
                    ensure!(
                        out.insert(k.clone(), convert(v)?).is_none(),
                        "duplicate key {k}"
                    );
                }
                Value::Object(out)
            }
            Y::Tagged(_) => anyhow::bail!("YAML tags are unsupported"),
        })
    }
    convert(parsed).with_context(|| format!("{}: JSON-compatible YAML", path.display()))
}

fn read_data(path: &Path) -> Result<Value> {
    ensure!(
        fs::metadata(path)?.len() <= 1_000_000,
        "{} exceeds 1 MB",
        path.display()
    );
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    match path.extension().and_then(|x| x.to_str()) {
        Some("json") => {
            let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
            let value = StrictJson::deserialize(&mut deserializer)
                .with_context(|| format!("parse {}", path.display()))?;
            deserializer
                .end()
                .with_context(|| format!("parse {}", path.display()))?;
            Ok(value.0)
        }
        Some("yaml") => yaml_json(&bytes, path),
        _ => anyhow::bail!("unsupported data file {}", path.display()),
    }
}

// serde_json::Value keeps the last duplicate key, which can silently replace a
// reviewed test input or expected answer. Deserialize recursively before the
// object is collapsed into a map.
struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct StrictVisitor;

        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictJson;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JSON value without duplicate object keys")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::Bool(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::from(value)))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::from(value)))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
                let number = serde_json::Number::from_f64(value)
                    .ok_or_else(|| E::custom("non-finite JSON number"))?;
                Ok(StrictJson(Value::Number(number)))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::String(value.to_owned())))
            }
            fn visit_string<E: de::Error>(
                self,
                value: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::String(value)))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<StrictJson>()? {
                    values.push(value.0);
                }
                Ok(StrictJson(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate key {key}")));
                    }
                    let value = map.next_value::<StrictJson>()?;
                    values.insert(key, value.0);
                }
                Ok(StrictJson(Value::Object(values)))
            }
        }

        deserializer.deserialize_any(StrictVisitor)
    }
}
fn read_text(path: &Path, limit: u64) -> Result<String> {
    ensure!(
        fs::metadata(path)?.len() <= limit,
        "{} exceeds {} bytes",
        path.display(),
        limit
    );
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn validate_directory_release(root: &Path) -> Result<ValidatedRelease> {
    let problem_root = root.join("problems");
    let kind = fs::symlink_metadata(&problem_root)?.file_type();
    ensure!(
        kind.is_dir() && !kind.is_symlink(),
        "problems must be a directory"
    );
    let mut all_files = HashSet::new();
    files(&problem_root, &problem_root, &mut all_files)?;
    let mut dirs = fs::read_dir(&problem_root)?
        .map(|e| e.map(|x| x.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    dirs.sort();
    ensure!(!dirs.is_empty(), "catalog contains no problems");
    let mut keys = HashSet::new();
    let mut titles = HashSet::new();
    let mut problems = Vec::new();
    let mut digest = Sha256::new();
    for dir in dirs {
        ensure!(
            dir.is_dir() && !fs::symlink_metadata(&dir)?.file_type().is_symlink(),
            "expected problem directory: {}",
            dir.display()
        );
        let metadata = dir.join("problem.yaml");
        let mut data = read_data(&metadata)?;
        let object = data
            .as_object_mut()
            .context("problem.yaml must be a mapping")?;
        ensure!(
            object.remove("schema") == Some(json!(1)),
            "{}: source schema must be 1",
            metadata.display()
        );
        let key: String =
            serde_json::from_value(object.remove("key").context("missing catalog key")?)?;
        ensure!(
            key.len() >= 3
                && key.len() <= 200
                && key.bytes().enumerate().all(|(i, c)| c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || (i > 0 && b"._:/-".contains(&c))),
            "invalid catalog key {key}"
        );
        ensure!(keys.insert(key.clone()), "duplicate catalog key {key}");
        object.insert("schema".into(), json!(3));
        if let Some(difficulty) = object.get("difficulty")
            && difficulty.is_number()
        {
            ensure!(
                !object.contains_key("difficulty_score"),
                "{}: integer difficulty cannot have difficulty_score",
                metadata.display()
            );
            let score = difficulty
                .as_u64()
                .filter(|score| (1..=5).contains(score))
                .with_context(|| {
                    format!(
                        "{}: difficulty must be an integer from 1 to 5",
                        metadata.display()
                    )
                })?;
            object.insert(
                "difficulty".into(),
                json!(Problem::difficulty_band(score as u8)),
            );
            object.insert("difficulty_score".into(), json!(score));
        }
        let mut expected = HashSet::from([PathBuf::from("problem.yaml")]);
        let statement_path = dir.join("statement.md");
        if object.contains_key("statement") {
            ensure!(
                !statement_path.exists(),
                "{}: statement is defined in both problem.yaml and statement.md",
                dir.display()
            );
        } else {
            object.insert(
                "statement".into(),
                json!(read_text(&statement_path, 100_000)?),
            );
            expected.insert(PathBuf::from("statement.md"));
        }
        let hint_dir = dir.join("hints");
        if object.contains_key("hints") {
            ensure!(
                !hint_dir.exists(),
                "{}: hints are defined in both problem.yaml and hints/",
                dir.display()
            );
        } else {
            let mut hints = Vec::new();
            if hint_dir.exists() {
                for path in sorted_files(&hint_dir, "md")? {
                    expected.insert(path.strip_prefix(&dir)?.to_owned());
                    hints.push(read_text(&path, 10_000)?);
                }
            }
            object.insert("hints".into(), json!(hints));
        }
        let mut cases = Vec::new();
        let mut case_paths = Vec::new();
        if let Some(inline) = object.remove("tests") {
            ensure!(
                !dir.join("tests").exists(),
                "{}: tests are defined in both problem.yaml and tests/",
                dir.display()
            );
            let groups = inline.as_object().with_context(|| {
                format!(
                    "{}: tests must have visible and hidden lists",
                    metadata.display()
                )
            })?;
            ensure!(
                groups.keys().all(|key| key == "visible" || key == "hidden"),
                "{}: unknown tests group",
                metadata.display()
            );
            for (visibility, hidden) in [("visible", false), ("hidden", true)] {
                let Some(group) = groups.get(visibility) else {
                    continue;
                };
                let entries = group.as_array().with_context(|| {
                    format!("{}: tests.{visibility} must be a list", metadata.display())
                })?;
                for (index, source_case) in entries.iter().enumerate() {
                    let path = PathBuf::from(format!(
                        "{}:tests.{visibility}[{index}]",
                        metadata.display()
                    ));
                    let mut case = source_case.clone();
                    let map = case
                        .as_object_mut()
                        .with_context(|| format!("{}: case must be mapping", path.display()))?;
                    ensure!(
                        !map.contains_key("hidden"),
                        "{}: visibility comes from tests group",
                        path.display()
                    );
                    map.insert("hidden".into(), json!(hidden));
                    cases.push(case);
                    case_paths.push(path);
                }
            }
        } else {
            for (visibility, hidden) in [("visible", false), ("hidden", true)] {
                let test_dir = dir.join("tests").join(visibility);
                if !test_dir.exists() {
                    continue;
                }
                for path in sorted_data_files(&test_dir)? {
                    expected.insert(path.strip_prefix(&dir)?.to_owned());
                    let mut case = read_data(&path)?;
                    let map = case
                        .as_object_mut()
                        .with_context(|| format!("{}: case must be mapping", path.display()))?;
                    ensure!(
                        !map.contains_key("hidden"),
                        "{}: visibility comes from directory",
                        path.display()
                    );
                    map.insert("hidden".into(), json!(hidden));
                    cases.push(case);
                    case_paths.push(path);
                }
            }
        }
        object.insert("tests".into(), json!(cases));
        let refs = [
            ("reference.py", Language::Python),
            ("reference.cpp", Language::Cpp),
            ("reference.java", Language::Java),
            ("reference.kt", Language::Kotlin),
        ]
        .into_iter()
        .filter(|(name, _)| dir.join(name).exists())
        .collect::<Vec<_>>();
        ensure!(
            refs.len() <= 1,
            "{}: only one reference solution allowed",
            dir.display()
        );
        let reference = if let Some((name, language)) = refs.first() {
            expected.insert(PathBuf::from(name));
            let source = read_text(&dir.join(name), 100_000)?;
            ensure!(
                !source.trim().is_empty() && source.len() <= 100_000,
                "{}: invalid reference source",
                dir.join(name).display()
            );
            Some(ReferenceSource {
                language: *language,
                hash: sha256(source.as_bytes()),
                source,
            })
        } else {
            None
        };
        let actual = all_files
            .iter()
            .filter_map(|p| p.strip_prefix(dir.file_name()?).ok().map(Path::to_owned))
            .collect::<HashSet<_>>();
        ensure!(
            actual == expected,
            "{}: unknown or missing files: {:?}",
            dir.display(),
            actual.symmetric_difference(&expected).collect::<Vec<_>>()
        );
        let problem: Problem =
            serde_json::from_value(data).with_context(|| format!("assemble {}", dir.display()))?;
        for (case, path) in problem.tests.iter().zip(&case_paths) {
            problem
                .validate_case(case)
                .with_context(|| format!("validate {}", path.display()))?;
            if matches!(
                problem.interface,
                crate::contract::Interface::Function { .. }
            ) {
                ensure!(
                    case.expected.is_some(),
                    "{}: function test needs expected",
                    path.display()
                );
            }
        }
        problem
            .validate()
            .with_context(|| format!("validate {}", dir.display()))?;
        ensure!(
            titles.insert(problem.title.trim().to_lowercase()),
            "duplicate title {}",
            problem.title
        );
        let hash = problem_hash(&problem)?;
        let path = dir.strip_prefix(root)?.to_string_lossy().into_owned();
        let mut paths = actual.into_iter().collect::<Vec<_>>();
        paths.sort();
        for p in paths {
            digest.update(path.as_bytes());
            digest.update([0]);
            digest.update(p.to_string_lossy().as_bytes());
            digest.update([0]);
            digest.update(fs::read(dir.join(p))?);
            digest.update([0]);
        }
        problems.push(ValidatedProblem {
            key,
            path,
            hash,
            problem,
            reference,
        });
    }
    Ok(ValidatedRelease {
        source_checksum: hex::encode(digest.finalize()),
        problems,
    })
}

fn sorted_files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)?
        .map(|e| e.map(|x| x.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    for path in &paths {
        ensure!(
            path.extension().and_then(|x| x.to_str()) == Some(extension)
                && path.is_file()
                && !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "unexpected file {}",
            path.display()
        );
    }
    Ok(paths)
}
fn sorted_data_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)?
        .map(|e| e.map(|x| x.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    for path in &paths {
        ensure!(
            matches!(
                path.extension().and_then(|x| x.to_str()),
                Some("json" | "yaml")
            ) && path.is_file()
                && !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "unexpected case file {}",
            path.display()
        );
    }
    Ok(paths)
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
    fn fixture() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("locoder-catalog-test-{}", uuid::Uuid::new_v4()));
        let p = root.join("problems/example");
        fs::create_dir_all(p.join("tests/visible")).unwrap();
        fs::write(p.join("problem.yaml"), "schema: 1\nkey: example/add-one\ntitle: Add One\ndifficulty: easy\ntags: [example]\ninterface:\n  kind: function\n  name: addOne\n  params: [{name: value, ty: int}]\n  returns: int\n").unwrap();
        fs::write(p.join("statement.md"), "Add one.").unwrap();
        fs::write(p.join("tests/visible/01.yaml"), "args: [2]\nexpected: 3\n").unwrap();
        root
    }
    #[test]
    fn directory_reference_changes_do_not_change_problem_identity() {
        let root = fixture();
        let first = validate_release(&root).unwrap();
        fs::write(
            root.join("problems/example/reference.py"),
            "def addOne(value): return value + 1\n",
        )
        .unwrap();
        let second = validate_release(&root).unwrap();
        assert_eq!(first.problems[0].hash, second.problems[0].hash);
        assert_ne!(first.source_checksum, second.source_checksum);
        assert!(second.problems[0].reference.is_some());
        fs::remove_file(root.join("problems/example/reference.py")).unwrap();
        fs::write(
            root.join("problems/example/reference.cpp"),
            "int addOne(int value) { return value + 1; }\n",
        )
        .unwrap();
        let cpp = validate_release(&root).unwrap();
        assert_eq!(first.problems[0].hash, cpp.problems[0].hash);
        assert_eq!(
            cpp.problems[0].reference.as_ref().unwrap().language,
            Language::Cpp
        );
        fs::remove_file(root.join("problems/example/reference.cpp")).unwrap();
        for (filename, source, language) in [
            (
                "reference.java",
                "class Solution { public Integer addOne(Integer value) { return value + 1; } }",
                Language::Java,
            ),
            (
                "reference.kt",
                "fun addOne(value: Int): Int = value + 1",
                Language::Kotlin,
            ),
        ] {
            let path = root.join("problems/example").join(filename);
            fs::write(&path, source).unwrap();
            let release = validate_release(&root).unwrap();
            assert_eq!(first.problems[0].hash, release.problems[0].hash);
            assert_eq!(
                release.problems[0].reference.as_ref().unwrap().language,
                language
            );
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_hints_and_cases_change_problem_identity() {
        let root = fixture();
        let p = root.join("problems/example");
        let original = validate_release(&root).unwrap().problems[0].hash.clone();
        fs::create_dir(p.join("hints")).unwrap();
        fs::write(p.join("hints/01.md"), "Add one.\n").unwrap();
        let hinted = validate_release(&root).unwrap().problems[0].hash.clone();
        assert_ne!(original, hinted);
        fs::write(p.join("tests/visible/01.yaml"), "args: [4]\nexpected: 5\n").unwrap();
        let changed_case = validate_release(&root).unwrap().problems[0].hash.clone();
        assert_ne!(hinted, changed_case);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_supports_inline_paragraphs_multiple_hints_and_integer_difficulty() {
        let root = fixture();
        let p = root.join("problems/example");
        fs::remove_file(p.join("statement.md")).unwrap();
        fs::write(
            p.join("problem.yaml"),
            "schema: 1\nkey: example/add-one\ntitle: Add One\ndifficulty: 2\nstatement: |\n  First paragraph.\n\n  Second paragraph.\n\n  * A Markdown list item.\nhints:\n  - |\n    First hint.\n  - |\n    Second hint.\ntags: [example]\ninterface:\n  kind: function\n  name: addOne\n  params: [{name: value, ty: int}]\n  returns: int\n",
        )
        .unwrap();
        let release = validate_release(&root).unwrap();
        let problem = &release.problems[0].problem;
        assert_eq!(
            problem.statement,
            "First paragraph.\n\nSecond paragraph.\n\n* A Markdown list item.\n"
        );
        assert_eq!(problem.hints, ["First hint.\n", "Second hint.\n"]);
        assert_eq!(problem.difficulty, "easy");
        assert_eq!(problem.difficulty_score, Some(2));

        fs::write(p.join("statement.md"), "Conflicting statement.\n").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_file(p.join("statement.md")).unwrap();
        fs::create_dir(p.join("hints")).unwrap();
        fs::write(p.join("hints/01.md"), "Conflicting hint.\n").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_dir_all(p.join("hints")).unwrap();
        let source = fs::read_to_string(p.join("problem.yaml")).unwrap();
        fs::write(
            p.join("problem.yaml"),
            source.replace("difficulty: 2\n", "difficulty: 2\ndifficulty_score: 2\n"),
        )
        .unwrap();
        let error = validate_release(&root).unwrap_err();
        assert!(format!("{error:#}").contains("integer difficulty cannot have difficulty_score"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_supports_inline_visible_and_hidden_cases() {
        let root = fixture();
        let p = root.join("problems/example");
        fs::create_dir_all(p.join("tests/hidden")).unwrap();
        fs::write(p.join("tests/hidden/01.yaml"), "args: [4]\nexpected: 5\n").unwrap();
        let original_hash = validate_release(&root).unwrap().problems[0].hash.clone();
        fs::remove_dir_all(p.join("tests")).unwrap();
        let source = fs::read_to_string(p.join("problem.yaml")).unwrap();
        let source = format!(
            "{source}tests:\n  visible:\n    - args: [2]\n      expected: 3\n  hidden:\n    - args: [4]\n      expected: 5\n"
        );
        fs::write(p.join("problem.yaml"), &source).unwrap();
        let release = validate_release(&root).unwrap();
        assert_eq!(release.problems[0].hash, original_hash);
        let cases = &release.problems[0].problem.tests;
        assert_eq!(cases.len(), 2);
        assert!(!cases[0].hidden);
        assert!(cases[1].hidden);
        assert_eq!(cases[0].expected, Some(json!(3)));
        assert_eq!(cases[1].expected, Some(json!(5)));

        fs::create_dir_all(p.join("tests/visible")).unwrap();
        fs::write(p.join("tests/visible/01.yaml"), "args: [2]\nexpected: 3\n").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_dir_all(p.join("tests")).unwrap();
        fs::write(
            p.join("problem.yaml"),
            source.replace("expected: 3\n", "expected: 3\n      hidden: true\n"),
        )
        .unwrap();
        let error = validate_release(&root).unwrap_err();
        assert!(format!("{error:#}").contains("visibility comes from tests group"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_rejects_ambiguous_cases_and_unknown_files() {
        let root = fixture();
        let p = root.join("problems/example");
        fs::write(
            p.join("tests/visible/01.yaml"),
            "args: [2]\nexpected: 3\nexpected: 4\n",
        )
        .unwrap();
        assert!(validate_release(&root).is_err());
        fs::write(
            p.join("tests/visible/01.yaml"),
            "args: [&shared 2]\nexpected: 3\n",
        )
        .unwrap();
        assert!(validate_release(&root).is_err());
        fs::write(p.join("tests/visible/01.yaml"), "args: [2]\nexpected: 3\n").unwrap();
        fs::write(p.join("extra.txt"), "unexpected").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn directory_rejects_duplicate_json_keys_at_any_depth() {
        let root = fixture();
        let path = root.join("problems/example/tests/visible/01.json");
        fs::remove_file(root.join("problems/example/tests/visible/01.yaml")).unwrap();
        for contents in [
            r#"{"args":[2],"expected":3,"expected":4}"#,
            r#"{"args":[{"value":2,"value":3}],"expected":3}"#,
        ] {
            fs::write(&path, contents).unwrap();
            let error = validate_release(&root).unwrap_err();
            assert!(format!("{error:#}").contains("duplicate key"), "{error:#}");
        }
        fs::remove_dir_all(root).unwrap();
    }
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
