<p align="center">
  <strong>English</strong> | <a href="./README.zh-CN.md">中文</a>
</p>

# agent-query

`agent-query` is a repository analysis skill for agents that support local skills.

This repository contains the installable skill bundle in `skills/agent-query/`. The bundle includes a prebuilt `agent-query` binary and the skill instructions that tell an agent when to use it.

## Why use it

Models are usually strong once they are in the right files. They are much less reliable during the first pass over a medium or large codebase.

Without a structural tool, an agent often:

- opens files at random
- guesses the architecture from names alone
- misses entrypoints and hub modules
- edits a leaf module before understanding its dependencies
- spends too much context on repo discovery instead of the actual task

`agent-query` gives the agent that missing first-pass map.

## What the skill gives you

- An installable skill package for agents that support local skills.
- A bundled binary for fast repository analysis.
- Better repo context before editing, reviewing, or planning work.
- Faster answers to questions about structure, dependencies, symbols, hotspots, and summaries.

## Install

Clone the repository:

```bash
git clone https://github.com/huxint/agent-query.git
cd agent-query
```

Copy the skill into your agent's skills directory:

```bash
mkdir -p "$AGENT_SKILLS_DIR"
cp -R skills/agent-query "$AGENT_SKILLS_DIR/agent-query"
```

If your environment uses a Codex-compatible path, the equivalent location is:

```bash
mkdir -p ~/.codex/skills
cp -R skills/agent-query ~/.codex/skills/agent-query
```

## Invoke the skill

The exact syntax depends on your agent.

- In Codex-style environments, invoke it as `$agent-query`.
- In other agents, call the installed skill by name using that agent's skill syntax.

Typical requests:

- `Use agent-query to inspect this repository before making changes.`
- `Use agent-query to tell me which files I should read first.`
- `Use agent-query to summarize the architecture of this repository.`
- `Use agent-query to trace the dependencies around this module.`

## When it helps most

- The repository is large enough that manual file walking is slow and noisy.
- You want the agent to choose files based on structure instead of guesswork.
- You want a reusable structural layer before code edits or reviews.

## Scope

This repository is meant to provide the skill package. It is not intended to be a full CLI manual.
