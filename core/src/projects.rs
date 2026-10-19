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

use crate::model::{infallible, slugify};
use crate::store::{self, Store};
use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::fmt::Write;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// What a project's description holds, as `context` and the hooks ask for it, in one wording so they can't drift
/// apart; the agent section spells it out too, wrapped, and a test holds it to this. A literal for `concat!`. The
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
pub(crate) const CARDS_DIR: &str = "projects";
/// What the central brain is called where projects are listed; no project can take the name.
pub const CENTRAL: &str = "central";

/// What a new project's card says until someone describes the project.
const DESCRIPTION_TEMPLATE: &str = "What the application is, its services and how they connect, and the \
conventions they share: what an agent should know before touching any of its repositories.";

/// A registered repository of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The project it belongs to; repositories with the same project share its card.
    pub project: String,
    /// Its `.brain/` folder.
    pub dir: PathBuf,
}

impl Repo {
    /// The repository's name in its project: the one its card gives, else the folder holding its `.brain/`.
    pub fn name(&self) -> String {
        read_repo_card(&self.dir).and_then(|card| card.repo).unwrap_or_else(|| self.folder())
    }

    /// The name of the folder holding its `.brain/`, the repository's name unless its card gives another.
    pub fn folder(&self) -> String {
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

/// The central brain's folder: `$KANBRAIN_DIR`, else the platform data folder. Errors when `$KANBRAIN_DIR` is a
/// relative path, which would name another folder from each folder `kb` runs in.
pub fn central_dir() -> Result<PathBuf> {
    central_dir_from(std::env::var_os("KANBRAIN_DIR"), data_dir)
}

/// The platform's data folder.
#[cfg(not(test))]
fn data_dir() -> Option<PathBuf> {
    dirs::data_dir()
}

/// In this crate's tests, a temporary folder instead: a test that loses its central brain (a mutant of how
/// `KANBRAIN_DIR` is read) once wrote into the real one of whoever ran them.
#[cfg(test)]
#[expect(clippy::unnecessary_wraps, reason = "the signature of `dirs::data_dir`, which it stands in for")]
fn data_dir() -> Option<PathBuf> {
    Some(std::env::temp_dir().join("kanbrain-tests-data"))
}

/// [`central_dir`] for the value `KANBRAIN_DIR` has, `data_dir` giving the platform's data folder.
fn central_dir_from(var: Option<OsString>, data_dir: impl FnOnce() -> Option<PathBuf>) -> Result<PathBuf> {
    // Set but empty is unset (`KANBRAIN_DIR= kb …`): taken as is, it made whatever folder `kb` ran in the central
    // brain, writing `format` there.
    if let Some(dir) = var.filter(|v| !v.is_empty()).map(PathBuf::from) {
        // Hooks run from each project's folder, so a relative one would split the central brain among them. On
        // Windows `\todo` (rooted, no drive) and `D:todo` count as relative: each depends on the current folder.
        if !dir.is_absolute() {
            let what = dir.display();
            // Only a shell expands `~`, and not inside quotes: what sets the variable without one passes it on as is.
            if dir.as_os_str().as_encoded_bytes().starts_with(b"~") {
                bail!(
                    "KANBRAIN_DIR must be an absolute path, not `{what}`: `~` isn't expanded there (as when quoted, or \
                     in a systemd environment.d file); write it with the home folder's full path"
                );
            }
            bail!("KANBRAIN_DIR must be an absolute path, not `{what}`: set it to the folder's full path");
        }
        return Ok(dir);
    }
    Ok(data_dir().context("could not determine the data directory; set KANBRAIN_DIR")?.join("kanbrain"))
}

/// The registered repositories, in the order they were registered. No registry means none yet.
pub fn load(central: &Path) -> Result<Vec<Repo>> {
    let path = central.join(REGISTRY_FILE);
    let text = match store::read_text(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("- ")?.split_once(": "))
        // Plain, as `init` registers it now: released versions registered Windows' verbatim form (`\\?\C:\…`).
        .map(|(project, dir)| Repo { project: project.trim().to_string(), dir: simplified(Path::new(dir.trim())) })
        .collect())
}

/// The repository `label` names (`shop`, or `shop/api` when the project has several), in any case when that
/// names only one; the error lists the labels there are.
pub fn find(central: &Path, label: &str) -> Result<Repo> {
    let repos = load(central)?;
    if let Some(repo) = repos.iter().find(|r| r.label(&repos) == label) {
        return Ok(repo.clone());
    }
    // Typed as it's said (`acme corp` for `Acme Corp`), when that names only one.
    let lower = label.to_lowercase();
    let mut alike = repos.iter().filter(|r| r.label(&repos).to_lowercase() == lower);
    if let (Some(repo), None) = (alike.next(), alike.next()) {
        return Ok(repo.clone());
    }
    if repos.is_empty() {
        bail!("no project `{label}`: none is registered yet (see `kb init`)");
    }
    let labels: Vec<String> = repos.iter().map(|r| r.label(&repos)).collect();
    // A project of several repositories exists, but `--project` takes one of them.
    let several: Vec<String> = repos.iter().filter(|r| r.project == label).map(|r| r.label(&repos)).collect();
    if !several.is_empty() {
        bail!("project `{label}` has several repositories: name one, {}", several.join(", "));
    }
    bail!("no project `{label}`; there are {}, and `{CENTRAL}`", labels.join(", "))
}

/// Registers the repository in `dir` as part of `project`, moving it if it was registered under
/// another project. Two live repositories with the same name in one project are refused, since
/// `--project` and the app's filter tell them apart by name; a registration whose folder is gone
/// gives way.
pub fn register(central: &Path, project: &str, dir: &Path) -> Result<()> {
    check_name(project)?;
    let _lock = lock(central)?;
    let mut repos = load(central)?;
    let new = Repo { project: project.to_string(), dir: dir.to_path_buf() };
    // By folder, not path: `init` registers the canonical one, which another registration's may not be (`/var`
    // is `/private/var` on macOS, Windows prefixes `\\?\`), and a move must replace it, not add a second.
    if repos.iter().any(|r| r.project == project && same_dir(&r.dir, dir)) {
        // As it was, unless a released version wrote it with Windows' verbatim prefix, which `load` read plain.
        let text = store::read_text(&central.join(REGISTRY_FILE)).unwrap_or_default();
        return if text.contains(r"\\?\") && text != render(&repos) { save(central, &repos) } else { Ok(()) };
    }
    repos.retain(|r| !same_dir(&r.dir, dir));
    if let Some(twin) = repos.iter().find(|r| r.project == project && r.name() == new.name() && r.dir.is_dir()) {
        bail!(
            "project `{project}` already has a repository named `{}`, at {}; name this one with `kb init --repo NAME`, \
             or rename that one with `kb project repo NAME`",
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
    let _lock = lock(central)?;
    let mut repos = load(central)?;
    let Some(at) = repos.iter().position(|r| same_dir(&r.dir, dir)) else { return Ok(None) };
    let repo = repos.remove(at);
    save(central, &repos)?;
    Ok(Some(repo))
}

/// The name the repository whose `.brain/` is `dir` takes in `project`: `wanted` if given, refused when another
/// of the project's repositories has it; else the one its card gives if it stays in that project, else its
/// folder's. When another repository has that one (two folders named `app` in different places), the folder
/// above goes before it (`backend-app`), then a number: two repositories of a project can't share a name, as
/// `--project` and links (`app#3`) tell them apart by it.
pub fn repo_name(central: &Path, project: &str, dir: &Path, wanted: Option<&str>) -> Result<String> {
    // One whose folder is gone keeps its name while it's registered, for its folder may come back (an unmounted
    // disk): taken, two repositories would answer to it.
    let others: Vec<Repo> =
        load(central)?.into_iter().filter(|r| r.project == project && !same_dir(&r.dir, dir)).collect();
    let taken: Vec<String> = others.iter().map(Repo::name).collect();
    if let Some(wanted) = wanted {
        check_repo_name(wanted)?;
        if others.iter().any(|r| r.dir.is_dir() && r.name() == wanted) {
            bail!("project `{project}` already has a repository named `{wanted}`; pick another");
        }
        if taken.iter().any(|t| t == wanted) {
            bail!(
                "project `{project}` has a repository named `{wanted}` whose folder is gone: take it out with \
                 `kb --project {project}/{wanted} deinit`, or pick another name"
            );
        }
        return Ok(wanted.to_string());
    }
    let here = Repo { project: project.to_string(), dir: dir.to_path_buf() };
    let card = read_repo_card(dir).filter(|card| card.project == project).and_then(|card| card.repo);
    // A folder named `My App` makes a name links can hold (`My-App#3`), kept on the card as it isn't the folder's;
    // so does a name a card kept from before 0.4.0 that links can't hold (`My Work-app`, `-app`).
    let own = card.unwrap_or_else(|| here.folder());
    let own = if check_repo_name(&own).is_ok() { own } else { repo_name_of_folder(&own) };
    let above = dir.parent().and_then(Path::parent).and_then(Path::file_name).map(|n| n.to_string_lossy());
    let candidates = std::iter::once(own.clone())
        .chain(above.map(|above| format!("{}-{own}", repo_name_of_folder(&above))))
        .chain((2..).map(|n| format!("{own}-{n}")));
    Ok(candidates.into_iter().find(|name| !taken.contains(name)).expect("the numbered names never run out"))
}

/// Names are written as `- name: path` lines and typed after `--project`, where `/` separates the
/// repository, so they're one line without `:` or `/`, and not the central brain's name.
pub fn check_name(name: &str) -> Result<()> {
    // Nor start with `-`: typed after `project rename` or `move`, it's an option mistyped, not a name.
    if name.trim().is_empty() || name.contains([':', '/', '\n', '\r']) || name.trim() != name || name.starts_with('-') {
        bail!(
            "`{name}` can't be a project name: it needs to be one line, without `:`, `/`, surrounding spaces or a \
             leading `-`"
        );
    }
    // In any case: `--project Central` reaches the central brain, and a project `Central` would sit beside it.
    if name.eq_ignore_ascii_case(CENTRAL) {
        bail!("`{CENTRAL}` is the name of the central brain; pick another: `kb init NAME`");
    }
    Ok(())
}

/// The central brain's write lock, held from reading the registry to writing it back: two `kb init`s at once
/// would otherwise each register their repository over the other's.
fn lock(central: &Path) -> Result<store::BrainLock> {
    Store::at(central)?.lock()
}

/// Writes the registry.
fn save(central: &Path, repos: &[Repo]) -> Result<()> {
    Store::at(central)?;
    store::write_file(&central.join(REGISTRY_FILE), &render(repos))
}

/// The registry's text, as [`save`] writes it.
fn render(repos: &[Repo]) -> String {
    let mut text = String::from(
        "# Projects\n\nRepositories with their own `.brain/`, registered by `kb init`; several lines \
         with the same name are one project's repositories. The kanban app shows them all.\n\n",
    );
    for r in repos {
        infallible(writeln!(text, "- {}: {}", r.project, r.dir.display()));
    }
    text
}

/// What a repository's `.brain/project.md` says about it, travelling with its code (and its git):
///
/// ```markdown
/// # shop
/// - repo: web
///
/// The store: UI only.
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoCard {
    /// The project it belongs to: the `#` title.
    pub project: String,
    /// Its name in the project, when it isn't its folder's: the `- repo:` line right under the title. In the
    /// card rather than the central registry, so links to it (`web#3`) read the same on every machine.
    pub repo: Option<String>,
    /// Its role among the project's repositories (markdown).
    pub notes: String,
}

/// A `.brain/` folder's card; `None` if it has none (not a project's repository).
pub fn read_repo_card(dir: &Path) -> Option<RepoCard> {
    let text = store::read_text(&dir.join(PROJECT_FILE)).ok()?;
    let mut lines = text.lines();
    let project = lines.by_ref().find_map(|l| l.strip_prefix("# "))?.trim().to_string();
    let rest: Vec<&str> = lines.collect();
    // Only right under the title: notes start after a blank line, so a note's own `- repo:` stays a note.
    let (repo, notes) = match rest.first().and_then(|l| l.strip_prefix("- repo:")) {
        Some(name) => (Some(name.trim().to_string()).filter(|n| !n.is_empty()), &rest[1..]),
        None => (None, &rest[..]),
    };
    Some(RepoCard { project, repo, notes: notes.join("\n").trim().to_string() })
}

/// Writes a `.brain/` folder's card.
pub fn write_repo_card(dir: &Path, card: &RepoCard) -> Result<()> {
    let mut text = format!("# {}\n", card.project);
    if let Some(repo) = &card.repo {
        infallible(writeln!(text, "- repo: {repo}"));
    }
    let notes = card.notes.trim();
    if !notes.is_empty() {
        infallible(write!(text, "\n{notes}\n"));
    }
    store::write_file(&dir.join(PROJECT_FILE), &text)
}

/// Repository names are typed after `--project NAME/` and written in links as `REPO#ID`, which a space, `,` or `#`
/// would split, so they're one word without those or `:` and `/`.
pub fn check_repo_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains(not_in_repo_name) || name.starts_with('-') {
        bail!(
            "`{name}` can't be a repository name: it needs to be one word, without `,`, `#`, `:`, `/` or a leading `-`"
        );
    }
    Ok(())
}

/// A character a repository name can't hold (see [`check_repo_name`]).
fn not_in_repo_name(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | '#' | ':' | '/')
}

/// A folder's name made a repository name: the characters one can't hold become `-` (`My App` is `My-App`).
pub(crate) fn repo_name_of_folder(folder: &str) -> String {
    let name: String = folder.chars().map(|c| if not_in_repo_name(c) { '-' } else { c }).collect();
    let name = name.trim_matches('-');
    if name.is_empty() { "repo".to_string() } else { name.to_string() }
}

/// The folder of the projects' shared cards in the central brain.
pub fn cards_dir(central: &Path) -> PathBuf {
    central.join(CARDS_DIR)
}

/// Where a project's shared card is in the central brain: `projects/shop.md` for a name that is its own slug, as
/// every card was before 0.4.0. Any other name (`My_App`, `Shop`, `日記`) adds a hash of itself, since names that
/// slug alike (`my_app` and `my-app`; every name in another script slugs to `task`) shared one card, each
/// overwriting the other's description.
fn card_path(central: &Path, project: &str) -> PathBuf {
    let slug = slugify(project);
    let name =
        if slug == project { slug } else { format!("{slug}-{:08x}", store::fnv1a(project.as_bytes()) & 0xffff_ffff) };
    cards_dir(central).join(format!("{name}.md"))
}

/// The project a card is titled with (`# shop`), which is whose it is.
fn card_owner(path: &Path) -> Option<String> {
    let text = store::read_text(path).ok()?;
    text.lines().find_map(|l| l.strip_prefix("# ")).map(|name| name.trim().to_string())
}

/// The card a project kept before 0.4.0 under its slug alone, when it has one there that is its own.
fn legacy_card(central: &Path, project: &str) -> Option<PathBuf> {
    let path = cards_dir(central).join(format!("{}.md", slugify(project)));
    (path != card_path(central, project) && card_owner(&path).as_deref() == Some(project)).then_some(path)
}

/// The card holding the project's description: its own, else the one it kept before 0.4.0. One titled with
/// another project, which a name slugging alike overwrote, isn't it.
fn current_card(central: &Path, project: &str) -> Option<PathBuf> {
    let path = card_path(central, project);
    if card_owner(&path).as_deref() == Some(project) { Some(path) } else { legacy_card(central, project) }
}

/// The project's shared description; empty if it has no card, or only the placeholder `init` wrote,
/// so nobody reads the placeholder as a description (an agent pays for it in every `context`).
pub fn read_description(central: &Path, project: &str) -> String {
    let text = current_card(central, project).and_then(|path| store::read_text(&path).ok()).unwrap_or_default();
    let body = card_body(&text);
    if body == DESCRIPTION_TEMPLATE { String::new() } else { body }
}

/// What a card says under its `# name` title, trimmed.
fn card_body(text: &str) -> String {
    text.lines().skip_while(|l| !l.starts_with("# ")).skip(1).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// Writes the project's shared card. Another project's card where it goes, one whose name slugs alike from before
/// 0.4.0, moves to its own place first rather than be overwritten; the project's own card from then goes.
pub fn write_description(central: &Path, project: &str, description: &str) -> Result<()> {
    // Under the lock, as the registry: whose card is where is read, then written.
    let _lock = lock(central)?;
    let path = card_path(central, project);
    if let Some(owner) = card_owner(&path).filter(|owner| owner != project) {
        let theirs = card_path(central, &owner);
        if theirs != path && !theirs.exists() {
            store::rename(&path, &theirs)
                .with_context(|| format!("moving {} to {}", path.display(), theirs.display()))?;
        }
    }
    let legacy = legacy_card(central, project);
    store::write_file(&path, &format!("# {project}\n\n{}\n", description.trim()))?;
    if let Some(legacy) = legacy {
        store::remove_file(&legacy).with_context(|| format!("removing {}", legacy.display()))?;
    }
    Ok(())
}

/// Creates the project's shared card with a placeholder if it has none yet; returns whether it did.
pub fn ensure_description(central: &Path, project: &str) -> Result<bool> {
    let _lock = lock(central)?;
    if current_card(central, project).is_some() {
        return Ok(false);
    }
    write_description(central, project, DESCRIPTION_TEMPLATE)?;
    Ok(true)
}

/// Renames project `old` to `new` in the registry and its shared card (the file and its title). Its
/// repositories' own cards are the caller's: their folders may be elsewhere, or gone.
pub fn rename(central: &Path, old: &str, new: &str) -> Result<()> {
    let _lock = lock(central)?;
    let mut repos = load(central)?;
    for repo in repos.iter_mut().filter(|r| r.project == old) {
        new.clone_into(&mut repo.project);
    }
    save(central, &repos)?;
    let Some(from) = current_card(central, old) else { return Ok(()) };
    let text = store::read_text(&from).with_context(|| format!("reading {}", from.display()))?;
    let body = card_body(&text);
    // The old card already where the new one goes (`Shop`'s from before 0.4.0, renamed `shop`): rewritten there,
    // not moved aside as another project's would be.
    if card_path(central, new) == from {
        return store::write_file(&from, &format!("# {new}\n\n{}\n", body.trim()));
    }
    write_description(central, new, &body)?;
    store::remove_file(&from).with_context(|| format!("removing {}", from.display()))?;
    Ok(())
}

/// Deletes the project's shared card; returns whether there was one. Only its own: one a name slugging alike
/// wrote over it before 0.4.0 is that project's.
pub fn remove_description(central: &Path, project: &str) -> Result<bool> {
    let _lock = lock(central)?;
    let Some(path) = current_card(central, project) else { return Ok(false) };
    store::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    Ok(true)
}

/// What the repository in `dir` knows of its project: `None` outside a project. Without `central`,
/// only what its own card says.
pub fn info(dir: &Path, central: Option<&Path>) -> Option<ProjectInfo> {
    let RepoCard { project: name, notes, .. } = read_repo_card(dir)?;
    let (name, description, siblings) = match central {
        Some(central) => {
            let repos = load(central).unwrap_or_default();
            // The registry's name over the card's: a rename while this folder was away (an unmounted disk) left
            // the card on the old one, which `kb init --all` then fixes.
            let name = repos.iter().find(|r| same_dir(&r.dir, dir)).map_or(name, |r| r.project.clone());
            let siblings = repos
                .into_iter()
                .filter(|r| r.project == name && !same_dir(&r.dir, dir))
                .map(|r| (r.name(), r.dir))
                .collect();
            (name.clone(), read_description(central, &name), siblings)
        }
        None => (name, String::new(), Vec::new()),
    };
    let repo = Repo { project: name.clone(), dir: dir.to_path_buf() }.name();
    Some(ProjectInfo { name, description, repo, notes, siblings })
}

/// Whether two paths are the same folder, however they're written: a registered path may go through a
/// symlink (`/var` is `/private/var` on macOS) or be typed by hand, and still name the folder opened.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || fs::canonicalize(a).is_ok_and(|a| fs::canonicalize(b).ok() == Some(a))
}

/// `path`'s canonical form, as `init` registers it and the app opens it: [`fs::canonicalize`], then
/// [`simplified`], so a repository reads `C:\code\api` in `projects.md`, `kb`'s output and the app rather than
/// `\\?\C:\code\api`, and matches the same path typed or given without the prefix.
pub fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    fs::canonicalize(path).map(|path| simplified(&path))
}

/// `path` without Windows' verbatim prefix, which canonicalizing a path there adds (`\\?\C:\…` as `C:\…`,
/// `\\?\UNC\server\share\…` as `\\server\share\…`), when the plain path names the same folder; else, or when
/// it isn't verbatim, `path` as it is. Pure, so it's the same on every system: elsewhere no path starts with
/// `\\?\` but one read from a registry Windows wrote.
pub fn simplified(path: &Path) -> PathBuf {
    path.to_str().and_then(without_verbatim_prefix).map_or_else(|| path.to_path_buf(), PathBuf::from)
}

/// `path` without its verbatim prefix where Windows reads the plain one alike, as the `dunce` crate decides it
/// (drive paths only there; a share's here, by the same rules): Windows takes a verbatim path's names literally,
/// but trims a plain one's trailing dots and spaces, reads `con`, `nul`, `lpt1.txt`… as devices, and stops at
/// `MAX_PATH` in programs that aren't long-path aware (an editor given the path). Any of those, or anything it
/// doesn't recognize (`\\?\Volume{…}`, `..`, a doubled `\\`), keeps the prefix.
fn without_verbatim_prefix(path: &str) -> Option<String> {
    const MAX_PATH: usize = 260;
    let (plain, names, least) = if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        (r"\\", rest, 2)
    } else {
        let rest = path.strip_prefix(r"\\?\")?;
        let (drive, names) = (rest.get(..3)?, rest.get(3..)?);
        let [letter, b':', b'\\'] = drive.as_bytes() else { return None };
        if !letter.is_ascii_alphabetic() {
            return None;
        }
        (drive, names, 0)
    };
    let plain_names = names.is_empty() || names.split('\\').all(plain_name);
    let enough = names.split('\\').filter(|name| !name.is_empty()).count() >= least;
    (plain_names && enough && path.encode_utf16().count() <= MAX_PATH).then(|| format!("{plain}{names}"))
}

/// Whether Windows reads `name` the same in a plain path as in a verbatim one, as `dunce` checks it, with Windows'
/// own device rules: a name is a device by what's before its first dot (`con.tar.gz` is `CON`), trimmed of spaces,
/// and `COM¹`, `LPT³`, `CONIN$` are devices too.
fn plain_name(name: &str) -> bool {
    let forbidden = |c: char| matches!(c, '\0'..='\x1f' | '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
    let base = name.split('.').next().unwrap_or(name).trim_end_matches(' ').to_ascii_uppercase();
    let numbered = ["COM", "LPT"].iter().filter_map(|port| base.strip_prefix(port)).any(|n| {
        let mut digits = n.chars();
        digits.next().is_some_and(|c| c.is_ascii_digit() || matches!(c, '¹' | '²' | '³')) && digits.next().is_none()
    });
    let device = numbered || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str());
    !name.is_empty() && !name.contains(forbidden) && !name.ends_with([' ', '.']) && !device
}

#[cfg(test)]
#[path = "../tests/projects.rs"]
mod tests;
