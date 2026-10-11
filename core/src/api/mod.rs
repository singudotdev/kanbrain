//! Operations by entry id, for the command line: what an agent (or a script)
//! does without a screen. They go through the same `Brain` actions as the desktop app, so tracking
//! rules, tag colors and file names behave exactly as there.
//!
//! - [`json`]: entries as JSON, for scripts.
//! - [`text`]: entries as compact text, for agents.
//! - [`context`]: the markdown summary an agent reads at the start of a session.

mod context;
pub mod json;
pub mod text;

use crate::brain::{Brain, NewEntry};
use crate::model::{Kind, RepoLink, Status, Task, parse_tags};
use anyhow::Result;
use std::fmt;

/// Why an operation by id was refused, as the CLI prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// No entry has this id.
    NotFound(u32),
    /// Only tasks have a status; this entry is a note or decision.
    NotATask(u32, Kind),
    /// Only decisions are accepted or rejected; this entry is a task or note.
    NotADecision(u32, Kind),
    /// A title, comment or description that is empty once trimmed.
    Empty(&'static str),
    /// A link from an entry to itself.
    SelfLink(u32),
    /// The entry has no comment at this index (counting from 0).
    NoComment(u32, usize),
    /// A link to a repository that isn't another one of this brain's project, or can't be read.
    NoRepo(String),
    /// A link to an entry that the other repository doesn't have.
    NotFoundIn(RepoLink),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::NotFound(id) => write!(f, "no entry #{id}"),
            ApiError::NotATask(id, kind) => write!(f, "#{id} is a {}: only tasks have a status", kind.as_str()),
            ApiError::NotADecision(id, kind) => {
                write!(f, "#{id} is a {}: only decisions are accepted or rejected", kind.as_str())
            }
            ApiError::Empty(what) => write!(f, "the {what} is empty"),
            ApiError::SelfLink(id) => write!(f, "#{id} can't link to itself"),
            ApiError::NoComment(id, index) => write!(f, "#{id} has no comment {}", index + 1),
            ApiError::NoRepo(repo) => write!(f, "`{repo}` isn't another repository of this project"),
            ApiError::NotFoundIn(link) => write!(f, "no entry {link}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl Brain {
    /// The entry with this id, or `NotFound`.
    pub fn entry(&self, id: u32) -> Result<&Task, ApiError> {
        self.task(id).ok_or(ApiError::NotFound(id))
    }

    /// Creates an entry and returns its id. Links must point at existing entries.
    pub fn add(&mut self, new: NewEntry) -> Result<u32> {
        if new.title.trim().is_empty() {
            return Err(ApiError::Empty("title").into());
        }
        for &link in &new.links {
            self.entry(link)?;
        }
        self.check_repo_links(&new.repo_links)?;
        let new = NewEntry { title: new.title.trim().to_string(), ..new };
        self.create(new)
    }

    /// Moves a task to `status`. `doing` starts its clock and parks the running task, whose id it
    /// returns, so the agent learns its move set another task aside.
    pub fn change_status(&mut self, id: u32, status: Status) -> Result<Option<u32>> {
        let running = self.running().map(|t| t.id).filter(|&r| r != id && status == Status::Doing);
        self.set_status(id, status)?;
        Ok(running)
    }

    /// Adds a timestamped comment, e.g. progress notes from an agent.
    pub fn comment(&mut self, id: u32, text: &str) -> Result<()> {
        self.add_comment(id, text).map(drop)
    }

    /// Replaces the description.
    pub fn describe(&mut self, id: u32, text: &str) -> Result<()> {
        self.set_description(id, text).map(drop)
    }

    /// Adds links from `id` to each of `others` and of `repo_links` (entries of the project's other
    /// repositories), skipping ones it already has.
    pub fn link(&mut self, id: u32, others: &[u32], repo_links: &[RepoLink]) -> Result<()> {
        for &other in others {
            if other == id {
                return Err(ApiError::SelfLink(id).into());
            }
            self.entry(other)?;
        }
        self.check_repo_links(repo_links)?;
        let index = self.index_of(id)?;
        let task = &mut self.tasks[index];
        for &other in others {
            if !task.links.contains(&other) {
                task.links.push(other);
            }
        }
        for link in repo_links {
            if !task.repo_links.contains(link) {
                task.repo_links.push(link.clone());
            }
        }
        self.store.save(task)
    }

    /// Checks that each link names another repository of this project, and an entry it has.
    fn check_repo_links(&self, links: &[RepoLink]) -> Result<(), ApiError> {
        for link in links {
            let other = self.sibling(&link.repo).ok_or_else(|| ApiError::NoRepo(link.repo.clone()))?;
            other.task(link.id).ok_or_else(|| ApiError::NotFoundIn(link.clone()))?;
        }
        Ok(())
    }

    /// Entries matching every whitespace-separated term in their title, tags, description or comments
    /// (`#term` matches a tag prefix only), case-insensitive; tasks not done first, then newest first.
    pub fn search(&self, query: &str) -> Vec<&Task> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut found: Vec<&Task> = self.tasks.iter().filter(|t| terms.iter().all(|term| mentions(t, term))).collect();
        found.sort_by_key(|t| (t.is_task() && t.status == Status::Done, std::cmp::Reverse(t.id)));
        found
    }
}

/// Whether the entry mentions `term` (already lowercase): `#tag` matches a tag prefix only,
/// anything else the title, a tag, the description or a comment.
fn mentions(task: &Task, term: &str) -> bool {
    match term.strip_prefix('#') {
        Some("") => true,
        Some(tag) => parse_tags(tag).first().is_some_and(|tag| task.tags.iter().any(|t| t.starts_with(tag))),
        None => {
            task.title.to_lowercase().contains(term)
                || task.tags.iter().any(|t| t.contains(term))
                || task.description.to_lowercase().contains(term)
                || task.comments.iter().any(|c| c.body.to_lowercase().contains(term))
        }
    }
}

#[cfg(test)]
#[path = "../../tests/api.rs"]
mod tests;
