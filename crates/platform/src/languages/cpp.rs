use super::{Execution, LanguageImplementation};
use crate::contract::Type;

pub(super) struct Cpp;

impl LanguageImplementation for Cpp {
    fn descriptor(&self) -> crate::contract::LanguageDescriptor {
        crate::contract::LanguageDescriptor {
            id: crate::contract::Language::Cpp,
            label: "C++".into(),
            editor_label: "C++20".into(),
            monaco_language: "cpp".into(),
            file_extension: ".cpp".into(),
            line_comment: "//".into(),
            editor_filename: "solution.cpp".into(),
            editor_uri: self.editor_uri().into(),
        }
    }

    fn identifier(&self) -> &'static str {
        "cpp"
    }

    fn type_spelling(&self, ty: &Type) -> String {
        match ty {
            Type::Void => "void".into(),
            Type::Int => "int".into(),
            Type::Int64 => "long long".into(),
            Type::Float => "double".into(),
            Type::Bool => "bool".into(),
            Type::String => "string".into(),
            Type::Array(inner) => format!("vector<{}>", self.type_spelling(inner)),
            Type::Nullable(inner) => format!("optional<{}>", self.type_spelling(inner)),
            Type::Named(name) => name.clone(),
        }
    }

    fn execution(&self) -> Execution {
        Execution {
            source_filename: "solution.cpp",
            compilation_required: true,
            compiled_artifact_filename: "program.b64",
        }
    }

    fn editor_uri(&self) -> &'static str {
        "file:///workspace/solution.cpp"
    }

    fn lsp_command(&self) -> &'static [&'static str] {
        &[
            "clangd",
            "--background-index=false",
            "--clang-tidy=false",
            "--log=error",
            "--compile-commands-dir=/workspace",
        ]
    }

    fn reserved_identifiers(&self) -> &'static [&'static str] {
        &[
            "class",
            "return",
            "int",
            "bool",
            "auto",
            "void",
            "public",
            "private",
            "Solution",
            "main",
            "template",
            "for",
            "while",
            "if",
            "else",
            "try",
            "catch",
            "delete",
            "new",
            "operator",
            "switch",
            "case",
            "break",
            "continue",
            "const",
            "static",
            "virtual",
            "typename",
            "namespace",
            "using",
            "do",
            "double",
            "float",
            "char",
            "long",
            "short",
            "unsigned",
            "signed",
            "sizeof",
            "this",
            "null",
            "nullptr",
            "true",
            "false",
            "enum",
            "struct",
            "union",
            "extern",
            "volatile",
            "register",
            "inline",
            "friend",
            "protected",
            "throw",
            "typedef",
            "asm",
            "alignas",
            "alignof",
            "compl",
            "bitand",
            "bitor",
            "xor",
            "typeid",
            "concept",
            "requires",
            "constexpr",
            "consteval",
            "constinit",
            "threadlocal",
            "noexcept",
            "decltype",
            "mutable",
            "export",
            "explicit",
            "wchar",
        ]
    }
}
