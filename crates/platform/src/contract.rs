use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Int,
    Bool,
    String,
    Array(Box<Type>),
}
impl Type {
    pub fn valid(&self, v: &Value) -> bool {
        match self {
            Self::Int => v.as_i64().is_some_and(|n| i32::try_from(n).is_ok()),
            Self::Bool => v.is_boolean(),
            Self::String => v.is_string(),
            Self::Array(t) => v.as_array().is_some_and(|a| a.iter().all(|v| t.valid(v))),
        }
    }
    pub fn depth(&self) -> usize {
        match self {
            Self::Array(t) => 1 + t.depth(),
            _ => 0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub method: String,
    pub params: Vec<Parameter>,
    pub returns: Type,
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        && [Language::Cpp, Language::Python]
            .iter()
            .all(|language| !language.is_reserved(s))
}
impl Signature {
    pub fn validate(&self) -> Result<()> {
        ensure!(identifier(&self.method), "invalid method name");
        ensure!(self.params.len() <= 16, "too many parameters");
        let mut names = std::collections::HashSet::new();
        for p in &self.params {
            ensure!(
                identifier(&p.name) && names.insert(&p.name) && p.ty.depth() <= 8,
                "invalid parameter"
            );
        }
        ensure!(self.returns.depth() <= 8, "type too deep");
        Ok(())
    }
    pub fn args_valid(&self, args: &Value) -> bool {
        args.as_array().is_some_and(|a| {
            a.len() == self.params.len() && a.iter().zip(&self.params).all(|(v, p)| p.ty.valid(v))
        })
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Cpp,
    Python,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub time_ms: u64,
    pub memory_mib: u64,
    pub output_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            time_ms: 2000,
            memory_mib: 256,
            output_bytes: 1048576,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub args: Value,
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub hidden: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    pub title: String,
    pub statement: String,
    pub difficulty: String,
    pub tags: Vec<String>,
    pub signature: Signature,
    #[serde(default)]
    pub limits: Limits,
    pub tests: Vec<Case>,
}
impl Problem {
    pub fn validate(&self) -> Result<()> {
        self.signature.validate()?;
        ensure!(
            !self.title.trim().is_empty()
                && self.title.len() <= 200
                && self.statement.len() <= 100000,
            "invalid title or statement"
        );
        ensure!(
            ["easy", "medium", "hard"].contains(&self.difficulty.as_str()),
            "invalid difficulty"
        );
        ensure!(
            self.tags.len() <= 20 && self.tags.iter().all(|s| s.len() <= 40),
            "invalid tags"
        );
        ensure!(
            !self.tests.is_empty()
                && self.tests.len() <= 200
                && self.tests.iter().any(|t| !t.hidden),
            "need visible tests, maximum 200"
        );
        ensure!(
            self.limits.time_ms > 0
                && self.limits.time_ms <= crate::env("MAX_TIME_MS", "10000").parse()?
                && self.limits.memory_mib >= 32
                && self.limits.memory_mib <= crate::env("MAX_MEMORY_MIB", "1024").parse()?
                && self.limits.output_bytes > 0
                && self.limits.output_bytes <= 1048576,
            "limits outside operator ceilings"
        );
        for t in &self.tests {
            if !self.signature.args_valid(&t.args)
                || !t
                    .expected
                    .as_ref()
                    .is_some_and(|v| self.signature.returns.valid(v))
            {
                bail!("invalid typed test")
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    #[serde(default)]
    pub attempt_base: u32,
    pub generation: Uuid,
    pub schema: u8,
    pub id: Uuid,
    pub language: Language,
    pub version: Uuid,
    pub signature: Signature,
    pub limits: Limits,
    pub mode: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accepted,
    WrongAnswer,
    CompilationError,
    RuntimeError,
    TimeLimit,
    MemoryLimit,
    OutputLimit,
    InfrastructureFailure,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub hidden: bool,
    pub verdict: Option<Verdict>,
    pub output: Option<Value>,
    pub log: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    #[serde(default)]
    pub elapsed_ms: u64,
    pub verdict: Verdict,
    pub passed: usize,
    pub total: usize,
    pub cases: Vec<CaseResult>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub generation: Uuid,
    pub schema: u8,
    pub id: Uuid,
    pub token: String,
    pub attempt: u32,
    pub outcome: Option<Outcome>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn types() {
        assert!(Type::Int.valid(&json!(-2147483648i64)));
        assert!(!Type::Int.valid(&json!(2147483648i64)));
        assert!(!Type::Int.valid(&json!(true)));
        assert!(!Type::Bool.valid(&json!(1)));
        let t = Type::Array(Box::new(Type::Array(Box::new(Type::String))));
        assert!(t.valid(&json!([[], ["你好", "🦀"]])));
        assert!(!t.valid(&json!([[1]])));
    }
    #[test]
    fn identifiers() {
        assert!(!identifier("a);system(x)"));
        assert!(!identifier("__init__"));
        assert!(!identifier("class"));
        assert!(identifier("rowTotals"));
    }
}
