---
name: agent-query
description: Analyze source repositories with the bundled `agent-query` CLI. Use when you needs fast repository structure, dependency, symbol, hub, Git hotspot, or fixed-format Markdown summary information for a local codebase, especially when repeated queries should reuse a scanned AST JSON instead of rescanning the repo.
---

# Agent Query

## Overview

Run the bundled `agent-query` binary to inspect a repository's structure without hand-walking files. Prefer it when the task is architectural understanding, dependency tracing, symbol lookup, or generating an LLM-friendly project summary.

## Binary

Run:

```bash
{SKILL_BASE_DIR}/bin/agent-query
```

Pass `--repo <path>` to analyze another repository. Omit it to analyze the current directory.

## Recommended Workflow

Start with a broad pass, then narrow down:

```bash
# Big picture
{SKILL_BASE_DIR}/bin/agent-query overview --repo /path/to/repo
{SKILL_BASE_DIR}/bin/agent-query tree --repo /path/to/repo
{SKILL_BASE_DIR}/bin/agent-query hub --repo /path/to/repo --top 10

# Drill into a specific file
{SKILL_BASE_DIR}/bin/agent-query file --repo /path/to/repo src/main.rs
{SKILL_BASE_DIR}/bin/agent-query deps --repo /path/to/repo src/main.rs --depth 2
```

Use `summary` when directory-level architecture is enough. Use `search` when you need symbol definitions rather than raw text matches.

## Reusable AST Workflow

Avoid rescanning when you expect multiple queries:

```bash
{SKILL_BASE_DIR}/bin/agent-query scan --repo /path/to/repo -o ast.json

{SKILL_BASE_DIR}/bin/agent-query overview --ast ast.json
{SKILL_BASE_DIR}/bin/agent-query deps --ast ast.json src/main.rs --depth 2
{SKILL_BASE_DIR}/bin/agent-query markdown --ast ast.json --mode default --save structure.md
```

Use `scan` for scripts, repeated analysis, or when another step needs a stable JSON snapshot.

## Markdown Modes

- `compact`: high-level architecture for tight token budgets
- `default`: balanced summary for most LLM context packs
- `full`: every source file expanded with classes, methods, functions, and dependency notes

Prefer `default` unless the user explicitly needs the smallest or most exhaustive output.

## Commands

- `overview`: languages, file counts, top-level structure
- `tree`: repository file tree
- `file`: classes, methods, functions, imports for one file
- `search`: symbol definition lookup
- `deps`: upstream and downstream internal dependencies
- `hub`: high fan-in and fan-out files
- `summary`: directory-level architecture summary
- `hotspots`: Git change hotspots for repositories with commit history
- `markdown`: fixed-format project summary in Markdown
- `scan`: AST JSON snapshot for reuse

## Notes

- Keep file paths relative to the analyzed repository root.
- Prefer `agent-query search` for definitions and `rg` for raw text.
- Use `hotspots` only when the repository has meaningful Git history.
- Save long outputs with `--save <path>` when another step needs a file artifact.

## Bundled Asset

The release binary is bundled at `{SKILL_BASE_DIR}/bin/agent-query`.
