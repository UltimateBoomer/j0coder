use super::{Execution, LanguageImplementation};
use crate::contract::{Signature, Type};

const STARTER_TEMPLATE: &str = include_str!("../../templates/starter.cpp.j2");

pub(super) struct Cpp;

impl LanguageImplementation for Cpp {
    fn identifier(&self) -> &'static str {
        "cpp"
    }

    fn type_spelling(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "int32_t".into(),
            Type::Bool => "bool".into(),
            Type::String => "std::string".into(),
            Type::Array(inner) => format!("std::vector<{}>", self.type_spelling(inner)),
        }
    }

    fn starter_template(&self) -> &'static str {
        STARTER_TEMPLATE
    }

    fn wrapper(&self, signature: &Signature, source: &str) -> String {
        let args = signature
            .params
            .iter()
            .enumerate()
            .map(|(i, parameter)| format!("a.at({i}).get<{}>()", self.type_spelling(&parameter.ty)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "#include <nlohmann/json.hpp>\n#include <fstream>\n#include <iostream>\n{}\nint main() {{ auto a=nlohmann::json::parse(std::cin); auto r=Solution().{}({}); std::ofstream f(\"/work/result\"); f << nlohmann::json(r).dump(); }}\n",
            source, signature.method, args
        )
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
            "return"c++,
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
