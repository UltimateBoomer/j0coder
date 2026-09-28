use locoder::{catalog, contract::*};
use serde_json::{Value, json};

fn sample() -> Problem {
    serde_json::from_str(include_str!("../../../samples/1.json")).unwrap()
}

#[test]
fn parameter_constraints_are_optional_validated_and_versioned() {
    let original = sample();
    let original_hash = catalog::problem_hash(&original).unwrap();
    let original_value = serde_json::to_value(&original).unwrap();
    assert!(
        original_value["interface"]["params"][0]
            .as_object()
            .unwrap()
            .get("constraints")
            .is_none()
    );

    let mut explicit_null = original_value.clone();
    explicit_null["interface"]["params"][0]["constraints"] = Value::Null;
    let explicit_null: Problem = serde_json::from_value(explicit_null).unwrap();
    assert_eq!(
        catalog::problem_hash(&explicit_null).unwrap(),
        original_hash
    );

    let mut constrained = original_value;
    constrained["interface"]["params"][0]["constraints"] = json!("1 ≤ value ≤ 100");
    let constrained: Problem = serde_json::from_value(constrained).unwrap();
    constrained.validate().unwrap();
    assert_ne!(catalog::problem_hash(&constrained).unwrap(), original_hash);

    for invalid in ["  \n".to_string(), "é".repeat(501)] {
        let mut bad = constrained.clone();
        if let Interface::Function { params, .. } = &mut bad.interface {
            params[0].constraints = Some(invalid);
        }
        assert!(bad.validate().is_err());
    }

    let mut boundary = constrained;
    if let Interface::Function { params, .. } = &mut boundary.interface {
        params[0].constraints = Some("x".repeat(1000));
    }
    boundary.validate().unwrap();
}

#[test]
fn published_samples_validate_and_reject_legacy_contracts() {
    for text in [
        include_str!("../../../samples/1.json"),
        include_str!("../../../samples/2.json"),
        include_str!("../../../samples/3.json"),
    ] {
        let p: Problem = serde_json::from_str(text).unwrap();
        p.validate().unwrap();
        let mut value: Value = serde_json::from_str(text).unwrap();
        value["schema"] = json!(1);
        assert!(catalog::validate_problem(&serde_json::to_vec(&value).unwrap()).is_err());
        value["schema"] = json!(2);
        assert!(catalog::validate_problem(&serde_json::to_vec(&value).unwrap()).is_err());
        value["schema"] = json!(3);
        value.as_object_mut().unwrap().remove("interface");
        assert!(catalog::validate_problem(&serde_json::to_vec(&value).unwrap()).is_err());
        value["interface"] = serde_json::to_value(&p.interface).unwrap();
        value["signature"] = json!({"method":"solve","params":[],"returns":"int"});
        assert!(catalog::validate_problem(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn typed_boundaries_and_case_validation() {
    let t = Type::Array(Box::new(Type::Array(Box::new(Type::Int))));
    assert!(t.valid(&json!([[], [-2147483648i64, 2147483647]])));
    for v in [
        json!(null),
        json!([[1.0]]),
        json!([[true]]),
        json!([[2147483648i64]]),
    ] {
        assert!(!t.valid(&v));
    }
    let mut p = sample();
    p.tests[0].args = Some(json!([true, 2, 3]));
    assert!(p.validate().is_err());
    let mut p = sample();
    if let Interface::Function { params, .. } = &mut p.interface {
        params[0].name = "class".into();
    }
    assert!(p.validate().is_err());
}

#[test]
fn function_starters_and_wrappers_use_top_level_functions() {
    let p = sample();
    for lang in [Language::Cpp, Language::Python] {
        let starter = lang.starter_interface(&p.interface).unwrap();
        let wrapper = lang.wrapper_interface(&p.interface, &starter).unwrap();
        assert!(!starter.contains("class Solution"));
        assert!(starter.contains("between("));
        assert!(wrapper.contains("/work/result"));
        assert!(!wrapper.contains("2147483648"));
    }
}

#[test]
fn reserved_function_names_are_rejected() {
    for name in ["asm", "concept", "def", "lambda"] {
        let mut p = sample();
        if let Interface::Function { name: function, .. } = &mut p.interface {
            *function = name.into();
        }
        assert!(p.validate().is_err(), "{name}");
    }
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
            "#include <bits/stdc++.h>\nusing namespace std;\nbool between(int value,int left,int right){return value>=min(left,right)&&value<=max(left,right);}",
            "def between(value,left,right): return min(left,right)<=value<=max(left,right)",
        ),
        (
            "#include <bits/stdc++.h>\nusing namespace std;\nvector<int> quietPeaks(vector<int> values){vector<int> r;for(int i=0;i<(int)values.size();i++)if((i==0||values[i]>values[i-1])&&(i+1==(int)values.size()||values[i]>values[i+1]))r.push_back(i);return r;}",
            "def quietPeaks(values): return [i for i,v in enumerate(values) if (i==0 or v>values[i-1]) and (i+1==len(values) or v>values[i+1])]",
        ),
        (
            "#include <bits/stdc++.h>\nusing namespace std;\nvector<vector<int>> foldRows(vector<vector<int>> rows){reverse(rows.begin(),rows.end());for(auto &r:rows)reverse(r.begin(),r.end());return rows;}",
            "def foldRows(rows): return [r[::-1] for r in rows[::-1]]",
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
            Language::Cpp
                .wrapper_interface(&p.interface, sources[i].0)
                .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("{i}.py")),
            Language::Python
                .wrapper_interface(&p.interface, sources[i].1)
                .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("{i}.json")),
            serde_json::to_vec(&p.tests).unwrap(),
        )
        .unwrap();
    }
}
