//! `kb hook prompt|stop|edit`: what Claude Code's hooks run so that an agent describes the project (and,
//! with several repositories, this one's role) before anything else, and keeps to `kb` for the brain's files.
//! Asked to in the section and in `context`, agents still skipped describing it on their first turn; these have
//! Claude Code remind them:
//!
//! - `prompt` (`UserPromptSubmit`): while something is missing, every prompt arrives with the request
//!   to write it first;
//! - `stop` (`Stop`): an agent ending its turn with it still missing is reminded to write it, as context
//!   Claude Code adds rather than a block (the turn ends all the same). Only once a turn (`stop_hook_active`).
//!
//! `stop` also catches the board falling behind: a task an agent started and hasn't commented on since
//! gets a reminder to say where it stands, as the section asks, so the person watching sees it.
//!
//! Once the project is described and the tasks the agent started commented on, they print nothing, so they cost
//! nothing.
//!
//! `edit` (`PreToolUse`, on the tools that write files) refuses instead: an agent about to write a brain's file is
//! sent to `kb`. The section tells it so, yet an agent seeing markdown files may edit them: that skips the brain's
//! lock (the board and other agents write there too), the numbering of new entries and the format `kb` keeps.

use crate::enums::named_enum;
use crate::model::{Author, Status, Task};
use crate::projects::{self, ProjectInfo};
use crate::store::{self, Store};
use serde_json::{Value, json};
use std::fs;
use std::path::{Component, Path, PathBuf};

named_enum! {
    /// Which hook is asking, as `kb hook NAME` names it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HookEvent {
        /// `UserPromptSubmit`: a prompt is about to reach the agent.
        Prompt => "prompt",
        /// `Stop`: the agent is about to end its turn.
        Stop => "stop",
        /// `PreToolUse`: the agent is about to write a file.
        Edit => "edit",
    }
}

/// What to print for `event`, given the JSON Claude Code sent (`input`) and the central brain: `None` when
/// there's nothing to remind or refuse, as for `prompt` and `stop` outside a project. Malformed input reads as empty, so a hook
/// never breaks a session: it falls back to the folder it runs in, `here`, and lets the turn end.
pub fn respond(event: HookEvent, input: &str, central: &Path, here: &Path) -> Option<String> {
    let input: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let cwd = input["cwd"].as_str().map_or_else(|| here.to_path_buf(), PathBuf::from);
    match event {
        // Claude Code gives the path absolute; joined to the session's folder in case it isn't.
        HookEvent::Edit => refuse_brain_write(&lexical(&cwd.join(input["tool_input"]["file_path"].as_str()?)), central),
        HookEvent::Prompt | HookEvent::Stop => remind(event, &input, &cwd, central),
    }
}

/// `edit`'s answer to an agent about to write `file`: refused, and sent to `kb`, when it's in a brain's folder.
fn refuse_brain_write(file: &Path, central: &Path) -> Option<String> {
    in_brain(file, central).then(|| {
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "kanbrain: a brain's files are written only through `kb`, which takes \
                    the brain's lock (the board and other agents write there too), numbers new entries and keeps \
                    their format. Make this change with `kb` (`kb --help` lists the commands); if none makes it, \
                    tell the user what to change instead of editing the file.",
            },
        })
        .to_string()
    })
}

/// Whether `file` is one of a brain's: anything in a project's `.brain/`, or what kanbrain keeps in the central
/// brain ([`central_owned`]). The central brain may be a notes vault, or hold repositories: its other files are the
/// agent's to write. A link to either counts, through the nearest of `file`'s folders that exists (a file being
/// created doesn't yet).
fn in_brain(file: &Path, central: &Path) -> bool {
    let canonical = central.canonicalize().ok();
    let centrals = [Some(central), canonical.as_deref()];
    let owned = |path: &Path| {
        path.ancestors().any(|dir| dir.file_name().is_some_and(|name| name == store::PROJECT_DIR))
            || centrals.into_iter().flatten().any(|c| path.strip_prefix(c).is_ok_and(|rel| central_owned(c, rel)))
    };
    owned(file) || real_path(file).is_some_and(|real| owned(&real))
}

/// Whether `rel`, a path inside the central brain `central`, is kanbrain's: `tags.md`, `projects.md`, `last-id`,
/// `format`, an entry, or anything in the project cards' or reports' folders. An entry is a file at the top holding
/// a `- id:` line, as `Store::load` reads them, or one yet to be written named as one (`0007-title.md`): the vault's
/// other notes, there too, aren't.
fn central_owned(central: &Path, rel: &Path) -> bool {
    let mut parts = rel.components();
    let Some(first) = parts.next().map(Component::as_os_str) else { return false };
    if parts.next().is_some() {
        return first == projects::CARDS_DIR || first == store::REPORTS_DIR;
    }
    let kanbrains = [store::TAGS_FILE, projects::REGISTRY_FILE, store::LAST_ID_FILE, crate::migrate::FORMAT_FILE];
    let name = first.to_string_lossy();
    kanbrains.contains(&&*name)
        || name.ends_with(".md")
            && match fs::read_to_string(central.join(rel)) {
                Ok(text) => text.lines().any(|line| line.trim_start().starts_with("- id:")),
                Err(_) => store::entry_named(&name),
            }
}

/// `path` with its `.` and `..` resolved as text, as Claude Code resolves the path it writes: through the folder
/// named, which needn't exist, rather than where a link in it leads.
fn lexical(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            _ => resolved.push(part),
        }
    }
    resolved
}

/// `file` with its links resolved: its nearest folder that exists, canonical, then the rest of the path.
fn real_path(file: &Path) -> Option<PathBuf> {
    file.ancestors().find_map(|dir| Some(dir.canonicalize().ok()?.join(file.strip_prefix(dir).ok()?)))
}

/// `prompt`'s and `stop`'s reminders, for the project the session runs in (`cwd`).
fn remind(event: HookEvent, input: &Value, cwd: &Path, central: &Path) -> Option<String> {
    let dir = store::find_project(cwd)?;
    let project = projects::info(&dir, Some(central))?;
    let missing = missing(&project).join("; and ");
    if event == HookEvent::Prompt {
        return (!missing.is_empty()).then(|| {
            format!(
                "kanbrain: this project isn't described yet. Before anything else in this turn, read the README \
                 and manifests and write {missing}. Then go on with the request."
            )
        });
    }
    if input["stop_hook_active"].as_bool() == Some(true) {
        return None;
    }
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
        // Which session started it isn't known, so the agent of another, or one that crashed, is told
        // apart by the agent reading this: never told to finish a task it didn't start.
        format!(
            "kanbrain: #{0} {1} is in doing with no comment since an agent started it. If you started it, \
             before ending this turn `kb comment {0}` what changed and what's next, or `kb done {0}` if \
             it's finished; if you didn't, leave it alone.",
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
