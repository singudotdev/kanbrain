//! Managing projects and their repositories after `init`: renaming a project or a repository, moving a
//! repository to another project, and taking a repository or a whole project out, keeping their tasks or deleting
//! them. The kanban app's Manage project and `kb project` run these. Each returns what it did, one line each.
//!
//! Links between a project's repositories name the other one (`web#3`), so a repository's name and project are
//! part of its siblings' files: renaming it rewrites them, and moving or removing it drops them, on both sides,
//! each sibling under its write lock.

use super::init;
use crate::model::Task;
use crate::projects::{self, Repo, RepoCard};
use crate::store::{self, PROJECT_DIR, Store};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// Renames project `old` to `new`: the registry, its shared card, and each of its repositories' cards.
pub fn rename_project(central: &Path, old: &str, new: &str) -> Result<Vec<String>> {
    projects::check_name(new)?;
    let repos = projects::load(central)?;
    if !repos.iter().any(|r| r.project == old) {
        bail!("no project `{old}`");
    }
    if old == new {
        return Ok(vec![format!("project `{old}` already has that name")]);
    }
    if repos.iter().any(|r| r.project == new) {
        bail!("there's already a project `{new}`: move this one's repositories to it instead");
    }
    projects::rename(central, old, new)?;
    for repo in repos.iter().filter(|r| r.project == old && r.dir.is_dir()) {
        if let Some(card) = projects::read_repo_card(&repo.dir) {
            projects::write_repo_card(&repo.dir, &RepoCard { project: new.to_string(), ..card })?;
        }
    }
    Ok(vec![format!("renamed project `{old}` to `{new}`; its repositories and card follow")])
}

/// Renames the repository whose `.brain/` is `dir` to `new` in its project, and rewrites its siblings' links to
/// it (`old#3` becomes `new#3`).
pub fn rename_repo(central: &Path, dir: &Path, new: &str) -> Result<Vec<String>> {
    let repo = registered(central, dir)?;
    let old = repo.name();
    if old == new {
        return Ok(vec![format!("repository `{old}` already has that name")]);
    }
    let new = projects::repo_name(central, &repo.project, &repo.dir, Some(new))?;
    let card = projects::read_repo_card(&repo.dir).unwrap_or_else(|| RepoCard {
        project: repo.project.clone(),
        repo: None,
        notes: String::new(),
    });
    let stored = (new != repo.folder()).then(|| new.clone());
    projects::write_repo_card(&repo.dir, &RepoCard { repo: stored, ..card })?;
    let rewritten = rewrite_sibling_links(central, &repo, |link_repo| (link_repo == old).then(|| new.clone()))?;
    let mut done = vec![format!("renamed repository `{old}` of project `{}` to `{new}`", repo.project)];
    if rewritten > 0 {
        done.push(format!("rewrote the links to it in {rewritten} entries of its other repositories"));
    }
    Ok(done)
}

/// Moves the repository whose `.brain/` is `dir` to project `to`, created if new, as `kb init to` there would.
/// Links between it and its old project's repositories are dropped on both sides: they'd name repositories of
/// another project.
pub fn move_repo(central: &Path, dir: &Path, to: &str) -> Result<Vec<String>> {
    let repo = registered(central, dir)?;
    if repo.project == to {
        return Ok(vec![format!("{} is already a repository of project `{to}`", repo.name())]);
    }
    projects::check_name(to)?;
    let root = repo.dir.parent().filter(|_| repo.dir.is_dir()).context("its folder is gone: remove it instead")?;
    let dropped = unlink(central, &repo)?;
    let mut done = init::init(root, Some(to), None, central)?;
    if dropped > 0 {
        done.push(format!("dropped {dropped} links between it and project `{}`'s repositories", repo.project));
    }
    Ok(done)
}

/// Takes the repository whose `.brain/` is `dir` out of its project, as `kb deinit` does, and drops its old
/// siblings' links to it. With `purge`, its `.brain/` goes too: its tasks, notes and decisions.
pub fn remove_repo(central: &Path, dir: &Path, purge: bool) -> Result<Vec<String>> {
    let repo = registered(central, dir)?;
    unlink(central, &repo)?;
    let mut done = init::deinit(&repo.dir, central)?;
    if !repo.dir.is_dir() {
        return Ok(done);
    }
    if !purge {
        done.push(format!(
            "kept {}: commands run here still use it; `deinit --purge` would have deleted its tasks, notes and \
             decisions",
            repo.dir.display()
        ));
        return Ok(done);
    }
    // Only ever a `.brain/` folder: `dir` comes from the registry, which a hand edit could point elsewhere.
    if repo.dir.file_name().is_none_or(|n| n != PROJECT_DIR) {
        bail!("{} isn't a `.brain/` folder: deleted nothing", repo.dir.display());
    }
    let cache = store::cache_of(&repo.dir)?;
    fs::remove_dir_all(&repo.dir).with_context(|| format!("deleting {}", repo.dir.display()))?;
    done.push(format!("deleted {}: its tasks, notes and decisions are gone", repo.dir.display()));
    // Best effort: it's the cache, and on Windows a lock this process holds open can't be deleted yet.
    for path in cache {
        let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
    }
    Ok(done)
}

/// Takes every repository of `project` out, as [`remove_repo`] does each, which removes the project's card with
/// the last.
pub fn remove_project(central: &Path, project: &str, purge: bool) -> Result<Vec<String>> {
    let repos: Vec<Repo> = projects::load(central)?.into_iter().filter(|r| r.project == project).collect();
    if repos.is_empty() {
        bail!("no project `{project}`");
    }
    let mut done = Vec::new();
    for repo in repos {
        done.extend(remove_repo(central, &repo.dir, purge)?);
    }
    Ok(done)
}

/// The registered repository whose `.brain/` is `dir`.
fn registered(central: &Path, dir: &Path) -> Result<Repo> {
    projects::load(central)?
        .into_iter()
        .find(|r| projects::same_dir(&r.dir, dir))
        .with_context(|| format!("{} isn't a registered repository", dir.display()))
}

/// Drops the links between `repo` and the other repositories of its project, on both sides. Returns how many
/// entries changed.
fn unlink(central: &Path, repo: &Repo) -> Result<usize> {
    let name = repo.name();
    let siblings: Vec<String> = siblings(central, repo)?.iter().map(Repo::name).collect();
    let mut changed = rewrite_sibling_links(central, repo, |link_repo| (link_repo == name).then(String::new))?;
    if repo.dir.is_dir() {
        changed += rewrite_links(&repo.dir, |link_repo| siblings.contains(&link_repo.to_string()).then(String::new))?;
    }
    Ok(changed)
}

/// The project's other repositories whose folders are there.
fn siblings(central: &Path, repo: &Repo) -> Result<Vec<Repo>> {
    Ok(projects::load(central)?
        .into_iter()
        .filter(|r| r.project == repo.project && !projects::same_dir(&r.dir, &repo.dir) && r.dir.is_dir())
        .collect())
}

/// Rewrites, in every other repository of `repo`'s project, the links whose repository `change` maps: to the
/// name it returns, or dropped for an empty one. Returns how many entries changed.
fn rewrite_sibling_links(central: &Path, repo: &Repo, change: impl Fn(&str) -> Option<String>) -> Result<usize> {
    siblings(central, repo)?.iter().map(|sibling| rewrite_links(&sibling.dir, &change)).sum()
}

/// Rewrites the links of the brain in `dir` as [`rewrite_sibling_links`] does, under its write lock.
fn rewrite_links(dir: &Path, change: impl Fn(&str) -> Option<String>) -> Result<usize> {
    let store = Store::at(dir)?;
    let _lock = store.lock()?;
    let (tasks, _) = store.load()?;
    let mut changed = 0;
    for mut task in tasks {
        if rewrite(&mut task, &change) {
            store.save(&mut task)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Applies `change` to `task`'s links to other repositories; returns whether any changed.
fn rewrite(task: &mut Task, change: impl Fn(&str) -> Option<String>) -> bool {
    let mut changed = false;
    task.repo_links.retain_mut(|link| match change(&link.repo) {
        Some(name) if name.is_empty() => {
            changed = true;
            false
        }
        Some(name) => {
            changed |= name != link.repo;
            link.repo = name;
            true
        }
        None => true,
    });
    changed
}

#[cfg(test)]
#[path = "../../tests/cli/manage.rs"]
mod tests;
