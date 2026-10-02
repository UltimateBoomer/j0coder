use super::{Execution, LanguageImplementation};
use crate::contract::Type;

pub(super) struct Java;
impl LanguageImplementation for Java {
    fn descriptor(&self) -> crate::contract::LanguageDescriptor {
        crate::contract::LanguageDescriptor {
            id: crate::contract::Language::Java,
            label: "Java".into(),
            editor_label: "Java 21".into(),
            monaco_language: "java".into(),
            file_extension: ".java".into(),
            line_comment: "//".into(),
            editor_filename: "Solution.java".into(),
            editor_uri: self.editor_uri().into(),
        }
    }

    fn identifier(&self) -> &'static str {
        "java"
    }
    fn type_spelling(&self, ty: &Type) -> String {
        match ty {
            Type::Void => "void".into(),
            Type::Int => "java.lang.Integer".into(),
            Type::Int64 => "java.lang.Long".into(),
            Type::Float => "java.lang.Double".into(),
            Type::Bool => "java.lang.Boolean".into(),
            Type::String => "java.lang.String".into(),
            Type::Array(t) => format!("java.util.List<{}>", self.type_spelling(t)),
            Type::Nullable(t) => self.type_spelling(t),
            Type::Named(n) => n.clone(),
        }
    }
    fn execution(&self) -> Execution {
        Execution {
            source_filename: "solution.java",
            compilation_required: true,
            compiled_artifact_filename: "program.jar.b64",
        }
    }
    fn editor_uri(&self) -> &'static str {
        "file:///workspace/Solution.java"
    }
    fn lsp_command(&self) -> &'static [&'static str] {
        &[
            "/opt/jdtls/bin/jdtls",
            "-configuration",
            "/workspace/.cache/jdtls-config",
            "-data",
            "/workspace/.cache/jdtls",
        ]
    }
    fn reserved_identifiers(&self) -> &'static [&'static str] {
        &[
            "class",
            "interface",
            "enum",
            "record",
            "public",
            "private",
            "protected",
            "static",
            "void",
            "int",
            "long",
            "double",
            "float",
            "boolean",
            "char",
            "byte",
            "short",
            "new",
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
            "switch",
            "case",
            "default",
            "break",
            "continue",
            "try",
            "catch",
            "finally",
            "throw",
            "throws",
            "import",
            "package",
            "extends",
            "implements",
            "abstract",
            "final",
            "native",
            "synchronized",
            "transient",
            "volatile",
            "strictfp",
            "assert",
            "instanceof",
            "var",
            "yield",
            "sealed",
            "permits",
            "Solution",
            "JudgeMain",
        ]
    }
}
