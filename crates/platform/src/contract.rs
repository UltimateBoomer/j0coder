use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use strum::IntoEnumIterator;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Void,
    Int,
    Int64,
    Float,
    Bool,
    String,
    Array(Box<Type>),
    Nullable(Box<Type>),
    Named(String),
}
impl Type {
    pub fn valid(&self, v: &Value) -> bool {
        self.valid_with(v, &[])
    }
    pub fn valid_with(&self, v: &Value, defs: &[TypeDefinition]) -> bool {
        let mut n = 0;
        self.check(v, defs, &mut n, 0).is_ok()
    }
    fn check(&self, v: &Value, defs: &[TypeDefinition], n: &mut usize, depth: usize) -> Result<()> {
        *n += 1;
        ensure!(
            *n <= 10_000 && depth <= 64,
            "structured value limit exceeded"
        );
        match self {
            Self::Void => ensure!(v.is_null(), "expected null for void"),
            Self::Int => ensure!(
                v.as_i64().is_some_and(|x| i32::try_from(x).is_ok()),
                "expected int32"
            ),
            Self::Int64 => ensure!(
                v.as_i64().is_some() || v.as_u64().is_some_and(|x| x <= i64::MAX as u64),
                "expected int64"
            ),
            Self::Float => ensure!(
                v.as_f64().is_some_and(f64::is_finite),
                "expected finite float"
            ),
            Self::Bool => ensure!(v.is_boolean(), "expected bool"),
            Self::String => ensure!(v.is_string(), "expected string"),
            Self::Array(t) => {
                for x in v
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("expected array"))?
                {
                    t.check(x, defs, n, depth + 1)?
                }
            }
            Self::Nullable(t) => {
                if !v.is_null() {
                    t.check(v, defs, n, depth + 1)?
                }
            }
            Self::Named(name) => defs
                .iter()
                .find(|d| d.name == *name)
                .ok_or_else(|| anyhow::anyhow!("unknown named type {name}"))?
                .check(v, defs, n, depth + 1)?,
        };
        Ok(())
    }
    pub fn depth(&self) -> usize {
        match self {
            Self::Array(t) | Self::Nullable(t) => 1 + t.depth(),
            _ => 0,
        }
    }
    fn refs(&self, out: &mut Vec<String>, by_value: bool) {
        match self {
            Self::Named(n) if by_value => out.push(n.clone()),
            Self::Array(t) | Self::Nullable(t) => t.refs(out, false),
            _ => {}
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    #[default]
    Record,
    SinglyLinkedList,
    BinaryTree,
    NaryTree,
    ObjectGraph,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TypeDefinition {
    pub name: String,
    #[serde(default)]
    pub codec: Codec,
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub fields: Vec<Field>,
}
impl TypeDefinition {
    fn value_type(&self) -> Result<&Type> {
        self.fields
            .iter()
            .find(|f| f.name == "value" || f.name == "val")
            .map(|f| &f.ty)
            .ok_or_else(|| anyhow::anyhow!("{} requires a value field", self.name))
    }
    fn check(&self, v: &Value, defs: &[TypeDefinition], n: &mut usize, depth: usize) -> Result<()> {
        match self.codec {
            Codec::Record => {
                let o = v
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("expected record"))?;
                ensure!(o.len() == self.fields.len(), "unknown or missing fields");
                for f in &self.fields {
                    f.ty.check(
                        o.get(&f.name)
                            .ok_or_else(|| anyhow::anyhow!("missing field {}", f.name))?,
                        defs,
                        n,
                        depth + 1,
                    )?
                }
            }
            Codec::SinglyLinkedList => {
                for x in v
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("linked list must be array"))?
                {
                    self.value_type()?.check(x, defs, n, depth + 1)?
                }
            }
            Codec::BinaryTree => {
                let a = v
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("binary tree must be level-order array"))?;
                if !a.is_empty() {
                    ensure!(!a[0].is_null(), "null root")
                };
                for x in a {
                    if !x.is_null() {
                        self.value_type()?.check(x, defs, n, depth + 1)?
                    }
                }
            }
            Codec::NaryTree => {
                if v.is_null() {
                    return Ok(());
                }
                let o = v
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("n-ary tree must be record"))?;
                ensure!(
                    o.len() == 2 && o.contains_key("value") && o.contains_key("children"),
                    "invalid n-ary tree"
                );
                self.value_type()?.check(&o["value"], defs, n, depth + 1)?;
                for x in o["children"]
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("children must be array"))?
                {
                    self.check(x, defs, n, depth + 1)?
                }
            }
            Codec::ObjectGraph => check_graph(v, self, defs, n, depth)?,
        };
        Ok(())
    }
}
fn check_graph(
    v: &Value,
    d: &TypeDefinition,
    defs: &[TypeDefinition],
    n: &mut usize,
    depth: usize,
) -> Result<()> {
    let o = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("graph must be record"))?;
    ensure!(o.len() == 2, "graph requires roots and nodes");
    let roots = o
        .get("roots")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("roots must be array"))?;
    let nodes = o
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("nodes must be array"))?;
    let mut ids = HashSet::new();
    for node in nodes {
        let r = node
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("node must be record"))?;
        let id = r
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("node id must be string"))?;
        ensure!(ids.insert(id), "duplicate IDs")
    }
    let mut refs = Vec::new();
    for root in roots {
        if !root.is_null() {
            refs.push(
                root.as_str()
                    .ok_or_else(|| anyhow::anyhow!("root must be ID"))?,
            )
        }
    }
    for node in nodes {
        let r = node.as_object().unwrap();
        ensure!(r.len() == d.fields.len() + 1, "unknown graph fields");
        for f in &d.fields {
            let x = r
                .get(&f.name)
                .ok_or_else(|| anyhow::anyhow!("missing graph field"))?;
            let is_ref = matches!(&f.ty,Type::Named(s) if s==&d.name)
                || matches!(&f.ty,Type::Nullable(t) if matches!(&**t,Type::Named(s) if s==&d.name));
            if is_ref {
                if !x.is_null() {
                    refs.push(
                        x.as_str()
                            .ok_or_else(|| anyhow::anyhow!("reference must be ID"))?,
                    )
                }
            } else {
                f.ty.check(x, defs, n, depth + 1)?
            }
        }
    }
    ensure!(
        refs.iter().all(|x| ids.contains(x)),
        "unreachable reference"
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constructor {
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub params: Vec<Parameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub name: String,
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub params: Vec<Parameter>,
    pub returns: Type,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Interface {
    Function {
        name: String,
        #[serde(default)]
        #[schemars(length(max = 16))]
        params: Vec<Parameter>,
        returns: Type,
    },
    DataStructure {
        name: String,
        constructor: Constructor,
        #[schemars(length(min = 1, max = 32))]
        methods: Vec<Method>,
    },
}

impl Interface {
    pub fn validate_with(&self, defs: &[TypeDefinition]) -> Result<()> {
        match self {
            Self::Function { .. } => {
                let Self::Function {
                    name,
                    params,
                    returns,
                } = self
                else {
                    unreachable!()
                };
                ensure!(identifier(name), "invalid function name");
                validate_params(params, defs)?;
                ensure!(
                    returns.depth() <= 8 && !contains_void(returns),
                    "invalid function return"
                );
                type_refs(returns, &defs.iter().map(|d| d.name.as_str()).collect())?;
            }
            Self::DataStructure {
                name,
                constructor,
                methods,
            } => {
                ensure!(identifier(name), "invalid class name");
                validate_params(&constructor.params, defs)?;
                ensure!(
                    !methods.is_empty() && methods.len() <= 32,
                    "invalid methods"
                );
                let mut names = HashSet::new();
                for m in methods {
                    ensure!(
                        identifier(&m.name) && names.insert(&m.name),
                        "invalid or duplicate method"
                    );
                    validate_params(&m.params, defs)?;
                    ensure!(m.returns.depth() <= 8, "type too deep");
                    type_refs(&m.returns, &defs.iter().map(|d| d.name.as_str()).collect())?;
                    ensure!(
                        !contains_nested_void(&m.returns),
                        "void only allowed as a method return"
                    );
                }
            }
        }
        Ok(())
    }
    pub fn method(&self, name: &str) -> Option<&Method> {
        match self {
            Self::DataStructure { methods, .. } => methods.iter().find(|m| m.name == name),
            _ => None,
        }
    }
}

fn contains_void(t: &Type) -> bool {
    match t {
        Type::Void => true,
        Type::Array(x) | Type::Nullable(x) => contains_void(x),
        _ => false,
    }
}
fn contains_nested_void(t: &Type) -> bool {
    match t {
        Type::Void => false,
        Type::Array(x) | Type::Nullable(x) => contains_void(x),
        _ => false,
    }
}
fn validate_params(params: &[Parameter], defs: &[TypeDefinition]) -> Result<()> {
    ensure!(params.len() <= 16, "too many parameters");
    let known: HashSet<_> = defs.iter().map(|d| d.name.as_str()).collect();
    let mut names = HashSet::new();
    for p in params {
        ensure!(
            identifier(&p.name)
                && names.insert(&p.name)
                && p.ty.depth() <= 8
                && !contains_void(&p.ty),
            "invalid parameter"
        );
        type_refs(&p.ty, &known)?;
        if let Some(constraints) = &p.constraints {
            ensure!(
                !constraints.trim().is_empty() && constraints.len() <= 1000,
                "invalid constraints for parameter {}",
                p.name
            );
        }
    }
    Ok(())
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || (i > 0 && (c.is_ascii_digit() || c == b'_')))
        && Language::iter().all(|l| !l.is_reserved(s))
}
fn type_refs(t: &Type, known: &HashSet<&str>) -> Result<()> {
    match t {
        Type::Named(n) => ensure!(known.contains(n.as_str()), "unknown named type {n}"),
        Type::Array(x) | Type::Nullable(x) => type_refs(x, known)?,
        _ => {}
    }
    Ok(())
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Cpp,
    Python,
    Java,
    Kotlin,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[schemars(range(min = 1))]
    pub time_ms: u64,
    #[schemars(range(min = 32))]
    pub memory_mib: u64,
    #[schemars(range(min = 1, max = 1048576))]
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Case {
    #[serde(default)]
    pub args: Option<Value>,
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub constructor_args: Option<Value>,
    #[serde(default)]
    pub operations: Option<Vec<Operation>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub method: String,
    pub args: Value,
    pub expected: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub provider: String,
    pub source_url: String,
    #[serde(default)]
    pub notice: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArrayComparison {
    #[default]
    Ordered,
    Set,
    Multiset,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    #[serde(default)]
    pub array: ArrayComparison,
    #[serde(default)]
    #[schemars(range(min = 0))]
    pub absolute_tolerance: f64,
    #[serde(default)]
    #[schemars(range(min = 0))]
    pub relative_tolerance: f64,
    #[serde(default)]
    pub items: Option<Box<Comparison>>,
}
impl Default for Comparison {
    fn default() -> Self {
        Self {
            array: ArrayComparison::Ordered,
            absolute_tolerance: 0.0,
            relative_tolerance: 0.0,
            items: None,
        }
    }
}
impl Comparison {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.absolute_tolerance.is_finite()
                && self.absolute_tolerance >= 0.0
                && self.relative_tolerance.is_finite()
                && self.relative_tolerance >= 0.0,
            "invalid tolerance"
        );
        if let Some(x) = &self.items {
            x.validate()?
        }
        Ok(())
    }
    pub fn matches(&self, t: &Type, a: &Value, b: &Value) -> bool {
        compare(self, t, a, b)
    }
}
fn compare(p: &Comparison, t: &Type, a: &Value, b: &Value) -> bool {
    match t {
        Type::Float => a.as_f64().zip(b.as_f64()).is_some_and(|(x, y)| {
            (x - y).abs() <= p.absolute_tolerance + p.relative_tolerance * x.abs().max(y.abs())
        }),
        Type::Nullable(x) => {
            if a.is_null() || b.is_null() {
                a == b
            } else {
                compare(p, x, a, b)
            }
        }
        Type::Array(x) => {
            let Some((aa, bb)) = a.as_array().zip(b.as_array()) else {
                return false;
            };
            if aa.len() != bb.len() {
                return false;
            }
            let q = p.items.as_deref().unwrap_or(p);
            match p.array {
                ArrayComparison::Ordered => aa.iter().zip(bb).all(|(a, b)| compare(q, x, a, b)),
                ArrayComparison::Set => {
                    aa.iter().all(|a| bb.iter().any(|b| compare(q, x, a, b)))
                        && bb.iter().all(|b| aa.iter().any(|a| compare(q, x, a, b)))
                }
                ArrayComparison::Multiset => {
                    let mut used = vec![false; bb.len()];
                    aa.iter().all(|a| {
                        bb.iter()
                            .enumerate()
                            .position(|(i, b)| !used[i] && compare(q, x, a, b))
                            .is_some_and(|i| {
                                used[i] = true;
                                true
                            })
                    })
                }
            }
        }
        _ => a == b,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    pub schema: u8,
    pub title: String,
    #[serde(default)]
    #[schemars(length(max = 300))]
    pub summary: String,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    pub difficulty: String,
    #[serde(default)]
    pub difficulty_score: Option<u8>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub attribution: Option<Attribution>,
    #[serde(default)]
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub comparison: Comparison,
    pub interface: Interface,
    #[serde(default)]
    pub limits: Limits,
    pub tests: Vec<Case>,
}
impl Problem {
    pub fn effective_score(&self) -> u8 {
        self.difficulty_score
            .unwrap_or(match self.difficulty.as_str() {
                "easy" => 2,
                "medium" => 3,
                "hard" => 4,
                _ => 0,
            })
    }
    pub fn difficulty_band(n: u8) -> &'static str {
        if n <= 2 {
            "easy"
        } else if n == 3 {
            "medium"
        } else {
            "hard"
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 3, "unsupported problem schema");
        ensure!(self.type_definitions.len() <= 32, "too many definitions");
        let mut names = HashSet::new();
        for d in &self.type_definitions {
            ensure!(
                identifier(&d.name) && names.insert(&d.name),
                "invalid definition"
            );
            ensure!(d.fields.len() <= 32, "too many fields");
            let mut fs = HashSet::new();
            for f in &d.fields {
                ensure!(identifier(&f.name) && fs.insert(&f.name), "invalid field");
                ensure!(!contains_void(&f.ty), "void is not allowed in records");
                let mut refs = vec![];
                f.ty.refs(&mut refs, true);
                ensure!(!refs.contains(&d.name), "by-value recursive record")
            }
        }
        let known: HashSet<_> = self
            .type_definitions
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        for d in &self.type_definitions {
            for f in &d.fields {
                type_refs(&f.ty, &known)?;
            }
        }
        self.interface.validate_with(&self.type_definitions)?;
        ensure!(
            !self.title.trim().is_empty()
                && self.title.len() <= 200
                && self.statement.len() <= 100000,
            "invalid title or statement"
        );
        ensure!(self.summary.chars().count() <= 300, "summary too long");
        ensure!(
            self.hints.len() <= 20
                && self
                    .hints
                    .iter()
                    .all(|h| !h.trim().is_empty() && h.len() <= 10000),
            "invalid hints"
        );
        ensure!(
            ["easy", "medium", "hard"].contains(&self.difficulty.as_str()),
            "invalid difficulty"
        );
        let score = self.effective_score();
        ensure!((1..=5).contains(&score), "difficulty_score must be 1-5");
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
        self.comparison.validate()?;
        for t in &self.tests {
            self.validate_case(t)?;
            if matches!(self.interface, Interface::Function { .. }) {
                ensure!(
                    t.expected.is_some(),
                    "published function tests require expected"
                );
            }
        }
        Ok(())
    }
    pub fn validate_case(&self, t: &Case) -> Result<()> {
        match &self.interface {
            Interface::Function { .. } => {
                ensure!(
                    t.constructor_args.is_none() && t.operations.is_none(),
                    "function case shape"
                );
                let Interface::Function {
                    params, returns, ..
                } = &self.interface
                else {
                    unreachable!()
                };
                ensure!(
                    t.args
                        .as_ref()
                        .and_then(Value::as_array)
                        .is_some_and(|a| args_match(a, params, &self.type_definitions))
                        && t.expected
                            .as_ref()
                            .is_none_or(|v| returns.valid_with(v, &self.type_definitions)),
                    "invalid typed test"
                );
            }
            Interface::DataStructure { constructor, .. } => {
                ensure!(
                    t.args.is_none() && t.expected.is_none(),
                    "stateful case shape"
                );
                let ca = t
                    .constructor_args
                    .as_ref()
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow::anyhow!("missing constructor_args"))?;
                ensure!(
                    args_match(ca, &constructor.params, &self.type_definitions),
                    "invalid constructor arguments"
                );
                let ops = t
                    .operations
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing operations"))?;
                ensure!(ops.len() <= 200, "maximum 200 operations");
                for op in ops {
                    let m = self
                        .interface
                        .method(&op.method)
                        .ok_or_else(|| anyhow::anyhow!("unknown method {}", op.method))?;
                    let args = op
                        .args
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("operation args must be array"))?;
                    ensure!(
                        args_match(args, &m.params, &self.type_definitions),
                        "invalid operation arguments"
                    );
                    ensure!(
                        m.returns.valid_with(&op.expected, &self.type_definitions),
                        "invalid operation expected value"
                    );
                }
            }
        }
        Ok(())
    }
}
fn args_match(args: &[Value], params: &[Parameter], defs: &[TypeDefinition]) -> bool {
    args.len() == params.len()
        && args
            .iter()
            .zip(params)
            .all(|(v, p)| p.ty.valid_with(v, defs))
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Job {
    #[serde(default)]
    pub attempt_base: u32,
    pub generation: Uuid,
    pub schema: u8,
    pub id: Uuid,
    pub language: Language,
    pub version: Uuid,
    pub interface: Interface,
    pub limits: Limits,
    pub mode: String,
    #[serde(default)]
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub comparison: Comparison,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
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
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaseResult {
    pub hidden: bool,
    pub verdict: Option<Verdict>,
    pub output: Option<Value>,
    pub log: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Outcome {
    #[serde(default)]
    pub elapsed_ms: u64,
    pub verdict: Verdict,
    pub passed: usize,
    pub total: usize,
    pub cases: Vec<CaseResult>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    pub generation: Uuid,
    pub schema: u8,
    pub id: Uuid,
    pub token: String,
    pub attempt: u32,
    pub outcome: Option<Outcome>,
}

/// Metadata shared by API consumers and the editor. Execution filenames are separate.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LanguageDescriptor {
    pub id: Language,
    pub label: String,
    pub editor_label: String,
    pub monaco_language: String,
    pub file_extension: String,
    pub line_comment: String,
    pub editor_filename: String,
    pub editor_uri: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Capabilities {
    pub registration: String,
    pub guest_browsing: bool,
    pub web_admin: bool,
    pub languages: Vec<LanguageDescriptor>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProblemDetail {
    pub id: Uuid,
    pub version: Uuid,
    pub problem: Problem,
    pub starters: std::collections::BTreeMap<Language, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProblemSummary {
    pub id: Uuid,
    pub version: Uuid,
    pub title: String,
    pub summary: Option<String>,
    pub difficulty: String,
    pub difficulty_score: u8,
    pub tags: Vec<String>,
    pub cursor: String,
}

/// Current inline YAML authoring input; semantic validation follows assembly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthoringProblem {
    #[schemars(range(min = 1, max = 1))]
    pub schema: u8,
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9._:/-]{2,199}$"))]
    pub key: String,
    pub title: String,
    #[serde(default)]
    #[schemars(length(max = 300))]
    pub summary: String,
    #[schemars(range(min = 1, max = 5))]
    pub difficulty: u8,
    pub statement: String,
    #[serde(default)]
    #[schemars(length(max = 20))]
    pub hints: Vec<String>,
    #[schemars(length(max = 20))]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attribution: Option<Attribution>,
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub comparison: Comparison,
    pub interface: Interface,
    #[serde(default)]
    pub limits: Limits,
    pub tests: AuthoringTests,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthoringTests {
    #[schemars(length(min = 1, max = 200))]
    pub visible: Vec<AuthoringCase>,
    #[serde(default)]
    #[schemars(length(max = 200))]
    pub hidden: Vec<AuthoringCase>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum AuthoringCase {
    Function {
        args: Vec<Value>,
        expected: Value,
    },
    Stateful {
        constructor_args: Vec<Value>,
        #[schemars(length(max = 200))]
        operations: Vec<Operation>,
    },
}
impl AuthoringCase {
    fn assemble(self, hidden: bool) -> Case {
        match self {
            Self::Function { args, expected } => Case {
                args: Some(Value::Array(args)),
                expected: Some(expected),
                hidden,
                constructor_args: None,
                operations: None,
            },
            Self::Stateful {
                constructor_args,
                operations,
            } => Case {
                args: None,
                expected: None,
                hidden,
                constructor_args: Some(Value::Array(constructor_args)),
                operations: Some(operations),
            },
        }
    }
}
impl AuthoringProblem {
    pub fn assemble(self) -> Result<(String, Problem)> {
        ensure!(self.schema == 1, "source schema must be 1");
        ensure!(
            (1..=5).contains(&self.difficulty),
            "difficulty must be an integer from 1 to 5"
        );
        let tests = self
            .tests
            .visible
            .into_iter()
            .map(|c| c.assemble(false))
            .chain(self.tests.hidden.into_iter().map(|c| c.assemble(true)))
            .collect();
        let problem = Problem {
            schema: 3,
            title: self.title,
            summary: self.summary,
            statement: self.statement,
            hints: self.hints,
            difficulty: Problem::difficulty_band(self.difficulty).into(),
            difficulty_score: Some(self.difficulty),
            tags: self.tags,
            attribution: self.attribution,
            type_definitions: self.type_definitions,
            comparison: self.comparison,
            interface: self.interface,
            limits: self.limits,
            tests,
        };
        problem.validate()?;
        Ok((self.key, problem))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CatalogDraft {
    pub id: Uuid,
    pub draft: Problem,
    pub version: Option<Uuid>,
    pub managed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreatedProblem {
    pub id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PublishedProblem {
    pub version: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidatedDefinition {
    pub valid: bool,
    pub content_hash: String,
}
