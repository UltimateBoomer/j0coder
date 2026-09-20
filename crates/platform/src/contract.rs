use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    #[default]
    Record,
    SinglyLinkedList,
    BinaryTree,
    NaryTree,
    ObjectGraph,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypeDefinition {
    pub name: String,
    #[serde(default)]
    pub codec: Codec,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constructor {
    #[serde(default)]
    pub params: Vec<Parameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    #[serde(default)]
    pub params: Vec<Parameter>,
    pub returns: Type,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Interface {
    Function {
        name: String,
        #[serde(default)]
        params: Vec<Parameter>,
        returns: Type,
    },
    DataStructure {
        name: String,
        constructor: Constructor,
        methods: Vec<Method>,
    },
}

impl Interface {
    pub fn function_signature(&self) -> Option<Signature> {
        match self {
            Self::Function {
                name,
                params,
                returns,
            } => Some(Signature {
                method: name.clone(),
                params: params.clone(),
                returns: returns.clone(),
            }),
            _ => None,
        }
    }
    pub fn validate_with(&self, defs: &[TypeDefinition]) -> Result<()> {
        match self {
            Self::Function { .. } => {
                let s = self.function_signature().unwrap();
                s.validate_with(defs)?;
                ensure!(
                    !matches!(s.returns, Type::Void),
                    "function cannot return void"
                );
                ensure!(
                    s.params.iter().all(|p| !contains_void(&p.ty)),
                    "void parameter"
                );
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
    }
    Ok(())
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || (i > 0 && (c.is_ascii_digit() || c == b'_')))
        && [Language::Cpp, Language::Python]
            .iter()
            .all(|l| !l.is_reserved(s))
}
impl Signature {
    pub fn validate(&self) -> Result<()> {
        self.validate_with(&[])
    }
    pub fn validate_with(&self, defs: &[TypeDefinition]) -> Result<()> {
        ensure!(identifier(&self.method), "invalid method name");
        ensure!(self.params.len() <= 16, "too many parameters");
        let known: HashSet<_> = defs.iter().map(|d| d.name.as_str()).collect();
        let mut names = HashSet::new();
        for p in &self.params {
            ensure!(
                identifier(&p.name) && names.insert(&p.name) && p.ty.depth() <= 8,
                "invalid parameter"
            );
            type_refs(&p.ty, &known)?
        }
        ensure!(self.returns.depth() <= 8, "type too deep");
        ensure!(
            !contains_void(&self.returns),
            "legacy signature cannot use void"
        );
        type_refs(&self.returns, &known)
    }
    pub fn args_valid(&self, v: &Value) -> bool {
        self.args_valid_with(v, &[])
    }
    pub fn args_valid_with(&self, v: &Value, d: &[TypeDefinition]) -> bool {
        v.as_array().is_some_and(|a| {
            a.len() == self.params.len()
                && a.iter()
                    .zip(&self.params)
                    .all(|(v, p)| p.ty.valid_with(v, d))
        })
    }
}
fn type_refs(t: &Type, known: &HashSet<&str>) -> Result<()> {
    match t {
        Type::Named(n) => ensure!(known.contains(n.as_str()), "unknown named type {n}"),
        Type::Array(x) | Type::Nullable(x) => type_refs(x, known)?,
        _ => {}
    }
    Ok(())
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub method: String,
    pub args: Value,
    pub expected: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    pub provider: String,
    pub source_url: String,
    #[serde(default)]
    pub notice: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArrayComparison {
    #[default]
    Ordered,
    Set,
    Multiset,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comparison {
    #[serde(default)]
    pub array: ArrayComparison,
    #[serde(default)]
    pub absolute_tolerance: f64,
    #[serde(default)]
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

fn one() -> u8 {
    1
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    #[serde(default = "one")]
    pub schema: u8,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub statement: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<Interface>,
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
        ensure!([1, 2, 3].contains(&self.schema), "unsupported schema");
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
        if self.schema == 3 {
            ensure!(self.signature.is_none(), "schema 3 rejects signature");
            self.interface
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("schema 3 requires interface"))?
                .validate_with(&self.type_definitions)?;
        } else {
            ensure!(self.interface.is_none(), "legacy schema rejects interface");
            self.signature
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("legacy schema requires signature"))?
                .validate_with(&self.type_definitions)?;
        }
        ensure!(
            !self.title.trim().is_empty()
                && self.title.len() <= 200
                && self.statement.len() <= 100000,
            "invalid title or statement"
        );
        ensure!(self.summary.chars().count() <= 300, "summary too long");
        ensure!(
            ["easy", "medium", "hard"].contains(&self.difficulty.as_str()),
            "invalid difficulty"
        );
        let score = self.effective_score();
        ensure!((1..=5).contains(&score), "difficulty_score must be 1-5");
        if self.schema == 2 {
            ensure!(
                self.difficulty == Self::difficulty_band(score),
                "difficulty does not match score"
            )
        }
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
            if self.signature.is_some()
                || matches!(self.interface, Some(Interface::Function { .. }))
            {
                ensure!(
                    t.expected.is_some(),
                    "published function tests require expected"
                );
            }
        }
        Ok(())
    }
    pub fn validate_case(&self, t: &Case) -> Result<()> {
        if let Some(sig) = &self.signature {
            ensure!(
                t.constructor_args.is_none() && t.operations.is_none(),
                "legacy case shape"
            );
            let args = t
                .args
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing args"))?;
            ensure!(
                sig.args_valid_with(args, &self.type_definitions)
                    && t.expected
                        .as_ref()
                        .is_none_or(|v| sig.returns.valid_with(v, &self.type_definitions)),
                "invalid typed test"
            );
            return Ok(());
        }
        match self.interface.as_ref().unwrap() {
            Interface::Function { .. } => {
                ensure!(
                    t.constructor_args.is_none() && t.operations.is_none(),
                    "function case shape"
                );
                let sig = self
                    .interface
                    .as_ref()
                    .unwrap()
                    .function_signature()
                    .unwrap();
                ensure!(
                    t.args
                        .as_ref()
                        .is_some_and(|a| sig.args_valid_with(a, &self.type_definitions))
                        && t.expected
                            .as_ref()
                            .is_none_or(|v| sig.returns.valid_with(v, &self.type_definitions)),
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
                        .as_ref()
                        .unwrap()
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    #[serde(default)]
    pub attempt_base: u32,
    pub generation: Uuid,
    pub schema: u8,
    pub id: Uuid,
    pub language: Language,
    pub version: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<Interface>,
    pub limits: Limits,
    pub mode: String,
    #[serde(default)]
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub comparison: Comparison,
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
