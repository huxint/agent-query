use crate::graph::types::NodeType;
use std::collections::{HashMap, HashSet};

/// SOURCE_ROOT_MARKERS: common source root directory patterns.
/// When a module path starts with one of these, we generate an alias
/// that strips the marker prefix.
const SOURCE_ROOT_MARKERS: &[&[&str]] = &[
    &["src"],
    &["backend", "src"],
    &["frontend", "src"],
    &["client", "src"],
    &["src", "main", "python"],
    &["src", "test", "python"],
    &["src", "main", "java"],
    &["src", "test", "java"],
    &["src", "main", "kotlin"],
    &["src", "test", "kotlin"],
];

pub struct ImportResolver;

impl ImportResolver {
    /// Generate module aliases for a given module ID and file path.
    /// This enables resolving imports like `from api.routes import ...`
    /// when the actual module ID is `src.api.routes`.
    pub fn module_aliases(module_id: &str, path: &str) -> HashSet<String> {
        let mut aliases = HashSet::new();
        aliases.insert(module_id.to_string());

        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        if parts.is_empty() {
            return aliases;
        }

        // Build normalized_parts: strip extension, treat __init__/mod as directory modules.
        let last = parts.last().unwrap();
        let stem = match last.rfind('.') {
            Some(pos) => &last[..pos],
            None => last,
        };
        let mut normalized_parts: Vec<&str> = parts[..parts.len() - 1].to_vec();
        if stem != "__init__" && stem != "mod" {
            normalized_parts.push(stem);
        }

        // Check against SOURCE_ROOT_MARKERS
        for marker in SOURCE_ROOT_MARKERS {
            if normalized_parts.len() > marker.len() {
                let matches = marker
                    .iter()
                    .zip(normalized_parts.iter())
                    .all(|(m, p)| m == p);
                if matches {
                    let alias = normalized_parts[marker.len()..].join(".");
                    if !alias.is_empty() {
                        aliases.insert(alias);
                    }
                }
            }
        }

        // Also: any "src" component → alias from after src
        for (idx, part) in normalized_parts.iter().enumerate() {
            if *part == "src" && idx + 1 < normalized_parts.len() {
                let alias = normalized_parts[idx + 1..].join(".");
                if !alias.is_empty() {
                    aliases.insert(alias);
                }
            }
        }

        // Rust sibling imports often use only the last module segment, such as
        // `use types::NodeType` inside `graph/mod.rs`. Keep this alias only when
        // it is non-empty; ambiguity is filtered later by requiring a unique match.
        if let Some(last_part) = normalized_parts.last()
            && !last_part.is_empty()
        {
            aliases.insert((*last_part).to_string());
        }

        aliases.retain(|a| !a.is_empty());
        aliases
    }

    /// Resolve an import target string to a canonical module ID.
    /// Strategy: direct match → alias match → prefix stripping.
    pub fn resolve(
        target: &str,
        nodes_by_id: &HashMap<String, usize>,
        nodes: &[crate::graph::types::Node],
        alias_to_module_ids: &HashMap<String, HashSet<String>>,
    ) -> Option<String> {
        let normalized_target = if target.starts_with("./") || target.starts_with("../") {
            target
                .strip_prefix("./")
                .or_else(|| target.strip_prefix("../"))
                .unwrap_or(target)
                .replace('/', ".")
                .trim_start_matches('.')
                .to_string()
        } else {
            let mut normalized = target.replace("::", ".");
            for prefix in ["crate.", "self.", "super."] {
                while let Some(stripped) = normalized.strip_prefix(prefix) {
                    normalized = stripped.to_string();
                }
            }
            normalized
                .split('.')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(".")
        };
        let target = normalized_target.as_str();

        // Direct match
        if let Some(&idx) = nodes_by_id.get(target)
            && nodes[idx].node_type == NodeType::Module
        {
            return Some(target.to_string());
        }

        // Alias match (only if exactly one module maps)
        if let Some(matches) = alias_to_module_ids.get(target)
            && matches.len() == 1
        {
            return Some(matches.iter().next().unwrap().clone());
        }

        // Iterative prefix stripping: a.b.c.d → a.b.c → a.b → a
        let mut parts: Vec<&str> = target.split('.').collect();
        while parts.len() > 1 {
            parts.pop();
            let candidate = parts.join(".");
            if let Some(matches) = alias_to_module_ids.get(&candidate)
                && matches.len() == 1
            {
                return Some(matches.iter().next().unwrap().clone());
            }
        }

        None
    }
}
