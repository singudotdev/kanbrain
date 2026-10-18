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
//! A hook is kanbrain's, to update or take out, as `init` writes it or with `kb` given by a plain path (see
//! [`kanbrains`]). Anything else, however much it looks like one, is the user's: a shell line can't be read for sure
//! without running it (PowerShell takes typographic quotes for quotes; a hook without `shell` runs in PowerShell on
//! Windows without Git Bash), and `deinit` must never take out a hook of the user's. When one looks like kanbrain's,
//! `init` adds none beside it, and `init` and `deinit` say so, for the user to replace or remove it.
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

/// A hook that isn't kanbrain's but looks like one of them: kanbrain never updates or removes it, nor adds that one
/// beside it, and says so for the user to sort out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    /// The Claude Code event it runs on.
    pub event: &'static str,
    /// What it runs: its command, then its exec form's arguments, as written.
    pub command: String,
    /// The hook it looks like, as `init` writes it.
    pub hook: &'static str,
}

/// Adds the hooks to `root`'s settings, creating the file if needed and keeping everything else in it. A hook edited by
/// hand gets none beside it; each is returned, for the user to replace.
pub fn add_hooks(root: &Path) -> Result<(Change, Vec<Edited>)> {
    let path = root.join(SETTINGS);
    let mut settings = read(&path)?;
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| format!("can't add the hooks to {}: its `hooks` isn't an object", path.display()))?;
    let mut changed = false;
    let mut edited = Vec::new();
    for hook in &HOOKS {
        let groups =
            hooks.entry(hook.event).or_insert_with(|| json!([])).as_array_mut().with_context(|| {
                format!("can't add the hooks to {}: its `{}` isn't a list", path.display(), hook.event)
            })?;
        if groups.iter().any(|group| runs(group, hook.command)) {
            continue;
        }
        let before = edited.len();
        edited.extend(look_alikes(groups, hook));
        if edited.len() > before {
            continue;
        }
        changed = true;
        groups.push(group(hook));
    }
    if !changed {
        return Ok((Change::Unchanged, edited));
    }
    write(&path, &settings)?;
    Ok((Change::Written, edited))
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
    // Unsaid: what `kb context` prints goes into the agent's context, and `init` says it to the user.
    Ok(add_hooks(root)?.0)
}

/// The matcher group written for `hook`.
fn group(hook: &Hook) -> Value {
    let handler = json!([{ "type": "command", "command": hook.command }]);
    match hook.matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": handler }),
        None => json!({ "hooks": handler }),
    }
}

/// Takes the hooks out of `root`'s settings, and the keys they leave empty; deletes the file if nothing else is in it.
/// A hook edited by hand stays; each is returned, for the user to remove.
pub fn remove_hooks(root: &Path) -> Result<(Change, Vec<Edited>)> {
    let path = root.join(SETTINGS);
    if !path.is_file() {
        return Ok((Change::Unchanged, Vec::new()));
    }
    let mut settings = read(&path)?;
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok((Change::Unchanged, Vec::new()));
    };
    let mut changed = false;
    let mut edited = Vec::new();
    for hook in &HOOKS {
        let Some(groups) = hooks.get_mut(hook.event).and_then(Value::as_array_mut) else { continue };
        let before = groups.len();
        groups.retain(|group| !runs(group, hook.command));
        changed |= groups.len() != before;
        edited.extend(look_alikes(groups, hook));
        if groups.is_empty() {
            hooks.remove(hook.event);
        }
    }
    if !changed {
        return Ok((Change::Unchanged, edited));
    }
    if hooks.is_empty() {
        settings.remove("hooks");
    }
    if settings.is_empty() {
        crate::store::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        // `.claude/` too, if the file was all it held, as when `init` created both.
        if let Some(dir) = path.parent()
            && fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
        {
            fs::remove_dir(dir).with_context(|| format!("removing {}", dir.display()))?;
        }
        return Ok((Change::Removed, edited));
    }
    write(&path, &settings)?;
    Ok((Change::Written, edited))
}

/// Whether a matcher group runs `command` as kanbrain's, to update or remove (see [`kanbrains`]).
fn runs(group: &Value, command: &str) -> bool {
    group["hooks"].as_array().is_some_and(|hooks| hooks.iter().any(|handler| kanbrains(handler, command)))
}

/// Whether a hook handler runs `command` (`kb hook tool`) as kanbrain's: as `init` writes it, or with `kb` given by
/// a plain path (`/opt/bin/kb hook tool`, `'C:\Users\John Doe\…\kb.exe' hook tool`, after `& ` in PowerShell), or in
/// the exec form (`"command"` the path, `"args"` the arguments). Nothing else: a path is taken only from letters,
/// digits and the characters below, the arguments only exactly, so that whichever shell runs it (bash, or PowerShell
/// on Windows without Git Bash, even for a hook without `shell`) reads it as that one command, with no quoting,
/// escape, variable, substitution or operator for either to read otherwise. Bash takes a `\` out of a bare path
/// and, before some characters, out of `"…"`, so in bash only `'…'` holds one; it expands a bare path's leading `~/`,
/// which PowerShell and the exec form don't. What this refuses is the user's, which `deinit` must never take out.
fn kanbrains(handler: &Value, command: &str) -> bool {
    if handler["type"] != "command" {
        return false;
    }
    let Some(written) = handler["command"].as_str() else { return false };
    if let Some(given) = handler.get("args") {
        // No shell reads it: a path's spaces and `\`s are its own.
        let args = command.split(' ').skip(1).map(Some);
        return written.chars().all(|c| in_quotes(c) || c == '\\')
            && is_kb(written)
            && given.as_array().is_some_and(|given| given.iter().map(Value::as_str).eq(args));
    }
    let powershell = match handler.get("shell").map(Value::as_str) {
        None | Some(Some("bash")) => false,
        Some(Some("powershell")) => true,
        _ => return false,
    };
    // PowerShell's call operator, which runs what follows.
    let (call, line) = match written.strip_prefix("& ") {
        Some(line) if powershell => (true, line),
        _ => (false, written),
    };
    let (program, rest) = if let Some(quote @ ('"' | '\'')) = line.chars().next() {
        // PowerShell takes a quoted path without `&` for a string, which runs nothing.
        if powershell && !call {
            return false;
        }
        let Some((path, rest)) = line[1..].split_once(quote) else { return false };
        let backslash = powershell || quote == '\'';
        if !path.chars().all(|c| in_quotes(c) || (backslash && c == '\\')) {
            return false;
        }
        (path, rest)
    } else {
        let (path, rest) = line.split_at(line.find(' ').unwrap_or(line.len()));
        let plain = if powershell { path } else { path.strip_prefix("~/").unwrap_or(path) };
        if !plain.chars().all(|c| bare(c) || (powershell && c == '\\')) {
            return false;
        }
        (path, rest)
    };
    is_kb(program) && rest.trim_end_matches([' ', '\t']) == &command["kb".len()..]
}

/// Whether `c` may be in a bare path: no shell reads any of these but as itself. Letters beyond ASCII too
/// (`/home/josé/…`): PowerShell's own dashes, quotes and spaces aren't letters or digits.
fn bare(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | ':' | '-')
}

/// Whether `c` may be in a quoted path: a space and parentheses too (`C:\Program Files (x86)\…`), which quotes
/// keep from both shells.
fn in_quotes(c: char) -> bool {
    bare(c) || matches!(c, ' ' | '(' | ')')
}

/// Whether the path `program` names `kb`: `kb`, or `kb.exe` in any case, as Windows names it. A `\` separates
/// folders only in a Windows path (`C:\…`, `\\server\…`, without a `/`); in any other, a `\` is bash's to take out
/// (`/usr/bin\x/kb` runs `/usr/binx/kb`) or part of a file's name, so the path isn't `kb`'s.
fn is_kb(program: &str) -> bool {
    let drive = program.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) && program.get(1..3) == Some(":\\");
    let windows = (drive || program.starts_with("\\\\")) && !program.contains('/');
    if !windows && program.contains('\\') {
        return false;
    }
    let name = program.rsplit(if windows { '\\' } else { '/' }).next().unwrap_or_default();
    name == "kb" || name.eq_ignore_ascii_case("kb.exe")
}

/// The command hooks in `groups` that look like `hook` but aren't kanbrain's (see [`kanbrains`]): among its words
/// (split at whitespace, quotes and the like trimmed off each), one naming `kb` or `kb.exe` by its path or not,
/// followed directly by `hook`'s arguments (`hook tool`), wherever it is: `kb` run another way (`env … kb`,
/// `bash -c '…'`, in an `if`) is still kanbrain's hook, which must not be added again. `kb` in prose
/// (`echo kb context`) matches too; that only costs kanbrain's hook not added for that event, which the line said
/// tells how to add. A hook is never taken out for it.
fn look_alikes<'a>(groups: &'a [Value], hook: &'a Hook) -> impl Iterator<Item = Edited> + 'a {
    let args: Vec<&str> = hook.command.split(' ').skip(1).collect();
    let handlers = groups.iter().filter_map(|group| group["hooks"].as_array()).flatten();
    handlers.filter(|handler| handler["type"] == "command").filter_map(move |handler| {
        let mut command = handler["command"].as_str()?.to_string();
        for arg in handler["args"].as_array().into_iter().flatten() {
            command.push(' ');
            command.push_str(&arg.as_str().map_or_else(|| arg.to_string(), str::to_string));
        }
        let words: Vec<&str> = command
            .split_whitespace()
            .map(|word| {
                word.trim_matches(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '\\' | ':')))
            })
            .collect();
        let found = words.windows(args.len() + 1).any(|run| {
            let name = run[0].rsplit(['/', '\\']).next().unwrap_or_default();
            (name.eq_ignore_ascii_case("kb") || name.eq_ignore_ascii_case("kb.exe")) && run[1..] == args[..]
        });
        found.then_some(Edited { event: hook.event, command, hook: hook.command })
    })
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
