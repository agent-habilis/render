//! `ahr setup` / `ahr teardown`: install or remove the `/render` agent skill
//! across agents. The skill's `SKILL.md` is embedded at compile time, so a
//! brew/cargo-installed binary carries it with no repo checkout. Both commands
//! are dry-run by default; `--execute` mutates. Agent selection is flag-driven
//! (`--agent`), defaulting to the agents detected on this machine.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use console::style;

/// The portable `/render` skill, baked into the binary from `skill/SKILL.md`.
const SKILL: &str = include_str!("../../skill/SKILL.md");

/// The skill's directory name under each agent's `skills/` dir.
const SKILL_NAME: &str = "render";

/// An agent the `/render` skill can be installed into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Agent {
    /// Claude Code (`~/.claude/skills`).
    #[value(name = "claude", alias = "claude-code")]
    ClaudeCode,
    /// pi (`~/.pi/agent/skills`).
    Pi,
    /// A generic agent following the `~/.agent/skills` convention.
    Generic,
}

impl Agent {
    /// Every agent, in display order.
    const ALL: [Agent; 3] = [Agent::ClaudeCode, Agent::Pi, Agent::Generic];

    /// The agent's skills directory — the parent of its `render/` skill dir.
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

    /// Is the agent present? True when its own dir (the parent of `skills/`)
    /// exists — seeds the install default set.
    fn detected(self, home: &Path) -> bool {
        self.skills_dir(home).parent().is_some_and(Path::exists)
    }
}

/// Install the embedded skill into the selected agents. Dry run unless
/// `execute`; `--execute` replaces each target `render/` wholesale.
///
/// # Errors
/// `$HOME` unset, or a filesystem error while creating a dir or writing.
pub(crate) fn setup(execute: bool, agents: &[Agent]) -> Result<()> {
    let home = home_dir()?;
    let selected = resolve_agents(&home, agents, Op::Install);
    for agent in &selected {
        install(&agent.skills_dir(&home), execute)?;
    }
    finish(Op::Install, execute, selected.len());
    Ok(())
}

/// Remove the skill from the selected agents — symmetric to [`setup`]. Dry run
/// unless `execute`.
///
/// # Errors
/// `$HOME` unset, or a filesystem error while removing.
pub(crate) fn teardown(execute: bool, agents: &[Agent]) -> Result<()> {
    let home = home_dir()?;
    let selected = resolve_agents(&home, agents, Op::Remove);
    for agent in &selected {
        remove(&agent.skills_dir(&home), execute)?;
    }
    finish(Op::Remove, execute, selected.len());
    Ok(())
}

/// Which operation the default set is for, so install vs remove pick the right
/// agents (and the `Finished` summary reads correctly).
#[derive(Clone, Copy)]
enum Op {
    Install,
    Remove,
}

impl Op {
    /// Gerund for the `Finished` summary line.
    fn gerund(self) -> &'static str {
        match self {
            Op::Install => "installing",
            Op::Remove => "removing",
        }
    }
}

/// Decide which agents to act on: explicit `--agent` flags win; otherwise the
/// default set for `op`.
fn resolve_agents(home: &Path, agents: &[Agent], op: Op) -> Vec<Agent> {
    if agents.is_empty() {
        default_agents(home, op)
    } else {
        dedup(agents)
    }
}

/// The default agent set: for `Install`, agents detected as present; for
/// `Remove`, agents that actually have `/render` installed.
fn default_agents(home: &Path, op: Op) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|agent| match op {
            Op::Install => agent.detected(home),
            Op::Remove => has_skill(*agent, home),
        })
        .collect()
}

/// Is `/render` present for `agent` (its `render/` dir or a symlink exists)?
fn has_skill(agent: Agent, home: &Path) -> bool {
    let dir = agent.skills_dir(home).join(SKILL_NAME);
    dir.is_symlink() || dir.exists()
}

/// `agents` with duplicates removed, preserving order.
fn dedup(agents: &[Agent]) -> Vec<Agent> {
    let mut out: Vec<Agent> = Vec::new();
    for &agent in agents {
        if !out.contains(&agent) {
            out.push(agent);
        }
    }
    out
}

/// Final note: a `Finished` line on execute, a dry-run reminder otherwise, or a
/// hint when nothing was selected.
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

/// Install the skill into `<skills_dir>/render/SKILL.md`. `--execute` is
/// authoritative: it removes any existing `render/` (symlink or dir) first, then
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

/// Remove `<skills_dir>/render`. A no-op (with a note) when it isn't present.
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

/// Delete `path` if it exists, whatever it is. A symlink (or file) is unlinked
/// without touching its target; a real directory is removed recursively.
/// `is_symlink` is checked first so a symlinked dir is unlinked, not recursed.
fn remove_existing(path: &Path) -> Result<()> {
    if path.is_symlink() || path.is_file() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    } else if path.is_dir() {
        std::fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

/// The user's home directory from `$HOME`.
fn home_dir() -> Result<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => bail!("$HOME is not set; cannot locate the agent skills directories"),
    }
}

/// A cargo-style status line on stderr: a right-aligned (12-col) green-bold
/// `verb` then `msg`, e.g. `   Installing /render (~/.claude/skills/render/SKILL.md)`.
/// `console` strips the color when stderr isn't a terminal, so piped/captured
/// output (an agent running `ahr setup`) stays clean.
fn status(verb: &str, msg: &str) {
    eprintln!("{} {msg}", style(format!("{verb:>12}")).green().bold());
}

/// A cargo-style `warning: {msg}` line (bold-yellow `warning`) on stderr.
fn warn(msg: &str) {
    eprintln!("{}: {msg}", style("warning").yellow().bold());
}

#[cfg(test)]
mod tests {
    use super::{Agent, SKILL};
    use std::path::Path;

    #[test]
    fn embedded_skill_has_render_frontmatter() {
        assert!(SKILL.starts_with("---"));
        assert!(SKILL.contains("name: render"));
        assert!(SKILL.contains("allowed-tools: Bash(ahr:*)"));
    }

    #[test]
    fn agent_skills_dirs_are_under_home() {
        let home = Path::new("/home/x");
        assert!(Agent::ClaudeCode.skills_dir(home).ends_with(".claude/skills"));
        assert!(Agent::Pi.skills_dir(home).ends_with(".pi/agent/skills"));
        assert!(Agent::Generic.skills_dir(home).ends_with(".agent/skills"));
    }
}
