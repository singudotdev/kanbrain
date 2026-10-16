//! Claude Code's hooks, written by `init`:
//!
//! - at the start of every session, and again after the conversation is compacted, Claude Code runs
//!   `kb context` and adds its output to the agent's context. The agent then knows the brain's state
//!   without spending a tool call on it, and keeps knowing it in long sessions, where compaction would
//!   otherwise leave only the section in `CLAUDE.md`;
//! - with every prompt, and when the agent would end its turn, `kb hook prompt` and `kb hook stop`
//!   check that the project is described (see [`super::hook`]). Agents skipped describing it when only
//!   asked to, so until it's done the prompt carries the request and the turn's end repeats it.
//!   `stop` also reminds an agent of each task it started and hasn't commented on since;
//! - before the agent writes a file with Claude Code's tools, `kb hook edit` refuses one in a brain's folder
//!   and sends it to `kb`, which the section already asks for;
//! - before every tool call, `kb hook tool` tells the agent of the user's comment on a task it's working on, made
//!   since it last commented there, until it does: the user is heard mid-work.
//!
//! `kb context` brings a repository's hooks up to date once kanbrain's are there: a release that adds one reaches
//! every project with its next session, without `kb init` run again in each.
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
}

/// The hooks, in the order they're written.
const HOOKS: [Hook; 5] = [
    // A new session, a resumed one, after `/clear`, and after compaction.
    Hook { event: "SessionStart", matcher: Some("startup|resume|clear|compact"), command: "kb context" },
    Hook { event: "UserPromptSubmit", matcher: None, command: "kb hook prompt" },
    Hook { event: "Stop", matcher: None, command: "kb hook stop" },
    // The tools that write files; a shell command's writes aren't seen.
    Hook { event: "PreToolUse", matcher: Some("Edit|Write|MultiEdit"), command: "kb hook edit" },
    // Every tool: no matcher.
    Hook { event: "PreToolUse", matcher: None, command: "kb hook tool" },
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

/// Adds the hooks to `root`'s settings, creating the file if needed and keeping everything else in it.
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
        groups.push(group(hook));
    }
    if !changed {
        return Ok(Change::Unchanged);
    }
    write(&path, &settings)?;
    Ok(Change::Written)
}

/// Adds what's missing of the hooks to `root`'s settings, if any of kanbrain's is there: never anew, so a
/// repository whose hooks were taken out (`kb deinit`), or never written (`AGENTS.md` only), stays without.
pub fn update_hooks(root: &Path) -> Result<Change> {
    let path = root.join(SETTINGS);
    if !path.is_file() {
        return Ok(Change::Unchanged);
    }
    let hooks = read(&path)?.remove("hooks").unwrap_or(Value::Null);
    let installed = HOOKS.iter().any(|hook| {
        hooks[hook.event].as_array().is_some_and(|groups| groups.iter().any(|group| runs(group, hook.command)))
    });
    if !installed {
        return Ok(Change::Unchanged);
    }
    add_hooks(root)
}

/// The matcher group written for `hook`.
fn group(hook: &Hook) -> Value {
    let handler = json!([{ "type": "command", "command": hook.command }]);
    match hook.matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": handler }),
        None => json!({ "hooks": handler }),
    }
}

/// Takes the hooks out of `root`'s settings, and the keys they leave
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
        groups.retain(|group| !runs(group, hook.command));
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

/// Whether a matcher group runs `command`, as written or with `kb` given by its path (`/opt/bin/kb hook tool`), as a
/// user may have changed it: one to keep, update or remove, never to add again beside itself.
fn runs(group: &Value, command: &str) -> bool {
    let args = command.strip_prefix("kb");
    let same = |written: &str| {
        written == command
            || args
                .and_then(|args| written.strip_suffix(args))
                .is_some_and(|program| Path::new(program).file_stem().is_some_and(|stem| stem == "kb"))
    };
    group["hooks"].as_array().is_some_and(|hooks| hooks.iter().any(|h| h["command"].as_str().is_some_and(same)))
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
