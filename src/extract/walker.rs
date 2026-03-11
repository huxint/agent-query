use crate::output;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const MAX_SOURCE_FILE_BYTES: u64 = 1_048_576;

/// Directories to always exclude (supplement .gitignore for non-git repos)
const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "__pycache__",
    ".venv",
    "venv",
    "node_modules",
    "dist",
    "build",
    ".mypy_cache",
    ".pytest_cache",
    "site-packages",
    ".nexus-map",
    ".tox",
    ".eggs",
    "target",
    "cmake-build-debug",
    ".vs",
    "out",
    "_build",
    "vendor",
    ".ruff_cache",
    ".godot",
    ".idea",
    ".vscode",
    ".nox",
];

/// File suffixes to exclude
const EXCLUDE_FILE_SUFFIXES: &[&str] = &[".import", ".vulkan.cache"];

/// Check if a path should be skipped during traversal.
fn should_skip_path(rel_path: &Path) -> bool {
    for component in rel_path.components() {
        if let std::path::Component::Normal(name) = component
            && let Some(name_str) = name.to_str()
            && EXCLUDE_DIRS.contains(&name_str)
        {
            return true;
        }
    }
    if let Some(name) = rel_path.file_name().and_then(|n| n.to_str()) {
        for suffix in EXCLUDE_FILE_SUFFIXES {
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

/// Collect all source files in the repo that have a known language mapping.
/// Respects .gitignore when inside a git repository.
/// Returns (files, supported_counts) where files is Vec<(path, lang_name)>.
pub fn collect_source_files(
    repo_path: &Path,
    extension_map: &std::collections::HashMap<&str, &str>,
    available_languages: &HashSet<&str>,
) -> (
    Vec<(PathBuf, String)>,
    std::collections::HashMap<String, usize>,
) {
    let mut files = Vec::new();
    let mut supported_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    let walker = WalkBuilder::new(repo_path)
        .hidden(true) // skip hidden files/dirs by default
        .git_ignore(true) // respect .gitignore
        .git_global(true) // respect global gitignore
        .git_exclude(true) // respect .git/info/exclude
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let rel_path = match path.strip_prefix(repo_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Additional exclusions beyond .gitignore
        if should_skip_path(rel_path) {
            continue;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => format!(".{}", e.to_lowercase()),
            None => continue,
        };

        if let Some(&lang) = extension_map.get(ext.as_str())
            && available_languages.contains(lang)
        {
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(err) => {
                    output::warn(&format!(
                        "Warning: skipping {} due to metadata error: {}",
                        path.display(),
                        err
                    ));
                    continue;
                }
            };

            if metadata.len() > MAX_SOURCE_FILE_BYTES {
                output::warn(&format!(
                    "Warning: skipping large file ({} bytes > {} bytes): {}",
                    metadata.len(),
                    MAX_SOURCE_FILE_BYTES,
                    path.display()
                ));
                continue;
            }

            files.push((path.to_path_buf(), lang.to_string()));
            *supported_counts.entry(lang.to_string()).or_insert(0) += 1;
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    (files, supported_counts)
}
