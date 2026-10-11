//! The brain: every entry in the data folder, and the operations on them. Time is always tracked:
//! `doing` holds one task at a time, whose clock runs; each brain (the central one, each project
//! repository) has its own, so work in different projects can run side by side. The command line and
//! the desktop app both go through it, so tracking rules, tag colors and file names behave
//! the same everywhere. It holds no screen state; the desktop app keeps its own.
//!
//! - [`actions`]: changes to entries and tags, each written to disk immediately.
//! - [`report`]: time per task or tag over a period, from the tracked sessions.
//! - [`calendar`]: each day's interactions over a period: sessions, moves, comments, new entries.

mod actions;
mod calendar;
mod report;
#[cfg(test)]
#[path = "../../tests/brain.rs"]
mod tests;

pub use actions::NewEntry;
pub use calendar::{Calendar, CalendarView, Day, Event, Show, What};
pub use report::{Group, Report, ReportRow, ReportView, Slot};

use crate::api::ApiError;
use crate::model::{Author, Kind, RepoLink, Status, Task, now};
use crate::projects::{self, ProjectInfo};
use crate::store::Store;
use crate::tags::TagColors;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Every entry in the data folder, with the tag colors and settings.
#[derive(Debug)]
pub struct Brain {
    /// The data folder entries are read from and written to.
    pub store: Store,
    /// Every entry, in no particular order.
    pub tasks: Vec<Task>,
    /// Tag → color assignments, kept in `tags.md`.
    pub tag_colors: TagColors,
    /// Files skipped by the last load, with why; the desktop app shows them.
    pub warnings: Vec<String>,
    /// What a project's repository knows of its project; `None` for the central brain.
    pub project: Option<ProjectInfo>,
    /// The central brain, which lists the projects and keeps their shared cards. `None` (as from
    /// [`Brain::open`]) leaves the project's description and sibling repositories out.
    pub central: Option<PathBuf>,
    /// Who the comments and status changes made through this brain are recorded as: a person unless
    /// the command line found it runs for an agent.
    pub author: Author,
}

impl Brain {
    /// Loads every entry and the settings.
    pub fn open(store: Store) -> Result<Self> {
        Self::open_with(store, None)
    }

    /// Loads every entry and the settings, knowing the central brain if `central` is given.
    fn open_with(store: Store, central: Option<PathBuf>) -> Result<Self> {
        let mut brain = Self {
            store,
            tasks: Vec::new(),
            tag_colors: TagColors::default(),
            warnings: Vec::new(),
            project: None,
            central,
            author: Author::Human,
        };
        brain.reload()?;
        Ok(brain)
    }

    /// Loads every entry and the settings as the files have them, writing nothing: for reading a brain
    /// nobody is working in, such as another repository's to show links to it, or to complete a command
    /// line. [`Brain::open`] also repairs what it finds, in files that may be in another git repository.
    pub fn read_only(store: Store) -> Result<Self> {
        let (tasks, warnings) = store.load()?;
        let tag_colors = store.load_tag_colors()?;
        Ok(Self { store, tasks, tag_colors, warnings, project: None, central: None, author: Author::Human })
    }

    /// Loads every entry, knowing the central brain at `central` (see [`Brain::central`]), and so the
    /// project's other repositories, whose deleted entries' links it drops.
    pub fn open_in(store: Store, central: &Path) -> Result<Self> {
        Self::open_with(store, Some(central.to_path_buf()))
    }

    /// Re-reads every file from disk.
    pub fn reload(&mut self) -> Result<()> {
        let (tasks, warnings) = self.store.load()?;
        self.tasks = tasks;
        self.warnings = warnings;
        // Sessions first, so a parked task's session ends when the newer one started, not now.
        self.keep_one_session_running()?;
        self.keep_one_task_doing()?;
        self.drop_dangling_links()?;
        self.remember_highest_id()?;
        self.tag_colors = self.store.load_tag_colors()?;
        self.project = projects::info(self.store.dir(), self.central.as_deref());
        self.drop_dangling_repo_links()?;
        self.sync_tag_colors()
    }

    /// The entry with this id.
    pub fn task(&self, id: u32) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Index into `tasks` of the entry with this id.
    pub(crate) fn find(&self, id: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    /// Index into `tasks` of the entry with this id, or `NotFound`: for actions on an entry by its id.
    pub(crate) fn index_of(&self, id: u32) -> Result<usize, ApiError> {
        self.find(id).ok_or(ApiError::NotFound(id))
    }

    /// The task whose clock is running (there is at most one).
    pub fn running(&self) -> Option<&Task> {
        self.tasks.iter().find(|t| t.is_running())
    }

    /// Tasks with `status`, most recently moved there first.
    pub fn column(&self, status: Status) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.iter().filter(|t| t.is_task() && t.status == status).collect();
        tasks.sort_by(|a, b| b.status_since().cmp(&a.status_since()).then(b.id.cmp(&a.id)));
        tasks
    }

    /// Entries that link to the one with this id, oldest first.
    pub fn backlinks(&self, id: u32) -> Vec<&Task> {
        let mut from: Vec<&Task> = self.tasks.iter().filter(|t| t.links.contains(&id)).collect();
        from.sort_by_key(|t| t.id);
        from
    }

    /// Entries related to the one with this id, whichever side stores the link: notes, then
    /// decisions, then tasks, newest first within each.
    pub fn related(&self, id: u32) -> Vec<&Task> {
        let links = self.task(id).map_or(&[][..], |t| t.links.as_slice());
        let mut related: Vec<&Task> =
            self.tasks.iter().filter(|t| t.id != id && (links.contains(&t.id) || t.links.contains(&id))).collect();
        related.sort_by_key(|t| (related_rank(t.kind), std::cmp::Reverse(t.id)));
        related
    }

    /// Whether the two entries are related, whichever side stores the link.
    pub fn are_related(&self, a: u32, b: u32) -> bool {
        let links = |x: u32, y: u32| self.task(x).is_some_and(|t| t.links.contains(&y));
        links(a, b) || links(b, a)
    }

    /// The brain of the project's repository named `repo`, read from disk without changing it, to check or
    /// show links to its entries. `None` when this brain isn't in a project, the project has no such other
    /// repository, or its folder is gone: it's never created here.
    pub fn sibling(&self, repo: &str) -> Option<Brain> {
        let (_, dir) = self.project.as_ref()?.siblings.iter().find(|(name, _)| name == repo)?;
        if !dir.is_dir() {
            return None;
        }
        Brain::read_only(Store::at(dir).ok()?).ok()
    }

    /// Entries of the project's other repositories related to the one with this id, whichever side
    /// stores the link, as `(repository, entry)`: by repository, then as [`Brain::related`] orders them.
    /// A repository that can't be read, or an entry deleted there since, is left out.
    pub fn repo_related(&self, id: u32) -> Vec<(String, Task)> {
        let Some(project) = &self.project else { return Vec::new() };
        let links = self.task(id).map_or(&[][..], |t| t.repo_links.as_slice());
        let here = RepoLink { repo: project.repo.clone(), id };
        let mut related = Vec::new();
        for (name, _) in &project.siblings {
            let Some(other) = self.sibling(name) else { continue };
            let mut found: Vec<Task> = other
                .tasks
                .into_iter()
                .filter(|t| links.contains(&RepoLink { repo: name.clone(), id: t.id }) || t.repo_links.contains(&here))
                .collect();
            found.sort_by_key(|t| (related_rank(t.kind), std::cmp::Reverse(t.id)));
            related.extend(found.into_iter().map(|t| (name.clone(), t)));
        }
        related
    }

    /// All tags in use, sorted.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Takes the id for a new entry: one more than the highest ever given, by `last-id` or by the entries
    /// in memory, whichever is higher, since another program may have added one since this brain loaded.
    /// It's recorded before the entry is written, so a failure in between leaves a gap, never a reused id.
    fn take_id(&self) -> Result<u32> {
        let highest = self.tasks.iter().map(|t| t.id).max().unwrap_or(0);
        let id = highest.max(self.store.last_id()) + 1;
        self.store.save_last_id(id)?;
        Ok(id)
    }

    /// Links to entries of the project's other repositories that were deleted there (by hand, or by an
    /// older version) are removed, as deleting in the app does. Only where that repository can be read:
    /// one whose folder is missing for now, an unmounted disk say, keeps the links to it.
    fn drop_dangling_repo_links(&mut self) -> Result<()> {
        let mut repos: Vec<&str> = self.tasks.iter().flat_map(|t| &t.repo_links).map(|l| l.repo.as_str()).collect();
        repos.sort_unstable();
        repos.dedup();
        let mut gone: Vec<RepoLink> = Vec::new();
        for repo in repos {
            let Some(other) = self.sibling(repo) else { continue };
            for link in self.tasks.iter().flat_map(|t| &t.repo_links).filter(|l| l.repo == repo) {
                if other.task(link.id).is_none() && !gone.contains(link) {
                    gone.push(link.clone());
                }
            }
        }
        for link in &gone {
            self.drop_repo_links_to(link)?;
        }
        Ok(())
    }

    /// Records the highest id among the entries when `last-id` is behind it (a folder from an older
    /// version, or entries copied in), so deleting the newest entry can't free its id.
    fn remember_highest_id(&self) -> Result<()> {
        let highest = self.tasks.iter().map(|t| t.id).max().unwrap_or(0);
        if highest > self.store.last_id() {
            self.store.save_last_id(highest)?;
        }
        Ok(())
    }

    /// Time is always tracked, so `doing` holds one task: data from older versions, hand edits or a
    /// sync conflict can leave several there, and all but the most recently started are parked.
    fn keep_one_task_doing(&mut self) -> Result<()> {
        let doing = |t: &&mut Task| t.is_task() && t.status == Status::Doing;
        let Some(latest) = self.tasks.iter_mut().filter(doing).map(|t| (t.status_since(), t.id)).max() else {
            return Ok(());
        };
        let now = now();
        for task in self.tasks.iter_mut().filter(doing) {
            if (task.status_since(), task.id) != latest {
                task.set_status(Status::Parked, now, self.author);
                self.store.save(task)?;
            }
        }
        Ok(())
    }

    /// Entries deleted outside kanbrain (by hand, `rm`, a sync) leave links to them behind; they're
    /// removed so nothing points at an entry that's gone.
    fn drop_dangling_links(&mut self) -> Result<()> {
        let ids: std::collections::HashSet<u32> = self.tasks.iter().map(|t| t.id).collect();
        for task in &mut self.tasks {
            let before = task.links.len();
            task.links.retain(|link| ids.contains(link));
            if task.links.len() != before {
                self.store.save(task)?;
            }
        }
        Ok(())
    }

    /// Hand edits or a sync conflict can leave several sessions open; only the most recently
    /// started keeps running, and the others end when it started.
    fn keep_one_session_running(&mut self) -> Result<()> {
        let started = |t: &Task| t.sessions.last().map(|s| s.start);
        let Some(latest) = self.tasks.iter().filter(|t| t.is_running()).filter_map(started).max() else {
            return Ok(());
        };
        for task in &mut self.tasks {
            if task.is_running() && started(task) != Some(latest) {
                task.stop_session(latest);
                self.store.save(task)?;
            }
        }
        Ok(())
    }

    /// Colors any new tags, oldest entries first so earlier tags claim colors first.
    fn sync_tag_colors(&mut self) -> Result<()> {
        let mut by_age: Vec<&Task> = self.tasks.iter().collect();
        by_age.sort_by_key(|t| t.id);
        if self.tag_colors.assign(by_age.iter().flat_map(|t| &t.tags)) {
            self.store.save_tag_colors(&self.tag_colors)?;
        }
        Ok(())
    }
}

/// Where a kind's related entries are listed: notes and decisions (what a task needs to know) before tasks.
fn related_rank(kind: Kind) -> u8 {
    match kind {
        Kind::Note => 0,
        Kind::Decision => 1,
        Kind::Task => 2,
    }
}
