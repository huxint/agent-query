pub mod languages;
pub mod walker;

use crate::graph::types::{Edge, EdgeType, GraphData, Node, NodeType, Stats};
use languages::LangConfig;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node as TsNode, Parser, Query, QueryCursor};

struct CompiledQueries {
    struct_query: Option<Query>,
    imports_query: Option<Query>,
}

#[derive(Clone, Copy)]
struct FileExtractionRange {
    node_start: usize,
    node_end: usize,
    edge_start: usize,
    edge_end: usize,
}

/// Convert a file path to a dotted module ID.
/// e.g. src/nexus/api/routes.py → src.nexus.api.routes
///      src/core/__init__.py    → src.core
fn file_module_id(repo_path: &Path, file_path: &Path) -> String {
    let rel = file_path.strip_prefix(repo_path).unwrap_or(file_path);
    let mut parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if let Some(last) = parts.last_mut() {
        // Remove extension
        if let Some(dot_pos) = last.rfind('.') {
            *last = last[..dot_pos].to_string();
        }
    }
    // Python: __init__ merges into package path
    if parts.last().is_some_and(|p| p == "__init__") && parts.len() > 1 {
        parts.pop();
    }
    parts.join(".")
}

/// Extract AST nodes and edges from a single source file.
fn extract_file(
    repo_path: &Path,
    file_path: &Path,
    lang_name: &str,
    config: &LangConfig,
    queries: &CompiledQueries,
) -> (Vec<Node>, Vec<Edge>, Vec<String>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut errors = Vec::new();

    let source = match std::fs::read(file_path) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("{}: read error: {}", file_path.display(), e));
            return (nodes, edges, errors);
        }
    };

    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&config.language) {
        errors.push(format!("{}: language error: {}", file_path.display(), e));
        return (nodes, edges, errors);
    }

    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => {
            errors.push(format!("{}: parse returned None", file_path.display()));
            return (nodes, edges, errors);
        }
    };

    let rel_path = file_path
        .strip_prefix(repo_path)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");
    let module_id = file_module_id(repo_path, file_path);
    let line_count = source.iter().filter(|&&b| b == b'\n').count() + 1;

    // Module node (file-level)
    nodes.push(Node {
        id: module_id.clone(),
        node_type: NodeType::Module,
        label: module_id
            .split('.')
            .next_back()
            .unwrap_or(&module_id)
            .to_string(),
        path: rel_path.clone(),
        lang: Some(lang_name.to_string()),
        lines: Some(line_count),
        parent: None,
        start_line: None,
        end_line: None,
    });

    // Struct query: classes and functions
    if let Some(query) = queries.struct_query.as_ref() {
        let capture_names: Vec<String> = query
            .capture_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut cursor = QueryCursor::new();
        // Track class byte ranges for nesting detection
        let mut class_ranges: Vec<(usize, usize, String)> = Vec::new();
        let mut seen_class_node_ids = HashSet::new();

        let mut matches = cursor.matches(query, tree.root_node(), source.as_slice());
        while let Some(m) = matches.next() {
            // Group captures by name — must collect data before next()
            let mut capture_data: Vec<(String, usize, usize, usize, usize, usize, usize)> =
                Vec::new();
            for cap in m.captures {
                let name = capture_names[cap.index as usize].clone();
                capture_data.push((
                    name,
                    cap.node.start_byte(),
                    cap.node.end_byte(),
                    cap.node.start_position().row,
                    cap.node.start_position().column,
                    cap.node.end_position().row,
                    cap.node.end_position().column,
                ));
            }

            let is_class = capture_data.iter().any(|(k, ..)| k.starts_with("class."));
            let name_key = if is_class { "class.name" } else { "func.name" };

            let def_cap = match capture_data.iter().find(|(k, ..)| {
                if is_class {
                    k == "class.def" || k == "class.scope"
                } else {
                    k == "func.def"
                }
            }) {
                Some(c) => c,
                None => continue,
            };
            let name_cap = match capture_data.iter().find(|(k, ..)| k == name_key) {
                Some(c) => c,
                None => continue,
            };

            let def_start_byte = def_cap.1;
            let def_end_byte = def_cap.2;
            let def_start_row = def_cap.3;
            let def_end_row = def_cap.5;
            let name_start_byte = name_cap.1;
            let name_end_byte = name_cap.2;
            let name = std::str::from_utf8(&source[name_start_byte..name_end_byte])
                .unwrap_or("<invalid utf8>")
                .to_string();

            if is_class {
                let node_id = format!("{}.{}", module_id, name);
                if capture_data.iter().any(|(k, ..)| k == "class.def")
                    && seen_class_node_ids.insert(node_id.clone())
                {
                    nodes.push(Node {
                        id: node_id.clone(),
                        node_type: NodeType::Class,
                        label: name,
                        path: rel_path.clone(),
                        lang: None,
                        lines: None,
                        parent: Some(module_id.clone()),
                        start_line: Some(def_start_row + 1),
                        end_line: Some(def_end_row + 1),
                    });
                    edges.push(Edge {
                        source: module_id.clone(),
                        target: node_id.clone(),
                        edge_type: EdgeType::Contains,
                    });
                }
                class_ranges.push((def_start_byte, def_end_byte, node_id.clone()));
            } else {
                // Check if this function is inside a class
                let mut parent_id = module_id.clone();
                for (cls_start, cls_end, cls_id) in &class_ranges {
                    if *cls_start <= def_start_byte && def_end_byte <= *cls_end {
                        parent_id = cls_id.clone();
                        break;
                    }
                }
                let node_id = format!("{}.{}", parent_id, name);
                nodes.push(Node {
                    id: node_id.clone(),
                    node_type: NodeType::Function,
                    label: name,
                    path: rel_path.clone(),
                    lang: None,
                    lines: None,
                    parent: Some(parent_id.clone()),
                    start_line: Some(def_start_row + 1),
                    end_line: Some(def_end_row + 1),
                });
                edges.push(Edge {
                    source: parent_id,
                    target: node_id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
    }

    // Imports query
    if lang_name == "rust" {
        for target in extract_rust_imports(tree.root_node(), source.as_slice()) {
            edges.push(Edge {
                source: module_id.clone(),
                target,
                edge_type: EdgeType::Imports,
            });
        }
    } else if let Some(query) = queries.imports_query.as_ref() {
        let capture_names: Vec<String> = query
            .capture_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_slice());
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = &capture_names[cap.index as usize];
                if name == "mod" {
                    let target =
                        std::str::from_utf8(&source[cap.node.start_byte()..cap.node.end_byte()])
                            .unwrap_or("")
                            .trim_matches(|c| {
                                c == '"' || c == '\'' || c == '<' || c == '>' || c == ' '
                            })
                            .to_string();
                    if !target.is_empty() {
                        edges.push(Edge {
                            source: module_id.clone(),
                            target,
                            edge_type: EdgeType::Imports,
                        });
                    }
                }
            }
        }
    }

    (nodes, edges, errors)
}

fn collect_node_ranges_by_kind(node: TsNode<'_>, kind: &str, out: &mut Vec<(usize, usize)>) {
    if node.kind() == kind {
        out.push((node.start_byte(), node.end_byte()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_ranges_by_kind(child, kind, out);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RustUseToken {
    Ident(String),
    DoubleColon,
    LBrace,
    RBrace,
    Comma,
    Star,
    As,
}

fn tokenize_rust_use_tree(input: &str) -> Vec<RustUseToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut idx = 0usize;

    while idx < chars.len() {
        match chars[idx] {
            c if c.is_whitespace() => idx += 1,
            ':' if idx + 1 < chars.len() && chars[idx + 1] == ':' => {
                tokens.push(RustUseToken::DoubleColon);
                idx += 2;
            }
            '{' => {
                tokens.push(RustUseToken::LBrace);
                idx += 1;
            }
            '}' => {
                tokens.push(RustUseToken::RBrace);
                idx += 1;
            }
            ',' => {
                tokens.push(RustUseToken::Comma);
                idx += 1;
            }
            '*' => {
                tokens.push(RustUseToken::Star);
                idx += 1;
            }
            c if c.is_ascii_alphanumeric() || c == '_' => {
                let start = idx;
                idx += 1;
                while idx < chars.len() && (chars[idx].is_ascii_alphanumeric() || chars[idx] == '_')
                {
                    idx += 1;
                }
                let token = chars[start..idx].iter().collect::<String>();
                if token == "as" {
                    tokens.push(RustUseToken::As);
                } else {
                    tokens.push(RustUseToken::Ident(token));
                }
            }
            _ => idx += 1,
        }
    }

    tokens
}

fn normalize_rust_use_path(segments: &[String]) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    let mut normalized = segments.to_vec();
    if normalized.last().is_some_and(|segment| segment == "self") {
        normalized.pop();
    }

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.join("::"))
    }
}

fn parse_rust_use_list(
    tokens: &[RustUseToken],
    pos: &mut usize,
    prefix: &[String],
    out: &mut Vec<String>,
) {
    while *pos < tokens.len() && !matches!(tokens.get(*pos), Some(RustUseToken::RBrace)) {
        parse_rust_use_tree(tokens, pos, prefix, out);
        if matches!(tokens.get(*pos), Some(RustUseToken::Comma)) {
            *pos += 1;
        } else {
            break;
        }
    }
}

fn parse_rust_use_tree(
    tokens: &[RustUseToken],
    pos: &mut usize,
    prefix: &[String],
    out: &mut Vec<String>,
) {
    let mut segments = Vec::new();

    while let Some(token) = tokens.get(*pos) {
        match token {
            RustUseToken::Ident(name) => {
                segments.push(name.clone());
                *pos += 1;
                if matches!(tokens.get(*pos), Some(RustUseToken::DoubleColon)) {
                    *pos += 1;
                    if matches!(tokens.get(*pos), Some(RustUseToken::LBrace)) {
                        break;
                    }
                    continue;
                }
                break;
            }
            _ => break,
        }
    }

    let mut current_prefix = prefix.to_vec();
    current_prefix.extend(segments);

    match tokens.get(*pos) {
        Some(RustUseToken::LBrace) => {
            *pos += 1;
            parse_rust_use_list(tokens, pos, &current_prefix, out);
            if matches!(tokens.get(*pos), Some(RustUseToken::RBrace)) {
                *pos += 1;
            }
        }
        Some(RustUseToken::Star) => {
            *pos += 1;
            if let Some(path) = normalize_rust_use_path(&current_prefix) {
                out.push(path);
            }
        }
        Some(RustUseToken::As) => {
            *pos += 1;
            if matches!(tokens.get(*pos), Some(RustUseToken::Ident(_))) {
                *pos += 1;
            }
            if let Some(path) = normalize_rust_use_path(&current_prefix) {
                out.push(path);
            }
        }
        _ => {
            if let Some(path) = normalize_rust_use_path(&current_prefix) {
                out.push(path);
            }
        }
    }
}

fn parse_rust_use_declaration(declaration: &str) -> Vec<String> {
    let trimmed = declaration.trim();
    let body = trimmed
        .strip_prefix("use")
        .unwrap_or(trimmed)
        .trim()
        .trim_end_matches(';')
        .trim();
    let tokens = tokenize_rust_use_tree(body);
    let mut pos = 0usize;
    let mut paths = Vec::new();
    parse_rust_use_list(&tokens, &mut pos, &[], &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn extract_rust_imports(root: TsNode<'_>, source: &[u8]) -> Vec<String> {
    let mut ranges = Vec::new();
    collect_node_ranges_by_kind(root, "use_declaration", &mut ranges);

    let mut imports = Vec::new();
    for (start, end) in ranges {
        let Ok(text) = std::str::from_utf8(&source[start..end]) else {
            continue;
        };
        imports.extend(parse_rust_use_declaration(text));
    }
    imports.sort();
    imports.dedup();
    imports
}

fn rewrite_node_id(id: &str, old_prefix: &str, new_prefix: &str) -> String {
    if id == old_prefix {
        return new_prefix.to_string();
    }

    match id.strip_prefix(old_prefix) {
        Some(suffix) if suffix.starts_with('.') => format!("{new_prefix}{suffix}"),
        _ => id.to_string(),
    }
}

fn deduplicate_node_ids(
    nodes: &mut [Node],
    edges: &mut [Edge],
    file_ranges: &[FileExtractionRange],
) {
    let mut seen_module_ids: HashMap<String, usize> = HashMap::new();

    for range in file_ranges {
        if range.node_start >= range.node_end {
            continue;
        }

        let old_module_id = nodes[range.node_start].id.clone();
        let duplicate_count = seen_module_ids.entry(old_module_id.clone()).or_insert(0);
        *duplicate_count += 1;

        if *duplicate_count == 1 {
            continue;
        }

        let new_module_id = format!("{}_{}", old_module_id, duplicate_count);

        for node in &mut nodes[range.node_start..range.node_end] {
            node.id = rewrite_node_id(&node.id, &old_module_id, &new_module_id);
            if let Some(parent) = node.parent.as_mut() {
                *parent = rewrite_node_id(parent, &old_module_id, &new_module_id);
            }
        }

        for edge in &mut edges[range.edge_start..range.edge_end] {
            edge.source = rewrite_node_id(&edge.source, &old_module_id, &new_module_id);
            if edge.edge_type != EdgeType::Imports {
                edge.target = rewrite_node_id(&edge.target, &old_module_id, &new_module_id);
            }
        }
    }
}

/// Truncate nodes when exceeding max_nodes, prioritizing Module/Class over Function.
pub fn apply_max_nodes(
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    max_nodes: usize,
) -> (Vec<Node>, Vec<Edge>, bool, usize) {
    if nodes.len() <= max_nodes {
        return (nodes, edges, false, 0);
    }

    let all_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let priority_nodes: Vec<Node> = nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Module || n.node_type == NodeType::Class)
        .cloned()
        .collect();
    let func_nodes: Vec<Node> = nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Function)
        .cloned()
        .collect();

    let kept_priority: Vec<Node> = priority_nodes.into_iter().take(max_nodes).collect();
    let remaining_slots = max_nodes.saturating_sub(kept_priority.len());
    let kept_funcs: Vec<Node> = func_nodes.iter().take(remaining_slots).cloned().collect();
    let truncated_count = nodes
        .len()
        .saturating_sub(kept_priority.len() + kept_funcs.len());

    let mut kept_nodes = kept_priority;
    kept_nodes.extend(kept_funcs);

    let kept_ids: HashSet<&str> = kept_nodes.iter().map(|n| n.id.as_str()).collect();
    let kept_edges: Vec<Edge> = edges
        .into_iter()
        .filter(|e| {
            kept_ids.contains(e.source.as_str())
                && (kept_ids.contains(e.target.as_str()) || !all_ids.contains(e.target.as_str()))
        })
        .collect();

    (kept_nodes, kept_edges, true, truncated_count)
}

/// Extract AST structure from a repository. Returns GraphData.
pub fn extract_repo(repo_path: &Path, max_nodes: usize) -> anyhow::Result<GraphData> {
    let lang_configs = languages::build_language_configs();
    let ext_map = languages::extension_map();
    let available: HashSet<&str> = lang_configs.keys().copied().collect();
    let mut compiled_queries: HashMap<&str, CompiledQueries> = HashMap::new();
    let mut all_errors = Vec::new();

    for (&lang_name, config) in &lang_configs {
        let struct_query = if config.struct_query.is_empty() {
            None
        } else {
            match Query::new(&config.language, config.struct_query) {
                Ok(query) => Some(query),
                Err(err) => {
                    all_errors.push(format!("{}: struct query error: {}", lang_name, err));
                    None
                }
            }
        };
        let imports_query = if config.imports_query.is_empty() {
            None
        } else {
            match Query::new(&config.language, config.imports_query) {
                Ok(query) => Some(query),
                Err(err) => {
                    all_errors.push(format!("{}: import query error: {}", lang_name, err));
                    None
                }
            }
        };

        compiled_queries.insert(
            lang_name,
            CompiledQueries {
                struct_query,
                imports_query,
            },
        );
    }

    let (source_files, supported_counts) =
        walker::collect_source_files(repo_path, &ext_map, &available);

    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    let mut detected_langs: HashSet<String> = HashSet::new();
    let mut total_lines = 0usize;
    let mut processed_files = 0usize;
    let mut file_ranges = Vec::new();

    let languages_with_struct_queries: Vec<String> = lang_configs
        .iter()
        .filter(|(_, cfg)| !cfg.struct_query.is_empty())
        .map(|(name, _)| name.to_string())
        .collect();

    for (file_path, lang_name) in &source_files {
        let config = match lang_configs.get(lang_name.as_str()) {
            Some(c) => c,
            None => continue,
        };
        let queries = match compiled_queries.get(lang_name.as_str()) {
            Some(q) => q,
            None => continue,
        };

        processed_files += 1;
        let node_start = all_nodes.len();
        let edge_start = all_edges.len();
        let (nodes, edges, errors) = extract_file(repo_path, file_path, lang_name, config, queries);
        if let Some(first) = nodes.first() {
            detected_langs.insert(lang_name.clone());
            total_lines += first.lines.unwrap_or(0);
        }
        all_nodes.extend(nodes);
        all_edges.extend(edges);
        file_ranges.push(FileExtractionRange {
            node_start,
            node_end: all_nodes.len(),
            edge_start,
            edge_end: all_edges.len(),
        });
        all_errors.extend(errors);
    }

    deduplicate_node_ids(&mut all_nodes, &mut all_edges, &file_ranges);

    let (final_nodes, final_edges, truncated, truncated_count) =
        apply_max_nodes(all_nodes, all_edges, max_nodes);

    let mut languages: Vec<String> = detected_langs.into_iter().collect();
    languages.sort();

    let mut struct_queries_sorted = languages_with_struct_queries;
    struct_queries_sorted.sort();

    let data = GraphData {
        languages,
        stats: Stats {
            total_files: processed_files,
            total_lines,
            parse_errors: all_errors.len(),
            truncated,
            truncated_nodes: truncated_count,
            supported_file_counts: supported_counts,
            languages_with_structural_queries: struct_queries_sorted,
        },
        nodes: final_nodes,
        edges: final_edges,
        warnings: None,
        errors: if all_errors.is_empty() {
            None
        } else {
            Some(all_errors.into_iter().take(20).collect())
        },
    };

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ASTGraph;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(files: &[(&str, &str)]) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-query-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).unwrap();
            for (relative, content) in files {
                let file_path = path.join(relative);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(file_path, content).unwrap();
            }
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn apply_max_nodes_keeps_late_module_nodes() {
        let repo = TempRepo::new(&[
            (
                "a.rs",
                "fn a1() {}\nfn a2() {}\nfn a3() {}\nfn a4() {}\nfn a5() {}\n",
            ),
            ("b.rs", "fn b1() {}\n"),
        ]);

        let data = extract_repo(repo.path(), 3).unwrap();
        let module_paths = data
            .nodes
            .iter()
            .filter(|node| node.node_type == NodeType::Module)
            .map(|node| node.path.as_str())
            .collect::<Vec<_>>();

        assert!(module_paths.contains(&"a.rs"));
        assert!(module_paths.contains(&"b.rs"));
    }

    #[test]
    fn rust_grouped_use_imports_resolve_to_internal_modules() {
        let repo = TempRepo::new(&[
            (
                "lib.rs",
                "mod types;\nuse crate::types::{Thing, Other as Alias};\nfn main() {}\n",
            ),
            ("types.rs", "pub struct Thing;\npub struct Other;\n"),
        ]);

        let graph = ASTGraph::new(extract_repo(repo.path(), 100).unwrap());
        let deps = graph.query_deps("lib.rs", 2, true, false);

        assert!(deps.contains("types.rs"), "deps output was:\n{}", deps);
    }

    #[test]
    fn rust_impl_blocks_do_not_duplicate_class_nodes() {
        let repo = TempRepo::new(&[("lib.rs", "struct Foo;\nimpl Foo { fn bar(&self) {} }\n")]);

        let data = extract_repo(repo.path(), 100).unwrap();
        let foo_classes = data
            .nodes
            .iter()
            .filter(|node| {
                node.node_type == NodeType::Class && node.path == "lib.rs" && node.label == "Foo"
            })
            .collect::<Vec<_>>();
        let bar_methods = data
            .nodes
            .iter()
            .filter(|node| {
                node.node_type == NodeType::Function && node.path == "lib.rs" && node.label == "bar"
            })
            .collect::<Vec<_>>();

        assert_eq!(foo_classes.len(), 1);
        assert_eq!(bar_methods.len(), 1);
        assert_eq!(bar_methods[0].parent.as_deref(), Some("lib.Foo"));
    }
}
