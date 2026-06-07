use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::util::output::{status, warn};

const SKILL: &str = include_str!("../../skill/SKILL.md");

const SKILL_NAME: &str = "render";

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Agent {
    #[value(name = "claude", alias = "claude-code")]
    ClaudeCode,
    Pi,
    Generic,
}

impl Agent {
    const ALL: [Agent; 3] = [Agent::ClaudeCode, Agent::Pi, Agent::Generic];

    fn skills_dir(self, home: &Path) -> PathBuf {
        let parts: &[&str] = match self {
            Agent::ClaudeCode => &[".claude", "skills"],
            Agent::Pi => &[".pi", "agent", "skills"],
            Agent::Generic => &[".agent", "skills"],
        };
        parts
            .iter()
            .fold(home.to_path_buf(), |dir, part| dir.join(part))
    }

    /// True when the agent's own dir (the parent of `skills/`) exists — seeds
    /// the install default set.
    fn detected(self, home: &Path) -> bool {
        self.skills_dir(home).parent().is_some_and(Path::exists)
    }
}

/// Dry run unless `execute`; `--execute` replaces each target `render/` wholesale.
pub(crate) fn setup(execute: bool, agents: &[Agent]) -> Result<()> {
    let home = home_dir()?;
    let selected = resolve_agents(&home, agents, Op::Install);
    for agent in &selected {
        install(&agent.skills_dir(&home), execute)?;
    }
    finish(Op::Install, execute, selected.len());
    Ok(())
}

/// Symmetric to [`setup`]. Dry run unless `execute`.
pub(crate) fn teardown(execute: bool, agents: &[Agent]) -> Result<()> {
    let home = home_dir()?;
    let selected = resolve_agents(&home, agents, Op::Remove);
    for agent in &selected {
        remove(&agent.skills_dir(&home), execute)?;
    }
    finish(Op::Remove, execute, selected.len());
    Ok(())
}

/// Lets install vs remove pick the right default agents (and the `Finished`
/// summary read correctly).
#[derive(Clone, Copy)]
enum Op {
    Install,
    Remove,
}

impl Op {
    fn gerund(self) -> &'static str {
        match self {
            Op::Install => "installing",
            Op::Remove => "removing",
        }
    }
}

/// Explicit `--agent` flags win; otherwise the default set for `op`.
fn resolve_agents(home: &Path, agents: &[Agent], op: Op) -> Vec<Agent> {
    if agents.is_empty() {
        default_agents(home, op)
    } else {
        dedup(agents)
    }
}

/// For `Install`, agents detected as present; for `Remove`, agents that
/// actually have `/render` installed.
fn default_agents(home: &Path, op: Op) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|agent| match op {
            Op::Install => agent.detected(home),
            Op::Remove => has_skill(*agent, home),
        })
        .collect()
}

fn has_skill(agent: Agent, home: &Path) -> bool {
    let dir = agent.skills_dir(home).join(SKILL_NAME);
    dir.is_symlink() || dir.exists()
}

/// Preserves order, unlike a `HashSet`.
fn dedup(agents: &[Agent]) -> Vec<Agent> {
    let mut out: Vec<Agent> = Vec::new();
    for &agent in agents {
        if !out.contains(&agent) {
            out.push(agent);
        }
    }
    out
}

fn finish(op: Op, execute: bool, count: usize) {
    if count == 0 {
        warn("no agents selected — pass --agent claude|pi|generic");
    } else if execute {
        status(
            "Finished",
            &format!("{} /{SKILL_NAME} · {count} agent(s)", op.gerund()),
        );
    } else {
        warn("dry run; re-run with --execute to apply");
    }
}

/// `--execute` removes any existing `render/` (symlink or dir) first, then
/// writes fresh — so a stale dir or a foreign symlink never lingers.
fn install(skills_dir: &Path, execute: bool) -> Result<()> {
    let dir = skills_dir.join(SKILL_NAME);
    let file = dir.join("SKILL.md");
    status("Installing", &format!("/{SKILL_NAME} ({})", file.display()));
    if !execute {
        return Ok(());
    }
    remove_existing(&dir)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(&file, SKILL).with_context(|| format!("writing {}", file.display()))?;
    Ok(())
}

fn remove(skills_dir: &Path, execute: bool) -> Result<()> {
    let dir = skills_dir.join(SKILL_NAME);
    let present = dir.is_symlink() || dir.exists();
    if !present {
        status(
            "Skipping",
            &format!("/{SKILL_NAME} (not present at {})", dir.display()),
        );
        return Ok(());
    }
    status("Removing", &format!("/{SKILL_NAME} ({})", dir.display()));
    if !execute {
        return Ok(());
    }
    remove_existing(&dir)?;
    Ok(())
}

/// `is_symlink` is checked first so a symlinked dir is unlinked (without
/// touching its target), not recursed into.
fn remove_existing(path: &Path) -> Result<()> {
    if path.is_symlink() || path.is_file() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    } else if path.is_dir() {
        std::fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => bail!("$HOME is not set; cannot locate the agent skills directories"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, SKILL};
    use std::path::Path;

    #[test]
    fn embedded_skill_has_render_frontmatter() {
        assert!(SKILL.starts_with("---"));
        assert!(SKILL.contains("name: render"));
        assert!(SKILL.contains("allowed-tools: Bash(ah-r:*)"));
    }

    #[test]
    fn agent_skills_dirs_are_under_home() {
        let home = Path::new("/home/x");
        assert!(
            Agent::ClaudeCode
                .skills_dir(home)
                .ends_with(".claude/skills")
        );
        assert!(Agent::Pi.skills_dir(home).ends_with(".pi/agent/skills"));
        assert!(Agent::Generic.skills_dir(home).ends_with(".agent/skills"));
    }
}
