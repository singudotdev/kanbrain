//! `kb init`: makes the current folder a project. It gets its own `.brain/` with a card
//! (`project.md`: its name and what it's about), is registered in the central brain so the kanban app
//! shows it and can filter by it, and its coding agents are told how to use it, by a short section in
//! their instructions file (`CLAUDE.md`, `AGENTS.md`). Without that section an agent doesn't know the
//! brain exists until someone pastes it by hand.
//!
//! `kb deinit` undoes it, except for `.brain/` itself: its tasks, notes and decisions are the
//! repository's, often committed with it, so deleting them is left to the user.

use super::claude;
use crate::projects;
use crate::store::{self, PROJECT_DIR, Store};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// Instructions files agents read at the start of a session: Claude Code's, and the cross-agent one.
const AGENT_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];
/// Created when the project has none of `AGENT_FILES`.
const DEFAULT_AGENT_FILE: &str = "CLAUDE.md";
/// Marks where the section starts and ends, so running `init` again updates it in place instead of
/// adding another, and everything around it is left alone.
const START: &str = "<!-- kanbrain:start -->";
/// See [`START`].
const END: &str = "<!-- kanbrain:end -->";

/// The section. Read at the start of every session, so it's short, yet the whole CLI an agent needs is
/// here. It has to win over the agent's own habits: Claude Code, for one, has a built-in to-do list,
/// and works through a plan without a word to the board unless told the board is watched live.
const SECTION: &str = concat!(
    "\
## Project memory: kanbrain

This project's tasks, to-dos, decisions and notes live in `.brain/`, managed only with `kb`, kanbrain's CLI.
The user follows them live on a kanban board: keep it showing what you're doing as you go, not at the end.

- Start of session: `kb context`, unless its output is already above (a session hook adds it).
- Work that changes the repository, or that the user should see, gets a task; questions and lookups don't. Find it
  (`search WORDS`) or create it (`add TITLE`), then `start ID` (`add --start TITLE` does both). \"Todos\", a plan or
  what's pending, in any language, are tasks here, one `add` each; your built-in to-do list isn't on the board, so
  keep it for steps within the task you've started. A task with real work gets a body, `add --body - TITLE`:
  **Goal** (what and why, a line or two) and **Done when** (what the user will check).
- While working: `comment ID TEXT` when a step is done, when you find a cause, and before you stop (one or two
  lines: what changed, what's next); `done ID` as soon as one is finished. Stopping before it is: `comment`, then
  `park ID`. Doing isn't limited for you: several of your tasks can run at once. Never park, finish or move a task
  you didn't start (the user's, or another session's).
- Splitting work between subagents: give each part that changes the repository its own task, and run the board
  yourself: `add --start` it before launching the subagent, then `comment` its result and `done` it. Subagents
  only do the work; lookups and research get no task. `kb` commands can run in parallel.
- Blocked on the user (a requirement, access, a choice that's theirs): `ask ID TEXT` parks the task with the
  question; don't guess. A choice you shouldn't make alone and weren't asked to (architecture, a dependency,
  storage, a public interface): `propose ID --body - TITLE`, Context / Options / Recommendation on stdin; it parks
  the task until the user accepts or rejects it. Then, unless the user said they're away, if your tool can run a
  command in the background and wake you when it exits, run `kb wait ID` (the task) that way and end your turn: it
  prints the answer, or that there's none yet (then stop, don't wait again). Otherwise go on with other work or
  stop; answers come first in `context`, under Answered: `show ID`, then pick the task up. If the user answers in
  the chat instead, `start` the task (that ends the wait). `accept`/`reject` a decision only when the user tells
  you to.
- Choosing between approaches within your task: `search` for earlier decisions (only accepted ones are in force),
  then `add --decision --link ID --body - TITLE` with Context / Decision / Consequences on stdin. Non-obvious findings: `add --note --link ID --body - TITLE`;
  a problem someone has to fix is a task instead, in the repository that has to change.
- `## Project` in `context` is the application's shared context. If it isn't described, or you're asked to set up
  or load the brain, describe it first, from the README and manifests: `project describe -` (",
    crate::projects::description_scope!(),
    "). Rewrite it when the big picture changes (a service added, another storage), not after every change. With
  several repositories, this one's role too: `project notes -`. How to build, test and run this repository, and its
  code style, go in this file, outside this section: add them as you learn them.
- Also: `show ID` (one entry in full), `describe ID TEXT`, `link ID OTHER…` (an id here, or `REPO#ID` in another
  repository of the project); `-` reads text from stdin. Never edit the files in `.brain/` by hand: `kb` keeps
  them consistent.",
);

/// Makes `root` a repository of project `name` (else the one its card names, else one named after
/// the folder), named `repo` in it (else as [`projects::repo_name`] picks): creates its `.brain/` and card,
/// registers it in `central` (joining the project's other repositories if it has some), creates the project's
/// shared card if it's new, and writes or refreshes the section in each agent file of `root` (creating
/// `CLAUDE.md` if there's none). Returns what it did, one line each.
pub fn init(root: &Path, name: Option<&str>, repo: Option<&str>, central: &Path) -> Result<Vec<String>> {
    let mut done = Vec::new();
    let dir = root.join(PROJECT_DIR);
    if dir.is_dir() {
        done.push(format!("{} already exists", dir.display()));
    } else {
        Store::at(&dir)?;
        done.push(format!("created {}: kanbrain uses it from here and every folder below", dir.display()));
        done.push("commit it to share tasks, notes and decisions with the project, or add it to .gitignore".into());
    }
    // Registered by its absolute path, so the kanban app finds it from anywhere.
    let dir = dir.canonicalize().with_context(|| format!("reading {}", dir.display()))?;
    let folder = root.canonicalize().ok().and_then(|r| Some(r.file_name()?.to_string_lossy().into_owned()));
    let card = projects::read_repo_card(&dir);
    let project = match (name, &card, folder) {
        (Some(name), _, _) => name.to_string(),
        (None, Some(card), _) => card.project.clone(),
        (None, None, Some(folder)) => folder,
        (None, None, None) => bail!("this folder has no name to give the project: `kb init NAME`"),
    };
    projects::check_name(&project)?;
    let named = projects::repo_name(central, &project, &dir, repo)?;
    // The name goes on the card before registering, which tells repositories apart by it; only when it isn't
    // the folder's, so a card stays as it was for the usual case.
    let folder_name = projects::Repo { project: project.clone(), dir: dir.clone() }.folder();
    let notes = card.as_ref().map(|card| card.notes.clone()).unwrap_or_default();
    let new_card =
        projects::RepoCard { project: project.clone(), repo: (named != folder_name).then_some(named), notes };
    if card.as_ref() != Some(&new_card) {
        projects::write_repo_card(&dir, &new_card)?;
    }
    projects::register(central, &project, &dir)?;
    if projects::ensure_description(central, &project)? {
        done.push(format!(
            "created project `{project}`'s card in the central brain: describe the project with `kb project \
             describe` or from the kanban app (or let the agent do it); all its repositories share it"
        ));
    }
    let repos = projects::load(central)?;
    let here = repos.iter().find(|r| r.dir == dir).map_or_else(|| project.clone(), |r| r.label(&repos));
    let others: Vec<String> =
        repos.iter().filter(|r| r.project == project && r.dir != dir).map(projects::Repo::name).collect();
    if others.is_empty() {
        done.push(format!(
            "registered as project `{here}`: the kanban app shows it, and `--project {here}` reaches it"
        ));
    } else {
        done.push(format!(
            "registered as `{here}`, a repository of project `{project}` with {}: they share its card",
            others.join(", ")
        ));
    }
    let existing: Vec<&str> = AGENT_FILES.into_iter().filter(|name| root.join(name).is_file()).collect();
    let targets = if existing.is_empty() { vec![DEFAULT_AGENT_FILE] } else { existing };
    done.extend(write_agent_files(root, &targets)?.into_iter().map(|(_, line)| line));
    if !on_path(std::env::var_os("PATH"), "kb") {
        done.push(
            "`kb` isn't on your PATH, and agents and the hooks run it: install the command line too \
             (the installer's `--app both`)"
                .into(),
        );
    }
    Ok(done)
}

/// Brings every registered repository's agent instructions up to this version's: the section in each of its
/// agent files that has one, and Claude Code's hooks where `CLAUDE.md` has it. A repository whose files have no
/// section (taken out by hand) is left alone, and so is one whose folder is gone. Returns what changed, one
/// line each, naming the repository.
pub fn init_all(central: &Path) -> Result<Vec<String>> {
    let repos = projects::load(central)?;
    let mut done = Vec::new();
    let mut current = 0;
    for repo in repos.iter().filter(|r| r.dir.is_dir()) {
        let Some(root) = repo.dir.parent() else { continue };
        let targets: Vec<&str> = AGENT_FILES
            .into_iter()
            .filter(|name| fs::read_to_string(root.join(name)).is_ok_and(|text| text.contains(START)))
            .collect();
        if targets.is_empty() {
            continue;
        }
        let changes: Vec<String> = write_agent_files(root, &targets)?
            .into_iter()
            .filter(|(changed, _)| *changed)
            .map(|(_, line)| line)
            .collect();
        if changes.is_empty() {
            current += 1;
        }
        let label = repo.label(&repos);
        done.extend(changes.into_iter().map(|line| format!("{label}: {line}")));
    }
    if current > 0 {
        done.push(format!("{current} more already had this version's instructions"));
    }
    Ok(done)
}

/// Writes or updates the section in each of `targets`, agent files of `root`, and Claude Code's hooks when
/// `CLAUDE.md` is one of them. Returns what it did, one line each, and whether that changed anything.
fn write_agent_files(root: &Path, targets: &[&str]) -> Result<Vec<(bool, String)>> {
    let mut done = Vec::new();
    // Claude Code's hooks go with its file; agents that read only AGENTS.md have no use for them.
    if targets.contains(&"CLAUDE.md") {
        done.push(match claude::add_hooks(root) {
            Ok(claude::Change::Unchanged) => (false, format!("{} already has kanbrain's hooks", claude::SETTINGS)),
            Ok(_) => (
                true,
                format!(
                    "added kanbrain's hooks to {}: Claude Code reads `kb context` when a session starts and \
                     after compaction, and an agent can't end its turn while the project isn't described",
                    claude::SETTINGS
                ),
            ),
            // The section still works without them, so a settings file it can't read doesn't stop init.
            Err(e) => (false, format!("left out the Claude Code hooks: {e:#}")),
        });
    }
    for &name in targets {
        let path = root.join(name);
        let before = fs::read_to_string(&path).unwrap_or_default();
        let after = with_section(&before);
        if after == before {
            done.push((false, format!("{name} already tells agents about kanbrain")));
            continue;
        }
        store::write_file(&path, &after)?;
        let what = if before.is_empty() {
            "created"
        } else if before.contains(START) {
            "updated"
        } else {
            "added to"
        };
        done.push((true, format!("{what} {name}: the \"Project memory\" section tells agents how to use the brain")));
    }
    Ok(done)
}

/// Whether a program named `name` is in one of the folders of `path`, a `PATH` value.
fn on_path(path: Option<std::ffi::OsString>, name: &str) -> bool {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    path.is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(&file).is_file()))
}

/// Takes the repository whose `.brain/` is `dir` out of the project registry, deleting the project's
/// card if it was its last repository, and takes the section out of each agent file of the repository
/// (deleting a file left empty, which `init` created). `.brain/` is kept. Returns what it did, one
/// line each.
pub fn deinit(dir: &Path, central: &Path) -> Result<Vec<String>> {
    let Some(repo) = projects::unregister(central, dir)? else {
        bail!("{} isn't a registered project: there's nothing to deinit", dir.display());
    };
    let mut done = vec![format!("unregistered {}: the kanban app no longer shows it", repo.dir.display())];
    let others: Vec<String> =
        projects::load(central)?.iter().filter(|r| r.project == repo.project).map(projects::Repo::name).collect();
    if !others.is_empty() {
        done.push(format!(
            "project `{}` keeps its card for its other repositories: {}",
            repo.project,
            others.join(", ")
        ));
    } else if projects::remove_description(central, &repo.project)? {
        done.push(format!("removed project `{}`'s card from the central brain", repo.project));
    }
    let Some(root) = repo.dir.parent().filter(|_| repo.dir.is_dir()) else {
        done.push(format!("{} is gone, so there was nothing else to undo", repo.dir.display()));
        return Ok(done);
    };
    match claude::remove_hooks(root) {
        Ok(claude::Change::Unchanged) => {}
        Ok(claude::Change::Written) => done.push(format!("removed kanbrain's hooks from {}", claude::SETTINGS)),
        Ok(claude::Change::Removed) => done.push(format!("removed {}: it only held the hooks", claude::SETTINGS)),
        Err(e) => done.push(format!("left the Claude Code hooks in place: {e:#}")),
    }
    for name in AGENT_FILES {
        let path = root.join(name);
        let Ok(before) = fs::read_to_string(&path) else { continue };
        let after = without_section(&before);
        if after == before {
            continue;
        }
        if after.trim().is_empty() {
            fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            done.push(format!("removed {name}: it only held the \"Project memory\" section"));
        } else {
            store::write_file(&path, &after)?;
            done.push(format!("removed the \"Project memory\" section from {name}"));
        }
    }
    done.push(format!(
        "kept {}: commands run here still use it; delete it to drop its tasks, notes and decisions",
        repo.dir.display()
    ));
    Ok(done)
}

/// Where the section is in `text`, with the line break after its end marker. It's the last start
/// marker before the first end marker, so a stray unclosed start marker above it is never taken for
/// the start and the text after it is kept.
fn section(text: &str) -> Option<(usize, usize)> {
    let end = text.find(END)? + END.len();
    let start = text[..end].rfind(START)?;
    Some((start, if text[end..].starts_with('\n') { end + 1 } else { end }))
}

/// `text` with the section between the markers: replaced where it already is, else appended after
/// a blank line.
fn with_section(text: &str) -> String {
    let block = format!("{START}\n{SECTION}\n{END}\n");
    if let Some((start, end)) = section(text) {
        return format!("{}{block}{}", &text[..start], &text[end..]);
    }
    match text.trim_end() {
        "" => block,
        kept => format!("{kept}\n\n{block}"),
    }
}

/// `text` without the section, and without the blank line `with_section` put before it.
fn without_section(text: &str) -> String {
    let Some((start, end)) = section(text) else { return text.to_string() };
    let before = text[..start].trim_end();
    let after = text[end..].trim_start_matches('\n');
    match (before, after) {
        ("", after) => after.to_string(),
        (before, "") => format!("{before}\n"),
        (before, after) => format!("{before}\n\n{after}"),
    }
}

#[cfg(test)]
#[path = "../../tests/cli/init.rs"]
mod tests;
