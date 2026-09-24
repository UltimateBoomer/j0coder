use super::{Execution, LanguageImplementation};
use crate::contract::Type;

pub(super) struct Python;

impl LanguageImplementation for Python {
    fn identifier(&self) -> &'static str {
        "python"
    }

    fn type_spelling(&self, ty: &Type) -> String {
        match ty {
            Type::Void => "None".into(),
            Type::Int => "int".into(),
            Type::Int64 => "int".into(),
            Type::Float => "float".into(),
            Type::Bool => "bool".into(),
            Type::String => "str".into(),
            Type::Array(inner) => format!("list[{}]", self.type_spelling(inner)),
            Type::Nullable(inner) => format!("{} | None", self.type_spelling(inner)),
            Type::Named(name) => name.clone(),
        }
    }

    fn execution(&self) -> Execution {
        Execution {
            source_filename: "solution.py",
            compilation_required: false,
            compiled_artifact_filename: "program.b64",
        }
    }

    fn editor_uri(&self) -> &'static str {
        "file:///workspace/solution.py"
    }

    fn lsp_command(&self) -> &'static [&'static str] {
        &["pyright-langserver", "--stdio"]
    }

    fn reserved_identifiers(&self) -> &'static [&'static str] {
        &[
            "class", "return", "def", "int", "bool", "str", "self", "Solution", "for", "while",
            "if", "else", "try", "lambda", "import", "from", "pass", "True", "False", "None",
            "break", "continue", "async", "await", "yield", "raise", "with", "as", "in", "is",
            "and", "or", "not", "global", "nonlocal", "assert", "finally", "except", "match",
            "del",
        ]
    }
}
