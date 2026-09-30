mod cpp;
mod java;
mod kotlin;
mod python;

use self::{cpp::Cpp, java::Java, kotlin::Kotlin, python::Python};
use crate::contract::{Codec, Interface, Language, Type, TypeDefinition};
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
static JAVA: Java = Java;
static KOTLIN: Kotlin = Kotlin;

#[derive(Serialize)]
struct JvmWrapperContext {
    source_json: String,
    interface_json: String,
    definitions_json: String,
}

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
    models: String,
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
            Self::Java => &JAVA,
            Self::Kotlin => &KOTLIN,
        }
    }

    pub fn identifier(self) -> &'static str {
        self.implementation().identifier()
    }

    pub fn starter_interface(self, interface: &Interface) -> Result<String> {
        self.starter_with_definitions(interface, &[])
    }

    fn source_name(self, name: &str) -> String {
        if self == Language::Kotlin && name == "val" {
            format!("`{name}`")
        } else {
            name.to_owned()
        }
    }

    pub fn starter_with_definitions(
        self,
        interface: &Interface,
        defs: &[TypeDefinition],
    ) -> Result<String> {
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
                    Language::Java => include_str!("../../templates/function_starter.java.j2"),
                    Language::Kotlin => include_str!("../../templates/function_starter.kt.j2"),
                },
                InterfaceContext {
                    source: "",
                    name: self.source_name(name),
                    parameters: params
                        .iter()
                        .map(|p| ParameterContext {
                            name: self.source_name(&p.name),
                            ty: self.type_with_definitions(&p.ty, defs),
                        })
                        .collect(),
                    return_type: self.type_with_definitions(returns, defs),
                    methods: vec![],
                    models: self.models(defs, interface),
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
                    Language::Java => {
                        include_str!("../../templates/data_structure_starter.java.j2")
                    }
                    Language::Kotlin => {
                        include_str!("../../templates/data_structure_starter.kt.j2")
                    }
                },
                InterfaceContext {
                    source: "",
                    name: self.source_name(name),
                    parameters: constructor
                        .params
                        .iter()
                        .map(|p| ParameterContext {
                            name: self.source_name(&p.name),
                            ty: self.type_with_definitions(&p.ty, defs),
                        })
                        .collect(),
                    return_type: String::new(),
                    models: self.models(defs, interface),
                    methods: methods
                        .iter()
                        .map(|m| MethodContext {
                            name: self.source_name(&m.name),
                            return_type: self.type_with_definitions(&m.returns, defs),
                            parameters: m
                                .params
                                .iter()
                                .map(|p| ParameterContext {
                                    name: self.source_name(&p.name),
                                    ty: self.type_with_definitions(&p.ty, defs),
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
        self.wrapper_with_definitions(interface, &[], source)
    }

    pub fn wrapper_with_definitions(
        self,
        interface: &Interface,
        defs: &[TypeDefinition],
        source: &str,
    ) -> Result<String> {
        if matches!(self, Language::Java | Language::Kotlin) {
            let template = if self == Language::Java {
                include_str!("../../templates/wrapper.java.j2")
            } else {
                include_str!("../../templates/wrapper.kt.j2")
            };
            return render_template(
                "jvm_wrapper",
                template,
                JvmWrapperContext {
                    source_json: serde_json::to_string(source)?,
                    interface_json: serde_json::to_string(interface)?,
                    definitions_json: serde_json::to_string(defs)?,
                },
            );
        }
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
            _ => unreachable!(),
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
                    models: self.models(defs, interface),
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
                    models: String::new(),
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

    fn type_with_definitions(self, ty: &Type, defs: &[TypeDefinition]) -> String {
        if self != Language::Kotlin {
            return self.implementation().type_spelling(ty);
        }
        match ty {
            Type::Array(t) => format!(
                "kotlin.collections.MutableList<{}>",
                self.type_with_definitions(t, defs)
            ),
            Type::Nullable(t) => {
                let inner = self.type_with_definitions(t, defs);
                format!("{}?", inner.trim_end_matches('?'))
            }
            Type::Named(name)
                if defs.iter().any(|d| {
                    d.name == *name
                        && matches!(
                            d.codec,
                            Codec::SinglyLinkedList | Codec::BinaryTree | Codec::NaryTree
                        )
                }) =>
            {
                format!("{}?", self.source_name(name))
            }
            Type::Named(name) => self.source_name(name),
            _ => self.implementation().type_spelling(ty),
        }
    }

    fn models(self, defs: &[TypeDefinition], interface: &Interface) -> String {
        if !matches!(self, Language::Java | Language::Kotlin) {
            return String::new();
        }
        let mut out = String::new();
        for d in defs {
            let graph = d.codec == Codec::ObjectGraph;
            let class_name = if graph {
                let candidate = format!("{}Node", d.name);
                if defs.iter().any(|d| d.name == candidate)
                    || matches!(interface, Interface::DataStructure { name, .. } if *name == candidate)
                {
                    format!("__JudgeGraphNode_{}", d.name)
                } else {
                    candidate
                }
            } else {
                self.source_name(&d.name)
            };
            let mut fields = String::new();
            if graph {
                if self == Language::Java {
                    fields.push_str("    public java.lang.String __judgeId;\n");
                } else {
                    fields.push_str("    var __judgeId: kotlin.String? = null\n");
                }
            }
            for f in &d.fields {
                let ty = if graph {
                    match &f.ty {
                        Type::Named(n) if n == &d.name => class_name.clone(),
                        Type::Nullable(t) if matches!(&**t, Type::Named(n) if n == &d.name) => {
                            if self == Language::Java {
                                class_name.clone()
                            } else {
                                format!("{class_name}?")
                            }
                        }
                        _ => self.type_with_definitions(&f.ty, defs),
                    }
                } else {
                    self.type_with_definitions(&f.ty, defs)
                };
                if self == Language::Java {
                    fields.push_str(&format!("    public {ty} {};\n", f.name));
                } else {
                    let init = if ty.ends_with('?') {
                        Some("null")
                    } else {
                        match &f.ty {
                            Type::Int | Type::Int64 => Some("0"),
                            Type::Float => Some("0.0"),
                            Type::Bool => Some("false"),
                            Type::Nullable(_) => Some("null"),
                            _ => None,
                        }
                    };
                    let field_name = self.source_name(&f.name);
                    fields.push_str(&match init {
                        Some(v) => format!("    var {field_name}: {ty} = {v}\n"),
                        None => format!("    lateinit var {field_name}: {ty}\n"),
                    });
                }
            }
            if self == Language::Java {
                out.push_str(&format!(
                    "class {class_name} {{\n{fields}    public {class_name}() {{}}\n}}\n"
                ));
                if graph {
                    out.push_str(&format!("class {} {{\n    public java.util.List<{class_name}> roots = new java.util.ArrayList<>();\n    public java.util.List<{class_name}> nodes = new java.util.ArrayList<>();\n}}\n",self.source_name(&d.name)));
                }
            } else {
                out.push_str(&format!("class {class_name} {{\n{fields}}}\n"));
                if graph {
                    out.push_str(&format!("class {} {{\n    var roots: kotlin.collections.MutableList<{class_name}?> = kotlin.collections.mutableListOf()\n    var nodes: kotlin.collections.MutableList<{class_name}> = kotlin.collections.mutableListOf()\n}}\n",self.source_name(&d.name)));
                }
            }
        }
        out
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
    fn jvm_starters_avoid_model_and_keyword_collisions() {
        let defs: Vec<TypeDefinition> = serde_json::from_value(serde_json::json!([
            {"name":"String","fields":[{"name":"value","ty":"string"}]},
            {"name":"Graph","codec":"object_graph","fields":[{"name":"next","ty":{"nullable":{"named":"Graph"}}}]},
            {"name":"GraphNode","fields":[]}
        ])).unwrap();
        let interface = Interface::Function {
            name: "val".into(),
            params: vec![],
            returns: Type::String,
        };
        let java = Language::Java
            .starter_with_definitions(&interface, &defs)
            .unwrap();
        assert!(java.contains("java.lang.String value;"), "{java}");
        assert!(java.contains("class __JudgeGraphNode_Graph"), "{java}");
        assert_eq!(java.matches("class GraphNode ").count(), 1);
        let kotlin = Language::Kotlin
            .starter_with_definitions(&interface, &defs)
            .unwrap();
        assert!(kotlin.contains("fun `val`(): kotlin.String"), "{kotlin}");
        assert!(kotlin.contains("value: kotlin.String"), "{kotlin}");
        assert!(kotlin.contains("class __JudgeGraphNode_Graph"), "{kotlin}");
    }

    #[test]
    fn kotlin_starters_preserve_codec_nullability_in_nested_types() {
        for codec in [Codec::SinglyLinkedList, Codec::BinaryTree, Codec::NaryTree] {
            let defs = vec![TypeDefinition {
                name: "Node".into(),
                codec,
                fields: vec![crate::contract::Field {
                    name: "children".into(),
                    ty: Type::Array(Box::new(Type::Named("Node".into()))),
                }],
            }];
            let interface = Interface::Function {
                name: "echo".into(),
                params: vec![crate::contract::Parameter {
                    name: "node".into(),
                    ty: Type::Named("Node".into()),
                    constraints: None,
                }],
                returns: Type::Nullable(Box::new(Type::Named("Node".into()))),
            };
            let starter = Language::Kotlin
                .starter_with_definitions(&interface, &defs)
                .unwrap();
            assert!(
                starter.contains("fun echo(node: Node?): Node?"),
                "{starter}"
            );
            assert!(starter.contains("MutableList<Node?>"), "{starter}");
            assert!(!starter.contains("??"));
        }
        let defs = vec![TypeDefinition {
            name: "Box".into(),
            codec: Codec::Record,
            fields: vec![],
        }];
        assert_eq!(
            Language::Kotlin.type_with_definitions(&Type::Named("Box".into()), &defs),
            "Box"
        );
    }

    #[test]
    fn implementations_expose_language_behavior() {
        assert_eq!(Language::Cpp.identifier(), "cpp");
        assert_eq!(Language::Python.identifier(), "python");
        assert_eq!(
            Language::Cpp
                .implementation()
                .type_spelling(&Type::Array(Box::new(Type::Int))),
            "vector<int>"
        );
        assert_eq!(
            Language::Cpp.implementation().type_spelling(&Type::Int64),
            "long long"
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
