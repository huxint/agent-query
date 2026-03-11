mod cli;
mod extract;
mod graph;
mod hotspots;
mod output;

use clap::Parser;
use cli::{Cli, Command, DepDirection, MarkdownModeArg, SourceArgs};
use graph::ASTGraph;
use graph::query::MarkdownMode;
use graph::types::GraphData;
use std::path::Path;

fn load_graph_data(path: &str) -> anyhow::Result<GraphData> {
    let text = std::fs::read_to_string(path)?;
    let json_start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("No JSON object found in {}", path))?;
    Ok(serde_json::from_str(&text[json_start..])?)
}

fn load_graph(source: &SourceArgs, max_nodes: usize) -> anyhow::Result<ASTGraph> {
    if let Some(ast_path) = source.ast.as_deref() {
        let data = load_graph_data(ast_path)?;
        return Ok(ASTGraph::new(data));
    }

    let repo_path = source.repo.as_deref().unwrap_or(".");
    let repo = Path::new(repo_path).canonicalize()?;
    let data = extract::extract_repo(&repo, max_nodes)?;
    Ok(ASTGraph::new(data))
}

fn scan_repo(repo_path: Option<&str>, max_nodes: usize) -> anyhow::Result<String> {
    let repo = Path::new(repo_path.unwrap_or(".")).canonicalize()?;
    let data = extract::extract_repo(&repo, max_nodes)?;
    Ok(serde_json::to_string_pretty(&data)?)
}

fn dep_direction_flags(direction: &DepDirection) -> (bool, bool) {
    match direction {
        DepDirection::Both => (true, true),
        DepDirection::Up => (true, false),
        DepDirection::Down => (false, true),
    }
}

fn markdown_mode(mode: &MarkdownModeArg) -> MarkdownMode {
    match mode {
        MarkdownModeArg::Compact => MarkdownMode::Compact,
        MarkdownModeArg::Default => MarkdownMode::Default,
        MarkdownModeArg::Full => MarkdownMode::Full,
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    output::set_quiet(cli.quiet);

    match cli.command {
        Command::Overview {
            source,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_overview();
            output::write_output(&result, save.as_deref())?;
        }

        Command::Tree {
            source,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_tree();
            output::write_output(&result, save.as_deref())?;
        }

        Command::File {
            file_path,
            source,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_file(&file_path);
            output::write_output(&result, save.as_deref())?;
        }

        Command::Search {
            pattern,
            source,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_search(&pattern);
            output::write_output(&result, save.as_deref())?;
        }

        Command::Deps {
            file_path,
            source,
            depth,
            direction,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let (up, down) = dep_direction_flags(&direction);
            let result = graph.query_deps(&file_path, depth, up, down);
            output::write_output(&result, save.as_deref())?;
        }

        Command::Hub {
            source,
            top,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_hub_analysis(top);
            output::write_output(&result, save.as_deref())?;
        }

        Command::Summary {
            source,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_summary();
            output::write_output(&result, save.as_deref())?;
        }

        Command::Hotspots {
            repo,
            days,
            top,
            save,
        } => {
            let result = hotspots::query_hotspots(repo.as_deref(), days, top)?;
            output::write_output(&result, save.as_deref())?;
        }

        Command::Markdown {
            source,
            mode,
            max_nodes,
            save,
        } => {
            let graph = load_graph(&source, max_nodes)?;
            let result = graph.query_markdown(markdown_mode(&mode));
            output::write_output(&result, save.as_deref())?;
        }

        Command::Scan {
            repo,
            output,
            max_nodes,
        } => {
            let json = scan_repo(repo.as_deref(), max_nodes)?;
            output::write_output(&json, output.as_deref())?;
        }
    }

    Ok(())
}
