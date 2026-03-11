use crate::graph::ASTGraph;
use crate::graph::types::NodeType;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownMode {
    Compact,
    Default,
    Full,
}

fn directory_bucket(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.len() {
        0 | 1 => "(root)/".to_string(),
        2 => format!("{}/", parts[0]),
        _ => format!("{}/", parts[..2].join("/")),
    }
}

impl ASTGraph {
    // ── Existing queries ───────────────────────────────────────

    /// --file: Show file structure and imports.
    pub fn query_file(&self, file_query: &str) -> String {
        let mid = match self.resolve_to_module_id(file_query) {
            Some(m) => m,
            None => return format!("[NOT FOUND] No module matching '{}'", file_query),
        };

        let idx = self.nodes_by_id[&mid];
        let module_node = &self.data.nodes[idx];
        let path = &module_node.path;
        let lines = module_node.lines.map_or("?".to_string(), |l| l.to_string());
        let lang = module_node.lang.as_deref().unwrap_or("?");

        let mut out = vec![
            format!("=== {} ===", path),
            format!("Module: {} ({} lines, {})", mid, lines, lang),
            String::new(),
        ];

        // Classes and top-level functions
        let children_indices = self
            .contains_children
            .get(&mid)
            .cloned()
            .unwrap_or_default();
        let classes: Vec<usize> = children_indices
            .iter()
            .filter(|&&i| self.data.nodes[i].node_type == NodeType::Class)
            .copied()
            .collect();
        let top_funcs: Vec<usize> = children_indices
            .iter()
            .filter(|&&i| self.data.nodes[i].node_type == NodeType::Function)
            .copied()
            .collect();

        if !classes.is_empty() {
            out.push("Classes:".to_string());
            for &cls_idx in &classes {
                let cls = &self.data.nodes[cls_idx];
                let sl = cls.start_line.map_or("?".to_string(), |l| l.to_string());
                let el = cls.end_line.map_or("?".to_string(), |l| l.to_string());
                out.push(format!("  {} (L{}-L{})", cls.label, sl, el));

                // Methods of this class
                let method_indices = self
                    .contains_children
                    .get(&cls.id)
                    .cloned()
                    .unwrap_or_default();
                let methods: Vec<usize> = method_indices
                    .iter()
                    .filter(|&&i| self.data.nodes[i].node_type == NodeType::Function)
                    .copied()
                    .collect();
                for (i, &m_idx) in methods.iter().enumerate() {
                    let m = &self.data.nodes[m_idx];
                    let prefix = if i == methods.len() - 1 {
                        "\u{2514}\u{2500}"
                    } else {
                        "\u{251C}\u{2500}"
                    };
                    let ml = m.start_line.map_or("?".to_string(), |l| l.to_string());
                    let me = m.end_line.map_or("?".to_string(), |l| l.to_string());
                    out.push(format!("    {} {} (L{}-L{})", prefix, m.label, ml, me));
                }
            }
            out.push(String::new());
        }

        if !top_funcs.is_empty() {
            out.push("Top-level Functions:".to_string());
            for &f_idx in &top_funcs {
                let f = &self.data.nodes[f_idx];
                let sl = f.start_line.map_or("?".to_string(), |l| l.to_string());
                let el = f.end_line.map_or("?".to_string(), |l| l.to_string());
                out.push(format!("  {} (L{}-L{})", f.label, sl, el));
            }
            out.push(String::new());
        }

        // Imports
        let imports: Vec<String> = self
            .imports_forward
            .get(&mid)
            .map(|s| {
                let mut v: Vec<String> = s.iter().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default();

        if !imports.is_empty() {
            let import_set: HashSet<String> = imports.into_iter().collect();
            let (internal, external) = self.classify_imports(&import_set);
            out.push("Imports:".to_string());
            for (raw_imp, resolved_imp) in &internal {
                let imp_path = self.resolve_to_path(resolved_imp);
                let suffix = imp_path
                    .as_ref()
                    .map(|p| format!(" ({})", p))
                    .unwrap_or_default();
                if raw_imp == resolved_imp {
                    out.push(format!("  \u{2192} {}{}", raw_imp, suffix));
                } else {
                    out.push(format!(
                        "  \u{2192} {}  [resolved: {}{}]",
                        raw_imp, resolved_imp, suffix
                    ));
                }
            }
            for imp in &external {
                out.push(format!("  \u{2192} {} (external)", imp));
            }
            out.push(String::new());
        }

        if classes.is_empty() && top_funcs.is_empty() && !self.imports_forward.contains_key(&mid) {
            out.push("(no classes, functions, or imports detected)".to_string());
        }

        out.join("\n")
    }

    /// --hub-analysis: Identify high fan-in/fan-out hub modules.
    pub fn query_hub_analysis(&self, top_n: usize) -> String {
        let fan_in: HashMap<&String, usize> = self
            .internal_imports_reverse
            .iter()
            .map(|(target, sources)| (target, sources.len()))
            .collect();

        let fan_out: HashMap<&String, usize> = self
            .internal_imports_forward
            .iter()
            .map(|(source, targets)| (source, targets.len()))
            .collect();

        let mut out = vec!["=== Hub Analysis ===".to_string(), String::new()];

        // Top fan-in
        let mut top_fan_in: Vec<(&&String, &usize)> = fan_in.iter().collect();
        top_fan_in.sort_by(|a, b| b.1.cmp(a.1));
        top_fan_in.truncate(top_n);

        out.push("Top fan-in (most imported by others):".to_string());
        if !top_fan_in.is_empty() {
            for (i, (mid, count)) in top_fan_in.iter().enumerate() {
                let path = self.resolve_to_path(mid).unwrap_or_default();
                out.push(format!(
                    "  {}. {} \u{2014} imported by {} module(s)  [{}]",
                    i + 1,
                    mid,
                    count,
                    path
                ));
            }
        } else {
            out.push("  (no internal import relationships found)".to_string());
        }
        out.push(String::new());

        // Top fan-out
        let mut top_fan_out: Vec<(&&String, &usize)> = fan_out.iter().collect();
        top_fan_out.sort_by(|a, b| b.1.cmp(a.1));
        top_fan_out.truncate(top_n);

        out.push("Top fan-out (imports most others):".to_string());
        if !top_fan_out.is_empty() {
            for (i, (mid, count)) in top_fan_out.iter().enumerate() {
                let path = self.resolve_to_path(mid).unwrap_or_default();
                out.push(format!(
                    "  {}. {} \u{2014} imports {} internal module(s)  [{}]",
                    i + 1,
                    mid,
                    count,
                    path
                ));
            }
        } else {
            out.push("  (no internal import relationships found)".to_string());
        }

        out.join("\n")
    }

    /// --summary: Per-directory structural summary.
    pub fn query_summary(&self) -> String {
        struct DirStat {
            modules: usize,
            classes: usize,
            functions: usize,
            lines: usize,
            class_names: Vec<String>,
            import_dirs: HashSet<String>,
        }

        let mut dir_stats: HashMap<String, DirStat> = HashMap::new();

        // Determine aggregation key: first 2 path components
        for node in &self.data.nodes {
            if node.path.is_empty() {
                continue;
            }
            let dk = directory_bucket(&node.path);
            let stat = dir_stats.entry(dk).or_insert_with(|| DirStat {
                modules: 0,
                classes: 0,
                functions: 0,
                lines: 0,
                class_names: Vec::new(),
                import_dirs: HashSet::new(),
            });

            match node.node_type {
                NodeType::Module => {
                    stat.modules += 1;
                    stat.lines += node.lines.unwrap_or(0);
                }
                NodeType::Class => {
                    stat.classes += 1;
                    stat.class_names.push(node.label.clone());
                }
                NodeType::Function => {
                    stat.functions += 1;
                }
            }
        }

        // Collect import source directories for each directory (use resolved internal imports)
        for (mid, targets) in &self.internal_imports_forward {
            let src_node_idx = match self.nodes_by_id.get(mid) {
                Some(&idx) => idx,
                None => continue,
            };
            let src_node = &self.data.nodes[src_node_idx];
            if src_node.node_type != NodeType::Module || src_node.path.is_empty() {
                continue;
            }
            let src_dk = directory_bucket(&src_node.path);

            for t in targets {
                if let Some(&t_idx) = self.nodes_by_id.get(t) {
                    let t_node = &self.data.nodes[t_idx];
                    if !t_node.path.is_empty() {
                        let t_dk = directory_bucket(&t_node.path);
                        if t_dk != src_dk
                            && let Some(stat) = dir_stats.get_mut(&src_dk)
                        {
                            stat.import_dirs
                                .insert(t_dk.trim_end_matches('/').to_string());
                        }
                    }
                }
            }
        }

        let mut out = vec!["=== Directory Summary ===".to_string(), String::new()];

        let mut sorted_keys: Vec<&String> = dir_stats.keys().collect();
        sorted_keys.sort();

        for dk in sorted_keys {
            let s = &dir_stats[dk];
            out.push(format!(
                "{} ({} modules, {} classes, {} functions, {} lines)",
                dk, s.modules, s.classes, s.functions, s.lines
            ));
            if !s.class_names.is_empty() {
                let names: Vec<&str> = s.class_names.iter().take(8).map(|s| s.as_str()).collect();
                let suffix = if s.class_names.len() > 8 {
                    format!(" ... +{}", s.class_names.len() - 8)
                } else {
                    String::new()
                };
                out.push(format!("  Key classes: {}{}", names.join(", "), suffix));
            }
            let mut import_dirs: Vec<&str> = s.import_dirs.iter().map(|s| s.as_str()).collect();
            import_dirs.sort();
            if !import_dirs.is_empty() {
                out.push(format!("  Key imports from: {}", import_dirs.join(", ")));
            } else {
                out.push("  Key imports from: (none / external only)".to_string());
            }
            out.push(String::new());
        }

        if dir_stats.is_empty() {
            out.push("(no modules found in ast_nodes.json)".to_string());
        }

        out.join("\n")
    }

    // ── New queries ────────────────────────────────────────────

    /// --overview: Project-wide overview with language stats and directory structure.
    pub fn query_overview(&self) -> String {
        // Aggregate per-language stats from Module nodes
        let mut lang_stats: HashMap<String, (usize, usize)> = HashMap::new(); // (files, lines)
        for node in &self.data.nodes {
            if node.node_type == NodeType::Module {
                let lang = node.lang.as_deref().unwrap_or("unknown");
                let entry = lang_stats.entry(lang.to_string()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += node.lines.unwrap_or(0);
            }
        }

        // Top-level directory structure
        let mut dir_info: HashMap<String, (usize, HashSet<String>)> = HashMap::new(); // (files, languages)
        for node in &self.data.nodes {
            if node.node_type == NodeType::Module && !node.path.is_empty() {
                let parts: Vec<&str> = node.path.split('/').collect();
                let dir = if parts.len() > 1 {
                    format!("{}/", parts[0])
                } else {
                    "(root)".to_string()
                };
                let entry = dir_info.entry(dir).or_insert((0, HashSet::new()));
                entry.0 += 1;
                if let Some(lang) = &node.lang {
                    entry.1.insert(lang.clone());
                }
            }
        }

        let mut out = vec!["=== Project Overview ===".to_string()];

        // Languages line (sorted by lines desc)
        let mut lang_list: Vec<_> = lang_stats.iter().collect();
        lang_list.sort_by(|a, b| b.1.1.cmp(&a.1.1));
        let lang_strs: Vec<String> = lang_list
            .iter()
            .map(|(lang, (files, lines))| format!("{} ({} files, {} lines)", lang, files, lines))
            .collect();
        out.push(format!("Languages: {}", lang_strs.join(", ")));

        let total_files: usize = lang_stats.values().map(|(f, _)| f).sum();
        let total_lines: usize = lang_stats.values().map(|(_, l)| l).sum();
        out.push(format!(
            "Total: {} files, {} lines",
            total_files, total_lines
        ));
        out.push(String::new());

        // Top-level structure (sorted by file count desc)
        out.push("Top-level structure:".to_string());
        let mut dir_list: Vec<_> = dir_info.iter().collect();
        dir_list.sort_by(|a, b| b.1.0.cmp(&a.1.0));

        for (dir, (files, langs)) in &dir_list {
            let mut lang_names: Vec<&str> = langs.iter().map(|s| s.as_str()).collect();
            lang_names.sort();
            out.push(format!(
                "  {:20} {:>3} files  ({})",
                dir,
                files,
                lang_names.join(", ")
            ));
        }

        if lang_stats.is_empty() {
            out.push("(no source files found)".to_string());
        }

        out.join("\n")
    }

    /// --tree: Annotated file tree showing language and line counts.
    pub fn query_tree(&self) -> String {
        // Collect and sort Module nodes by path
        let mut modules: Vec<_> = self
            .data
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Module && !n.path.is_empty())
            .collect();
        modules.sort_by(|a, b| a.path.cmp(&b.path));

        let mut out = vec!["=== File Tree ===".to_string()];

        // Compute dynamic alignment width from actual paths
        let max_width = modules
            .iter()
            .map(|n| {
                let parts: Vec<&str> = n.path.split('/').collect();
                let depth = parts.len().saturating_sub(1);
                depth * 2 + parts.last().map_or(0, |f| f.len())
            })
            .max()
            .unwrap_or(30)
            + 2; // +2 for minimum gap

        // Track which directories have been printed
        let mut printed_dirs: HashSet<String> = HashSet::new();

        for node in &modules {
            let parts: Vec<&str> = node.path.split('/').collect();

            // Print directory headers as needed
            for depth in 0..parts.len().saturating_sub(1) {
                let dir_path = parts[..=depth].join("/");
                if !printed_dirs.contains(&dir_path) {
                    let indent = "  ".repeat(depth);
                    out.push(format!("{}{}/", indent, parts[depth]));
                    printed_dirs.insert(dir_path);
                }
            }

            // Print file with annotation
            let depth = parts.len().saturating_sub(1);
            let indent = "  ".repeat(depth);
            let filename = parts.last().unwrap_or(&"");
            let lang = node.lang.as_deref().unwrap_or("?");
            let lines = node.lines.unwrap_or(0);
            let file_part = format!("{}{}", indent, filename);
            let padding = if file_part.len() < max_width {
                " ".repeat(max_width - file_part.len())
            } else {
                "  ".to_string()
            };
            out.push(format!(
                "{}{}[{}, {} lines]",
                file_part, padding, lang, lines
            ));
        }

        if modules.is_empty() {
            out.push("(no source files found)".to_string());
        }

        out.join("\n")
    }

    /// --search: Search for symbol definitions by name (case-insensitive substring).
    pub fn query_search(&self, pattern: &str) -> String {
        let pattern_lower = pattern.to_lowercase();
        let mut matches: Vec<(&str, &str, Option<usize>, &str)> = Vec::new(); // (type_label, path, start_line, label)

        for node in &self.data.nodes {
            if node.label.to_lowercase().contains(&pattern_lower) {
                let type_label = match node.node_type {
                    NodeType::Module => "Module",
                    NodeType::Class => "Class",
                    NodeType::Function => "Func",
                };
                matches.push((type_label, &node.path, node.start_line, &node.label));
            }
        }

        let mut out = vec![format!("=== Search: \"{}\" ===", pattern)];

        if matches.is_empty() {
            out.push("No matches found.".to_string());
            return out.join("\n");
        }

        out.push(format!("Found {} matches:", matches.len()));
        out.push(String::new());

        // Sort by path then start_line
        matches.sort_by(|a, b| a.1.cmp(b.1).then_with(|| a.2.cmp(&b.2)));

        for (type_label, path, start_line, label) in &matches {
            let line_str = start_line.map_or(String::new(), |l| format!(":{}", l));
            out.push(format!(
                "  {:6} {}{}\t{}",
                type_label, path, line_str, label
            ));
        }

        out.join("\n")
    }

    /// --deps: Transitive dependency chain (BFS with depth tracking and cycle detection).
    pub fn query_deps(
        &self,
        file_query: &str,
        max_depth: usize,
        show_upstream: bool,
        show_downstream: bool,
    ) -> String {
        let mid = match self.resolve_to_module_id(file_query) {
            Some(m) => m,
            None => return format!("[NOT FOUND] No module matching '{}'", file_query),
        };

        let idx = self.nodes_by_id[&mid];
        let path = &self.data.nodes[idx].path;

        let mut out = vec![format!("=== Dependency chain: {} ===", path), String::new()];

        let mut total_upstream = 0;
        let mut total_downstream = 0;

        if show_upstream {
            out.push("Upstream (transitive imports):".to_string());
            out.push(format!("  {}", mid));
            let mut path_stack = HashSet::new();
            let mut visited = HashSet::new();
            path_stack.insert(mid.clone());
            visited.insert(mid.clone());
            let mut upstream_lines = Vec::new();
            self.format_dep_tree(
                &mid,
                1,
                max_depth,
                &mut path_stack,
                &mut visited,
                &mut upstream_lines,
                true,
            );
            total_upstream = visited.len().saturating_sub(1);
            if upstream_lines.is_empty() {
                out.push("    (none)".to_string());
            } else {
                out.extend(upstream_lines);
            }
            out.push(String::new());

            let forward: HashSet<String> =
                self.imports_forward.get(&mid).cloned().unwrap_or_default();
            let (_, external_forward) = self.classify_imports(&forward);
            if !external_forward.is_empty() {
                out.push("External imports (direct only):".to_string());
                for dep in &external_forward {
                    out.push(format!("  \u{2192} {}", dep));
                }
                out.push(String::new());
            }
        }

        if show_downstream {
            out.push("Downstream (transitive importers):".to_string());
            out.push(format!("  {}", mid));
            let mut path_stack = HashSet::new();
            let mut visited = HashSet::new();
            path_stack.insert(mid.clone());
            visited.insert(mid.clone());
            let mut downstream_lines = Vec::new();
            self.format_dep_tree(
                &mid,
                1,
                max_depth,
                &mut path_stack,
                &mut visited,
                &mut downstream_lines,
                false,
            );
            total_downstream = visited.len().saturating_sub(1);
            if downstream_lines.is_empty() {
                out.push("    (none)".to_string());
            } else {
                out.extend(downstream_lines);
            }
            out.push(String::new());
        }

        out.push(format!(
            "Total internal: {} upstream, {} downstream (max depth: {})",
            total_upstream, total_downstream, max_depth
        ));

        out.join("\n")
    }

    /// Helper: DFS traversal for dependency tree rendering.
    /// `path_stack` tracks current DFS ancestors (for true cycle detection).
    /// `visited` tracks all nodes ever expanded (to avoid duplicate subtrees in diamonds).
    #[allow(clippy::too_many_arguments)]
    fn format_dep_tree(
        &self,
        current: &str,
        depth: usize,
        max_depth: usize,
        path_stack: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        out: &mut Vec<String>,
        forward: bool,
    ) {
        if depth > max_depth {
            return;
        }

        let neighbors = if forward {
            self.internal_imports_forward.get(current)
        } else {
            self.internal_imports_reverse.get(current)
        };

        if let Some(neighbors) = neighbors {
            let mut sorted: Vec<&String> = neighbors.iter().collect();
            sorted.sort();
            for next in sorted {
                let indent = "  ".repeat(depth + 1);
                let arrow = if forward { "\u{2192}" } else { "\u{2190}" };
                let depth_label = if depth == 1 {
                    " (direct)".to_string()
                } else {
                    format!(" (depth {})", depth)
                };
                let path_str = self
                    .resolve_to_path(next)
                    .map(|p| format!("  [{}]", p))
                    .unwrap_or_default();

                if path_stack.contains(next.as_str()) {
                    // True cycle: back-edge to an ancestor in the current DFS path
                    out.push(format!("{}{} {}{} [CYCLE]", indent, arrow, next, path_str));
                } else if visited.contains(next.as_str()) {
                    // Diamond dependency: already expanded in another branch
                    out.push(format!(
                        "{}{} {}{} (already listed)",
                        indent, arrow, next, path_str
                    ));
                } else {
                    out.push(format!(
                        "{}{} {}{}{}",
                        indent, arrow, next, depth_label, path_str
                    ));
                    visited.insert(next.clone());
                    path_stack.insert(next.clone());
                    self.format_dep_tree(
                        next,
                        depth + 1,
                        max_depth,
                        path_stack,
                        visited,
                        out,
                        forward,
                    );
                    path_stack.remove(next.as_str());
                }
            }
        }
    }

    /// --markdown: Fixed-format Markdown summary for LLM/project docs.
    pub fn query_markdown(&self, mode: MarkdownMode) -> String {
        struct DirStat {
            modules: usize,
            classes: usize,
            functions: usize,
            class_names: Vec<String>,
            import_dirs: HashSet<String>,
        }

        struct ClassSection {
            signature: String,
            methods: Vec<String>,
        }

        struct FileSection {
            path: String,
            internal_imports: Vec<String>,
            external_imports: Vec<String>,
            reverse_dep_count: usize,
            classes: Vec<ClassSection>,
            top_functions: Vec<String>,
        }

        let mut modules: Vec<_> = self
            .data
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Module && !n.path.is_empty())
            .collect();
        modules.sort_by(|a, b| a.path.cmp(&b.path));

        let mut top_level_dirs: HashMap<String, usize> = HashMap::new();
        for node in &modules {
            let parts: Vec<&str> = node.path.split('/').collect();
            let dir = if parts.len() > 1 {
                format!("{}/", parts[0])
            } else {
                "(root)".to_string()
            };
            *top_level_dirs.entry(dir).or_insert(0) += 1;
        }

        let format_line_range = |start: Option<usize>, end: Option<usize>| match (start, end) {
            (Some(s), Some(e)) if s == e => format!("L{}", s),
            (Some(s), Some(e)) => format!("L{}-L{}", s, e),
            (Some(s), None) => format!("L{}", s),
            _ => "?".to_string(),
        };

        let dedup_texts = |items: &mut Vec<String>| {
            items.sort();
            items.dedup();
        };
        let dedup_preserving_order = |items: &mut Vec<String>| {
            let mut seen = HashSet::new();
            items.retain(|item| seen.insert(item.clone()));
        };

        let mut dir_stats: HashMap<String, DirStat> = HashMap::new();
        for node in &self.data.nodes {
            if node.path.is_empty() {
                continue;
            }
            let dk = directory_bucket(&node.path);
            let stat = dir_stats.entry(dk).or_insert_with(|| DirStat {
                modules: 0,
                classes: 0,
                functions: 0,
                class_names: Vec::new(),
                import_dirs: HashSet::new(),
            });
            match node.node_type {
                NodeType::Module => {
                    stat.modules += 1;
                }
                NodeType::Class => {
                    stat.classes += 1;
                    stat.class_names.push(node.label.clone());
                }
                NodeType::Function => {
                    stat.functions += 1;
                }
            }
        }

        for (mid, targets) in &self.internal_imports_forward {
            let Some(&src_idx) = self.nodes_by_id.get(mid) else {
                continue;
            };
            let src_node = &self.data.nodes[src_idx];
            if src_node.node_type != NodeType::Module || src_node.path.is_empty() {
                continue;
            }
            let src_dk = directory_bucket(&src_node.path);
            for target in targets {
                if let Some(&target_idx) = self.nodes_by_id.get(target) {
                    let target_node = &self.data.nodes[target_idx];
                    if !target_node.path.is_empty() {
                        let target_dk = directory_bucket(&target_node.path);
                        if target_dk != src_dk
                            && let Some(stat) = dir_stats.get_mut(&src_dk)
                        {
                            stat.import_dirs
                                .insert(target_dk.trim_end_matches('/').to_string());
                        }
                    }
                }
            }
        }

        let mut file_sections = Vec::new();
        for module in &modules {
            let mid = module.id.clone();
            let path = module.path.clone();

            let children_indices = self
                .contains_children
                .get(&mid)
                .cloned()
                .unwrap_or_default();

            let mut classes_with_order: Vec<(usize, ClassSection)> = Vec::new();
            let mut top_functions_with_order: Vec<(usize, String)> = Vec::new();

            for idx in children_indices {
                let node = &self.data.nodes[idx];
                match node.node_type {
                    NodeType::Class => {
                        let method_indices = self
                            .contains_children
                            .get(&node.id)
                            .cloned()
                            .unwrap_or_default();
                        let mut methods: Vec<(usize, String)> = method_indices
                            .iter()
                            .filter_map(|&m_idx| {
                                let m = &self.data.nodes[m_idx];
                                if m.node_type != NodeType::Function {
                                    return None;
                                }
                                Some((
                                    m.start_line.unwrap_or(usize::MAX),
                                    format!(
                                        "`{}` ({})",
                                        m.label,
                                        format_line_range(m.start_line, m.end_line)
                                    ),
                                ))
                            })
                            .collect();
                        methods.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                        let mut method_entries = methods
                            .into_iter()
                            .map(|(_, text)| text)
                            .collect::<Vec<_>>();
                        dedup_preserving_order(&mut method_entries);
                        classes_with_order.push((
                            node.start_line.unwrap_or(usize::MAX),
                            ClassSection {
                                signature: format!(
                                    "`{}` ({})",
                                    node.label,
                                    format_line_range(node.start_line, node.end_line)
                                ),
                                methods: method_entries,
                            },
                        ));
                    }
                    NodeType::Function => {
                        top_functions_with_order.push((
                            node.start_line.unwrap_or(usize::MAX),
                            format!(
                                "`{}` ({})",
                                node.label,
                                format_line_range(node.start_line, node.end_line)
                            ),
                        ));
                    }
                    NodeType::Module => {}
                }
            }

            classes_with_order.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.signature.cmp(&b.1.signature))
            });
            top_functions_with_order.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

            let raw_imports = self.imports_forward.get(&mid).cloned().unwrap_or_default();
            let (internal_imports_raw, external_imports_raw) = self.classify_imports(&raw_imports);
            let mut internal_imports = internal_imports_raw
                .into_iter()
                .filter_map(|(_, resolved)| self.resolve_to_path(&resolved))
                .map(|target_path| format!("`{}`", target_path))
                .collect::<Vec<_>>();
            dedup_texts(&mut internal_imports);

            let mut external_imports = external_imports_raw
                .into_iter()
                .map(|item| format!("`{}`", item))
                .collect::<Vec<_>>();
            dedup_texts(&mut external_imports);

            let reverse_dep_count = self
                .internal_imports_reverse
                .get(&mid)
                .cloned()
                .unwrap_or_default()
                .len();

            let mut classes = Vec::new();
            let mut seen_class_keys = HashSet::new();
            for (_, class) in classes_with_order {
                let class_key = format!("{}::{:?}", class.signature, class.methods);
                if seen_class_keys.insert(class_key) {
                    classes.push(class);
                }
            }

            let mut top_functions = top_functions_with_order
                .into_iter()
                .map(|(_, text)| text)
                .collect::<Vec<_>>();
            dedup_preserving_order(&mut top_functions);

            file_sections.push(FileSection {
                path,
                internal_imports,
                external_imports,
                reverse_dep_count,
                classes,
                top_functions,
            });
        }

        let mut dir_edges: HashMap<(String, String), usize> = HashMap::new();
        let mut file_edges: HashMap<String, Vec<String>> = HashMap::new();
        for (source_module, targets) in &self.internal_imports_forward {
            let Some(source_path) = self.resolve_to_path(source_module) else {
                continue;
            };
            let source_dir = directory_bucket(&source_path);
            let entry = file_edges.entry(source_path.clone()).or_default();
            for target_module in targets {
                let Some(target_path) = self.resolve_to_path(target_module) else {
                    continue;
                };
                let target_dir = directory_bucket(&target_path);
                if source_dir != target_dir {
                    *dir_edges
                        .entry((
                            source_dir.clone(),
                            target_dir.trim_end_matches('/').to_string(),
                        ))
                        .or_insert(0) += 1;
                }
                entry.push(target_path);
            }
        }

        let mut dir_edge_list: Vec<_> = dir_edges.into_iter().collect();
        dir_edge_list.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.0.cmp(&b.0.0))
                .then_with(|| a.0.1.cmp(&b.0.1))
        });

        let mut file_edge_list: Vec<_> = file_edges.into_iter().collect();
        file_edge_list.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, targets) in &mut file_edge_list {
            targets.sort();
            targets.dedup();
        }

        let mut fan_in: Vec<(&String, usize)> = self
            .internal_imports_reverse
            .iter()
            .map(|(target, sources)| (target, sources.len()))
            .collect();
        fan_in.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        fan_in.truncate(10);

        let mut fan_out: Vec<(&String, usize)> = self
            .internal_imports_forward
            .iter()
            .map(|(source, targets)| (source, targets.len()))
            .collect();
        fan_out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        fan_out.truncate(10);

        let mut plain_tree_lines = Vec::new();
        let mut printed_dirs: HashSet<String> = HashSet::new();
        for node in &modules {
            let parts: Vec<&str> = node.path.split('/').collect();
            for depth in 0..parts.len().saturating_sub(1) {
                let dir_path = parts[..=depth].join("/");
                if printed_dirs.insert(dir_path) {
                    let indent = "  ".repeat(depth);
                    plain_tree_lines.push(format!("{}{}/", indent, parts[depth]));
                }
            }
            let depth = parts.len().saturating_sub(1);
            let indent = "  ".repeat(depth);
            let filename = parts.last().unwrap_or(&"");
            plain_tree_lines.push(format!("{}{}", indent, filename));
        }

        let basename =
            |path: &str| -> String { path.rsplit('/').next().unwrap_or(path).to_string() };
        let stem = |path: &str| -> String {
            let base = basename(path);
            base.rsplit_once('.')
                .map(|(name, _)| name.to_string())
                .unwrap_or(base)
        };
        let symbol_count =
            |file: &FileSection| -> usize { file.classes.len() + file.top_functions.len() };
        let summarize_items = |items: &[String], limit: usize| -> String {
            if items.is_empty() {
                return "none".to_string();
            }
            let shown = items.iter().take(limit).cloned().collect::<Vec<_>>();
            let suffix = if items.len() > shown.len() {
                format!(" ... +{}", items.len() - shown.len())
            } else {
                String::new()
            };
            format!("{}{}", shown.join(", "), suffix)
        };
        let is_entry_candidate = |path: &str| {
            matches!(
                stem(path).as_str(),
                "main" | "app" | "server" | "index" | "lib" | "cli"
            )
        };
        let describe_core_file = |file: &FileSection| -> String {
            let base = basename(&file.path);
            let stem_name = stem(&file.path);
            if stem_name == "main" {
                "entrypoint and command dispatch".to_string()
            } else if stem_name == "cli" {
                "CLI surface and command layout".to_string()
            } else if base == "mod.rs" && file.path.contains("/graph/") {
                "graph indexing and dependency model".to_string()
            } else if base == "mod.rs" && file.path.contains("/extract/") {
                "repository scanning and AST extraction pipeline".to_string()
            } else if stem_name == "query" {
                "query implementation and markdown rendering".to_string()
            } else if stem_name == "types" {
                "shared schema and core data types".to_string()
            } else if stem_name == "resolve" {
                "import resolution and alias normalization".to_string()
            } else if file.internal_imports.len() >= 2 {
                format!(
                    "composition file touching {} internal deps",
                    file.internal_imports.len()
                )
            } else if file.reverse_dep_count >= 2 {
                format!(
                    "shared file reused by {} internal files",
                    file.reverse_dep_count
                )
            } else if symbol_count(file) >= 5 {
                format!(
                    "symbol-dense file with {} top-level symbols",
                    symbol_count(file)
                )
            } else if is_entry_candidate(&file.path) {
                "likely entry or orchestration file".to_string()
            } else {
                "supporting implementation file".to_string()
            }
        };
        let core_score = |file: &FileSection| -> i32 {
            let base = basename(&file.path);
            let stem_name = stem(&file.path);
            let mut score = (file.reverse_dep_count as i32) * 9
                + (file.internal_imports.len() as i32) * 10
                + (file.external_imports.len() as i32) * 2
                + (file.classes.len() as i32) * 4
                + (file.top_functions.len() as i32) * 3;
            if stem_name == "main" {
                score += 90;
            } else if stem_name == "cli" {
                score += 60;
            } else if base == "mod.rs" && file.path.contains("/graph/") {
                score += 70;
            } else if base == "mod.rs" && file.path.contains("/extract/") {
                score += 55;
            } else if stem_name == "query" {
                score += 60;
            } else if stem_name == "types" {
                score += 45;
            } else if stem_name == "resolve" {
                score += 35;
            } else if is_entry_candidate(&file.path) {
                score += 25;
            }
            score
        };
        let reading_rank = |file: &FileSection| -> usize {
            let base = basename(&file.path);
            let stem_name = stem(&file.path);
            if stem_name == "main" {
                0
            } else if stem_name == "cli" {
                1
            } else if base == "mod.rs" && file.path.contains("/graph/") {
                2
            } else if base == "mod.rs" && file.path.contains("/extract/") {
                3
            } else if stem_name == "query" {
                4
            } else if stem_name == "types" {
                5
            } else if stem_name == "resolve" {
                6
            } else if file.internal_imports.len() >= 2 {
                7
            } else if file.reverse_dep_count >= 2 {
                8
            } else if symbol_count(file) >= 5 {
                9
            } else {
                10
            }
        };

        let mut core_candidates: Vec<(usize, i32, usize, String)> = file_sections
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                (
                    idx,
                    core_score(file),
                    reading_rank(file),
                    describe_core_file(file),
                )
            })
            .collect();
        core_candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| file_sections[a.0].path.cmp(&file_sections[b.0].path))
        });
        let core_limit = file_sections.len().min(match mode {
            MarkdownMode::Compact => 5,
            MarkdownMode::Default | MarkdownMode::Full => 6,
        });
        core_candidates.truncate(core_limit);

        let core_index_set: HashSet<usize> =
            core_candidates.iter().map(|(idx, _, _, _)| *idx).collect();

        let mut reading_order = core_candidates.clone();
        reading_order.sort_by(|a, b| {
            a.2.cmp(&b.2)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| file_sections[a.0].path.cmp(&file_sections[b.0].path))
        });

        let mut supporting_indices: Vec<usize> = file_sections
            .iter()
            .enumerate()
            .filter_map(|(idx, _)| (!core_index_set.contains(&idx)).then_some(idx))
            .collect();
        supporting_indices.sort_by(|a, b| file_sections[*a].path.cmp(&file_sections[*b].path));

        let dir_class_limit = match mode {
            MarkdownMode::Compact => 5,
            MarkdownMode::Default => 8,
            MarkdownMode::Full => usize::MAX,
        };

        let mut out = vec!["# Project Structure".to_string(), String::new()];

        out.push("## Repository Framework".to_string());
        out.push("### Top-Level Structure".to_string());
        if top_level_dirs.is_empty() {
            out.push("- None".to_string());
        } else {
            let mut dirs: Vec<_> = top_level_dirs.iter().collect();
            dirs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (dir, files) in dirs {
                out.push(format!("- `{}`: {} files", dir, files));
            }
        }
        out.push(String::new());

        out.push("### Directory Architecture".to_string());
        if dir_stats.is_empty() {
            out.push("- None".to_string());
        } else {
            let mut keys: Vec<_> = dir_stats.keys().collect();
            keys.sort();
            for key in keys {
                let stat = &dir_stats[key];
                out.push(format!("#### `{}`", key));
                out.push(format!(
                    "- Files: {} | Classes: {} | Functions: {}",
                    stat.modules, stat.classes, stat.functions
                ));
                if stat.class_names.is_empty() {
                    out.push("- Key classes: none".to_string());
                } else {
                    let mut classes = stat.class_names.clone();
                    classes.sort();
                    classes.dedup();
                    let shown: Vec<&str> = classes
                        .iter()
                        .take(dir_class_limit)
                        .map(|s| s.as_str())
                        .collect();
                    let suffix = if classes.len() > shown.len() {
                        format!(" ... +{}", classes.len() - shown.len())
                    } else {
                        String::new()
                    };
                    out.push(format!("- Key classes: {}{}", shown.join(", "), suffix));
                }
                let mut import_dirs: Vec<&str> =
                    stat.import_dirs.iter().map(|s| s.as_str()).collect();
                import_dirs.sort();
                if import_dirs.is_empty() {
                    out.push("- Imports from: none / external only".to_string());
                } else {
                    out.push(format!("- Imports from: {}", import_dirs.join(", ")));
                }
                out.push(String::new());
            }
        }

        if mode != MarkdownMode::Compact {
            out.push("### File Tree".to_string());
            out.push("```text".to_string());
            if plain_tree_lines.is_empty() {
                out.push("(no source files found)".to_string());
            } else {
                out.push(plain_tree_lines.join("\n"));
            }
            out.push("```".to_string());
            out.push(String::new());
        }

        out.push("## Dependency Relationships".to_string());
        out.push("### Directory Dependencies".to_string());
        if dir_edge_list.is_empty() {
            out.push("- No internal directory dependencies resolved.".to_string());
        } else {
            for ((source_dir, target_dir), count) in &dir_edge_list {
                out.push(format!(
                    "- `{}` -> `{}` ({} internal imports)",
                    source_dir, target_dir, count
                ));
            }
        }
        out.push(String::new());

        if mode != MarkdownMode::Compact {
            out.push("### Internal File Dependencies".to_string());
            if file_edge_list.is_empty() {
                out.push("- No internal file dependencies resolved.".to_string());
            } else {
                for (source_path, targets) in &file_edge_list {
                    out.push(format!(
                        "- `{}` -> {}",
                        source_path,
                        targets
                            .iter()
                            .map(|path| format!("`{}`", path))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            out.push(String::new());
        }

        out.push("### Hub Modules".to_string());
        out.push("#### Fan-In".to_string());
        out.push("- Meaning: how many internal files depend on this file.".to_string());
        if fan_in.is_empty() {
            out.push("- None".to_string());
        } else {
            for (module_id, count) in &fan_in {
                let path = self
                    .resolve_to_path(module_id)
                    .unwrap_or_else(|| module_id.to_string());
                out.push(format!("- `{}` ({})", path, count));
            }
        }
        out.push(String::new());

        out.push("#### Fan-Out".to_string());
        out.push("- Meaning: how many internal files this file depends on.".to_string());
        if fan_out.is_empty() {
            out.push("- None".to_string());
        } else {
            for (module_id, count) in &fan_out {
                let path = self
                    .resolve_to_path(module_id)
                    .unwrap_or_else(|| module_id.to_string());
                out.push(format!("- `{}` ({})", path, count));
            }
        }
        out.push(String::new());

        out.push("## Suggested Reading Order".to_string());
        if reading_order.is_empty() {
            out.push("- No source files found.".to_string());
        } else {
            for (position, (idx, _, _, reason)) in reading_order.iter().take(5).enumerate() {
                out.push(format!(
                    "{}. `{}` — {}",
                    position + 1,
                    file_sections[*idx].path,
                    reason
                ));
            }
        }
        out.push(String::new());

        out.push("## File Symbol Index".to_string());
        match mode {
            MarkdownMode::Compact => {
                out.push("### Core Files".to_string());
                if reading_order.is_empty() {
                    out.push("- None".to_string());
                } else {
                    for (idx, _, _, reason) in &reading_order {
                        let file = &file_sections[*idx];
                        let class_signatures = file
                            .classes
                            .iter()
                            .map(|class| class.signature.clone())
                            .collect::<Vec<_>>();
                        out.push(format!("#### `{}`", file.path));
                        out.push(format!("- Why: {}", reason));
                        out.push(format!(
                            "- Links: internal {} | external {} | reverse {}",
                            summarize_items(&file.internal_imports, 4),
                            summarize_items(&file.external_imports, 4),
                            file.reverse_dep_count
                        ));
                        out.push(format!(
                            "- Symbols: cls {} | fns {}",
                            summarize_items(&class_signatures, 4),
                            summarize_items(&file.top_functions, 6)
                        ));
                        out.push(String::new());
                    }
                }
            }
            MarkdownMode::Default => {
                out.push("### Core Files".to_string());
                if reading_order.is_empty() {
                    out.push("- None".to_string());
                } else {
                    for (idx, _, _, reason) in &reading_order {
                        let file = &file_sections[*idx];
                        out.push(format!("#### `{}`", file.path));
                        out.push(format!("- Why: {}", reason));
                        out.push(format!(
                            "- Connections: internal {} | external {} | reverse {}",
                            summarize_items(&file.internal_imports, 6),
                            summarize_items(&file.external_imports, 6),
                            file.reverse_dep_count
                        ));
                        if file.classes.is_empty() {
                            out.push("- Classes: none".to_string());
                        } else {
                            out.push("- Classes:".to_string());
                            for class in &file.classes {
                                if class.methods.is_empty() {
                                    out.push(format!("  - {}", class.signature));
                                } else {
                                    out.push(format!(
                                        "  - {}: {}",
                                        class.signature,
                                        summarize_items(&class.methods, 8)
                                    ));
                                }
                            }
                        }
                        out.push(format!(
                            "- Top-level functions: {}",
                            summarize_items(&file.top_functions, 10)
                        ));
                        out.push(String::new());
                    }
                }

                out.push("### Supporting Files".to_string());
                if supporting_indices.is_empty() {
                    out.push("- None".to_string());
                } else {
                    for idx in supporting_indices {
                        let file = &file_sections[idx];
                        let class_signatures = file
                            .classes
                            .iter()
                            .map(|class| class.signature.clone())
                            .collect::<Vec<_>>();
                        out.push(format!(
                            "- `{}` — cls: {}; fns: {}",
                            file.path,
                            summarize_items(&class_signatures, 6),
                            summarize_items(&file.top_functions, 8)
                        ));
                    }
                }
            }
            MarkdownMode::Full => {
                out.push("### All Files".to_string());
                if file_sections.is_empty() {
                    out.push("- None".to_string());
                } else {
                    for file in &file_sections {
                        out.push(format!("#### `{}`", file.path));
                        out.push(format!(
                            "- Connections: internal {} | external {} | reverse {}",
                            summarize_items(&file.internal_imports, usize::MAX),
                            summarize_items(&file.external_imports, usize::MAX),
                            file.reverse_dep_count
                        ));
                        if file.classes.is_empty() {
                            out.push("- Classes: none".to_string());
                        } else {
                            out.push("- Classes:".to_string());
                            for class in &file.classes {
                                out.push(format!("  - {}", class.signature));
                                if class.methods.is_empty() {
                                    out.push("    - Methods: none".to_string());
                                } else {
                                    out.push("    - Methods:".to_string());
                                    for method in &class.methods {
                                        out.push(format!("      - {}", method));
                                    }
                                }
                            }
                        }
                        if file.top_functions.is_empty() {
                            out.push("- Top-level functions: none".to_string());
                        } else {
                            out.push("- Top-level functions:".to_string());
                            for function in &file.top_functions {
                                out.push(format!("  - {}", function));
                            }
                        }
                        out.push(String::new());
                    }
                }
            }
        }

        if let Some(errors) = &self.data.errors
            && !errors.is_empty()
        {
            out.push(String::new());
            out.push("## Extraction Notes".to_string());
            for error in errors.iter().take(10) {
                out.push(format!("- {}", error));
            }
        }

        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::types::{Edge, EdgeType, GraphData, Node, Stats};
    use std::collections::HashMap;

    #[test]
    fn directory_bucket_uses_root_for_root_level_files() {
        assert_eq!(directory_bucket("main.py"), "(root)/");
        assert_eq!(directory_bucket("src/main.rs"), "src/");
        assert_eq!(directory_bucket("src/graph/query.rs"), "src/graph/");
    }

    #[test]
    fn query_summary_groups_root_level_files_under_root() {
        let data = GraphData {
            languages: vec!["python".to_string()],
            stats: Stats {
                total_files: 2,
                total_lines: 2,
                parse_errors: 0,
                truncated: false,
                truncated_nodes: 0,
                supported_file_counts: HashMap::new(),
                languages_with_structural_queries: vec!["python".to_string()],
            },
            nodes: vec![
                Node {
                    id: "main".to_string(),
                    node_type: NodeType::Module,
                    label: "main".to_string(),
                    path: "main.py".to_string(),
                    lang: Some("python".to_string()),
                    lines: Some(1),
                    parent: None,
                    start_line: None,
                    end_line: None,
                },
                Node {
                    id: "util".to_string(),
                    node_type: NodeType::Module,
                    label: "util".to_string(),
                    path: "util.py".to_string(),
                    lang: Some("python".to_string()),
                    lines: Some(1),
                    parent: None,
                    start_line: None,
                    end_line: None,
                },
            ],
            edges: vec![Edge {
                source: "main".to_string(),
                target: "util".to_string(),
                edge_type: EdgeType::Imports,
            }],
            warnings: None,
            errors: None,
        };

        let graph = ASTGraph::new(data);
        let summary = graph.query_summary();

        assert!(summary.contains("(root)/ (2 modules"));
        assert!(!summary.contains("main.py/"));
        assert!(!summary.contains("util.py/"));
    }
}
