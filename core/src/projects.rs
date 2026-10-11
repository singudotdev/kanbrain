//! Projects: work that keeps its tasks next to its code, in one or more repositories. Each repository
//! has its own `.brain/` (its own tasks, numbered from 1) and belongs to a project; several services
//! of one project are several repositories with the same project name. `init` registers each in
//! the central brain's `projects.md`, so the kanban app can show every project and filter by one,
//! or by one of its repositories.
//!
//! What every repository of a project shares is its card, `projects/<name>.md` in the central brain:
//! the application's horizontal context (what it is, its services and how they connect, conventions
//! they share). A repository adds its role among them (what it owns, exposes and consumes) in its
//! `.brain/project.md`, whose title also says which project it belongs to. How to work on one
//! repository's code (build, test, style) is its `CLAUDE.md` / `AGENTS.md`'s, which agents already
//! read, so none of it is repeated here.
//!
//! Entries outside any project live in the central brain: `$KANBRAIN_DIR`, else the platform
//! data folder.

use crate::model::slugify;
use crate::store::{self, Store};
use anyhow::{Context, Result, bail};
use std::fmt::Write;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// What a project's description holds, as the agent section, `context` and the hooks ask for it, in one
/// wording so they can't drift apart. A literal for `concat!`, since the agent section is a `const`. The
/// bounds are spelled out because agents asked for "a few lines" still added tables, columns and function
/// signatures, details the code already holds and that go stale.
macro_rules! description_scope {
    () => {
        "what the app is, its services and how they connect, conventions they share: the big picture, in at most \
         five lines, without tables, columns, function signatures, files or dependencies, which the code \
         already says"
    };
}
pub(crate) use description_scope;

/// In the central brain: every registered repository, `- project: path of its .brain`.
pub const REGISTRY_FILE: &str = "projects.md";
/// In a repository's `.brain/`: the project it belongs to (the `#` title) and notes about it.
pub const PROJECT_FILE: &str = "project.md";
/// In the central brain: the projects' cards, one `<name>.md` each.
const CARDS_DIR: &str = "projects";
/// What the central brain is called where projects are listed; no project can take the name.
pub const CENTRAL: &str = "central";

/// What a new project's card says until someone describes the project.
const DESCRIPTION_TEMPLATE: &str = "What the application is, its services and how they connect, and the \
conventions they share: what an agent should know before touching any of its repositories.";
/// The placeholder of cards made by earlier versions, still read as no description.
const OLD_DESCRIPTION_TEMPLATE: &str = "What this project is for, its stack and its conventions: \
what an agent should know before touching any of its repositories.";

/// A registered repository of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The project it belongs to; repositories with the same project share its card.
    pub project: String,
    /// Its `.brain/` folder.
    pub dir: PathBuf,
}

impl Repo {
    /// The repository's name: the folder holding its `.brain/`.
    pub fn name(&self) -> String {
        self.dir
            .parent()
            .and_then(Path::file_name)
            .map_or_else(|| self.project.clone(), |n| n.to_string_lossy().into_owned())
    }

    /// How it's named among `repos`: `shop` when it's the project's only repository, else
    /// `shop/api`. This is what `--project` takes.
    pub fn label(&self, repos: &[Repo]) -> String {
        if repos.iter().filter(|r| r.project == self.project).count() > 1 {
            format!("{}/{}", self.project, self.name())
        } else {
            self.project.clone()
        }
    }
}

/// A repository's view of its project: the shared card, and its own notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    /// The project's name.
    pub name: String,
    /// The application's horizontal context (markdown), shared by its repositories.
    pub description: String,
    /// The repository's name.
    pub repo: String,
    /// This repository's role among the project's (markdown); empty with a single repository.
    pub notes: String,
    /// The project's other repositories, as `(name, .brain folder)`.
    pub siblings: Vec<(String, PathBuf)>,
}

/// The central brain's folder: `$KANBRAIN_DIR`, else the platform data folder.
pub fn central_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("KANBRAIN_DIR") {
        return Ok(dir.into());
    }
    Ok(dirs::data_dir().context("could not determine the data directory; set KANBRAIN_DIR")?.join("kanbrain"))
}

/// The registered repositories, in the order they were registered. No registry means none yet.
pub fn load(central: &Path) -> Result<Vec<Repo>> {
    let path = central.join(REGISTRY_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("- ")?.split_once(": "))
        .map(|(project, dir)| Repo { project: project.trim().to_string(), dir: PathBuf::from(dir.trim()) })
        .collect())
}

/// The repository `label` names (`shop`, or `shop/api` when the project has several); the error
/// lists the labels there are.
pub fn find(central: &Path, label: &str) -> Result<Repo> {
    let repos = load(central)?;
    if let Some(repo) = repos.iter().find(|r| r.label(&repos) == label) {
        return Ok(repo.clone());
    }
    if repos.is_empty() {
        bail!("no project `{label}`: none is registered yet (see `kb init`)");
    }
    let labels: Vec<String> = repos.iter().map(|r| r.label(&repos)).collect();
    let hint = if repos.iter().any(|r| r.project == label) { " (it has several repositories: name one)" } else { "" };
    bail!("no project `{label}`{hint}; there are {}, and `{CENTRAL}`", labels.join(", "))
}

/// Registers the repository in `dir` as part of `project`, moving it if it was registered under
/// another project. Two live repositories with the same name in one project are refused, since
/// `--project` and the app's filter tell them apart by name; a registration whose folder is gone
/// gives way.
pub fn register(central: &Path, project: &str, dir: &Path) -> Result<()> {
    check_name(project)?;
    let mut repos = load(central)?;
    let new = Repo { project: project.to_string(), dir: dir.to_path_buf() };
    if repos.contains(&new) {
        return Ok(());
    }
    repos.retain(|r| r.dir != dir);
    if let Some(twin) = repos.iter().find(|r| r.project == project && r.name() == new.name() && r.dir.is_dir()) {
        bail!(
            "project `{project}` already has a repository named `{}`, at {}; rename one of the folders",
            new.name(),
            twin.dir.display()
        );
    }
    repos.retain(|r| !(r.project == project && r.name() == new.name()));
    repos.push(new);
    save(central, &repos)
}

/// Takes the repository whose `.brain/` is `dir` out of the registry, and returns it; `None` if it
/// wasn't registered. Its folder may be gone: a deleted repository can still be taken out.
pub fn unregister(central: &Path, dir: &Path) -> Result<Option<Repo>> {
    let mut repos = load(central)?;
    let Some(at) = repos.iter().position(|r| same_dir(&r.dir, dir)) else { return Ok(None) };
    let repo = repos.remove(at);
    save(central, &repos)?;
    Ok(Some(repo))
}

/// Names are written as `- name: path` lines and typed after `--project`, where `/` separates the
/// repository, so they're one line without `:` or `/`, and not the central brain's name.
fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.contains([':', '/', '\n']) || name.trim() != name {
        bail!("`{name}` can't be a project name: it needs to be one line, without `:`, `/` or surrounding spaces");
    }
    if name == CENTRAL {
        bail!("`{CENTRAL}` is the name of the central brain; pick another: `kb init NAME`");
    }
    Ok(())
}

/// Writes the registry.
fn save(central: &Path, repos: &[Repo]) -> Result<()> {
    Store::at(central)?;
    let mut text = String::from(
        "# Projects\n\nRepositories with their own `.brain/`, registered by `kb init`; several lines \
         with the same name are one project's repositories. The kanban app shows them all.\n\n",
    );
    for r in repos {
        writeln!(text, "- {}: {}", r.project, r.dir.display()).expect("writing to a String cannot fail");
    }
    store::write_file(&central.join(REGISTRY_FILE), &text)
}

/// The project a `.brain/` folder says it belongs to, and its notes about itself.
pub fn read_repo_card(dir: &Path) -> Option<(String, String)> {
    let text = fs::read_to_string(dir.join(PROJECT_FILE)).ok()?;
    let mut lines = text.lines();
    let name = lines.by_ref().find_map(|l| l.strip_prefix("# "))?.trim().to_string();
    Some((name, lines.collect::<Vec<_>>().join("\n").trim().to_string()))
}

/// Writes a `.brain/` folder's card: the project it belongs to, and its notes.
pub fn write_repo_card(dir: &Path, project: &str, notes: &str) -> Result<()> {
    let notes = notes.trim();
    let text = if notes.is_empty() { format!("# {project}\n") } else { format!("# {project}\n\n{notes}\n") };
    store::write_file(&dir.join(PROJECT_FILE), &text)
}

/// Where a project's shared card is in the central brain.
fn card_path(central: &Path, project: &str) -> PathBuf {
    central.join(CARDS_DIR).join(format!("{}.md", slugify(project)))
}

/// The project's shared description; empty if it has no card, or only the placeholder `init` wrote,
/// so nobody reads the placeholder as a description (an agent pays for it in every `context`).
pub fn read_description(central: &Path, project: &str) -> String {
    let text = fs::read_to_string(card_path(central, project)).unwrap_or_default();
    let body = text.lines().skip_while(|l| !l.starts_with("# ")).skip(1).collect::<Vec<_>>().join("\n");
    match body.trim() {
        DESCRIPTION_TEMPLATE | OLD_DESCRIPTION_TEMPLATE => String::new(),
        described => described.to_string(),
    }
}

/// Writes the project's shared card.
pub fn write_description(central: &Path, project: &str, description: &str) -> Result<()> {
    store::write_file(&card_path(central, project), &format!("# {project}\n\n{}\n", description.trim()))
}

/// Creates the project's shared card with a placeholder if it has none yet; returns whether it did.
pub fn ensure_description(central: &Path, project: &str) -> Result<bool> {
    if card_path(central, project).is_file() {
        return Ok(false);
    }
    write_description(central, project, DESCRIPTION_TEMPLATE)?;
    Ok(true)
}

/// Deletes the project's shared card; returns whether there was one.
pub fn remove_description(central: &Path, project: &str) -> Result<bool> {
    let path = card_path(central, project);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
    }
}

/// What the repository in `dir` knows of its project: `None` outside a project. Without `central`,
/// only what its own card says.
pub fn info(dir: &Path, central: Option<&Path>) -> Option<ProjectInfo> {
    let (name, notes) = read_repo_card(dir)?;
    let repo = Repo { project: name.clone(), dir: dir.to_path_buf() }.name();
    let (description, siblings) = match central {
        Some(central) => {
            let siblings = load(central)
                .unwrap_or_default()
                .into_iter()
                .filter(|r| r.project == name && !same_dir(&r.dir, dir))
                .map(|r| (r.name(), r.dir))
                .collect();
            (read_description(central, &name), siblings)
        }
        None => (String::new(), Vec::new()),
    };
    Some(ProjectInfo { name, description, repo, notes, siblings })
}

/// Whether two paths are the same folder, however they're written: a registered path may go through a
/// symlink (`/var` is `/private/var` on macOS) or be typed by hand, and still name the folder opened.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || fs::canonicalize(a).is_ok_and(|a| fs::canonicalize(b).ok() == Some(a))
}

#[cfg(test)]
#[path = "../tests/projects.rs"]
mod tests;
