use anyhow::{Context, anyhow};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

#[derive(Default)]
struct HotspotStat {
    revisions: usize,
    added: usize,
    deleted: usize,
    authors: HashSet<String>,
}

impl HotspotStat {
    fn churn(&self) -> usize {
        self.added + self.deleted
    }
}

pub fn query_hotspots(
    repo_path: Option<&str>,
    days: Option<u32>,
    top: usize,
) -> anyhow::Result<String> {
    let repo = Path::new(repo_path.unwrap_or(".")).canonicalize()?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&repo)
        .arg("log")
        .arg("--numstat")
        .arg("--format=commit%x09%H%x09%an%x09%ad")
        .arg("--date=short")
        .arg("--no-renames");

    if let Some(days) = days {
        command.arg(format!("--since={} days ago", days));
    }

    let output = command
        .output()
        .with_context(|| format!("Failed to run git log in {}", repo.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("does not have any commits yet") {
            return Ok(format!(
                "=== Git Hotspots ===\n\nRepository: {}\nWindow: {}\nCommits analyzed: 0\nFiles touched: 0\nRanking: revisions desc, then churn desc\n\nNo commits found in repository history.\nThis command needs a Git repository with at least one commit.",
                repo.display(),
                match days {
                    Some(days) => format!("last {} days", days),
                    None => "all history".to_string(),
                }
            ));
        }
        return Err(anyhow!(
            "git log failed for {}{}",
            repo.display(),
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr)
            }
        ));
    }

    let stdout = String::from_utf8(output.stdout).context("git log returned non-UTF-8 output")?;
    let mut stats: HashMap<String, HotspotStat> = HashMap::new();
    let mut commit_count = 0usize;
    let mut current_author = None::<String>;

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("commit\t") {
            commit_count += 1;
            let parts = rest.splitn(3, '\t').collect::<Vec<_>>();
            current_author = parts.get(1).map(|author| (*author).to_string());
            continue;
        }

        if line.trim().is_empty() {
            continue;
        }

        let parts = line.splitn(3, '\t').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }

        let added = match parts[0].parse::<usize>() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let deleted = match parts[1].parse::<usize>() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = parts[2].trim();
        if path.is_empty() {
            continue;
        }

        let stat = stats.entry(path.to_string()).or_default();
        stat.revisions += 1;
        stat.added += added;
        stat.deleted += deleted;
        if let Some(author) = &current_author {
            stat.authors.insert(author.clone());
        }
    }

    let total_files_touched = stats.len();
    let mut rows = stats.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.1.revisions
            .cmp(&a.1.revisions)
            .then_with(|| b.1.churn().cmp(&a.1.churn()))
            .then_with(|| a.0.cmp(&b.0))
    });
    rows.truncate(top);

    let mut out = vec!["=== Git Hotspots ===".to_string(), String::new()];
    out.push(format!("Repository: {}", repo.display()));
    out.push(format!(
        "Window: {}",
        match days {
            Some(days) => format!("last {} days", days),
            None => "all history".to_string(),
        }
    ));
    out.push(format!("Commits analyzed: {}", commit_count));
    out.push(format!("Files touched: {}", total_files_touched));
    out.push("Ranking: revisions desc, then churn desc".to_string());
    out.push(String::new());

    if commit_count == 0 {
        out.push("No commits found in repository history.".to_string());
        out.push("This command needs a Git repository with at least one commit.".to_string());
        return Ok(out.join("\n"));
    }

    if rows.is_empty() {
        out.push("No changed files matched the selected history window.".to_string());
        return Ok(out.join("\n"));
    }

    for (index, (path, stat)) in rows.iter().enumerate() {
        out.push(format!(
            "{}. `{}` — rev {} | churn {} (+{}/-{}) | authors {}",
            index + 1,
            path,
            stat.revisions,
            stat.churn(),
            stat.added,
            stat.deleted,
            stat.authors.len()
        ));
    }

    Ok(out.join("\n"))
}
