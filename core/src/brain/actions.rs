//! Changes to entries, and writing reports. Every change is written to disk immediately.

use super::{Brain, ReportView};
use crate::api::ApiError;
use crate::export;
use crate::model::{
    Asked, Author, Comment, DecisionState, Kind, RepoLink, Status, Task, now, sanitize_comment, sanitize_description,
};
use crate::projects;
use anyhow::Result;
use std::path::PathBuf;

/// The body a new decision starts with: the usual sections of an architecture decision record.
const DECISION_TEMPLATE: &str = "### Context\n\nWhat forces us to decide, and what we know.\n\n\
                                 ### Decision\n\nWhat we chose.\n\n\
                                 ### Consequences\n\nWhat becomes easier or harder, and what we gave up.";

/// What a new entry starts with, from the desktop app, the command line or an agent.
#[derive(Debug, Default)]
pub struct NewEntry {
    /// Task, note or decision.
    pub kind: Kind,
    /// Must not be empty.
    pub title: String,
    /// Lowercase, without `#`.
    pub tags: Vec<String>,
    /// Ids of related entries.
    pub links: Vec<u32>,
    /// Related entries in the project's other repositories.
    pub repo_links: Vec<RepoLink>,
    /// `None` leaves it empty, or for a decision, the ADR sections to fill in.
    pub description: Option<String>,
    /// Start a task right away (moving it to `doing` and starting its clock).
    pub start: bool,
}

impl Brain {
    /// The index of the task with this id; notes and decisions are refused, since only tasks have a status.
    fn task_index(&self, id: u32) -> Result<usize, ApiError> {
        let index = self.index_of(id)?;
        match self.tasks[index].kind {
            Kind::Task => Ok(index),
            kind => Err(ApiError::NotATask(id, kind)),
        }
    }

    /// Moves a task to `status`, recording when. `doing` starts its clock; for a person, it's refused while
    /// `doing` holds a task a person started: making room is their call, never a side effect. Agents start
    /// as many as they work on. Returns whether anything changed.
    pub fn set_status(&mut self, id: u32, status: Status) -> Result<bool> {
        let index = self.task_index(id)?;
        if status == Status::Doing {
            return self.start_task(index);
        }
        if !self.tasks[index].set_status(status, now(), self.author) {
            return Ok(false);
        }
        self.store.save(&mut self.tasks[index])?;
        Ok(true)
    }

    /// Moves the task at `index` to `doing` and opens a session, if `doing` has room. A task already in
    /// `doing` without a session (from an older version) starts its clock.
    fn start_task(&mut self, index: usize) -> Result<bool> {
        if self.tasks[index].is_running() {
            return Ok(false);
        }
        if self.tasks[index].status != Status::Doing {
            self.check_doing_room(self.author)?;
        }
        let now = now();
        self.tasks[index].set_status(Status::Doing, now, self.author);
        self.tasks[index].start_session(now);
        self.store.save(&mut self.tasks[index])?;
        Ok(true)
    }

    /// Replaces the project's description, on the card all its repositories share.
    pub fn set_project_description(&mut self, text: &str) -> Result<()> {
        let (Some(project), Some(central)) = (&mut self.project, &self.central) else {
            anyhow::bail!("only a project's repository has a project description");
        };
        project.description = text.trim().to_string();
        projects::write_description(central, &project.name, &project.description)
    }

    /// Replaces this repository's own notes about itself, on its `.brain/project.md`.
    pub fn set_repo_notes(&mut self, text: &str) -> Result<()> {
        let Some(project) = &mut self.project else { anyhow::bail!("the central brain isn't a project's repository") };
        project.notes = text.trim().to_string();
        projects::write_repo_card(self.store.dir(), &project.name, &project.notes)
    }

    /// Refuses with [`ApiError::DoingFull`] a person starting a task while `doing` holds one a person
    /// started; an agent always has room.
    fn check_doing_room(&self, by: Author) -> Result<(), ApiError> {
        match self.person_doing() {
            Some(doing) if by == Author::Human => Err(ApiError::DoingFull(doing)),
            _ => Ok(()),
        }
    }

    /// Parks every running task a person started, when they stop for the day. Agents' keep running: they may
    /// still be at work, and each agent closes its own.
    pub fn park_people_running(&mut self) -> Result<()> {
        let now = now();
        for task in self.tasks.iter_mut().filter(|t| t.is_running() && t.started_by() == Author::Human) {
            task.set_status(Status::Parked, now, self.author);
            self.store.save(task)?;
        }
        Ok(())
    }

    /// Creates an entry with a new id and returns the id: a `todo` task, a note, or a decision
    /// that starts from the ADR sections unless it comes with a description. The caller has checked
    /// the title and links (see `Brain::add`).
    pub(crate) fn create(&mut self, new: NewEntry) -> Result<u32> {
        // Before anything is written: a task that can't start isn't created half-way.
        if new.start && new.kind == Kind::Task {
            self.check_doing_room(self.author)?;
        }
        let mut task = Task::new(self.take_id()?, new.title, now());
        task.kind = new.kind;
        task.description = match new.description {
            Some(text) => sanitize_description(&text).0,
            None if task.kind == Kind::Decision => DECISION_TEMPLATE.to_string(),
            None => String::new(),
        };
        task.tags = new.tags;
        task.links = new.links;
        task.repo_links = new.repo_links;
        self.store.save(&mut task)?;
        let id = task.id;
        self.tasks.push(task);
        self.sync_tag_colors()?;
        if new.start && new.kind == Kind::Task {
            self.set_status(id, Status::Doing)?;
        }
        Ok(id)
    }

    /// Saves a new title and tags (a new title also renames the file).
    pub fn update(&mut self, id: u32, title: &str, tags: Vec<String>) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ApiError::Empty("title").into());
        }
        let index = self.index_of(id)?;
        title.clone_into(&mut self.tasks[index].title);
        self.tasks[index].tags = tags;
        self.store.save(&mut self.tasks[index])?;
        self.sync_tag_colors()
    }

    /// Deletes the entry's file, after removing the links other entries have to it. The links go first:
    /// if a save fails, the entry is still there and nothing points nowhere.
    pub fn delete(&mut self, id: u32) -> Result<()> {
        let index = self.index_of(id)?;
        for task in self.tasks.iter_mut().filter(|t| t.links.contains(&id)) {
            task.links.retain(|&link| link != id);
            self.store.save(task)?;
        }
        self.store.delete(&self.tasks[index])?;
        self.tasks.remove(index);
        Ok(())
    }

    /// Moves the entry to `to`, another brain (another project or repository, or the central one), and
    /// returns its id there: a new one, since each brain numbers its own. Everything else comes
    /// along (status, history, sessions, description, comments, tags) except its links, which name it by
    /// its id here, so they're dropped on both sides (links to other repositories too). A running task keeps
    /// its clock; one a person started needs room in `to`'s `doing`, as starting it there would.
    ///
    /// The entry is saved in `to` before it's deleted here: if something fails in between, it's in both
    /// brains rather than in neither.
    pub fn move_to(&mut self, id: u32, to: &mut Brain) -> Result<u32> {
        let index = self.index_of(id)?;
        if projects::same_dir(self.store.dir(), to.store.dir()) {
            anyhow::bail!("#{id} is already there");
        }
        if self.tasks[index].status == Status::Doing {
            to.check_doing_room(self.tasks[index].started_by())?;
        }
        let mut entry = Task {
            id: to.take_id()?,
            links: Vec::new(),
            repo_links: Vec::new(),
            path: None,
            ..self.tasks[index].clone()
        };
        entry.unknown.links.clear();
        to.store.save(&mut entry)?;
        let new_id = entry.id;
        to.tasks.push(entry);
        to.sync_tag_colors()?;
        self.delete(id)?;
        Ok(new_id)
    }

    /// Removes every link to `link`, an entry of another repository of the project that's gone.
    pub fn drop_repo_links_to(&mut self, link: &RepoLink) -> Result<()> {
        for task in self.tasks.iter_mut().filter(|t| t.repo_links.contains(link)) {
            task.repo_links.retain(|l| l != link);
            self.store.save(task)?;
        }
        Ok(())
    }

    /// Relates entry `id` to entry `other_id` of `other`, another repository of this brain's project, or
    /// unrelates them if they already are: the link is added here, and removed from whichever side stores
    /// it. Returns whether they are related now.
    pub fn toggle_repo_link(&mut self, id: u32, other: &mut Brain, other_id: u32) -> Result<bool> {
        let (Some(here), Some(there)) = (&self.project, &other.project) else {
            return Err(ApiError::NoRepo(other.store.dir().display().to_string()).into());
        };
        if here.name != there.name || here.repo == there.repo {
            return Err(ApiError::NoRepo(there.repo.clone()).into());
        }
        let to = RepoLink { repo: there.repo.clone(), id: other_id };
        let back = RepoLink { repo: here.repo.clone(), id };
        let index = self.index_of(id)?;
        let other_index = other.find(other_id).ok_or_else(|| ApiError::NotFoundIn(to.clone()))?;
        let related = self.tasks[index].repo_links.contains(&to) || other.tasks[other_index].repo_links.contains(&back);
        if !related {
            self.tasks[index].repo_links.push(to);
            self.store.save(&mut self.tasks[index])?;
            return Ok(true);
        }
        for (brain, index, link) in [(&mut *self, index, &to), (&mut *other, other_index, &back)] {
            let task = &mut brain.tasks[index];
            if task.repo_links.contains(link) {
                task.repo_links.retain(|l| l != link);
                brain.store.save(task)?;
            }
        }
        Ok(false)
    }

    /// Relates two entries, or unrelates them if they already are: the link is added to `id`, and
    /// removed from whichever side stores it. Returns whether they are related now.
    pub fn toggle_link(&mut self, id: u32, other: u32) -> Result<bool> {
        if id == other {
            return Err(ApiError::SelfLink(id).into());
        }
        for side in [id, other] {
            self.find(side).ok_or(ApiError::NotFound(side))?;
        }
        let related = self.are_related(id, other);
        for (from, to) in [(id, other), (other, id)] {
            let Some(index) = self.find(from) else { continue };
            let task = &mut self.tasks[index];
            let before = task.links.len();
            if related {
                task.links.retain(|&l| l != to);
            } else if from == id {
                task.links.push(to);
            }
            if task.links.len() != before {
                self.store.save(task)?;
            }
        }
        Ok(!related)
    }

    /// Replaces the description. Returns whether `##` headings had to become `###`.
    pub fn set_description(&mut self, id: u32, text: &str) -> Result<bool> {
        let index = self.index_of(id)?;
        let (text, demoted) = sanitize_description(text);
        self.tasks[index].description = text;
        self.store.save(&mut self.tasks[index])?;
        Ok(demoted)
    }

    /// Adds a timestamped comment. Returns whether headings had to become `####`.
    pub fn add_comment(&mut self, id: u32, text: &str) -> Result<bool> {
        self.push_comment(id, text, false)
    }

    /// Asks the user a question on a task and parks it, until a person answers with a comment (see
    /// [`Task::asked`]). Returns whether headings had to become `####`.
    pub fn ask(&mut self, id: u32, question: &str) -> Result<bool> {
        let index = self.task_index(id)?;
        let demoted = self.push_comment(id, question, true)?;
        if self.tasks[index].set_status(Status::Parked, now(), self.author) {
            self.store.save(&mut self.tasks[index])?;
        }
        Ok(demoted)
    }

    /// Proposes a decision for task `id`, which waits for a person to accept or reject it: the decision
    /// is created as proposed, linked to the task, and the task asks about it and is parked. Returns
    /// the decision's id.
    pub fn propose(&mut self, id: u32, decision: NewEntry) -> Result<u32> {
        self.task_index(id)?;
        let mut links = decision.links;
        if !links.contains(&id) {
            links.insert(0, id);
        }
        let title = decision.title.trim().to_string();
        let new = NewEntry { kind: Kind::Decision, title: title.clone(), links, start: false, ..decision };
        let decision = self.add(new)?;
        let index = self.index_of(decision)?;
        self.tasks[index].decision = DecisionState::Proposed;
        self.store.save(&mut self.tasks[index])?;
        self.ask(id, &format!("Proposed decision #{decision}, {title}: accept or reject it."))?;
        Ok(decision)
    }

    /// Accepts or rejects decision `id`, with `reason` (needed to reject), and answers the tasks that wait
    /// on it. A decision can be reconsidered: an accepted one rejected later, and back. Returns false if it
    /// already was in `state`.
    pub fn resolve(&mut self, id: u32, state: DecisionState, reason: &str) -> Result<bool> {
        let index = self.index_of(id)?;
        let decision = &self.tasks[index];
        if decision.kind != Kind::Decision {
            return Err(ApiError::NotADecision(id, decision.kind).into());
        }
        if decision.decision == state {
            return Ok(false);
        }
        if state == DecisionState::Rejected && reason.trim().is_empty() {
            return Err(ApiError::Empty("reason").into());
        }
        let title = decision.title.clone();
        let verdict = match state {
            DecisionState::Accepted => "Accepted",
            DecisionState::Rejected => "Rejected",
            DecisionState::Proposed => "Proposed again",
        };
        let said = |what: &str| match reason.trim() {
            "" => format!("{what}."),
            reason => format!("{what}: {reason}"),
        };
        self.tasks[index].decision = state;
        self.store.save(&mut self.tasks[index])?;
        self.push_comment(id, &said(verdict), false)?;
        let waiting: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| matches!(t.asked(), Some(Asked::Waiting(_))) && self.are_related(t.id, id))
            .map(|t| t.id)
            .collect();
        for task in waiting {
            self.push_comment(task, &said(&format!("{verdict} decision #{id}, {title}")), false)?;
        }
        Ok(true)
    }

    /// Adds a comment by this brain's author, a question or not. Returns whether headings had to become `####`.
    fn push_comment(&mut self, id: u32, text: &str, question: bool) -> Result<bool> {
        let index = self.index_of(id)?;
        let (body, demoted) = sanitize_comment(text);
        if body.is_empty() {
            return Err(ApiError::Empty(if question { "question" } else { "comment" }).into());
        }
        let comment = Comment { question, ..Comment::new(now(), body, self.author) };
        self.tasks[index].comments.push(comment);
        self.store.save(&mut self.tasks[index])?;
        Ok(demoted)
    }

    /// Changes a comment's text and marks it edited, unless the text is unchanged.
    /// Returns whether headings had to become `####`.
    pub fn update_comment(&mut self, id: u32, comment: usize, text: &str) -> Result<bool> {
        let index = self.index_of(id)?;
        let (body, demoted) = sanitize_comment(text);
        if body.is_empty() {
            return Err(ApiError::Empty("comment").into());
        }
        let task = &mut self.tasks[index];
        let Some(existing) = task.comments.get_mut(comment) else {
            return Err(ApiError::NoComment(id, comment).into());
        };
        if existing.body != body {
            existing.body = body;
            existing.edited = Some(now());
            self.store.save(task)?;
        }
        Ok(demoted)
    }

    /// Deletes a comment.
    pub fn delete_comment(&mut self, id: u32, comment: usize) -> Result<()> {
        let index = self.index_of(id)?;
        let task = &mut self.tasks[index];
        if comment >= task.comments.len() {
            return Err(ApiError::NoComment(id, comment).into());
        }
        task.comments.remove(comment);
        self.store.save(task)
    }

    /// Where the markdown export of `view` goes: `<data dir>/reports/<name>.md`.
    pub fn report_path(&self, view: &ReportView) -> PathBuf {
        self.store.reports_dir().join(export::file_name(view))
    }

    /// Writes the report as markdown and returns the path.
    pub fn export_report(&self, view: &ReportView) -> Result<PathBuf> {
        self.write_report(&self.report(view), view)
    }

    /// Writes `report`, made for `view`, to this brain's `reports/` as markdown and returns the path;
    /// for a report computed elsewhere, such as several brains' merged.
    pub fn write_report(&self, report: &super::Report, view: &ReportView) -> Result<PathBuf> {
        let markdown = export::to_markdown(report, view, now());
        let path = self.report_path(view);
        crate::store::write_file(&path, &markdown)?;
        Ok(path)
    }
}
