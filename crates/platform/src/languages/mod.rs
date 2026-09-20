mod cpp;
mod python;

use self::{cpp::Cpp, python::Python};
use crate::contract::{Language, Signature, Type};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Execution {
    pub source_filename: &'static str,
    pub compilation_required: bool,
    pub compiled_artifact_filename: &'static str,
}

trait LanguageImplementation {
    fn identifier(&self) -> &'static str;
    fn type_spelling(&self, ty: &Type) -> String;
    fn starter_template(&self) -> &'static str;
    fn wrapper(&self, signature: &Signature, source: &str) -> String;
    fn execution(&self) -> Execution;
    fn editor_uri(&self) -> &'static str;
    fn lsp_command(&self) -> &'static [&'static str];
    fn reserved_identifiers(&self) -> &'static [&'static str];
}

static CPP: Cpp = Cpp;
static PYTHON: Python = Python;

#[derive(Serialize)]
struct ParameterContext {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Serialize)]
struct StarterContext {
    method: String,
    return_type: String,
    parameters: Vec<ParameterContext>,
}

impl Language {
    fn implementation(self) -> &'static dyn LanguageImplementation {
        match self {
            Self::Cpp => &CPP,
            Self::Python => &PYTHON,
        }
    }

    pub fn identifier(self) -> &'static str {
        self.implementation().identifier()
    }

    pub fn starter(self, signature: &Signature) -> Result<String> {
        let implementation = self.implementation();
        let context = StarterContext {
            method: signature.method.clone(),
            return_type: implementation.type_spelling(&signature.returns),
            parameters: signature
                .params
                .iter()
                .map(|parameter| ParameterContext {
                    name: parameter.name.clone(),
                    ty: implementation.type_spelling(&parameter.ty),
                })
                .collect(),
        };
        let mut environment = minijinja::Environment::new();
        environment.set_keep_trailing_newline(true);
        environment.add_template("starter", implementation.starter_template())?;
        Ok(environment
            .get_template("starter")?
            .render(serde_json::to_value(context)?)?)
    }

    pub fn wrapper(self, signature: &Signature, source: &str) -> String {
        self.implementation().wrapper(signature, source)
    }

    pub fn execution(self) -> Execution {
        self.implementation().execution()
    }

    pub fn editor_uri(self) -> &'static str {
        self.implementation().editor_uri()
    }

    pub fn lsp_command(self) -> &'static [&'static str] {
        self.implementation().lsp_command()
    }

    pub(crate) fn is_reserved(self, identifier: &str) -> bool {
        self.implementation()
            .reserved_identifiers()
            .contains(&identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementations_expose_language_behavior() {
        assert_eq!(Language::Cpp.identifier(), "cpp");
        assert_eq!(Language::Python.identifier(), "python");
        assert_eq!(
            Language::Cpp
                .implementation()
                .type_spelling(&Type::Array(Box::new(Type::Int))),
            "std::vector<int32_t>"
        );
        assert_eq!(
            Language::Python
                .implementation()
                .type_spelling(&Type::Array(Box::new(Type::Int))),
            "list[int]"
        );
        assert!(
            Language::Cpp
                .implementation()
                .starter_template()
                .contains("class Solution")
        );
        assert!(
            Language::Python
                .implementation()
                .starter_template()
                .contains("class Solution")
        );
        assert_eq!(
            Language::Cpp.execution(),
            Execution {
                source_filename: "solution.cpp",
                compilation_required: true,
                compiled_artifact_filename: "program.b64"
            }
        );
        assert_eq!(
            Language::Python.execution(),
            Execution {
                source_filename: "solution.py",
                compilation_required: false,
                compiled_artifact_filename: "program.b64"
            }
        );
        assert_eq!(Language::Cpp.editor_uri(), "file:///workspace/solution.cpp");
        assert_eq!(
            Language::Python.lsp_command(),
            ["pyright-langserver", "--stdio"]
        );
    }
}
