use locoder::{
    contract::*,
    sandbox::{Podman, judge},
};
use serde_json::json;
use uuid::Uuid;
fn job(language: Language) -> Job {
    Job {
        attempt_base: 0,
        generation: Uuid::new_v4(),
        schema: 3,
        id: Uuid::new_v4(),
        language,
        version: Uuid::new_v4(),
        interface: Interface::Function {
            name: "echo".into(),
            params: vec![Parameter {
                name: "value".into(),
                ty: Type::Int,
                constraints: None,
            }],
            returns: Type::Int,
        },
        limits: Limits {
            time_ms: 500,
            memory_mib: 64,
            output_bytes: 4096,
        },
        mode: "submit".into(),
        type_definitions: vec![],
        comparison: Comparison::default(),
    }
}
#[tokio::test]
#[ignore = "requires working Podman + gVisor; never run with a fallback runtime"]
async fn sandbox_verdicts_isolation_and_parallel_languages() -> anyhow::Result<()> {
    Podman::new().preflight().await?;
    let cases = vec![
        Case {
            args: Some(json!([42])),
            expected: Some(json!(42)),
            hidden: false,
            constructor_args: None,
            operations: None,
        },
        Case {
            args: Some(json!([-2147483648i64])),
            expected: Some(json!(-2147483648i64)),
            hidden: true,
            constructor_args: None,
            operations: None,
        },
    ];
    let py = job(Language::Python);
    let cpp = job(Language::Cpp);
    let (a, b) = tokio::join!(
        judge(&py, "def echo(value):\n  return value", &cases),
        judge(&cpp, "int echo(int value){return value;}", &cases)
    );
    assert_eq!(a?.verdict, Verdict::Accepted);
    assert_eq!(b?.verdict, Verdict::Accepted);
    let wrong = judge(
        &py,
        "def echo(value):\n \n  print('private runtime log',value)\n  return 0",
        &cases,
    )
    .await?;
    assert_eq!(wrong.verdict, Verdict::WrongAnswer);
    assert_eq!(wrong.cases.len(), 2);
    assert!(!wrong.cases[0].hidden);
    assert!(wrong.cases[1].hidden);
    assert!(wrong.cases[1].output.is_none());
    assert!(wrong.cases[1].log.is_empty());
    assert!(!serde_json::to_string(&wrong)?.contains("-2147483648"));
    let compilation = judge(&cpp, "invalid syntax", &cases).await?;
    assert_eq!(compilation.verdict, Verdict::CompilationError);
    assert_eq!(compilation.cases.len(), cases.len());
    assert!(compilation.cases.iter().all(|case| case.verdict.is_none()));
    assert_eq!(
        judge(&py, "def echo(value):\n \n  while True: pass", &cases)
            .await?
            .verdict,
        Verdict::TimeLimit
    );
    assert_eq!(
        judge(
            &py,
            "def echo(value):\n \n  print('x'*100000)\n  return value",
            &cases
        )
        .await?
        .verdict,
        Verdict::OutputLimit
    );
    assert_eq!(
        judge(&py, "def echo(value):\n  raise Exception('failed')", &cases)
            .await?
            .verdict,
        Verdict::RuntimeError
    );
    let memory = judge(
        &py,
        "def echo(value):\n \n  data=[]\n  while True: data.append(bytearray(8*1024*1024))",
        &cases,
    )
    .await?;
    assert_eq!(memory.verdict, Verdict::MemoryLimit);
    let isolated = "import os, socket\ndef echo(value):\n \n  assert not os.path.exists('/run/podman/podman.sock')\n  assert not os.path.exists('/var/run/docker.sock')\n  assert 'DATABASE_URL' not in os.environ\n  assert not os.path.exists('/fixtures')\n  assert set(os.listdir('/input')) == {'solution.py'}\n  try:\n   socket.create_connection(('1.1.1.1',80),timeout=0.05)\n   return 0\n  except OSError: return value";
    assert_eq!(
        judge(&py, isolated, &cases).await?.verdict,
        Verdict::Accepted
    );
    Ok(())
}
#[tokio::test]
async fn refuses_non_gvisor() {
    let b = Podman {
        image: "unused".into(),
        runtime: "crun".into(),
    };
    assert!(b.preflight().await.is_err());
}

#[tokio::test]
#[ignore = "requires the JVM toolchain image and working Podman + gVisor"]
async fn jvm_verdicts_and_hidden_case_redaction() -> anyhow::Result<()> {
    Podman::new().preflight().await?;
    let cases = vec![
        Case {
            args: Some(json!([42])),
            expected: Some(json!(42)),
            hidden: false,
            constructor_args: None,
            operations: None,
        },
        Case {
            args: Some(json!([-2147483648i64])),
            expected: Some(json!(-2147483648i64)),
            hidden: true,
            constructor_args: None,
            operations: None,
        },
    ];
    for (language, accepted, wrong) in [
        (
            Language::Java,
            "class Solution { public Integer echo(Integer value) { return value; } }",
            "class Solution { public Integer echo(Integer value) { return 0; } }",
        ),
        (
            Language::Kotlin,
            "fun echo(value: Int): Int = value",
            "fun echo(value: Int): Int = 0",
        ),
    ] {
        let mut job = job(language);
        job.limits.time_ms = 2000;
        assert_eq!(
            judge(&job, accepted, &cases).await?.verdict,
            Verdict::Accepted
        );
        let bad = judge(&job, wrong, &cases).await?;
        assert_eq!(bad.verdict, Verdict::WrongAnswer);
        assert!(bad.cases[1].hidden);
        assert!(bad.cases[1].output.is_none());
        assert!(bad.cases[1].log.is_empty());
        assert!(!serde_json::to_string(&bad)?.contains("-2147483648"));
        let compilation = judge(&job, "invalid syntax", &cases).await?;
        assert_eq!(compilation.verdict, Verdict::CompilationError);
        let invalid_output = if language == Language::Java {
            "class Solution { public Integer echo(Integer value) { return null; } }"
        } else {
            "fun echo(value: Int): Int? = null"
        };
        assert_eq!(
            judge(&job, invalid_output, &cases).await?.verdict,
            Verdict::RuntimeError
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires the JVM toolchain image and working Podman + gVisor"]
async fn generated_jvm_starters_compile_and_judge_named_values() -> anyhow::Result<()> {
    let definition = TypeDefinition {
        name: "Sample".into(),
        codec: Codec::Record,
        fields: vec![
            Field {
                name: "value".into(),
                ty: Type::Int64,
            },
            Field {
                name: "val".into(),
                ty: Type::Int,
            },
            Field {
                name: "items".into(),
                ty: Type::Array(Box::new(Type::Nullable(Box::new(Type::Int)))),
            },
            Field {
                name: "matrix".into(),
                ty: Type::Array(Box::new(Type::Array(Box::new(Type::Nullable(Box::new(
                    Type::Int64,
                )))))),
            },
        ],
    };
    let mut job = job(Language::Java);
    job.interface = Interface::Function {
        name: "echo".into(),
        params: vec![Parameter {
            name: "value".into(),
            ty: Type::Named("Sample".into()),
            constraints: None,
        }],
        returns: Type::Named("Sample".into()),
    };
    job.type_definitions = vec![definition];
    job.limits.time_ms = 2000;
    job.limits.memory_mib = 256;
    let value = json!({"value":9223372036854775807i64,"val":7,"items":[1,null,-1],"matrix":[[null,-9223372036854775808i64,9223372036854775807i64],[]]});
    let cases = [Case {
        args: Some(json!([value.clone()])),
        expected: Some(value),
        hidden: false,
        constructor_args: None,
        operations: None,
    }];
    for language in [Language::Java, Language::Kotlin] {
        job.language = language;
        let starter = language.starter_with_definitions(&job.interface, &job.type_definitions)?;
        let source = if language == Language::Java {
            starter.replace(
                "throw new UnsupportedOperationException();",
                "return value;",
            )
        } else {
            starter.replace("TODO(\"Implement\")", "return value")
        };
        assert_eq!(
            judge(&job, &source, &cases).await?.verdict,
            Verdict::Accepted
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires the JVM toolchain image and working Podman + gVisor"]
async fn generated_jvm_stateful_starters_judge_short_names_and_collisions() -> anyhow::Result<()> {
    let definitions: Vec<TypeDefinition> = serde_json::from_value(json!([
        {"name":"Sample","fields":[{"name":"label","ty":"string"},{"name":"flag","ty":"bool"},{"name":"fraction","ty":"float"},{"name":"items","ty":{"array":{"nullable":"int64"}}}]},
        {"name":"Link","codec":"singly_linked_list","fields":[{"name":"val","ty":"int"},{"name":"next","ty":{"nullable":{"named":"Link"}}}]},
        {"name":"Tree","codec":"binary_tree","fields":[{"name":"val","ty":"int"},{"name":"left","ty":{"nullable":{"named":"Tree"}}},{"name":"right","ty":{"nullable":{"named":"Tree"}}}]},
        {"name":"Nary","codec":"nary_tree","fields":[{"name":"val","ty":"int"},{"name":"children","ty":{"array":{"named":"Nary"}}}]},
        {"name":"Graph","codec":"object_graph","fields":[{"name":"value","ty":"int"},{"name":"next","ty":{"nullable":{"named":"Graph"}}}]}
    ]))?;
    let values = [
        json!({"label":"sample","flag":true,"fraction":1.25,"items":[null,-9223372036854775808i64,9223372036854775807i64]}),
        json!([1, 2, 3]),
        json!([1, null, 2, 3]),
        json!({"value":1,"children":[{"value":2,"children":[]}]}),
        json!({"roots":["a","a"],"nodes":[{"id":"a","value":1,"next":"b"},{"id":"b","value":2,"next":"a"}]}),
        json!([[1, null, -1], []]),
    ];
    for language in [Language::Java, Language::Kotlin] {
        for collisions in [false, true] {
            let class_name = match (language, collisions) {
                (Language::Java, true) => "List",
                (Language::Kotlin, true) => "MutableList",
                _ => "Store",
            };
            let interface: Interface = serde_json::from_value(json!({
                "kind":"data_structure","name":class_name,
                "constructor":{"params":[{"name":"labels","ty":{"array":"string"}}]},
                "methods":[
                    {"name":"record","params":[{"name":"value","ty":{"named":"Sample"}}],"returns":{"named":"Sample"}},
                    {"name":"list","params":[{"name":"value","ty":{"named":"Link"}}],"returns":{"named":"Link"}},
                    {"name":"tree","params":[{"name":"value","ty":{"named":"Tree"}}],"returns":{"named":"Tree"}},
                    {"name":"nary","params":[{"name":"value","ty":{"named":"Nary"}}],"returns":{"named":"Nary"}},
                    {"name":"graph","params":[{"name":"value","ty":{"named":"Graph"}}],"returns":{"named":"Graph"}},
                    {"name":"matrix","params":[{"name":"value","ty":{"array":{"array":{"nullable":"int"}}}}],"returns":{"array":{"array":{"nullable":"int"}}}},
                    {"name":"clear","params":[],"returns":"void"}
                ]
            }))?;
            let mut job = job(language);
            job.interface = interface;
            job.type_definitions = definitions.clone();
            job.limits.time_ms = 2000;
            job.limits.memory_mib = 256;
            if collisions {
                let names: &[&str] = if language == Language::Java {
                    &[
                        "Integer",
                        "Long",
                        "Double",
                        "Boolean",
                        "String",
                        "ArrayList",
                        "GraphNode",
                    ]
                } else {
                    &[
                        "Int",
                        "Long",
                        "Double",
                        "Boolean",
                        "String",
                        "Unit",
                        "mutableListOf",
                        "GraphNode",
                    ]
                };
                job.type_definitions
                    .extend(names.iter().map(|name| TypeDefinition {
                        name: (*name).into(),
                        codec: Codec::Record,
                        fields: vec![],
                    }));
            }
            let starter =
                language.starter_with_definitions(&job.interface, &job.type_definitions)?;
            let source = if language == Language::Java {
                starter.replace(
                    "throw new UnsupportedOperationException();",
                    "return value;",
                )
            } else {
                starter.replace("TODO(\"Implement\")", "return value")
            };
            let mut operations = Vec::new();
            for (method, value) in ["record", "list", "tree", "nary", "graph", "matrix"]
                .iter()
                .zip(&values)
            {
                operations.push(json!({"method":method,"args":[value],"expected":value}));
            }
            operations.push(json!({"method":"clear","args":[],"expected":null}));
            let cases = [Case {
                constructor_args: Some(json!([["initial"]])),
                operations: Some(serde_json::from_value(json!(operations))?),
                args: None,
                expected: None,
                hidden: false,
            }];
            let result = judge(&job, &source, &cases).await?;
            assert_eq!(
                result.verdict,
                Verdict::Accepted,
                "{language:?}, collisions={collisions}: {result:?}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires the JVM toolchain image and working Podman + gVisor"]
async fn generated_kotlin_starters_accept_empty_codec_values() -> anyhow::Result<()> {
    let definitions: Vec<TypeDefinition> = serde_json::from_value(json!([
        {"name":"Link","codec":"singly_linked_list","fields":[{"name":"val","ty":"int"},{"name":"next","ty":{"nullable":{"named":"Link"}}}]},
        {"name":"Tree","codec":"binary_tree","fields":[{"name":"val","ty":"int"},{"name":"left","ty":{"nullable":{"named":"Tree"}}},{"name":"right","ty":{"nullable":{"named":"Tree"}}}]},
        {"name":"Nary","codec":"nary_tree","fields":[{"name":"val","ty":"int"},{"name":"children","ty":{"array":{"named":"Nary"}}}]}
    ]))?;
    for (name, value) in [
        ("Link", json!([])),
        ("Tree", json!([])),
        ("Nary", json!(null)),
    ] {
        let mut job = job(Language::Kotlin);
        job.type_definitions = definitions.clone();
        job.limits.time_ms = 2000;
        job.interface = Interface::Function {
            name: "echo".into(),
            params: vec![Parameter {
                name: "value".into(),
                ty: Type::Named(name.into()),
                constraints: None,
            }],
            returns: Type::Named(name.into()),
        };
        let source = Language::Kotlin
            .starter_with_definitions(&job.interface, &job.type_definitions)?
            .replace("TODO(\"Implement\")", "return value");
        let cases = [Case {
            args: Some(json!([value.clone()])),
            expected: Some(value),
            hidden: false,
            constructor_args: None,
            operations: None,
        }];
        let result = judge(&job, &source, &cases).await?;
        assert_eq!(result.verdict, Verdict::Accepted, "{name}: {result:?}");
    }
    Ok(())
}
