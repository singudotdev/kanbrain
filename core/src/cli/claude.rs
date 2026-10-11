//! Claude Code's hooks, written by `init`:
//!
//! - at the start of every session, and again after the conversation is compacted, Claude Code runs
//!   `kb context` and adds its output to the agent's context. The agent then knows the brain's state
//!   without spending a tool call on it, and keeps knowing it in long sessions, where compaction would
//!   otherwise leave only the section in `CLAUDE.md`;
//! - with every prompt, and when the agent would end its turn, `kb hook prompt` and `kb hook stop`
//!   check that the project is described (see [`super::hook`]). Agents skipped describing it when only
//!   asked to, so until it's done the prompt carries the request and the turn can't end without it.
//!   `stop` also reminds an agent of a task it started and hasn't commented on since.
//!
//! They go in `.claude/settings.local.json`, the repository's personal settings, since `init` runs on
//! each machine anyway and a teammate without kanbrain installed shouldn't get failing hooks.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Where the hooks go, relative to the repository.
pub const SETTINGS: &str = ".claude/settings.local.json";

/// One of the hooks `init` writes.
struct Hook {
    /// The Claude Code event it runs on.
    event: &'static str,
    /// Which occurrences of the event; `None` for events that take no matcher.
    matcher: Option<&'static str>,
    /// What it runs; also how its entry is recognised, to update or remove it.
    command: &'static str,
    /// What it ran before the command line was `kb`, if it existed then: replaced, or removed.
    old_command: Option<&'static str>,
}

/// The hooks, in the order they're written.
const HOOKS: [Hook; 3] = [
    // A new session, a resumed one, after `/clear`, and after compaction.
    Hook {
        event: "SessionStart",
        matcher: Some("startup|resume|clear|compact"),
        command: "kb context",
        old_command: Some("kanbrain context"),
    },
    Hook { event: "UserPromptSubmit", matcher: None, command: "kb hook prompt", old_command: None },
    Hook { event: "Stop", matcher: None, command: "kb hook stop", old_command: None },
];

/// What adding or removing the hooks did.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    /// The file was written.
    Written,
    /// It was already as asked.
    Unchanged,
    /// The file was deleted: the hooks were all it held.
    Removed,
}

/// Adds the hooks to `root`'s settings, creating the file if needed and keeping everything else in it;
/// a hook written before `kb` is replaced rather than doubled.
pub fn add_hooks(root: &Path) -> Result<Change> {
    let path = root.join(SETTINGS);
    let mut settings = read(&path)?;
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| format!("can't add the hooks to {}: its `hooks` isn't an object", path.display()))?;
    let mut changed = false;
    for hook in &HOOKS {
        let groups =
            hooks.entry(hook.event).or_insert_with(|| json!([])).as_array_mut().with_context(|| {
                format!("can't add the hooks to {}: its `{}` isn't a list", path.display(), hook.event)
            })?;
        if groups.iter().any(|group| runs(group, hook.command)) {
            continue;
        }
        changed = true;
        match groups.iter_mut().find(|group| hook.old_command.is_some_and(|old| runs(group, old))) {
            Some(old) => *old = group(hook),
            None => groups.push(group(hook)),
        }
    }
    if !changed {
        return Ok(Change::Unchanged);
    }
    write(&path, &settings)?;
    Ok(Change::Written)
}

/// The matcher group written for `hook`.
fn group(hook: &Hook) -> Value {
    let handler = json!([{ "type": "command", "command": hook.command }]);
    match hook.matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": handler }),
        None => json!({ "hooks": handler }),
    }
}

/// Takes the hooks out of `root`'s settings (those written before `kb` too), and the keys they leave
/// empty; deletes the file if nothing else is in it.
pub fn remove_hooks(root: &Path) -> Result<Change> {
    let path = root.join(SETTINGS);
    if !path.is_file() {
        return Ok(Change::Unchanged);
    }
    let mut settings = read(&path)?;
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else { return Ok(Change::Unchanged) };
    let mut changed = false;
    for hook in &HOOKS {
        let Some(groups) = hooks.get_mut(hook.event).and_then(Value::as_array_mut) else { continue };
        let before = groups.len();
        groups.retain(|group| !runs(group, hook.command) && !hook.old_command.is_some_and(|old| runs(group, old)));
        changed |= groups.len() != before;
        if groups.is_empty() {
            hooks.remove(hook.event);
        }
    }
    if !changed {
        return Ok(Change::Unchanged);
    }
    if hooks.is_empty() {
        settings.remove("hooks");
    }
    if settings.is_empty() {
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        // `.claude/` too, if the file was all it held, as when `init` created both.
        if let Some(dir) = path.parent()
            && fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
        {
            fs::remove_dir(dir).with_context(|| format!("removing {}", dir.display()))?;
        }
        return Ok(Change::Removed);
    }
    write(&path, &settings)?;
    Ok(Change::Written)
}

/// Whether a matcher group runs `command`.
fn runs(group: &Value, command: &str) -> bool {
    group["hooks"].as_array().is_some_and(|hooks| hooks.iter().any(|h| h["command"] == command))
}

/// The settings in `path`; none yet if there's no file. A file that isn't a JSON object is refused,
/// rather than overwritten.
fn read(path: &Path) -> Result<Map<String, Value>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(settings)) => Ok(settings),
        Ok(_) => bail!("{} isn't a JSON object", path.display()),
        Err(e) => bail!("{} isn't valid JSON ({e})", path.display()),
    }
}

/// Writes the settings as Claude Code does: pretty-printed, keys in their order.
fn write(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    crate::store::write_file(path, &(serde_json::to_string_pretty(settings)? + "\n"))
}

#[cfg(test)]
#[path = "../../tests/cli/claude.rs"]
mod tests;
