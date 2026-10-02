mod cpp;
mod java;
mod kotlin;
mod python;

use self::{cpp::Cpp, java::Java, kotlin::Kotlin, python::Python};
use crate::contract::{Codec, Interface, Language, Type, TypeDefinition};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Execution {
    pub source_filename: &'static str,
    pub compilation_required: bool,
    pub compiled_artifact_filename: &'static str,
}

trait LanguageImplementation {
    fn descriptor(&self) -> crate::contract::LanguageDescriptor;
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

// Standard names may be shadowed by a model or the stateful submission class.
// Keep canonical qualified spellings in the language implementations and shorten
// each standard symbol only after all generated declarations are known.
struct StarterTypes<'a> {
    language: Language,
    definitions: &'a [TypeDefinition],
    declared_names: HashSet<String>,
    function_name: Option<&'a str>,
}

fn graph_node_name(
    definition: &TypeDefinition,
    defs: &[TypeDefinition],
    interface: &Interface,
) -> String {
    let candidate = format!("{}Node", definition.name);
    if defs.iter().any(|d| d.name == candidate)
        || matches!(interface, Interface::DataStructure { name, .. } if *name == candidate)
    {
        format!("__JudgeGraphNode_{}", definition.name)
    } else {
        candidate
    }
}

impl<'a> StarterTypes<'a> {
    fn new(
        language: Language,
        definitions: &'a [TypeDefinition],
        interface: &'a Interface,
    ) -> Self {
        let mut declared_names: HashSet<_> = definitions.iter().map(|d| d.name.clone()).collect();
        for d in definitions.iter().filter(|d| d.codec == Codec::ObjectGraph) {
            declared_names.insert(graph_node_name(d, definitions, interface));
        }
        let function_name = match interface {
            Interface::DataStructure { name, .. } => {
                declared_names.insert(name.clone());
                None
            }
            Interface::Function { name, .. } => Some(name.as_str()),
        };
        Self {
            language,
            definitions,
            declared_names,
            function_name,
        }
    }

    fn standard_name<'b>(&self, qualified: &'b str) -> &'b str {
        let short = qualified.rsplit('.').next().unwrap();
        if self.declared_names.contains(short) {
            qualified
        } else {
            short
        }
    }

    fn list_factory(&self) -> &str {
        if self.function_name == Some("mutableListOf") {
            "kotlin.collections.mutableListOf"
        } else {
            self.standard_name("kotlin.collections.mutableListOf")
        }
    }

    fn render(&self, ty: &Type) -> String {
        let language = self.language;
        if !matches!(language, Language::Java | Language::Kotlin) {
            return language.implementation().type_spelling(ty);
        }
        match ty {
            Type::Array(inner) => {
                let list = if language == Language::Java {
                    "java.util.List"
                } else {
                    "kotlin.collections.MutableList"
                };
                format!("{}<{}>", self.standard_name(list), self.render(inner))
            }
            Type::Nullable(inner) => {
                let inner = self.render(inner);
                if language == Language::Kotlin {
                    format!("{}?", inner.trim_end_matches('?'))
                } else {
                    inner
                }
            }
            Type::Named(name) => {
                if language == Language::Kotlin
                    && self.definitions.iter().any(|d| {
                        d.name == *name
                            && matches!(
                                d.codec,
                                Codec::SinglyLinkedList | Codec::BinaryTree | Codec::NaryTree
                            )
                    })
                {
                    format!("{}?", language.source_name(name))
                } else {
                    language.source_name(name)
                }
            }
            _ => self
                .standard_name(&language.implementation().type_spelling(ty))
                .to_owned(),
        }
    }
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
        let types = StarterTypes::new(self, defs, interface);
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
                            ty: types.render(&p.ty),
                        })
                        .collect(),
                    return_type: types.render(returns),
                    methods: vec![],
                    models: self.models(defs, interface, &types),
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
                            ty: types.render(&p.ty),
                        })
                        .collect(),
                    return_type: String::new(),
                    models: self.models(defs, interface, &types),
                    methods: methods
                        .iter()
                        .map(|m| MethodContext {
                            name: self.source_name(&m.name),
                            return_type: types.render(&m.returns),
                            parameters: m
                                .params
                                .iter()
                                .map(|p| ParameterContext {
                                    name: self.source_name(&p.name),
                                    ty: types.render(&p.ty),
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
                    models: String::new(),
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

    fn models(
        self,
        defs: &[TypeDefinition],
        interface: &Interface,
        types: &StarterTypes<'_>,
    ) -> String {
        if !matches!(self, Language::Java | Language::Kotlin) {
            return String::new();
        }
        let mut out = String::new();
        for d in defs {
            let graph = d.codec == Codec::ObjectGraph;
            let class_name = if graph {
                graph_node_name(d, defs, interface)
            } else {
                self.source_name(&d.name)
            };
            let mut fields = String::new();
            if graph {
                if self == Language::Java {
                    fields.push_str(&format!(
                        "    public {} __judgeId;\n",
                        types.render(&Type::String)
                    ));
                } else {
                    fields.push_str(&format!(
                        "    var __judgeId: {}? = null\n",
                        types.render(&Type::String)
                    ));
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
                        _ => types.render(&f.ty),
                    }
                } else {
                    types.render(&f.ty)
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
                    let list = types.standard_name("java.util.List");
                    let array_list = types.standard_name("java.util.ArrayList");
                    out.push_str(&format!("class {} {{\n    public {list}<{class_name}> roots = new {array_list}<>();\n    public {list}<{class_name}> nodes = new {array_list}<>();\n}}\n",self.source_name(&d.name)));
                }
            } else {
                out.push_str(&format!("class {class_name} {{\n{fields}}}\n"));
                if graph {
                    let list = types.standard_name("kotlin.collections.MutableList");
                    let factory = types.list_factory();
                    out.push_str(&format!("class {} {{\n    var roots: {list}<{class_name}?> = {factory}()\n    var nodes: {list}<{class_name}> = {factory}()\n}}\n",self.source_name(&d.name)));
                }
            }
        }
        out
    }

    pub fn descriptor(self) -> crate::contract::LanguageDescriptor {
        self.implementation().descriptor()
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
    fn jvm_standard_types_use_short_names_recursively() {
        let interface = Interface::Function {
            name: "echo".into(),
            params: vec![],
            returns: Type::Void,
        };
        for (language, expected) in [
            (
                Language::Java,
                ["void", "Integer", "Long", "Double", "Boolean", "String"],
            ),
            (
                Language::Kotlin,
                ["Unit", "Int", "Long", "Double", "Boolean", "String"],
            ),
        ] {
            let types = StarterTypes::new(language, &[], &interface);
            for (ty, expected) in [
                Type::Void,
                Type::Int,
                Type::Int64,
                Type::Float,
                Type::Bool,
                Type::String,
            ]
            .iter()
            .zip(expected)
            {
                assert_eq!(types.render(ty), expected);
            }
            let nested = Type::Array(Box::new(Type::Array(Box::new(Type::Nullable(Box::new(
                Type::Int64,
            ))))));
            assert_eq!(
                types.render(&nested),
                if language == Language::Java {
                    "List<List<Long>>"
                } else {
                    "MutableList<MutableList<Long?>>"
                }
            );
        }
    }

    #[test]
    fn jvm_function_and_stateful_starters_shorten_all_models() {
        let defs: Vec<TypeDefinition> = serde_json::from_value(serde_json::json!([
            {"name":"Sample","fields":[{"name":"label","ty":"string"},{"name":"items","ty":{"array":{"nullable":"int64"}}}]},
            {"name":"Link","codec":"singly_linked_list","fields":[{"name":"val","ty":"int"},{"name":"next","ty":{"nullable":{"named":"Link"}}}]},
            {"name":"Tree","codec":"binary_tree","fields":[{"name":"val","ty":"int"},{"name":"left","ty":{"nullable":{"named":"Tree"}}},{"name":"right","ty":{"nullable":{"named":"Tree"}}}]},
            {"name":"Nary","codec":"nary_tree","fields":[{"name":"val","ty":"int"},{"name":"children","ty":{"array":{"named":"Nary"}}}]},
            {"name":"Graph","codec":"object_graph","fields":[{"name":"label","ty":"string"},{"name":"next","ty":{"nullable":{"named":"Graph"}}}]}
        ])).unwrap();
        let interfaces: Vec<Interface> = serde_json::from_value(serde_json::json!([
            {"kind":"function","name":"echo","params":[{"name":"values","ty":{"array":{"nullable":"int"}}}],"returns":{"array":{"array":"int64"}}},
            {"kind":"data_structure","name":"Store","constructor":{"params":[{"name":"values","ty":{"array":{"nullable":"int"}}}]},"methods":[{"name":"echo","params":[{"name":"values","ty":{"array":{"nullable":"int"}}}],"returns":{"array":{"array":"int64"}}},{"name":"clear","params":[],"returns":"void"}]}
        ])).unwrap();
        for language in [Language::Java, Language::Kotlin] {
            for interface in &interfaces {
                let starter = language.starter_with_definitions(interface, &defs).unwrap();
                assert!(
                    !starter
                        .lines()
                        .filter(|line| !line.starts_with("import "))
                        .any(|line| line.contains("java.")),
                    "{starter}"
                );
                assert!(!starter.contains("kotlin."), "{starter}");
                if language == Language::Java {
                    assert!(starter.contains("List<Integer> values"), "{starter}");
                    assert!(starter.contains("List<List<Long>> echo("), "{starter}");
                    assert!(starter.contains("public String __judgeId;"), "{starter}");
                    assert!(
                        starter.contains("List<GraphNode> roots = new ArrayList<>();"),
                        "{starter}"
                    );
                } else {
                    assert!(starter.contains("values: MutableList<Int?>"), "{starter}");
                    assert!(
                        starter.contains("): MutableList<MutableList<Long>>"),
                        "{starter}"
                    );
                    assert!(
                        starter.contains("var __judgeId: String? = null"),
                        "{starter}"
                    );
                    assert!(starter.contains("MutableList<Nary?>"), "{starter}");
                    assert!(
                        starter.contains("MutableList<GraphNode?> = mutableListOf()"),
                        "{starter}"
                    );
                }
            }
        }
    }

    #[test]
    fn jvm_standard_names_qualify_only_model_and_stateful_collisions() {
        for language in [Language::Java, Language::Kotlin] {
            let scalar_types = [
                Type::Int,
                Type::Int64,
                Type::Float,
                Type::Bool,
                Type::String,
            ];
            let mut test_types = scalar_types.to_vec();
            test_types.push(Type::Array(Box::new(Type::Int)));
            if language == Language::Kotlin {
                test_types.push(Type::Void);
            }
            for ty in &test_types {
                let qualified = language.implementation().type_spelling(ty);
                let qualified_symbol = qualified.split('<').next().unwrap();
                let name = qualified_symbol.rsplit('.').next().unwrap();
                let named = Type::Named(name.into());
                let defs = vec![TypeDefinition {
                    name: name.into(),
                    codec: Codec::Record,
                    fields: vec![],
                }];
                let interfaces: Vec<Interface> = serde_json::from_value(serde_json::json!([
                    {"kind":"function","name":"echo","params":[{"name":"model","ty":{"named":name}}],"returns":ty},
                    {"kind":"data_structure","name":name,"constructor":{"params":[]},"methods":[{"name":"echo","params":[],"returns":ty}]}
                ])).unwrap();
                for (interface, definitions) in
                    [(&interfaces[0], defs.as_slice()), (&interfaces[1], &[][..])]
                {
                    let types = StarterTypes::new(language, definitions, interface);
                    assert!(types.render(ty).starts_with(qualified_symbol));
                    assert_eq!(types.render(&named), name);
                    for other in scalar_types.iter().filter(|other| *other != ty) {
                        assert!(!types.render(other).contains('.'));
                    }
                    let starter = language
                        .starter_with_definitions(interface, definitions)
                        .unwrap();
                    assert!(starter.contains(&types.render(ty)), "{starter}");
                }
            }
        }
    }

    #[test]
    fn jvm_graph_initializers_preserve_helper_collisions() {
        let defs: Vec<TypeDefinition> = serde_json::from_value(serde_json::json!([
            {"name":"Graph","codec":"object_graph","fields":[]},
            {"name":"ArrayList","fields":[]},
            {"name":"MutableList","fields":[]},
            {"name":"mutableListOf","fields":[]}
        ]))
        .unwrap();
        let interface = Interface::Function {
            name: "echo".into(),
            params: vec![],
            returns: Type::Void,
        };
        let java = Language::Java
            .starter_with_definitions(&interface, &defs)
            .unwrap();
        assert!(
            java.contains("List<GraphNode> roots = new java.util.ArrayList<>();"),
            "{java}"
        );
        let kotlin = Language::Kotlin
            .starter_with_definitions(&interface, &defs)
            .unwrap();
        assert!(
            kotlin.contains(
                "kotlin.collections.MutableList<GraphNode?> = kotlin.collections.mutableListOf()"
            ),
            "{kotlin}"
        );
        let interface = Interface::Function {
            name: "mutableListOf".into(),
            params: vec![],
            returns: Type::Void,
        };
        let kotlin = Language::Kotlin
            .starter_with_definitions(&interface, &defs[..1])
            .unwrap();
        assert!(
            kotlin.contains("MutableList<GraphNode?> = kotlin.collections.mutableListOf()"),
            "{kotlin}"
        );
    }

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
            StarterTypes::new(
                Language::Kotlin,
                &defs,
                &Interface::Function {
                    name: "echo".into(),
                    params: vec![],
                    returns: Type::Named("Box".into())
                }
            )
            .render(&Type::Named("Box".into())),
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
