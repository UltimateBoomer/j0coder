use locoder::contract::*;
use serde_json::{Value, json};

fn base(interface: Interface, tests: Vec<Case>) -> Problem {
    Problem {
        schema: 3,
        title: "Schema three".into(),
        summary: String::new(),
        statement: "Implement it.".into(),
        difficulty: "easy".into(),
        difficulty_score: Some(2),
        tags: vec!["design".into()],
        attribution: None,
        type_definitions: vec![],
        comparison: Comparison::default(),
        signature: None,
        interface: Some(interface),
        limits: Limits::default(),
        tests,
    }
}

#[test]
fn function_interface_generates_top_level_code() {
    let interface = Interface::Function {
        name: "twoSum".into(),
        params: vec![Parameter {
            name: "values".into(),
            ty: Type::Array(Box::new(Type::Int)),
        }],
        returns: Type::Array(Box::new(Type::Int)),
    };
    let p = base(
        interface.clone(),
        vec![Case {
            args: Some(json!([[2, 7]])),
            expected: Some(json!([0, 1])),
            hidden: false,
            constructor_args: None,
            operations: None,
        }],
    );
    p.validate().unwrap();
    let cpp = Language::Cpp.starter_interface(&interface).unwrap();
    let py = Language::Python.starter_interface(&interface).unwrap();
    assert!(cpp.contains("vector<int32_t> twoSum("));
    assert!(!cpp.contains("class Solution"));
    assert!(py.starts_with("def twoSum("));
}

fn lru_interface() -> Interface {
    Interface::DataStructure {
        name: "LRUCache".into(),
        constructor: Constructor {
            params: vec![Parameter {
                name: "capacity".into(),
                ty: Type::Int,
            }],
        },
        methods: vec![
            Method {
                name: "get".into(),
                params: vec![Parameter {
                    name: "key".into(),
                    ty: Type::Int,
                }],
                returns: Type::Int,
            },
            Method {
                name: "put".into(),
                params: vec![
                    Parameter {
                        name: "key".into(),
                        ty: Type::Int,
                    },
                    Parameter {
                        name: "value".into(),
                        ty: Type::Int,
                    },
                ],
                returns: Type::Void,
            },
        ],
    }
}

#[test]
fn stateful_trace_is_typed_and_void_requires_null() {
    let valid = Case {
        args: None,
        expected: None,
        hidden: false,
        constructor_args: Some(json!([2])),
        operations: Some(vec![
            Operation {
                method: "put".into(),
                args: json!([1, 10]),
                expected: Value::Null,
            },
            Operation {
                method: "get".into(),
                args: json!([1]),
                expected: json!(10),
            },
        ]),
    };
    let p = base(lru_interface(), vec![valid.clone()]);
    p.validate().unwrap();
    let mut bad = valid;
    bad.operations.as_mut().unwrap()[0].expected = json!(0);
    assert!(base(lru_interface(), vec![bad]).validate().is_err());
}

#[test]
fn rejects_unknown_duplicate_and_oversized_operations() {
    let mut interface = lru_interface();
    if let Interface::DataStructure { methods, .. } = &mut interface {
        methods.push(methods[0].clone());
    }
    assert!(interface.validate_with(&[]).is_err());
    let op = Operation {
        method: "missing".into(),
        args: json!([]),
        expected: json!(0),
    };
    let case = Case {
        args: None,
        expected: None,
        hidden: false,
        constructor_args: Some(json!([2])),
        operations: Some(vec![op]),
    };
    assert!(base(lru_interface(), vec![case]).validate().is_err());
}

#[test]
fn schema_three_rejects_legacy_signature_and_function_void() {
    let mut p = base(
        Interface::Function {
            name: "run".into(),
            params: vec![],
            returns: Type::Void,
        },
        vec![Case {
            args: Some(json!([])),
            expected: Some(Value::Null),
            hidden: false,
            constructor_args: None,
            operations: None,
        }],
    );
    assert!(p.validate().is_err());
    p.signature = Some(Signature {
        method: "run".into(),
        params: vec![],
        returns: Type::Int,
    });
    assert!(p.validate().is_err());
}

#[test]
fn stateful_starters_and_wrappers_parse_and_compile() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let interface = lru_interface();
    let starter = Language::Cpp.starter_interface(&interface).unwrap();
    let cpp = Language::Cpp
        .wrapper_interface(&interface, &starter)
        .unwrap();
    assert!(cpp.contains("operation \" << index"));
    let mut compiler = Command::new("g++")
        .args(["-std=c++20", "-x", "c++", "-fsyntax-only", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    compiler
        .stdin
        .take()
        .unwrap()
        .write_all(starter.as_bytes())
        .unwrap();
    assert!(compiler.wait().unwrap().success());
    let python = Language::Python
        .wrapper_interface(
            &interface,
            &Language::Python.starter_interface(&interface).unwrap(),
        )
        .unwrap();
    let mut parser = Command::new("python3")
        .args([
            "-c",
            "import sys; compile(sys.stdin.read(), '<wrapper>', 'exec')",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    parser
        .stdin
        .take()
        .unwrap()
        .write_all(python.as_bytes())
        .unwrap();
    assert!(parser.wait().unwrap().success());
}

#[test]
fn sibling_catalog_accepts_lru_fixture_when_present() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../code-practice-problems");
    if root.exists() {
        let bytes = std::fs::read(root.join("problems/lru-cache.json")).unwrap();
        locoder::catalog::validate_problem(&bytes).unwrap();
        let manifest = std::fs::read_to_string(root.join("catalog.json")).unwrap();
        assert!(manifest.contains("design/lru-cache"));
        assert!(manifest.contains(&locoder::catalog::sha256(&bytes)));
    }
}
