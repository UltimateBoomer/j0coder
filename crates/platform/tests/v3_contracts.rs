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
        interface,
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
    let p = base(
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
    let mut value = serde_json::to_value(&p).unwrap();
    value["signature"] = json!({"method":"run","params":[],"returns":"int"});
    assert!(serde_json::from_value::<Problem>(value).is_err());
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
fn cpp_wrappers_support_nullable_values_and_empty_constructors() {
    let function = Interface::Function {
        name: "height".into(),
        params: vec![Parameter {
            name: "tree".into(),
            ty: Type::Array(Box::new(Type::Nullable(Box::new(Type::Int)))),
        }],
        returns: Type::Int,
    };
    let wrapper = Language::Cpp
        .wrapper_interface(
            &function,
            "int32_t height(vector<optional<int32_t>>) { return 0; }",
        )
        .unwrap();
    assert!(wrapper.contains("adl_serializer<std::optional<T>>"));
    assert!(wrapper.contains("get<vector<optional<int32_t>>>()"));

    let stateful = Interface::DataStructure {
        name: "Empty".into(),
        constructor: Constructor { params: vec![] },
        methods: vec![Method {
            name: "get".into(),
            params: vec![],
            returns: Type::Int,
        }],
    };
    let wrapper = Language::Cpp
        .wrapper_interface(
            &stateful,
            "class Empty { public: int32_t get() { return 0; } };",
        )
        .unwrap();
    assert!(wrapper.contains("Empty object;"));
    assert!(!wrapper.contains("Empty object();"));
}

#[test]
fn sibling_catalog_validates_all_problems_when_present() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../code-practice-problems");
    if !root.exists() {
        return;
    }
    let checkout = std::env::temp_dir().join(format!("locoder-catalog-{}", std::process::id()));
    std::fs::create_dir_all(checkout.join("problems")).unwrap();
    std::fs::copy(root.join("catalog.json"), checkout.join("catalog.json")).unwrap();
    for entry in std::fs::read_dir(root.join("problems")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(
            entry.path(),
            checkout.join("problems").join(entry.file_name()),
        )
        .unwrap();
    }
    let release = locoder::catalog::validate_release(&checkout).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("catalog.json")).unwrap()).unwrap();
    assert_eq!(
        release.problems.len(),
        manifest["problems"].as_array().unwrap().len()
    );
    assert!(
        release
            .problems
            .iter()
            .all(|entry| entry.problem.schema == 3
                && entry.hash
                    == locoder::catalog::sha256(
                        &std::fs::read(checkout.join(&entry.path)).unwrap()
                    ))
    );
    std::fs::remove_dir_all(checkout).unwrap();
}

#[test]
fn write_trusted_data_structure_fixtures() {
    if std::env::var("WRITE_FIXTURES").is_err() {
        return;
    }
    let root = std::path::Path::new("/tmp/practice-fixtures");
    std::fs::create_dir_all(root).unwrap();
    let interface = lru_interface();
    let cpp = "#include <bits/stdc++.h>\nusing namespace std;\nclass LRUCache { int capacity; list<pair<int,int>> values; public: LRUCache(int c):capacity(c) {} int get(int key) { for(auto it=values.begin();it!=values.end();++it) if(it->first==key){int value=it->second;values.erase(it);values.push_front({key,value});return value;} return -1; } void put(int key,int value){ for(auto it=values.begin();it!=values.end();++it) if(it->first==key){values.erase(it);break;} values.push_front({key,value});if((int)values.size()>capacity)values.pop_back();} };";
    let py = "class LRUCache:\n def __init__(self,capacity): self.capacity=capacity; self.values={}\n def get(self,key):\n  if key not in self.values: return -1\n  value=self.values.pop(key); self.values[key]=value; return value\n def put(self,key,value):\n  self.values.pop(key,None); self.values[key]=value\n  if len(self.values)>self.capacity: self.values.pop(next(iter(self.values)))";
    std::fs::write(
        root.join("lru.cpp"),
        Language::Cpp.wrapper_interface(&interface, cpp).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("lru.py"),
        Language::Python.wrapper_interface(&interface, py).unwrap(),
    )
    .unwrap();
    let catalog = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../code-practice-problems/problems/lru-cache.json");
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(catalog).unwrap()).unwrap();
    std::fs::write(
        root.join("lru.json"),
        serde_json::to_vec(&cases["tests"]).unwrap(),
    )
    .unwrap();
}
