//! The brains the window shows: the central one and every registered project repository, each
//! with its own entries, and the scope that filters them (all, the central brain, a project, or one
//! repository of a project with several). Entries are known by [`Key`]: which brain, and the id
//! there, since every brain numbers its entries from 1.

use super::{AddProject, Board, Form, Mode, Picker, ProjectOption, WhereFor};
use crate::text::event::KeyEvent;
use anyhow::Result;
use kanbrain_core::brain::{Brain, Calendar, CalendarView, Report, ReportView};
use kanbrain_core::cli::{self, StepKind};
use kanbrain_core::migrate::TooNew;
use kanbrain_core::model::{Author, RepoLink, Task};
use kanbrain_core::projects::{self, Repo};
use kanbrain_core::store::{BrainLock, PROJECT_DIR, Store};
use std::path::{Path, PathBuf};

/// An entry in one of the brains: the brain's index in [`Board::sources`], and the entry's id there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Index of the brain in `Board::sources`.
    pub src: usize,
    /// The entry's id in that brain.
    pub id: u32,
}

impl Key {
    /// An entry of the brain at `src`.
    pub const fn new(src: usize, id: u32) -> Self {
        Self { src, id }
    }
}

/// How lists of brains and the scope filter name the central brain.
const CENTRAL: &str = "Central";

/// One brain the window shows.
#[derive(Debug)]
pub struct Source {
    /// Its entries.
    pub brain: Brain,
    /// How it's named on cards and in the scope filter (`shop`, `shop/api`); `None` for the central brain.
    pub label: Option<String>,
    /// The project it's a repository of; `None` for the central brain.
    pub project: Option<String>,
}

/// Which brains the window shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scope {
    /// Every brain together.
    #[default]
    All,
    /// The central brain: what's outside any project.
    Central,
    /// Every repository of this project.
    Project(String),
    /// One repository, of a project with several (index in `Board::sources`).
    Repo(usize),
}

/// The central brain, then every repository [`repos`] lists; and those left out. A repository that can't be read
/// (a newer kanbrain wrote it, its `format` file is damaged) is left out, with why: the rest of the board still
/// opens.
pub(super) fn load(central: &Path, also: Option<&Path>) -> Result<(Vec<Source>, Vec<Skipped>)> {
    // Each brain under its lock, in the order every window and `kb` take them: opening may repair files (renumber
    // ids, park a second task of yours, drop links to deleted entries), which would otherwise race an agent's write.
    // Held already when reloading under `BrainView::locked`, where taking it again costs nothing; one that can't be
    // taken (an unwritable cache folder) is opened all the same, as `Board::lock_brains` does.
    let mut locks = Vec::new();
    let central_store = Store::at(central)?;
    locks.extend(central_store.lock().ok());
    let repos = repos(central, also)?;
    let mut sources = vec![Source { brain: Brain::open_in(central_store, central)?, label: None, project: None }];
    let mut skipped = Vec::new();
    for repo in repos.iter().filter(|r| r.dir.is_dir()) {
        let store = Store::at(&repo.dir)?;
        locks.extend(store.lock().ok());
        match Brain::open_in(store, central) {
            Ok(brain) => {
                sources.push(Source { brain, label: Some(repo.label(&repos)), project: Some(repo.project.clone()) });
            }
            // One repository that can't be read (a newer kanbrain's, a damaged `format` file) mustn't keep the
            // board from showing the others.
            Err(e) => {
                let said = if e.downcast_ref::<TooNew>().is_some() {
                    format!("{e:#}")
                } else {
                    format!("{} left out: {e:#}", repo.label(&repos))
                };
                skipped.push(Skipped { dir: repo.dir.clone(), said });
            }
        }
    }
    Ok((sources, skipped))
}

/// A registered repository left out because it couldn't be read, with why.
#[derive(Debug)]
pub(super) struct Skipped {
    /// Its `.brain/`.
    pub dir: PathBuf,
    /// Why, as the status bar says it.
    pub said: String,
}

/// Every registered repository, and `also`, a `.brain/` the window was opened in that isn't registered
/// (from before projects were), under its folder's name. Those whose folder is gone are listed but not
/// shown, rather than created again.
fn repos(central: &Path, also: Option<&Path>) -> Result<Vec<Repo>> {
    let mut repos = projects::load(central)?;
    if let Some(dir) = also.filter(|d| !repos.iter().any(|r| projects::same_dir(&r.dir, d))) {
        let project = projects::read_repo_card(dir).map_or_else(|| folder_name(dir), |card| card.project);
        repos.push(Repo { project, dir: dir.to_path_buf() });
    }
    Ok(repos)
}

/// The name of the folder holding a `.brain/`.
fn folder_name(dir: &Path) -> String {
    dir.parent().and_then(Path::file_name).map_or_else(|| "project".into(), |n| n.to_string_lossy().into_owned())
}

impl Board {
    /// The brain at `src`.
    pub fn brain(&self, src: usize) -> &Brain {
        &self.sources[src].brain
    }

    /// The brain at `src`, to change.
    pub fn brain_mut(&mut self, src: usize) -> &mut Brain {
        &mut self.sources[src].brain
    }

    /// The brains at `a` and `b`, both to change at once, as a move or a link across repositories does: `a` and `b`
    /// must differ.
    pub(super) fn two_brains_mut(&mut self, a: usize, b: usize) -> (&mut Brain, &mut Brain) {
        if a < b {
            let (left, right) = self.sources.split_at_mut(b);
            (&mut left[a].brain, &mut right[0].brain)
        } else {
            let (left, right) = self.sources.split_at_mut(a);
            (&mut right[0].brain, &mut left[b].brain)
        }
    }

    /// The name of the project the brain at `src` is a repository of; empty for the central brain.
    pub(super) fn project_name(&self, src: usize) -> &str {
        self.sources[src].project.as_deref().unwrap_or_default()
    }

    /// The entry `key` names.
    pub fn task(&self, key: Key) -> Option<&Task> {
        self.sources.get(key.src)?.brain.task(key.id)
    }

    /// Whether the scope shows the brain at `src`.
    pub fn in_scope(&self, src: usize) -> bool {
        let source = &self.sources[src];
        match &self.scope {
            Scope::All => true,
            Scope::Central => source.project.is_none(),
            Scope::Project(name) => source.project.as_ref() == Some(name),
            Scope::Repo(repo) => *repo == src,
        }
    }

    /// The brains the scope shows, with their index.
    pub fn scoped(&self) -> impl Iterator<Item = (usize, &Brain)> {
        (0..self.sources.len()).filter(|&src| self.in_scope(src)).map(|src| (src, self.brain(src)))
    }

    /// Whether cards say which brain they're from: when the scope shows more than one.
    pub fn several(&self) -> bool {
        self.scoped().nth(1).is_some()
    }

    /// The label to show on an entry of the brain at `src`, when the scope shows several brains.
    pub fn source_label(&self, src: usize) -> Option<&str> {
        self.several().then(|| self.sources[src].label.as_deref().unwrap_or(projects::CENTRAL))
    }

    /// How messages name the entry `key`: `#3`, or `shop/api #3` with several brains shown, whose ids repeat.
    pub fn entry_name(&self, key: Key) -> String {
        match self.source_label(key.src) {
            Some(label) => format!("{label} #{}", key.id),
            None => format!("#{}", key.id),
        }
    }

    /// The scopes there are, with their names, in the order the filter shows them: all, the central
    /// brain, then each project followed by its repositories if it has several.
    pub fn scopes(&self) -> Vec<(Scope, String)> {
        let mut scopes = vec![(Scope::All, "All".to_string()), (Scope::Central, CENTRAL.to_string())];
        let mut seen: Vec<&str> = Vec::new();
        for source in &self.sources {
            let Some(project) = source.project.as_deref() else { continue };
            if seen.contains(&project) {
                continue;
            }
            seen.push(project);
            scopes.push((Scope::Project(project.to_string()), project.to_string()));
            let repos: Vec<usize> = self.repos_of(project).collect();
            if repos.len() > 1 {
                for src in repos {
                    let label = self.sources[src].label.clone().unwrap_or_default();
                    scopes.push((Scope::Repo(src), label));
                }
            }
        }
        scopes
    }

    /// Shows only what `scope` covers, keeping the selection if it's still shown.
    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.keep_selection();
    }

    /// The scopes whose name contains every word of the picker's search, ignoring case, in the
    /// filter's order.
    pub fn scope_candidates(&self, picker: &Picker) -> Vec<(Scope, String)> {
        self.scopes().into_iter().filter(|(_, name)| picker.matches(name)).collect()
    }

    /// Opens the project filter's dropdown, on the scope shown.
    pub fn open_scopes(&mut self) {
        let sel = self.scope_candidates(&Picker::default()).iter().position(|(s, _)| *s == self.scope).unwrap_or(0);
        self.mode = Mode::Scopes(Picker::at(sel));
    }

    /// Shows the dropdown's candidate at `index` and closes it.
    pub fn choose_scope(&mut self, index: usize) {
        let Mode::Scopes(picker) = &self.mode else { return };
        let Some((scope, _)) = self.scope_candidates(picker).into_iter().nth(index) else { return };
        self.mode = Mode::Normal;
        self.set_scope(scope);
    }

    /// Asks which project the folder `dir`, picked in the system's dialog, joins or starts. The name begins as the
    /// project of `from`, the repository whose Manage project it's added from, else the one its `.brain/` card gives
    /// if it has one (it was a repository before), else the folder's, selected so that typing replaces it.
    pub fn name_project(&mut self, dir: PathBuf, from: Option<&Path>) {
        let card = projects::read_repo_card(&dir.join(PROJECT_DIR)).map(|card| card.project);
        let folder = || Some(dir.file_name()?.to_string_lossy().into_owned());
        let back = from.and_then(|brain| self.src_at(brain));
        let project = back.and_then(|src| self.sources[src].project.clone());
        let name = project.or(card).or_else(folder).unwrap_or_default();
        self.mode = Mode::AddProject(AddProject { dir, picker: Picker::named(&name), back });
    }

    /// The add-project dialog's rows for what's typed: a new project of that name unless there's one already, then
    /// the projects there are whose name contains every word typed (all of them with nothing typed).
    pub fn add_project_options(&self, picker: &Picker) -> Vec<ProjectOption> {
        let typed = picker.search.text().trim().to_string();
        let mut names: Vec<&str> = Vec::new();
        for project in self.sources.iter().filter_map(|s| s.project.as_deref()) {
            if !names.contains(&project) {
                names.push(project);
            }
        }
        let new = (!typed.is_empty() && !names.contains(&typed.as_str())).then_some(ProjectOption::New(typed));
        let existing = names.into_iter().filter(|name| picker.matches(name));
        new.into_iter().chain(existing.map(|name| ProjectOption::Existing(name.to_string()))).collect()
    }

    /// The add-project dialog's keys: arrows move, typing names or searches, `enter` adds the folder to the
    /// highlighted row's project, `esc` cancels.
    pub(super) fn key_add_project(&mut self, add: AddProject, key: KeyEvent) {
        let count = self.add_project_options(&add.picker).len();
        let back = add.back;
        self.pick(add, key, count, |a| &mut a.picker, Mode::AddProject, Self::choose_add_project);
        // Cancelled from Manage project: back to it, on the repository it was on.
        if let (Mode::Normal, Some(src)) = (&self.mode, back) {
            self.mode = Mode::ManageProject(src);
        }
    }

    /// Makes the folder a repository of the project at `index` among the dialog's rows, as `kb init` does there,
    /// and shows that project. The dialog stays open, with the reason, when nothing is typed or the folder can't be
    /// written.
    pub fn choose_add_project(&mut self, index: usize) {
        let Mode::AddProject(add) = &self.mode else { return };
        let Some(option) = self.add_project_options(&add.picker).into_iter().nth(index) else {
            self.notify("type the project's name");
            return;
        };
        let name = option.name().to_string();
        let dir = add.dir.clone();
        let from_manage = add.back.is_some();
        let done = match cli::init_steps(&dir, Some(&name), None, &self.central) {
            Ok(done) => done,
            Err(e) => return self.notify_error(&e),
        };
        self.mode = Mode::Normal;
        let result = self.reload();
        self.report_error(result);
        let repos = self.repos_of(&name).count();
        self.set_scope(Scope::Project(name.clone()));
        // A repository of another project moved here: said as a move, with the links it dropped.
        let moved = done.first().is_some_and(|step| step.kind == StepKind::Moved);
        self.notify(if moved {
            done.iter()
                .filter(|step| matches!(step.kind, StepKind::Moved | StepKind::DroppedLinks))
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        } else if repos > 1 {
            format!("added {} to project `{name}`; its agent instructions tell agents to use kanbrain", dir.display())
        } else {
            format!("added project `{name}`; its agent instructions tell agents to use kanbrain")
        });
        // Added from Manage project: back to it, on the repository just added.
        let added = dir.join(PROJECT_DIR);
        if from_manage
            && let Some(src) = self.sources.iter().position(|s| projects::same_dir(s.brain.store.dir(), &added))
        {
            self.mode = Mode::ManageProject(src);
        }
    }

    /// A brain of the project shown, if the scope shows one project (or one of its repositories):
    /// the repository shown, else the project's first.
    pub fn project_src(&self) -> Option<usize> {
        match &self.scope {
            Scope::Repo(src) => Some(*src),
            Scope::Project(_) => self.scoped().next().map(|(src, _)| src),
            Scope::All | Scope::Central => None,
        }
    }

    /// The brains of project `name`, its repositories, in order.
    pub fn repos_of(&self, name: &str) -> impl Iterator<Item = usize> {
        (0..self.sources.len()).filter(move |&s| self.sources[s].project.as_deref() == Some(name))
    }

    /// The brains of the project the brain at `src` belongs to, in order.
    pub fn project_repos(&self, src: usize) -> Vec<usize> {
        self.sources[src].project.as_deref().map_or_else(Vec::new, |project| self.repos_of(project).collect())
    }

    /// A brain of the project shown, else of the selected entry's project: the one `I` and `E` act on.
    pub fn project_here(&self) -> Option<usize> {
        let selected = self.selected.map(|key| key.src).filter(|&src| self.sources[src].project.is_some());
        self.project_src().or(selected)
    }

    /// The project's repository whose role `N` edits: the selected entry's, else the one shown alone.
    pub fn repo_here(&self) -> Option<usize> {
        let shown = match self.scope {
            Scope::Repo(src) => Some(src),
            _ => None,
        };
        self.selected.map(|key| key.src).or(shown).filter(|&src| self.sources[src].project.is_some())
    }

    /// [`Self::project_here`] for a key that acts on it: with no project, says how to get one rather than doing
    /// nothing, since a key that does nothing reads as broken.
    pub(super) fn project_to_act_on(&mut self) -> Option<usize> {
        let here = self.project_here();
        if here.is_none() {
            self.notify("no project here: select one of its cards, or show it with P");
        }
        here
    }

    /// Opens Manage project on the project shown, else on the selected entry's project.
    pub fn open_manage_project(&mut self) {
        if let Some(src) = self.project_to_act_on() {
            self.mode = Mode::ManageProject(src);
        }
    }

    /// The brain the status bar names: the selected entry's if the scope shows it, else the first
    /// brain the scope shows (the central one for all).
    pub fn home(&self) -> usize {
        match self.selected.filter(|k| self.in_scope(k.src)) {
            Some(key) => key.src,
            None => self.scoped().next().map_or(0, |(src, _)| src),
        }
    }

    /// The brains the "which project?" picker offers, named as cards name them, whose name contains
    /// every word of the picker's search, ignoring case: for a quick-added task, those the scope shows;
    /// for a move, every brain but the entry's own, since it may go anywhere.
    pub fn where_candidates(&self, picker: &Picker, then: WhereFor) -> Vec<(usize, String)> {
        let offered: Vec<usize> = match then {
            WhereFor::Move(key) => (0..self.sources.len()).filter(|&src| src != key.src).collect(),
            WhereFor::QuickAdd(_) => self.scoped().map(|(src, _)| src).collect(),
        };
        offered
            .into_iter()
            .map(|src| (src, self.brain_name(src).to_string()))
            .filter(|(_, name)| picker.matches(name))
            .collect()
    }

    /// The brains the form's project field offers: every one, whatever the scope, in order, whose name
    /// contains every word of its search; none on a form without the field.
    pub fn form_projects(&self, form: &Form) -> Vec<(usize, String)> {
        let Some(picker) = &form.project else { return Vec::new() };
        (0..self.sources.len())
            .map(|src| (src, self.brain_name(src).to_string()))
            .filter(|(_, name)| picker.matches(name))
            .collect()
    }

    /// Entries of the project's other repositories related to `key`'s, whichever side stores the link: the
    /// core's `Brain::repo_related`, but from the brains already loaded, since it's drawn every frame.
    pub fn repo_related(&self, key: Key) -> Vec<Key> {
        let Some(here) = &self.brain(key.src).project else { return Vec::new() };
        let links = self.task(key).map_or(&[][..], |t| t.repo_links.as_slice());
        let back = RepoLink { repo: here.repo.clone(), id: key.id };
        let mut related = Vec::new();
        for (src, source) in self.sources.iter().enumerate().filter(|&(src, _)| src != key.src) {
            let Some(there) = source.brain.project.as_ref().filter(|p| p.name == here.name) else { continue };
            let linked =
                |t: &&Task| links.iter().any(|l| l.repo == there.repo && l.id == t.id) || t.repo_links.contains(&back);
            related.extend(source.brain.tasks.iter().filter(linked).map(|t| Key::new(src, t.id)));
        }
        related
    }

    /// The brains whose entries one in the brain at `src` can be related to: its own, then the other
    /// repositories of its project, if it's in one.
    pub fn link_scope(&self, src: usize) -> Vec<usize> {
        let project = self.brain(src).project.as_ref().map(|p| p.name.as_str());
        let others = (0..self.sources.len()).filter(|&other| {
            other != src && project.is_some() && self.brain(other).project.as_ref().map(|p| p.name.as_str()) == project
        });
        std::iter::once(src).chain(others).collect()
    }

    /// Every entry one in the brain at `src` can be related to (see [`Board::link_scope`]), with its key.
    pub fn linkable(&self, src: usize) -> impl Iterator<Item = (Key, &Task)> {
        self.link_scope(src)
            .into_iter()
            .flat_map(move |other| self.brain(other).tasks.iter().map(move |t| (Key::new(other, t.id), t)))
    }

    /// How a link from the brain at `src` names `to`: `#3` in the same brain, `shop/api #3` in another.
    pub fn link_name(&self, src: usize, to: Key) -> String {
        if to.src == src { format!("#{}", to.id) } else { format!("{} #{}", self.brain_name(to.src), to.id) }
    }

    /// Every entry related to `key`'s, in its brain and the project's other repositories.
    pub fn related_keys(&self, key: Key) -> Vec<Key> {
        let here = self.brain(key.src).related(key.id).into_iter().map(|t| Key::new(key.src, t.id));
        here.chain(self.repo_related(key)).collect()
    }

    /// Relates two entries, or unrelates them if they already are, in one brain or across a project's
    /// repositories (see `Brain::toggle_link`, `Brain::toggle_repo_link`). Returns whether they are now.
    pub fn toggle_link_between(&mut self, a: Key, b: Key) -> Result<bool> {
        if a.src == b.src {
            return self.brain_mut(a.src).toggle_link(a.id, b.id);
        }
        let (here, there) = self.two_brains_mut(a.src, b.src);
        here.toggle_repo_link(a.id, there, b.id)
    }

    /// How an entry of another repository of its project names `key`'s entry in its links.
    pub fn repo_link(&self, key: Key) -> RepoLink {
        let repo = self.brain(key.src).project.as_ref().map_or_else(String::new, |p| p.repo.clone());
        RepoLink { repo, id: key.id }
    }

    /// Removes the links the project's other repositories have to `key`'s entry, once it's deleted or
    /// moved away, so they don't point at an entry that's gone.
    pub fn drop_links_from_other_repos(&mut self, key: Key) -> Result<()> {
        let link = self.repo_link(key);
        for other in self.link_scope(key.src).into_iter().skip(1) {
            self.brain_mut(other).drop_repo_links_to(&link)?;
        }
        Ok(())
    }

    /// How a brain is named in lists of them: its label, or `Central`.
    pub fn brain_name(&self, src: usize) -> &str {
        self.sources[src].label.as_deref().unwrap_or(CENTRAL)
    }

    /// A tag's chip colors, background and text, from the first brain whose `tags.md` has it (the central brain's,
    /// then the repositories' in order): one tag, one color, wherever it shows and whatever the scope, though two
    /// brains may have given it different ones.
    pub fn tag_colors(&self, tag: &str) -> Option<(u32, u32)> {
        self.sources.iter().find_map(|s| s.brain.tag_colors.chip(tag))
    }

    /// Every tag the scope's entries use, sorted.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.scoped().flat_map(|(_, b)| b.all_tags()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// The tasks whose clocks run, in every brain whatever the scope shows.
    pub fn running(&self) -> impl Iterator<Item = (Key, &Task)> {
        self.sources.iter().enumerate().flat_map(|(src, s)| s.brain.running().map(move |t| (Key::new(src, t.id), t)))
    }

    /// Whether a task's clock runs, yours or an agent's, in any brain whatever the scope shows.
    pub fn any_running(&self) -> bool {
        self.running().next().is_some()
    }

    /// Whether a task you started runs, in any brain: what Park & quit parks, as agents' tasks keep running.
    pub fn any_mine_running(&self) -> bool {
        self.running().any(|(_, task)| task.started_by() == Author::Human)
    }

    /// The report tab's view with the tab's search.
    pub fn report_view(&self) -> ReportView {
        ReportView { query: self.search.text().to_string(), ..self.report.clone() }
    }

    /// The calendar tab's view with the tab's search.
    pub fn calendar_view(&self) -> CalendarView {
        CalendarView { query: self.search.text().to_string(), ..self.calendar.clone() }
    }

    /// The report of every brain in scope, merged; task rows say their brain when there are several.
    pub fn report(&self) -> Report {
        let view = self.report_view();
        let parts = self
            .scoped()
            .map(|(src, brain)| (self.source_label(src).map(str::to_string), brain.report(&view)))
            .collect();
        Report::merge(parts, &view)
    }

    /// The calendar of every brain in scope, merged, and which brain each part came from: an event's
    /// `source` indexes the returned list.
    pub fn calendar(&self) -> (Calendar, Vec<usize>) {
        let view = self.calendar_view();
        let (srcs, parts): (Vec<usize>, Vec<Calendar>) = self.scoped().map(|(src, b)| (src, b.calendar(&view))).unzip();
        (Calendar::merge(parts), srcs)
    }

    /// Writes the report tab's report, filtered as shown: into the one brain shown, or the central
    /// brain for several.
    pub(super) fn write_report(&self) -> Result<PathBuf> {
        let (report, view) = (self.report(), self.report_view());
        let mut scoped = self.scoped();
        let brain = match (scoped.next(), scoped.next()) {
            (Some((_, one)), None) => one,
            _ => self.brain(0),
        };
        brain.write_report(&report, &view)
    }

    /// Whether the repositories shown are still those registered, with the same projects and labels: a
    /// repository's name (`kb project repo`) is in its own `project.md`, which changes no other file.
    pub(super) fn same_repos(&self) -> Result<bool> {
        let repos = repos(&self.central, self.opened_in.as_deref())?;
        // Those that couldn't be read aren't shown: still left out, they're no change.
        let registered = repos
            .iter()
            .filter(|r| r.dir.is_dir() && !self.skipped.iter().any(|s| s.dir == r.dir))
            .map(|r| (r.dir.as_path(), Some(r.project.as_str()), Some(r.label(&repos))));
        let shown = self.sources.iter().skip(1).map(|s| (s.brain.store.dir(), s.project.as_deref(), s.label.clone()));
        Ok(registered.eq(shown))
    }

    /// Takes the write lock (see `Store::lock`) of every brain whose folder isn't in `tried` yet, in order, so two
    /// windows never wait on each other, and adds the folder there. One that can't be taken (an unwritable cache
    /// folder) is said in the status bar and skipped, rather than keeping the board from working; and not tried again
    /// in the same input, where the central brain's taken after a repository's would invert that order.
    pub fn lock_brains(&mut self, tried: &mut Vec<PathBuf>) -> Vec<BrainLock> {
        let mut locks = Vec::with_capacity(self.sources.len());
        for src in 0..self.sources.len() {
            let store = &self.sources[src].brain.store;
            if tried.iter().any(|dir| dir == store.dir()) {
                continue;
            }
            tried.push(store.dir().to_path_buf());
            match store.lock() {
                Ok(lock) => locks.push(lock),
                Err(e) => self.notify_error(&e),
            }
        }
        locks
    }

    /// Takes, into `locks`, the lock of each brain not `tried` yet in this input: one catching up found (a project
    /// added meanwhile), read before its lock was taken, which a `kb` may have written to since. With any taken,
    /// reads every brain again under the locks, so the input acts on what's there. A project registered during
    /// that read too is locked as well, and read again at the next input.
    pub fn lock_found_brains(&mut self, locks: &mut Vec<BrainLock>, tried: &mut Vec<PathBuf>) {
        let before = locks.len();
        locks.extend(self.lock_brains(tried));
        if locks.len() == before {
            return;
        }
        if let Err(e) = self.reload() {
            self.notify_error(&e);
        }
        locks.extend(self.lock_brains(tried));
    }

    /// The data folders to watch: every brain's, and the central brain's project cards.
    pub fn watched_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self.sources.iter().map(|s| s.brain.store.dir().to_path_buf()).collect();
        dirs.push(projects::cards_dir(&self.central));
        // Those left out too, so one repaired is shown.
        dirs.extend(self.skipped.iter().map(|s| s.dir.clone()));
        dirs
    }
}
