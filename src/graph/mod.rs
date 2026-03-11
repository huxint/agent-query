pub mod query;
pub mod resolve;
pub mod types;

use resolve::ImportResolver;
use std::collections::{HashMap, HashSet};
use types::{EdgeType, GraphData, NodeType};

/// In-memory AST graph index with multiple query capabilities.
pub struct ASTGraph {
    pub data: GraphData,
    pub nodes_by_id: HashMap<String, usize>,
    pub nodes_by_path: HashMap<String, Vec<usize>>,
    pub modules_by_path: HashMap<String, usize>,
    pub path_to_module_id: HashMap<String, String>,
    pub imports_forward: HashMap<String, HashSet<String>>,
    pub internal_imports_forward: HashMap<String, HashSet<String>>,
    pub internal_imports_reverse: HashMap<String, HashSet<String>>,
    pub contains_children: HashMap<String, Vec<usize>>,
    pub alias_to_module_ids: HashMap<String, HashSet<String>>,
}

impl ASTGraph {
    /// Build an ASTGraph from GraphData.
    pub fn new(data: GraphData) -> Self {
        let mut graph = ASTGraph {
            data,
            nodes_by_id: HashMap::new(),
            nodes_by_path: HashMap::new(),
            modules_by_path: HashMap::new(),
            path_to_module_id: HashMap::new(),
            imports_forward: HashMap::new(),
            internal_imports_forward: HashMap::new(),
            internal_imports_reverse: HashMap::new(),
            contains_children: HashMap::new(),
            alias_to_module_ids: HashMap::new(),
        };
        graph.build_index();
        graph
    }

    fn build_index(&mut self) {
        // Index nodes
        for (idx, node) in self.data.nodes.iter().enumerate() {
            self.nodes_by_id.insert(node.id.clone(), idx);
            if !node.path.is_empty() {
                self.nodes_by_path
                    .entry(node.path.clone())
                    .or_default()
                    .push(idx);
            }
            if node.node_type == NodeType::Module && !node.path.is_empty() {
                self.modules_by_path.insert(node.path.clone(), idx);
                self.path_to_module_id
                    .insert(node.path.clone(), node.id.clone());
                for alias in ImportResolver::module_aliases(&node.id, &node.path) {
                    self.alias_to_module_ids
                        .entry(alias)
                        .or_default()
                        .insert(node.id.clone());
                }
            }
        }

        // Index edges
        for edge in &self.data.edges {
            match edge.edge_type {
                EdgeType::Imports => {
                    self.imports_forward
                        .entry(edge.source.clone())
                        .or_default()
                        .insert(edge.target.clone());
                }
                EdgeType::Contains => {
                    if self.nodes_by_id.contains_key(&edge.target) {
                        self.contains_children
                            .entry(edge.source.clone())
                            .or_default()
                            .push(self.nodes_by_id[&edge.target]);
                    }
                }
            }
        }

        // Build internal import indices (resolved module-to-module)
        let module_ids: HashSet<String> = self
            .data
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Module)
            .map(|n| n.id.clone())
            .collect();

        let forward_clone = self.imports_forward.clone();
        for (source, targets) in &forward_clone {
            if !module_ids.contains(source) {
                continue;
            }
            for target in targets {
                if let Some(resolved) = self.resolve_import_target(target)
                    && module_ids.contains(&resolved)
                    && &resolved != source
                {
                    self.internal_imports_forward
                        .entry(source.clone())
                        .or_default()
                        .insert(resolved.clone());
                    self.internal_imports_reverse
                        .entry(resolved)
                        .or_default()
                        .insert(source.clone());
                }
            }
        }
    }

    /// Resolve a raw import target string to a module ID.
    pub fn resolve_import_target(&self, target: &str) -> Option<String> {
        ImportResolver::resolve(
            target,
            &self.nodes_by_id,
            &self.data.nodes,
            &self.alias_to_module_ids,
        )
    }

    /// Resolve a file path or module ID to a canonical module ID.
    pub fn resolve_to_module_id(&self, query: &str) -> Option<String> {
        // Direct module ID lookup
        if let Some(&idx) = self.nodes_by_id.get(query)
            && self.data.nodes[idx].node_type == NodeType::Module
        {
            return Some(query.to_string());
        }
        // File path lookup
        let normalized = query.replace('\\', "/");
        if let Some(mid) = self.path_to_module_id.get(&normalized) {
            return Some(mid.clone());
        }
        // Fuzzy suffix match with path-boundary requirement
        let mut candidates: Vec<(&String, &String)> = self
            .path_to_module_id
            .iter()
            .filter(|(path, _)| {
                path.as_str() == normalized
                    || path.ends_with(&format!("/{}", normalized))
                    || normalized.ends_with(&format!("/{}", path))
            })
            .collect();
        // Deterministic: return shortest path match
        candidates.sort_by_key(|(path, _)| path.len());
        if let Some((_, mid)) = candidates.first() {
            return Some((*mid).clone());
        }
        None
    }

    /// Reverse lookup: module ID → file path.
    pub fn resolve_to_path(&self, module_id: &str) -> Option<String> {
        self.nodes_by_id
            .get(module_id)
            .map(|&idx| self.data.nodes[idx].path.clone())
    }

    /// Classify imports into internal (resolved) and external.
    pub fn classify_imports(
        &self,
        imports: &HashSet<String>,
    ) -> (Vec<(String, String)>, Vec<String>) {
        let mut internal = Vec::new();
        let mut external = Vec::new();
        let mut sorted: Vec<&String> = imports.iter().collect();
        sorted.sort();
        for imp in sorted {
            if let Some(resolved) = self.resolve_import_target(imp) {
                internal.push((imp.clone(), resolved));
            } else {
                external.push(imp.clone());
            }
        }
        (internal, external)
    }
}
