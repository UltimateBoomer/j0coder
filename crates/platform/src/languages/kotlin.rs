use super::{Execution, LanguageImplementation};
use crate::contract::Type;

pub(super) struct Kotlin;
impl LanguageImplementation for Kotlin {
    fn identifier(&self) -> &'static str {
        "kotlin"
    }
    fn type_spelling(&self, ty: &Type) -> String {
        match ty {
            Type::Void => "kotlin.Unit".into(),
            Type::Int => "kotlin.Int".into(),
            Type::Int64 => "kotlin.Long".into(),
            Type::Float => "kotlin.Double".into(),
            Type::Bool => "kotlin.Boolean".into(),
            Type::String => "kotlin.String".into(),
            Type::Array(t) => format!("kotlin.collections.MutableList<{}>", self.type_spelling(t)),
            Type::Nullable(t) => format!("{}?", self.type_spelling(t)),
            Type::Named(n) => n.clone(),
        }
    }
    fn execution(&self) -> Execution {
        Execution {
            source_filename: "solution.kt",
            compilation_required: true,
            compiled_artifact_filename: "program.jar.b64",
        }
    }
    fn editor_uri(&self) -> &'static str {
        "file:///workspace/solution.kt"
    }
    fn lsp_command(&self) -> &'static [&'static str] {
        &[
            "/usr/local/bin/locoder-kotlin-lsp",
            "--stdio",
            "--system-path=/workspace/.cache/kotlin-lsp",
            "--data-sharing=none",
            "--region=americas",
        ]
    }
    fn reserved_identifiers(&self) -> &'static [&'static str] {
        &[
            "class",
            "interface",
            "object",
            "fun",
            "var",
            "return",
            "this",
            "super",
            "null",
            "true",
            "false",
            "if",
            "else",
            "for",
            "while",
            "do",
            "when",
            "is",
            "in",
            "as",
            "try",
            "catch",
            "finally",
            "throw",
            "import",
            "package",
            "typealias",
            "typeof",
            "break",
            "continue",
            "by",
            "constructor",
            "init",
            "companion",
            "open",
            "override",
            "private",
            "public",
            "protected",
            "internal",
            "lateinit",
            "const",
            "data",
            "sealed",
            "inline",
            "noinline",
            "crossinline",
            "reified",
            "suspend",
            "tailrec",
            "out",
            "infix",
            "operator",
            "field",
            "property",
            "where",
            "Solution",
            "JudgeMain",
        ]
    }
}
