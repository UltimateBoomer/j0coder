use practice::contract::*;
use serde_json::json;

#[test]
fn legacy_difficulty_maps_without_rewrite() {
    let p: Problem = serde_json::from_str(include_str!("../../../samples/1.json")).unwrap();
    assert_eq!(p.schema, 1);
    assert_eq!(p.effective_score(), 2);
    p.validate().unwrap()
}
#[test]
fn v2_float_and_nested_multiset() {
    let policy = Comparison {
        array: ArrayComparison::Multiset,
        absolute_tolerance: 0.001,
        relative_tolerance: 0.01,
        items: None,
    };
    let ty = Type::Array(Box::new(Type::Float));
    assert!(policy.matches(&ty, &json!([1.0, 2.0, 2.0]), &json!([2.001, 1.0, 2.0])));
    assert!(!policy.matches(&ty, &json!([1.0, 2.0, 2.0]), &json!([1.0, 2.0, 3.0])))
}
#[test]
fn record_rejects_unknown_fields() {
    let defs = vec![TypeDefinition {
        name: "Point".into(),
        codec: Codec::Record,
        fields: vec![Field {
            name: "x".into(),
            ty: Type::Int64,
        }],
    }];
    let ty = Type::Named("Point".into());
    assert!(ty.valid_with(&json!({"x":i64::MAX}), &defs));
    assert!(!ty.valid_with(&json!({"x":1,"y":2}), &defs))
}
#[test]
fn graph_rejects_duplicate_and_dangling_ids() {
    let d = TypeDefinition {
        name: "Node".into(),
        codec: Codec::ObjectGraph,
        fields: vec![
            Field {
                name: "value".into(),
                ty: Type::Int,
            },
            Field {
                name: "next".into(),
                ty: Type::Nullable(Box::new(Type::Named("Node".into()))),
            },
        ],
    };
    let t = Type::Named("Node".into());
    assert!(t.valid_with(
        &json!({"roots":["a"],"nodes":[{"id":"a","value":1,"next":"a"}]}),
        std::slice::from_ref(&d)
    ));
    assert!(!t.valid_with(
        &json!({"roots":["z"],"nodes":[{"id":"a","value":1,"next":null}]}),
        &[d]
    ))
}
