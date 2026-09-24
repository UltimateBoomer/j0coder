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
