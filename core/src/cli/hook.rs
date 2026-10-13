//! `kb hook prompt|stop`: what Claude Code's hooks run so that an agent describes the project (and,
//! with several repositories, this one's role) before anything else. Asked to in the section and in
//! `context`, agents still skipped it on their first turn; these make Claude Code hold them to it:
//!
//! - `prompt` (`UserPromptSubmit`): while something is missing, every prompt arrives with the request
//!   to write it first;
//! - `stop` (`Stop`): an agent about to end its turn with it still missing is sent back to write it.
//!   Only once a turn (`stop_hook_active`), so a describe that keeps failing can't loop.
//!
//! `stop` also catches the board falling behind: a task an agent started and hasn't commented on since
//! gets a reminder to say where it stands, as the section asks, so the person watching sees it.
//!
//! Once the project is described and the tasks the agent started commented on, they print nothing, so they cost
//! nothing.

use crate::model::{Author, Status, Task};
use crate::projects::{self, ProjectInfo};
use crate::store::{self, Store};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Which hook is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// `UserPromptSubmit`: a prompt is about to reach the agent.
    Prompt,
    /// `Stop`: the agent is about to end its turn.
    Stop,
}

impl HookEvent {
    /// The event `kb hook NAME` names.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "prompt" => Some(Self::Prompt),
            "stop" => Some(Self::Stop),
            _ => None,
        }
    }
}

/// What to print for `event`, given the JSON Claude Code sent (`input`) and the central brain: `None`
/// outside a project, or when there's nothing to remind. Malformed input reads as empty, so a hook never
/// breaks a session: it falls back to the current folder and lets the turn end.
pub fn respond(event: HookEvent, input: &str, central: &Path) -> Option<String> {
    let input: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let cwd = input["cwd"].as_str().map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    let dir = store::find_project(&cwd)?;
    let project = projects::info(&dir, Some(central))?;
    let missing = missing(&project).join("; and ");
    match event {
        HookEvent::Prompt if missing.is_empty() => None,
        HookEvent::Prompt => Some(format!(
            "kanbrain: this project isn't described yet. Before anything else in this turn, read the README \
             and manifests and write {missing}. Then go on with the request."
        )),
        HookEvent::Stop if input["stop_hook_active"].as_bool() == Some(true) => None,
        HookEvent::Stop => {
            let mut reminders = Vec::new();
            if !missing.is_empty() {
                reminders.push(format!(
                    "kanbrain: the project still isn't described. Before ending this turn, read the README and \
                     manifests and write {missing}."
                ));
            }
            // A brain that can't be read only loses the reminder, never the turn.
            let tasks = Store::at(&dir).and_then(|s| s.load()).map(|(tasks, _)| tasks).unwrap_or_default();
            // Each one: with subagents, an agent may have several running.
            reminders.extend(tasks.iter().filter(|t| uncommented(t)).map(|task| {
                format!(
                    "kanbrain: #{0} {1} is in doing with no comment since you started it. Before ending this turn, \
                     `kb comment {0}` what changed and what's next, or `kb done {0}` if it's finished.",
                    task.id, task.title
                )
            }));
            // As additional context rather than `decision: "block"`: the turn goes on all the same, but
            // Claude Code shows it as a reminder instead of a hook error.
            (!reminders.is_empty()).then(|| {
                json!({
                    "hookSpecificOutput": { "hookEventName": "Stop", "additionalContext": reminders.join("\n") },
                })
                .to_string()
            })
        }
    }
}

/// Whether `task` runs because an agent started it, and nobody has commented on it since. A task the
/// user started isn't the agent's to report on.
fn uncommented(task: &Task) -> bool {
    let Some(started) = task.history.last().filter(|c| c.to == Status::Doing && c.by == Author::Agent) else {
        return false;
    };
    task.is_running() && task.comments.iter().all(|c| c.created < started.at)
}

/// What an agent still has to write about the project: its description and, in a project of several
/// repositories, this one's role.
fn missing(project: &ProjectInfo) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if project.description.is_empty() {
        missing.push(concat!(
            "its description with `kb project describe -` (",
            crate::projects::description_scope!(),
            ")"
        ));
    }
    if !project.siblings.is_empty() && project.notes.is_empty() {
        missing.push("this repository's role with `kb project notes -` (what it owns, exposes and consumes)");
    }
    missing
}

#[cfg(test)]
#[path = "../../tests/cli/hook.rs"]
mod tests;
