use std::collections::HashMap;
use tree_sitter::Language;

/// Language configuration: tree-sitter queries for structure and imports extraction.
pub struct LangConfig {
    pub language: Language,
    pub struct_query: &'static str,
    pub imports_query: &'static str,
}

/// Extension → language name mapping (from languages.json)
pub fn extension_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    // Python
    m.insert(".py", "python");
    m.insert(".pyw", "python");
    m.insert(".pyi", "python");
    // JavaScript
    m.insert(".js", "javascript");
    m.insert(".mjs", "javascript");
    m.insert(".cjs", "javascript");
    m.insert(".jsx", "javascript");
    // TypeScript
    m.insert(".ts", "typescript");
    m.insert(".mts", "typescript");
    // TSX
    m.insert(".tsx", "tsx");
    // Bash
    m.insert(".sh", "bash");
    m.insert(".bash", "bash");
    m.insert(".zsh", "bash");
    // Java
    m.insert(".java", "java");
    // Go
    m.insert(".go", "go");
    // Rust
    m.insert(".rs", "rust");
    // C#
    m.insert(".cs", "csharp");
    // C
    m.insert(".c", "c");
    m.insert(".h", "c");
    // C++
    m.insert(".cpp", "cpp");
    m.insert(".cc", "cpp");
    m.insert(".cxx", "cpp");
    m.insert(".hpp", "cpp");
    m.insert(".hxx", "cpp");
    // Ruby
    m.insert(".rb", "ruby");
    // PHP
    m.insert(".php", "php");
    m
}

// ── Tree-sitter query strings (from languages.json) ──

const PYTHON_STRUCT: &str = r#"(class_definition name: (identifier) @class.name) @class.def
(function_definition name: (identifier) @func.name) @func.def"#;

const PYTHON_IMPORTS: &str = r#"(import_statement name: (dotted_name) @mod)
(import_from_statement module_name: (dotted_name) @mod)"#;

const JAVASCRIPT_STRUCT: &str = r#"(class_declaration name: (identifier) @class.name) @class.def
(function_declaration name: (identifier) @func.name) @func.def
(method_definition name: (property_identifier) @func.name) @func.def"#;

const JAVASCRIPT_IMPORTS: &str = r#"(import_statement source: (string (string_fragment) @mod))"#;

const TYPESCRIPT_STRUCT: &str = r#"(class_declaration name: (type_identifier) @class.name) @class.def
(function_declaration name: (identifier) @func.name) @func.def
(method_definition name: (property_identifier) @func.name) @func.def"#;

const TYPESCRIPT_IMPORTS: &str = r#"(import_statement source: (string (string_fragment) @mod))"#;

const TSX_STRUCT: &str = r#"(class_declaration name: (type_identifier) @class.name) @class.def
(function_declaration name: (identifier) @func.name) @func.def
(method_definition name: (property_identifier) @func.name) @func.def"#;

const TSX_IMPORTS: &str = r#"(import_statement source: (string (string_fragment) @mod))"#;

const JAVA_STRUCT: &str = r#"(class_declaration name: (identifier) @class.name) @class.def
(method_declaration name: (identifier) @func.name) @func.def
(interface_declaration name: (identifier) @class.name) @class.def"#;

const JAVA_IMPORTS: &str = r#"(import_declaration (scoped_identifier) @mod)"#;

// Go methods remain top-level functions in extraction for now.
// Nesting them under structs would require matching receiver types to struct names.
const GO_STRUCT: &str = r#"(type_declaration (type_spec name: (type_identifier) @class.name)) @class.def
(function_declaration name: (identifier) @func.name) @func.def
(method_declaration name: (field_identifier) @func.name) @func.def"#;

const GO_IMPORTS: &str = r#"(import_spec path: (interpreted_string_literal) @mod)"#;

const RUST_STRUCT: &str = r#"(struct_item name: (type_identifier) @class.name) @class.def
(enum_item name: (type_identifier) @class.name) @class.def
(impl_item type: (type_identifier) @class.name) @class.scope
(function_item name: (identifier) @func.name) @func.def"#;

// Rust imports are extracted manually from `use_declaration` nodes because
// grouped use trees like `use crate::types::{Thing, Other};` need recursive parsing.
const RUST_IMPORTS: &str = "";

const CSHARP_STRUCT: &str = r#"(class_declaration name: (identifier) @class.name) @class.def
(method_declaration name: (identifier) @func.name) @func.def
(interface_declaration name: (identifier) @class.name) @class.def"#;

const CSHARP_IMPORTS: &str = r#"(using_directive (qualified_name) @mod)
(using_directive (identifier) @mod)"#;

const CPP_STRUCT: &str = r#"(class_specifier name: (type_identifier) @class.name) @class.def
(function_definition
    declarator: (function_declarator
        declarator: (identifier) @func.name)) @func.def"#;

const CPP_IMPORTS: &str = r#"(preproc_include path: (system_lib_string) @mod)
(preproc_include path: (string_literal) @mod)"#;

const C_STRUCT: &str = r#"(struct_specifier name: (type_identifier) @class.name) @class.def
(function_definition
    declarator: (function_declarator
        declarator: (identifier) @func.name)) @func.def"#;

const C_IMPORTS: &str = r#"(preproc_include path: (system_lib_string) @mod)
(preproc_include path: (string_literal) @mod)"#;

const RUBY_STRUCT: &str = r#"(class name: (constant) @class.name) @class.def
(method name: (identifier) @func.name) @func.def"#;

const RUBY_IMPORTS: &str = r#"(call method: (identifier) @_method arguments: (argument_list (string (string_content) @mod)) (#match? @_method "^require"))"#;

const PHP_STRUCT: &str = r#"(class_declaration name: (name) @class.name) @class.def
(method_declaration name: (name) @func.name) @func.def
(function_definition name: (name) @func.name) @func.def"#;

const PHP_IMPORTS: &str =
    r#"(namespace_use_declaration (namespace_use_clause (qualified_name (name) @mod)))"#;

/// Build the language configuration registry.
/// Returns a HashMap of language name → LangConfig.
pub fn build_language_configs() -> HashMap<&'static str, LangConfig> {
    let mut configs = HashMap::new();

    macro_rules! register {
        ($name:expr, $lang_fn:expr, $struct_q:expr, $imports_q:expr) => {
            configs.insert(
                $name,
                LangConfig {
                    language: $lang_fn,
                    struct_query: $struct_q,
                    imports_query: $imports_q,
                },
            );
        };
    }

    register!(
        "python",
        tree_sitter_python::LANGUAGE.into(),
        PYTHON_STRUCT,
        PYTHON_IMPORTS
    );
    register!(
        "javascript",
        tree_sitter_javascript::LANGUAGE.into(),
        JAVASCRIPT_STRUCT,
        JAVASCRIPT_IMPORTS
    );
    register!(
        "typescript",
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        TYPESCRIPT_STRUCT,
        TYPESCRIPT_IMPORTS
    );
    register!(
        "tsx",
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        TSX_STRUCT,
        TSX_IMPORTS
    );
    register!(
        "java",
        tree_sitter_java::LANGUAGE.into(),
        JAVA_STRUCT,
        JAVA_IMPORTS
    );
    register!("go", tree_sitter_go::LANGUAGE.into(), GO_STRUCT, GO_IMPORTS);
    register!(
        "rust",
        tree_sitter_rust::LANGUAGE.into(),
        RUST_STRUCT,
        RUST_IMPORTS
    );
    register!(
        "csharp",
        tree_sitter_c_sharp::LANGUAGE.into(),
        CSHARP_STRUCT,
        CSHARP_IMPORTS
    );
    register!("c", tree_sitter_c::LANGUAGE.into(), C_STRUCT, C_IMPORTS);
    register!(
        "cpp",
        tree_sitter_cpp::LANGUAGE.into(),
        CPP_STRUCT,
        CPP_IMPORTS
    );
    register!(
        "ruby",
        tree_sitter_ruby::LANGUAGE.into(),
        RUBY_STRUCT,
        RUBY_IMPORTS
    );
    register!(
        "php",
        tree_sitter_php::LANGUAGE_PHP.into(),
        PHP_STRUCT,
        PHP_IMPORTS
    );
    // Bash: no struct or import queries
    register!("bash", tree_sitter_bash::LANGUAGE.into(), "", "");

    configs
}
