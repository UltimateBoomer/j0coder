use practice::contract::*;
use serde_json::json;
#[test]
fn published_samples_validate() {
    for text in [
        include_str!("../../../samples/1.json"),
        include_str!("../../../samples/2.json"),
        include_str!("../../../samples/3.json"),
    ] {
        let p: Problem = serde_json::from_str(text).unwrap();
        p.validate().unwrap();
        let copy: Problem = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(copy.tests.len(), p.tests.len());
    }
}
#[test]
fn typed_boundaries() {
    let t = Type::Array(Box::new(Type::Array(Box::new(Type::Int))));
    assert!(t.valid(&json!([[], [-2147483648i64, 2147483647]])));
    for v in [
        json!(null),
        json!([[1.0]]),
        json!([[true]]),
        json!([[2147483648i64]]),
    ] {
        assert!(!t.valid(&v))
    }
    assert!(Type::String.valid(&json!("\u{0}你好🦀")));
}

#[test]
fn starters_render_exactly_for_empty_and_typed_signatures() {
    let empty = Signature {
        method: "empty".into(),
        params: vec![],
        returns: Type::String,
    };
    assert_eq!(
        Language::Cpp.starter(&empty).unwrap(),
        "#include <bits/stdc++.h>\nusing namespace std;\n\nclass Solution {\npublic:\n    std::string empty() {\n        return {};\n    }\n};\n"
    );
    assert_eq!(
        Language::Python.starter(&empty).unwrap(),
        "class Solution:\n    def empty(self) -> str:\n        pass\n"
    );

    let typed = Signature {
        method: "combineValues".into(),
        params: vec![
            Parameter {
                name: "count".into(),
                ty: Type::Int,
            },
            Parameter {
                name: "enabled".into(),
                ty: Type::Bool,
            },
            Parameter {
                name: "label".into(),
                ty: Type::String,
            },
            Parameter {
                name: "matrix".into(),
                ty: Type::Array(Box::new(Type::Array(Box::new(Type::Int)))),
            },
        ],
        returns: Type::Array(Box::new(Type::Bool)),
    };
    assert_eq!(
        Language::Cpp.starter(&typed).unwrap(),
        "#include <bits/stdc++.h>\nusing namespace std;\n\nclass Solution {\npublic:\n    std::vector<bool> combineValues(int32_t count, bool enabled, std::string label, std::vector<std::vector<int32_t>> matrix) {\n        return {};\n    }\n};\n"
    );
    assert_eq!(
        Language::Python.starter(&typed).unwrap(),
        "class Solution:\n    def combineValues(self, count: int, enabled: bool, label: str, matrix: list[list[int]]) -> list[bool]:\n        pass\n"
    );
}

#[test]
fn generated_starters_parse_and_compile() {
    let signature = Signature {
        method: "combineValues".into(),
        params: vec![
            Parameter {
                name: "count".into(),
                ty: Type::Int,
            },
            Parameter {
                name: "enabled".into(),
                ty: Type::Bool,
            },
            Parameter {
                name: "label".into(),
                ty: Type::String,
            },
            Parameter {
                name: "matrix".into(),
                ty: Type::Array(Box::new(Type::Array(Box::new(Type::Int)))),
            },
        ],
        returns: Type::Array(Box::new(Type::Bool)),
    };
    let root = std::env::temp_dir().join(format!(
        "practice-starter-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&root).unwrap();
    let cpp_path = root.join("starter.cpp");
    std::fs::write(&cpp_path, Language::Cpp.starter(&signature).unwrap()).unwrap();
    let cpp_compiler = std::env::var_os("CXX").unwrap_or_else(|| "/usr/bin/c++".into());
    let cpp_status = std::process::Command::new(cpp_compiler)
        .args(["-std=c++17", "-fsyntax-only"])
        .arg(&cpp_path)
        .status()
        .expect("c++ is required to verify generated C++");
    assert!(cpp_status.success());

    let python = Language::Python.starter(&signature).unwrap();
    let mut python_process = std::process::Command::new("python3")
        .args(["-c", "import ast, sys; ast.parse(sys.stdin.read())"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("python3 is required to verify generated Python");
    use std::io::Write;
    python_process
        .stdin
        .take()
        .unwrap()
        .write_all(python.as_bytes())
        .unwrap();
    assert!(python_process.wait().unwrap().success());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn generated_wrappers_never_embed_tests() {
    let p: Problem = serde_json::from_str(include_str!("../../../samples/1.json")).unwrap();
    for lang in [Language::Cpp, Language::Python] {
        let wrapped = lang.wrapper(&p.signature, &lang.starter(&p.signature).unwrap());
        assert!(!wrapped.contains("2147483648"));
        assert!(wrapped.contains("/work/result"));
        assert!(wrapped.contains("between"));
    }
}
#[test]
fn argument_counts_and_reserved_names() {
    let mut p: Problem = serde_json::from_str(include_str!("../../../samples/1.json")).unwrap();
    assert!(!p.signature.args_valid(&json!([1, 2])));
    assert!(!p.signature.args_valid(&json!([true, 2, 3])));
    p.signature.params[0].name = "class".into();
    assert!(p.validate().is_err());
}
#[test]
fn write_trusted_wrapper_fixtures() {
    if std::env::var("WRITE_FIXTURES").is_err() {
        return;
    }
    let root = std::path::Path::new("/tmp/practice-fixtures");
    std::fs::create_dir_all(root).unwrap();
    let sources = [
        (
            "class Solution { public: bool between(int value,int left,int right){return value>=std::min(left,right)&&value<=std::max(left,right);} };",
            "class Solution:\n def between(self,value,left,right): return min(left,right)<=value<=max(left,right)",
        ),
        (
            "class Solution { public: std::vector<int> quietPeaks(std::vector<int> values){std::vector<int> r;for(int i=0;i<(int)values.size();i++)if((i==0||values[i]>values[i-1])&&(i+1==(int)values.size()||values[i]>values[i+1]))r.push_back(i);return r;} };",
            "class Solution:\n def quietPeaks(self,values): return [i for i,v in enumerate(values) if (i==0 or v>values[i-1]) and (i+1==len(values) or v>values[i+1])]",
        ),
        (
            "class Solution { public: std::vector<std::vector<int>> foldRows(std::vector<std::vector<int>> rows){std::reverse(rows.begin(),rows.end());for(auto &r:rows)std::reverse(r.begin(),r.end());return rows;} };",
            "class Solution:\n def foldRows(self,rows): return [r[::-1] for r in rows[::-1]]",
        ),
    ];
    for (i, text) in [
        include_str!("../../../samples/1.json"),
        include_str!("../../../samples/2.json"),
        include_str!("../../../samples/3.json"),
    ]
    .iter()
    .enumerate()
    {
        let p: Problem = serde_json::from_str(text).unwrap();
        std::fs::write(
            root.join(format!("{i}.cpp")),
            Language::Cpp.wrapper(
                &p.signature,
                &format!("#include <bits/stdc++.h>\n{}", sources[i].0),
            ),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("{i}.py")),
            Language::Python.wrapper(&p.signature, sources[i].1),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("{i}.json")),
            serde_json::to_vec(&p.tests).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn cpp_only_keywords_are_rejected() {
    let mut p: Problem = serde_json::from_str(include_str!("../../../samples/1.json")).unwrap();
    for name in [
        "asm", "alignas", "alignof", "compl", "bitand", "bitor", "xor", "typeid", "concept",
    ] {
        p.signature.method = name.into();
        assert!(p.signature.validate().is_err(), "reserved method: {name}");
    }
}

#[test]
fn python_only_keywords_are_rejected() {
    let mut p: Problem = serde_json::from_str(include_str!("../../../samples/1.json")).unwrap();
    for name in ["def", "lambda", "yield", "nonlocal", "match", "del"] {
        p.signature.method = name.into();
        assert!(p.signature.validate().is_err(), "reserved method: {name}");
    }
}
