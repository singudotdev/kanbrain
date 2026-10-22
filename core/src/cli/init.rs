//! `kb init`: makes the current folder a project. It gets its own `.brain/` with a card
//! (`project.md`: its name and what it's about), is registered in the central brain so the kanban app
//! shows it and can filter by it, and its coding agents are told how to use it, by a short section in
//! their instructions file (`CLAUDE.md`, `AGENTS.md`). Without that section an agent doesn't know the
//! brain exists until someone pastes it by hand.
//!
//! `kb deinit` undoes it, except for `.brain/` itself: its tasks, notes and decisions are the
//! repository's, often committed with it, so deleting them is left to the user.

use super::claude;
use super::manage::{Step, StepKind};
use crate::model::counted;
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
/// UTF-8's byte order mark, which some Windows editors start a file with.
const BOM: &str = "\u{FEFF}";

/// The section. Read at the start of every session, so it's short, yet the whole CLI an agent needs is
/// here. It has to win over the agent's own habits: Claude Code, for one, has a built-in to-do list,
/// and works through a plan without a word to the board unless told the board is watched live.
const SECTION: &str = "\
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
  you didn't start (the user's, or another session's). A pull request, issue, CI run, commit or page named in a
  comment or body goes with its link, `[PR #39](URL)`, not just `PR #39`: the user opens it from the board. So does
  an entry, written `#12`, or `REPO#12` in another repository of the project: the board opens it from there.
- Before ending a turn in which you changed the repository: that change is on a task you started (create it now if
  you skipped it), the turn's work is commented there, and a finished task is `done`. Work that goes through a pull
  request is finished once every check has passed and it's merged; until then its task stays in Doing. Nothing is
  `done` without its evidence commented first: the tests or checks run and their result, or what shows it works (a
  CI run's link, the output, a screenshot). What you couldn't check isn't done: `ask ID TEXT` the user to check it,
  saying what's left and how.
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
  then `add --decision --link ID --body - TITLE` with Context / Decision / Consequences on stdin. Non-obvious
  findings: `add --note --link ID --body - TITLE`; a problem someone has to fix is a task instead, in the
  repository that has to change.
- Work from official documentation: that of the language, library, tool or API, at the version this repository pins
  (its manifest or lockfile), over memory or posts elsewhere. A decision, note or comment that rests on it links the
  page, `[serde: field attributes](URL)`, so the user can check it.
- `## Project` in `context` is the application's shared context. If it isn't described, or you're asked to set up
  or load the brain, describe it first, from the README and manifests: `project describe -` (what the app is, its
  services and how they connect, conventions they share: the big picture, in at most five lines, without tables,
  columns, function signatures, files or dependencies, which the code already says). Rewrite it when the big
  picture changes (a service added, another storage), not after every change. With several repositories, this
  one's role too: `project notes -`. How to build, test and run this repository, and its code style, go in this
  file, outside this section: add them as you learn them.
- Also: `show ID` (one entry in full), `describe ID TEXT`, `rename ID TITLE`, `tag ID +TAG -TAG` (add or remove
  tags), `link ID OTHER…` (an id here, or `REPO#ID` in another repository of the project); `-` reads text from
  stdin. Never edit the files in `.brain/` by hand: `kb` keeps them consistent.";

/// [`init_steps`], each step as its line.
#[deprecated(since = "0.6.3", note = "use `init_steps`, which says what each line is")]
pub fn init(root: &Path, name: Option<&str>, repo: Option<&str>, central: &Path) -> Result<Vec<String>> {
    super::manage::lines(init_steps(root, name, repo, central))
}

/// Makes `root` a repository of project `name` (else the one its card names, else one named after
/// the folder), named `repo` in it (else as [`projects::repo_name`] picks): creates its `.brain/` and card,
/// registers it in `central` (joining the project's other repositories if it has some), creates the project's
/// shared card if it's new, and writes or refreshes the section in each agent file of `root` (creating
/// `CLAUDE.md` if there's none). Returns what it did, a [`Step`] a line.
///
/// A repository of another project, still there, is moved to `name` as `kb project move` moves it: the links
/// between it and its old project's repositories go, and so does that project's card if it was the last.
pub fn init_steps(root: &Path, name: Option<&str>, repo: Option<&str>, central: &Path) -> Result<Vec<Step>> {
    // Refused before anything is written: its card and agent files go with a brain this version can't read.
    crate::migrate::refuse_too_new(&root.join(PROJECT_DIR))?;
    if let Some(to) = name
        && let Ok(dir) = projects::canonical(&root.join(PROJECT_DIR))
        && projects::load(central)?.iter().any(|r| projects::same_dir(&r.dir, &dir) && r.project != to)
    {
        let mut done = super::manage::move_repo_steps(central, &dir, to)?;
        // Moved already: a name it can't take is said beside the move, not instead of it.
        if let Some(repo) = repo {
            match super::manage::rename_repo_steps(central, &dir, repo) {
                Ok(steps) => done.extend(steps),
                Err(e) => done.push(Step::other(format!("kept the name it has: {e:#}"))),
            }
        }
        return Ok(done);
    }
    join(root, name, repo, central)
}

/// What [`init_steps`] does for a folder that isn't a repository of another project: [`super::manage::move_repo`] runs
/// it once the repository has left its old project.
pub(crate) fn join(root: &Path, name: Option<&str>, repo: Option<&str>, central: &Path) -> Result<Vec<Step>> {
    let mut done: Vec<Step> = Vec::new();
    let dir = root.join(PROJECT_DIR);
    // The registry holds a path a line; one with a line break would read as a second, made-up registration.
    if root.to_string_lossy().contains(['\n', '\r']) {
        bail!("{} has a line break in its path: kanbrain can't register it", root.display());
    }
    // Nor one `projects.md`, a UTF-8 file, can't hold: written lossily, it would name another folder. The canonical
    // one, which is what's written: a link with a Unicode name can lead to a folder without one.
    if fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()).to_str().is_none() {
        bail!(
            "{} has a folder name that isn't valid Unicode, which projects.md can't hold: kanbrain can't register it; \
             rename that folder",
            root.display()
        );
    }
    // Registered, its `.brain/` would be both the central brain and a project's, and taking it out with `--purge`
    // would delete every project's registration and card.
    if projects::same_dir(&dir, central) {
        bail!("{} is the central brain, not a project's: run `kb init` in a project's folder", dir.display());
    }
    if dir.is_dir() {
        done.push(Step::new(StepKind::AlreadyExisted, format!("{} already exists", dir.display())));
    } else {
        Store::at(&dir)?;
        done.push(Step::other(format!("created {}: kanbrain uses it from here and every folder below", dir.display())));
        done.push(Step::other(
            "commit it to share tasks, notes and decisions with the project, or add it to .gitignore",
        ));
    }
    // Registered by its absolute path, so the kanban app finds it from anywhere.
    let dir = projects::canonical(&dir).with_context(|| format!("reading {}", dir.display()))?;
    let folder = root.canonicalize().ok().and_then(|r| Some(r.file_name()?.to_string_lossy().into_owned()));
    let card = projects::read_repo_card(&dir);
    // Re-run without a name, it stays in the project the registry has it in, which a rename while its folder was
    // away left its card behind on.
    let registered = projects::load(central)?.into_iter().find(|r| projects::same_dir(&r.dir, &dir)).map(|r| r.project);
    // A name a version before 0.4.0 registered, which `init --all` renames: said, rather than to pick another.
    if name.is_none()
        && let Some(old) = registered.as_deref()
        && projects::check_name(old).is_err()
    {
        bail!(
            "this repository is registered in project `{old}`, a name kanbrain no longer takes: `kb init --all` \
             renames such projects, keeping their repositories and card"
        );
    }
    let project = match (name, registered, &card, folder) {
        (Some(name), ..) => name.to_string(),
        (None, Some(registered), ..) => registered,
        (None, None, Some(card), _) => card.project.clone(),
        (None, None, None, Some(folder)) => folder,
        (None, None, None, None) => bail!("this folder has no name to give the project: `kb init NAME`"),
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
        done.push(Step::other(format!(
            "created project `{project}`'s card in the central brain: describe the project with `kb project \
             describe` or from the kanban app (or let the agent do it); all its repositories share it"
        )));
    }
    let repos = projects::load(central)?;
    let here =
        repos.iter().find(|r| projects::same_dir(&r.dir, &dir)).map_or_else(|| project.clone(), |r| r.label(&repos));
    let others: Vec<String> = repos
        .iter()
        .filter(|r| r.project == project && !projects::same_dir(&r.dir, &dir))
        .map(projects::Repo::name)
        .collect();
    if others.is_empty() {
        done.push(Step::other(format!(
            "registered as project `{here}`: the kanban app shows it, and `--project {here}` reaches it"
        )));
    } else {
        done.push(Step::other(format!(
            "registered as `{here}`, a repository of project `{project}` with {}: they share its card",
            others.join(", ")
        )));
    }
    // A folder moved or renamed by hand leaves its old place registered: said, since only the user knows if
    // this is it.
    for gone in repos.iter().filter(|r| r.project == project && !r.dir.is_dir()) {
        let name = gone.name();
        // Its name kept for it, this one took another (`repo_name`): once the old place is out, it can take that one
        // back.
        done.push(Step::other(format!(
            "`{project}/{name}`'s folder is gone ({}): if this is it, moved here, take the old place out with \
             `kb --project {project}/{name} deinit`, then `kb project repo {name}` here",
            gone.dir.display()
        )));
    }
    let existing: Vec<&str> = AGENT_FILES.into_iter().filter(|name| root.join(name).is_file()).collect();
    let targets = if existing.is_empty() { vec![DEFAULT_AGENT_FILE] } else { existing };
    done.extend(write_agent_files(root, &targets)?.into_iter().map(|(_, line)| Step::other(line)));
    if !on_path(std::env::var_os("PATH"), "kb") {
        done.push(Step::other(
            "`kb` isn't on your PATH, and agents and the hooks run it: install the command line too \
             (the installer's `--app both`)",
        ));
    }
    Ok(done)
}

/// Renames each project an older version named as none can be now (see [`projects::check_name`]): one starting
/// with `-`, which `--project` reads as an option, `Central` in any case, which reaches the central brain (both
/// before 0.4.0), one with a lone `\r`, a line break to markdown (before 0.5.0), or one with `:` or `/` (a registry
/// edited by hand). Without it, `kb init` in its repositories is refused.
/// Returns what it did, one line each.
fn rename_unreachable_projects(central: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = projects::load(central)?.into_iter().map(|r| r.project).collect();
    names.sort_unstable();
    names.dedup();
    let unreachable: Vec<String> = names.iter().filter(|name| projects::check_name(name).is_err()).cloned().collect();
    let mut done = Vec::new();
    for old in unreachable {
        let kept: String = old.chars().map(|c| if matches!(c, ':' | '/' | '\n' | '\r') { '-' } else { c }).collect();
        let base = match kept.trim().trim_start_matches('-').trim() {
            "" => "project",
            base => base,
        };
        // At most `base` is refused (`Central`; its `-project` forms never are) and every other project takes one: of
        // these, one more than there are projects, one is free.
        let new = (0..=names.len())
            .map(|n| match n {
                0 => base.to_string(),
                1 => format!("{base}-project"),
                n => format!("{base}-project-{n}"),
            })
            .find(|name| projects::check_name(name).is_ok() && !names.contains(name))
            .expect("more names than projects");
        // One that fails is said, as `init --all` says each repository it couldn't bring up to date.
        match super::manage::rename_project_steps(central, &old, &new) {
            Ok(steps) => done.extend(steps.into_iter().map(|step| step.line)),
            Err(e) => done.push(format!("couldn't rename project `{old}`: {e:#}")),
        }
        names.push(new);
    }
    Ok(done)
}

/// Brings every registered repository up to this version: its data format, migrated with the central brain's
/// (see [`crate::migrate`]), and its agent instructions, the section in each of its agent files that has one
/// and Claude Code's hooks where `CLAUDE.md` has it. A repository whose files have no section (taken out by
/// hand) keeps them as they are, and one whose folder is gone, or whose brain a newer kanbrain wrote, is left
/// alone. Returns what changed, one line each, naming the repository.
pub fn init_all(central: &Path) -> Result<Vec<String>> {
    // Migrated first: a brain a newer kanbrain wrote stops it before anything is written.
    let mut done = migrate_all(central, &projects::load(central)?)?;
    done.extend(rename_unreachable_projects(central)?);
    let repos = projects::load(central)?;
    let mut current = 0;
    // One a newer kanbrain wrote, said by `migrate_all`, gets nothing written: not even its card or agent files.
    for repo in repos.iter().filter(|r| r.dir.is_dir() && !crate::migrate::too_new(&r.dir)) {
        // Named after a folder whose name links can't hold (`My App`), by a version before 0.4.0: renamed.
        let name = repo.name();
        if projects::check_repo_name(&name).is_err() {
            // Its siblings' links to it change with it, and one a newer kanbrain wrote can't be.
            let left =
                repos.iter().find(|r| r.project == repo.project && r.dir.is_dir() && crate::migrate::too_new(&r.dir));
            if let Some(left) = left {
                done.push(format!(
                    "{}: keeps the name `{name}`, which links can't hold, until `kb update`: `{}`'s links to it can't \
                     be rewritten",
                    repo.label(&repos),
                    left.name()
                ));
            } else {
                let fixed = projects::repo_name(central, &repo.project, &repo.dir, None)?;
                done.extend(
                    super::manage::rename_repo_steps(central, &repo.dir, &fixed)?.into_iter().map(|step| step.line),
                );
            }
        }
        // A card left on a project's old name, renamed while this folder was away.
        if let Some(card) = projects::read_repo_card(&repo.dir).filter(|card| card.project != repo.project) {
            let old = card.project.clone();
            projects::write_repo_card(&repo.dir, &projects::RepoCard { project: repo.project.clone(), ..card })?;
            done.push(format!("{}: its card named project `{old}`, now `{}`: fixed", repo.label(&repos), repo.project));
        }
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
    // "More" only after the ones listed: alone, it reads as if some were left out.
    match (current, done.is_empty()) {
        (0, _) => {}
        (_, true) => done.push(format!(
            "{} already had this version's instructions",
            counted(current, "repository", "repositories")
        )),
        (_, false) => done.push(format!("{current} more already had this version's instructions")),
    }
    Ok(done)
}

/// Brings the central brain and every registered repository's to this version's data format (see
/// [`crate::migrate`]), as `kb update` runs it. A repository a newer kanbrain wrote is said and left alone; one
/// that fails (a damaged `format` file, a step that can't write) is said too, rather than keep every other
/// repository from being brought up to date.
fn migrate_all(central: &Path, repos: &[projects::Repo]) -> Result<Vec<String>> {
    let mut done = crate::migrate::migrate(&Store::at(central)?)?;
    for repo in repos.iter().filter(|r| r.dir.is_dir()) {
        match Store::at(&repo.dir).and_then(|store| crate::migrate::migrate(&store)) {
            Ok(lines) => done.extend(lines),
            Err(e) if e.is::<crate::migrate::TooNew>() => {
                done.push(format!("{}: left alone: {e}", repo.label(repos)));
            }
            Err(e) => done.push(format!(
                "{}: not brought up to date: {e:#}; fix it, then run `kb init --all` again",
                repo.label(repos)
            )),
        }
    }
    Ok(done)
}

/// Writes or updates the section in each of `targets`, agent files of `root`, and Claude Code's hooks when
/// `CLAUDE.md` is one of them. Returns what it did, one line each, and whether that changed anything.
fn write_agent_files(root: &Path, targets: &[&str]) -> Result<Vec<(bool, String)>> {
    let mut done = Vec::new();
    // Claude Code's hooks go with its file; agents that read only AGENTS.md have no use for them.
    if targets.contains(&"CLAUDE.md") {
        match claude::add_hooks(root) {
            Ok((change, edited)) => {
                if change == claude::Change::Written {
                    done.push((
                        true,
                        format!(
                            "added kanbrain's hooks to {}: Claude Code reads `kb context` when a session starts and \
                             after compaction, reminds an agent with every prompt and at the end of its turn to \
                             describe the project while it isn't, sends it to `kb` when it would write the brain's \
                             files itself, and tells it of your comments on a task it's working on with its next tool",
                            claude::SETTINGS
                        ),
                    ));
                } else if edited.is_empty() {
                    done.push((false, format!("{} already has kanbrain's hooks", claude::SETTINGS)));
                }
                done.extend(edited.iter().map(|hook| {
                    let todo = format!(
                        "kanbrain added none beside it; if it's kanbrain's, replace it with `{0}` so kanbrain can \
                         update and remove it, and if it isn't, add one running `{0}` for {1} beside it",
                        hook.hook, hook.event
                    );
                    (false, looks_like_kanbrains(hook, &todo))
                }));
            }
            // The section still works without them, so a settings file it can't read doesn't stop init.
            Err(e) => done.push((false, format!("left out the Claude Code hooks: {e:#}"))),
        }
    }
    for &name in targets {
        let path = root.join(name);
        // Only a missing file is empty: one it can't read (not UTF-8) would be written over with the section alone.
        let before = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                done.push((false, format!("left {name} as it is: can't read it ({e}); add the section by hand")));
                continue;
            }
        };
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

/// What's said of a hook that looks like kanbrain's but isn't (see [`claude::Edited`]): what it runs, then `todo`,
/// what kanbrain did with it and what's left to the user.
fn looks_like_kanbrains(hook: &claude::Edited, todo: &str) -> String {
    // Its line breaks and other control characters escaped, so that it's said on one line.
    let shown: String = hook
        .command
        .chars()
        .map(|c| if c.is_control() { c.escape_default().to_string() } else { c.to_string() })
        .collect();
    format!(
        "{}: a hook for {} runs `{shown}`, which looks like kanbrain's `{}`: {todo}",
        claude::SETTINGS,
        hook.event,
        hook.hook
    )
}

/// Whether a program named `name` is in one of the folders of `path`, a `PATH` value.
fn on_path(path: Option<std::ffi::OsString>, name: &str) -> bool {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    path.is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(&file).is_file()))
}

/// Takes the repository whose `.brain/` is `dir` out of the project registry, deleting the project's
/// card if it was its last repository, and takes the section out of each agent file of the repository
/// (deleting a file left empty, which `init` created). `.brain/` is left as it is: whether it goes is
/// [`super::manage::remove_repo`]'s. Returns what it did, one line each.
pub fn deinit(dir: &Path, central: &Path) -> Result<Vec<Step>> {
    let Some(repo) = projects::unregister(central, dir)? else {
        bail!("{} isn't a registered project: there's nothing to deinit", dir.display());
    };
    let mut done: Vec<Step> =
        vec![Step::other(format!("unregistered {}: the kanban app no longer shows it", repo.dir.display()))];
    let others: Vec<String> =
        projects::load(central)?.iter().filter(|r| r.project == repo.project).map(projects::Repo::name).collect();
    if !others.is_empty() {
        let line =
            format!("project `{}` keeps its card for its other repositories: {}", repo.project, others.join(", "));
        done.push(Step::new(StepKind::KeptCard, line));
    } else if projects::remove_description(central, &repo.project)? {
        done.push(Step::other(format!("removed project `{}`'s card from the central brain", repo.project)));
    }
    let Some(root) = repo.dir.parent().filter(|_| repo.dir.is_dir()) else {
        done.push(Step::other(format!("{} is gone, so there was nothing else to undo", repo.dir.display())));
        return Ok(done);
    };
    match claude::remove_hooks(root) {
        Ok((change, edited)) => {
            match change {
                claude::Change::Unchanged => {}
                claude::Change::Written => {
                    done.push(Step::other(format!("removed kanbrain's hooks from {}", claude::SETTINGS)));
                }
                claude::Change::Removed => {
                    done.push(Step::other(format!("removed {}: it only held the hooks", claude::SETTINGS)));
                }
            }
            let todo = "kanbrain left it in place; if it's kanbrain's, remove it yourself";
            done.extend(edited.iter().map(|hook| Step::other(looks_like_kanbrains(hook, todo))));
        }
        Err(e) => done.push(Step::other(format!("left the Claude Code hooks in place: {e:#}"))),
    }
    for name in AGENT_FILES {
        let path = root.join(name);
        let Ok(before) = fs::read_to_string(&path) else { continue };
        let after = without_section(&before);
        if after == before {
            continue;
        }
        if after.trim().is_empty() {
            store::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            done.push(Step::other(format!("removed {name}: it only held the \"Project memory\" section")));
        } else {
            store::write_file(&path, &after)?;
            done.push(Step::other(format!("removed the \"Project memory\" section from {name}")));
        }
    }
    Ok(done)
}

/// Where the section is in `text`, with the line break after its end marker. It's the last start
/// marker before the first end marker, so a stray unclosed start marker above it is never taken for
/// the start and the text after it is kept.
fn section(text: &str) -> Option<(usize, usize)> {
    let end = text.find(END)? + END.len();
    let start = text[..end].rfind(START)?;
    // `\n\r\n` is what an older kanbrain left in a CRLF file: its section ended with `\n`, and the `\r\n`
    // after the end marker it found there stayed, so both go, or the blank line they make would stay for good.
    let eol = ["\n\r\n", "\r\n", "\n"].into_iter().find(|eol| text[end..].starts_with(eol)).unwrap_or("");
    Some((start, end + eol.len()))
}

/// The line break `text` uses, `\r\n` or `\n`, for the section and the blank lines around it: git's
/// `core.autocrlf` checks agent files out with CRLF on Windows, and a section written with LF into one
/// would never equal what's there, so every `init` would rewrite it. It's the first line break of the
/// user's text, before the section, else after it, as editors go by the first: a file with both is one
/// an older kanbrain wrote its section with LF into, and the section's own breaks are kanbrain's. A
/// file with no other text goes by its last break, which that older section left as the CRLF it found.
fn line_break(text: &str) -> &'static str {
    let outside = match section(text) {
        Some((start, end)) => [&text[..start], &text[end..]].into_iter().find(|part| part.contains('\n')),
        None => Some(text),
    };
    match outside.and_then(|part| part.find('\n').map(|at| &part[..at])) {
        Some(line) if line.ends_with('\r') => "\r\n",
        None if text.ends_with("\r\n") => "\r\n",
        Some(_) | None => "\n",
    }
}

/// `text` with the section between the markers: replaced where it already is, else appended after
/// a blank line. Written with `text`'s line breaks.
fn with_section(text: &str) -> String {
    let eol = line_break(text);
    let block = format!("{START}\n{SECTION}\n{END}\n").replace('\n', eol);
    if let Some((start, end)) = section(text) {
        return format!("{}{block}{}", &text[..start], &text[end..]);
    }
    // A byte order mark alone is an empty file to the user (`trim` keeps it: it isn't whitespace).
    match text.trim_end() {
        kept @ ("" | BOM) => format!("{kept}{block}"),
        kept => format!("{kept}{eol}{eol}{block}"),
    }
}

/// `text` without the section, and without the blank line `with_section` put before it. Empty, for the
/// file to be removed, when nothing but a byte order mark is left.
fn without_section(text: &str) -> String {
    let Some((start, end)) = section(text) else { return text.to_string() };
    let eol = line_break(text);
    let before = text[..start].trim_end();
    let after = text[end..].trim_start_matches(['\r', '\n']);
    match (before, after) {
        ("" | BOM, "") => String::new(),
        ("" | BOM, after) => format!("{before}{after}"),
        (before, "") => format!("{before}{eol}"),
        (before, after) => format!("{before}{eol}{eol}{after}"),
    }
}

#[cfg(test)]
#[path = "../../tests/cli/init.rs"]
mod tests;
