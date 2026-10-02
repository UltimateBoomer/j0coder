//! Deterministic contracts generated from the types used at runtime.
use crate::contract::*;
use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Map, Value};

pub fn authoring_schema() -> Value {
    serde_json::to_value(
        SchemaSettings::draft07()
            .for_deserialize()
            .into_generator()
            .into_root_schema_for::<AuthoringProblem>(),
    )
    .expect("schema serialization")
}

fn component<T: JsonSchema>(out: &mut Map<String, Value>, name: &str, response: bool) {
    let settings = SchemaSettings::draft2020_12();
    let settings = if response {
        settings.for_serialize()
    } else {
        settings.for_deserialize()
    };
    let mut root =
        serde_json::to_value(settings.into_generator().into_root_schema_for::<T>()).unwrap();
    let suffix = if response { "" } else { "Input" };
    let definitions = root
        .as_object_mut()
        .unwrap()
        .remove("$defs")
        .unwrap_or_default();
    fn refs(value: &mut Value, suffix: &str) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(reference)) = map.get_mut("$ref")
                    && let Some(name) = reference.strip_prefix("#/$defs/")
                {
                    *reference = format!("#/components/schemas/{name}{suffix}");
                }
                for value in map.values_mut() {
                    refs(value, suffix);
                }
            }
            Value::Array(values) => {
                for value in values {
                    refs(value, suffix);
                }
            }
            _ => {}
        }
    }
    root.as_object_mut().unwrap().remove("$schema");
    refs(&mut root, suffix);
    if let Value::Object(definitions) = definitions {
        for (name, mut definition) in definitions {
            refs(&mut definition, suffix);
            out.insert(format!("{name}{suffix}"), definition);
        }
    }
    out.insert(format!("{name}{suffix}"), root);
}

pub fn api_components() -> Value {
    let mut out = Map::new();
    component::<Problem>(&mut out, "Problem", true);
    component::<Problem>(&mut out, "Problem", false);
    component::<ProblemDetail>(&mut out, "ProblemDetail", true);
    component::<ProblemSummary>(&mut out, "ProblemSummary", true);
    component::<Capabilities>(&mut out, "Capabilities", true);
    component::<Language>(&mut out, "Language", true);
    component::<Case>(&mut out, "Case", false);
    component::<CatalogDraft>(&mut out, "CatalogDraft", true);
    component::<CreatedProblem>(&mut out, "CreatedProblem", true);
    component::<PublishedProblem>(&mut out, "PublishedProblem", true);
    component::<ValidatedDefinition>(&mut out, "ValidatedDefinition", true);
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use strum::IntoEnumIterator;

    fn check_refs(value: &Value, root: &Value) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(reference)) = map.get("$ref") {
                    assert!(reference.starts_with('#'), "external reference {reference}");
                    assert!(
                        root.pointer(&reference[1..]).is_some(),
                        "unresolved {reference}"
                    );
                }
                for value in map.values() {
                    check_refs(value, root);
                }
            }
            Value::Array(values) => {
                for value in values {
                    check_refs(value, root);
                }
            }
            _ => {}
        }
    }
    #[test]
    fn generation_is_deterministic_and_recursive_references_resolve() {
        let api = api_components();
        assert_eq!(api, api_components());
        let root = json!({"components":{"schemas":api}});
        check_refs(&root, &root);
        let authoring = authoring_schema();
        assert_eq!(authoring, authoring_schema());
        check_refs(&authoring, &authoring);
        assert_eq!(
            authoring["$schema"],
            "http://json-schema.org/draft-07/schema#"
        );
    }
    #[test]
    fn serde_defaults_omissions_and_nulls_have_separate_contracts() {
        let schemas = api_components();
        let input = schemas["TypeDefinitionInput"]["required"]
            .as_array()
            .unwrap();
        let output = schemas["TypeDefinition"]["required"].as_array().unwrap();
        assert!(!input.contains(&json!("codec")));
        assert!(output.contains(&json!("codec")));
        let definition: TypeDefinition = serde_json::from_value(json!({"name":"Node"})).unwrap();
        assert_eq!(
            serde_json::to_value(definition).unwrap(),
            json!({"name":"Node","codec":"record","fields":[]})
        );
        let parameter: Parameter =
            serde_json::from_value(json!({"name":"value","ty":"int","constraints":null})).unwrap();
        assert_eq!(
            serde_json::to_value(parameter).unwrap(),
            json!({"name":"value","ty":"int"})
        );
        assert!(
            !schemas["Parameter"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("constraints"))
        );
        let case: Case = serde_json::from_value(json!({"args":[1],"expected":null})).unwrap();
        assert!(case.expected.is_none()); // Existing API deserialization semantics.
        let serialized = serde_json::to_value(case).unwrap();
        assert!(serialized["expected"].is_null());
        assert!(
            schemas["Case"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("expected"))
        );
        assert!(
            !schemas["CaseInput"]["required"]
                .as_array()
                .is_some_and(|required| required.contains(&json!("expected")))
        );
    }
    #[test]
    fn descriptors_languages_and_browser_fixture_agree() {
        let descriptors = Language::iter()
            .map(Language::descriptor)
            .collect::<Vec<_>>();
        let fixture = include_str!("../../../tests/browser/language-fixtures.ts");
        let fixture = fixture
            .strip_prefix("export const languageDescriptors=")
            .unwrap()
            .trim()
            .trim_end_matches(';');
        assert_eq!(
            serde_json::to_value(&descriptors).unwrap(),
            serde_json::from_str::<Value>(fixture).unwrap()
        );
        assert_eq!(
            serde_json::to_value(Language::iter().collect::<Vec<_>>()).unwrap(),
            api_components()["Language"]["enum"]
        );
        for descriptor in descriptors {
            assert_eq!(
                descriptor.editor_uri,
                format!("file:///workspace/{}", descriptor.editor_filename)
            );
            assert_eq!(descriptor.editor_uri, descriptor.id.editor_uri());
        }
        assert_eq!(Language::Python.descriptor().line_comment, "#");
        assert_eq!(Language::Java.descriptor().editor_filename, "Solution.java");
        assert_eq!(Language::Java.execution().source_filename, "solution.java");
    }
}
