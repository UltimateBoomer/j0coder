mod cpp;
mod python;

use self::{cpp::Cpp, python::Python};
use crate::contract::{Interface, Language, Type};
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
struct MethodContext {
    name: String,
    return_type: String,
    parameters: Vec<ParameterContext>,
    is_void: bool,
}
#[derive(Serialize)]
struct InterfaceContext<'a> {
    source: &'a str,
    name: String,
    parameters: Vec<ParameterContext>,
    return_type: String,
    methods: Vec<MethodContext>,
}

fn render_template(name: &str, template: &str, context: impl Serialize) -> Result<String> {
    let mut environment = minijinja::Environment::new();
    environment.set_keep_trailing_newline(true);
    environment.add_template(name, template)?;
    Ok(environment
        .get_template(name)?
        .render(serde_json::to_value(context)?)?)
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

    pub fn starter_interface(self, interface: &Interface) -> Result<String> {
        let i = self.implementation();
        match interface {
            Interface::Function {
                name,
                params,
                returns,
            } => render_template(
                "function_starter",
                match self {
                    Language::Cpp => include_str!("../../templates/function_starter.cpp.j2"),
                    Language::Python => include_str!("../../templates/function_starter.py.j2"),
                },
                InterfaceContext {
                    source: "",
                    name: name.clone(),
                    parameters: params
                        .iter()
                        .map(|p| ParameterContext {
                            name: p.name.clone(),
                            ty: i.type_spelling(&p.ty),
                        })
                        .collect(),
                    return_type: i.type_spelling(returns),
                    methods: vec![],
                },
            ),
            Interface::DataStructure {
                name,
                constructor,
                methods,
            } => render_template(
                "data_structure_starter",
                match self {
                    Language::Cpp => include_str!("../../templates/data_structure_starter.cpp.j2"),
                    Language::Python => {
                        include_str!("../../templates/data_structure_starter.py.j2")
                    }
                },
                InterfaceContext {
                    source: "",
                    name: name.clone(),
                    parameters: constructor
                        .params
                        .iter()
                        .map(|p| ParameterContext {
                            name: p.name.clone(),
                            ty: i.type_spelling(&p.ty),
                        })
                        .collect(),
                    return_type: String::new(),
                    methods: methods
                        .iter()
                        .map(|m| MethodContext {
                            name: m.name.clone(),
                            return_type: i.type_spelling(&m.returns),
                            parameters: m
                                .params
                                .iter()
                                .map(|p| ParameterContext {
                                    name: p.name.clone(),
                                    ty: i.type_spelling(&p.ty),
                                })
                                .collect(),
                            is_void: matches!(m.returns, Type::Void),
                        })
                        .collect(),
                },
            ),
        }
    }

    pub fn wrapper_interface(self, interface: &Interface, source: &str) -> Result<String> {
        let i = self.implementation();
        let template = match (self, interface) {
            (Language::Cpp, Interface::Function { .. }) => {
                include_str!("../../templates/function_wrapper.cpp.j2")
            }
            (Language::Python, Interface::Function { .. }) => {
                include_str!("../../templates/function_wrapper.py.j2")
            }
            (Language::Cpp, Interface::DataStructure { .. }) => {
                include_str!("../../templates/data_structure_wrapper.cpp.j2")
            }
            (Language::Python, Interface::DataStructure { .. }) => {
                include_str!("../../templates/data_structure_wrapper.py.j2")
            }
        };
        match interface {
            Interface::Function {
                name,
                params,
                returns,
            } => render_template(
                "interface_wrapper",
                template,
                InterfaceContext {
                    source,
                    name: name.clone(),
                    parameters: params
                        .iter()
                        .map(|p| ParameterContext {
                            name: p.name.clone(),
                            ty: i.type_spelling(&p.ty),
                        })
                        .collect(),
                    return_type: i.type_spelling(returns),
                    methods: vec![],
                },
            ),
            Interface::DataStructure {
                name,
                constructor,
                methods,
            } => render_template(
                "interface_wrapper",
                template,
                InterfaceContext {
                    source,
                    name: name.clone(),
                    parameters: constructor
                        .params
                        .iter()
                        .map(|p| ParameterContext {
                            name: p.name.clone(),
                            ty: i.type_spelling(&p.ty),
                        })
                        .collect(),
                    return_type: String::new(),
                    methods: methods
                        .iter()
                        .map(|m| MethodContext {
                            name: m.name.clone(),
                            return_type: i.type_spelling(&m.returns),
                            parameters: m
                                .params
                                .iter()
                                .map(|p| ParameterContext {
                                    name: p.name.clone(),
                                    ty: i.type_spelling(&p.ty),
                                })
                                .collect(),
                            is_void: matches!(m.returns, Type::Void),
                        })
                        .collect(),
                },
            ),
        }
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
            "vector<int32_t>"
        );
        assert_eq!(
            Language::Python
                .implementation()
                .type_spelling(&Type::Array(Box::new(Type::Int))),
            "list[int]"
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
