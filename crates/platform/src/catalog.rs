use crate::contract::{AuthoringProblem, Language, Problem};
use anyhow::{Context, Result, ensure};
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fmt, fs,
    path::{Path, PathBuf},
};
use strum::IntoEnumIterator;

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
    ensure!(
        !root.join("catalog.json").exists(),
        "legacy catalog.json is unsupported"
    );
    validate_directory_release(root)
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
        for legacy in ["statement.md", "hints", "tests"] {
            ensure!(
                !dir.join(legacy).exists(),
                "{}: legacy {legacy} layout is unsupported",
                dir.display()
            );
        }
        let input: AuthoringProblem = serde_json::from_value(read_data(&metadata)?)
            .with_context(|| format!("parse {}", metadata.display()))?;
        let (key, problem) = input
            .assemble()
            .with_context(|| format!("assemble {}", metadata.display()))?;
        ensure!(
            key.len() >= 3
                && key.len() <= 200
                && key.bytes().enumerate().all(|(i, c)| c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || (i > 0 && b"._:/-".contains(&c))),
            "invalid catalog key {key}"
        );
        ensure!(keys.insert(key.clone()), "duplicate catalog key {key}");
        let mut expected = HashSet::from([PathBuf::from("problem.yaml")]);
        let refs = Language::iter()
            .map(|language| {
                (
                    format!("reference{}", language.descriptor().file_extension),
                    language,
                )
            })
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
    use serde_json::json;
    fn fixture() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("j0coder-catalog-test-{}", uuid::Uuid::new_v4()));
        let p = root.join("problems/example");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("problem.yaml"), "schema: 1\nkey: example/add-one\ntitle: Add One\ndifficulty: 2\nstatement: Add one.\nhints: []\ntags: [example]\ninterface:\n  kind: function\n  name: addOne\n  params: [{name: value, ty: int}]\n  returns: int\ntests:\n  visible: [{args: [2], expected: 3}]\n  hidden: [{args: [4], expected: 5}]\n").unwrap();
        root
    }
    #[test]
    fn inline_assembly_preserves_order_and_reference_identity() {
        let root = fixture();
        let first = validate_release(&root).unwrap();
        let p = &first.problems[0].problem;
        assert_eq!(p.schema, 3);
        assert_eq!(p.difficulty, "easy");
        assert_eq!(p.difficulty_score, Some(2));
        assert!(!p.tests[0].hidden);
        assert!(p.tests[1].hidden);
        assert_eq!(p.tests[0].expected, Some(json!(3)));
        for language in Language::iter() {
            let file = root
                .join("problems/example")
                .join(format!("reference{}", language.descriptor().file_extension));
            fs::write(&file, "reference source").unwrap();
            let next = validate_release(&root).unwrap();
            assert_eq!(next.problems[0].hash, first.problems[0].hash);
            assert_ne!(next.source_checksum, first.source_checksum);
            assert_eq!(
                next.problems[0].reference.as_ref().unwrap().language,
                language
            );
            fs::remove_file(file).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_legacy_layouts_even_when_empty() {
        let root = fixture();
        let dir = root.join("problems/example");
        for legacy in ["hints", "tests", "tests/visible", "tests/hidden"] {
            fs::create_dir_all(dir.join(legacy)).unwrap();
            assert!(validate_release(&root).is_err(), "{legacy}");
            fs::remove_dir_all(dir.join(legacy.split('/').next().unwrap())).unwrap();
        }
        fs::write(dir.join("statement.md"), "statement").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_file(dir.join("statement.md")).unwrap();
        fs::write(root.join("catalog.json"), "{}").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_invalid_inline_inputs() {
        let root = fixture();
        let path = root.join("problems/example/problem.yaml");
        let source = fs::read_to_string(&path).unwrap();
        for invalid in [
            source.replace("schema: 1", "schema: 3"),
            source.replace("difficulty: 2", "difficulty: easy"),
            source.replace("difficulty: 2", "difficulty: 0"),
            source.replace("difficulty: 2", "difficulty: 6"),
            source.replace("difficulty: 2", "difficulty: 2.5"),
            format!("{source}difficulty_score: 2\n"),
            format!("{source}unknown: true\n"),
            source.replace("expected: 3}", "expected: 3, hidden: false}"),
            source.replace("expected: 3}", "expected: 3, constructor_args: []}"),
            source.replace("expected: 3}", "expected: 3, expected: 4}"),
            source.replace("args: [2]", "args: [&shared 2]"),
            source.replace("visible: [{args: [2], expected: 3}]", "visible: []"),
        ] {
            fs::write(&path, &invalid).unwrap();
            assert!(validate_release(&root).is_err(), "{invalid}");
        }
        fs::write(&path, &source).unwrap();
        fs::write(root.join("problems/example/extra.txt"), "unknown").unwrap();
        assert!(validate_release(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_duplicate_json_keys_at_any_depth() {
        let root = fixture();
        let path = root.join("data.json");
        for contents in [
            r#"{"args":[2],"expected":3,"expected":4}"#,
            r#"{"args":[{"value":2,"value":3}]}"#,
        ] {
            fs::write(&path, contents).unwrap();
            assert!(format!("{:#}", read_data(&path).unwrap_err()).contains("duplicate key"));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ssh_only() {
        assert!(validate_repository_url("git@example.com:a/b.git").is_ok());
        assert!(validate_repository_url("https://example.com/a").is_err());
    }
}
