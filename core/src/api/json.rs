//! Entries as JSON, for `--json`. A separate shape from `Task`, so the file
//! format and the JSON can evolve independently, and timestamps read the same as in the files.

use crate::brain::Brain;
use crate::model::{Asked, Author, Comment, Kind, Task, fmt_ts, now};
use serde::Serialize;

/// An entry with everything an agent needs to act on it.
#[derive(Debug, Serialize)]
pub struct Entry<'a> {
    /// The id to pass to other commands.
    pub id: u32,
    /// `task`, `note` or `decision`.
    pub kind: &'static str,
    /// One-line title.
    pub title: &'a str,
    /// Tasks only: `todo`, `doing`, `parked` or `done`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
    /// Decisions only: `accepted`, `proposed` or `rejected`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<&'static str>,
    /// A parked task's question: `waiting for an answer` or `answered`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked: Option<&'static str>,
    /// Lowercase, without `#`.
    pub tags: &'a [String],
    /// Ids this entry links to. Empty fields are left out, as every token counts for an agent.
    #[serde(skip_serializing_if = "<[u32]>::is_empty")]
    pub links: &'a [u32],
    /// Entries of the project's other repositories this one links to, as `shop-api#3`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub repo_links: Vec<String>,
    /// Ids of entries linking to this one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub linked_from: Vec<u32>,
    /// RFC 3339.
    pub created: String,
    /// Free text (markdown).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub description: &'a str,
    /// Oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<EntryComment<'a>>,
    /// Tracked time in minutes, if the task was ever tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracked_minutes: Option<i64>,
    /// Whether the task's clock is running now; only present when it is.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub running: bool,
    /// The markdown file, to read or edit directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// A comment on an entry.
#[derive(Debug, Serialize)]
pub struct EntryComment<'a> {
    /// RFC 3339.
    pub created: String,
    /// RFC 3339, if edited after posting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited: Option<String>,
    /// The comment text (markdown).
    pub body: &'a str,
    /// `agent` for an agent's; left out for a person's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<&'static str>,
    /// Whether it's a question to the user; only present when it is.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub question: bool,
}

/// A short form for lists and search results: no description or comments.
#[derive(Debug, Serialize)]
pub struct Summary<'a> {
    /// The id to pass to other commands.
    pub id: u32,
    /// `task`, `note` or `decision`.
    pub kind: &'static str,
    /// One-line title.
    pub title: &'a str,
    /// Tasks only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
    /// Decisions only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<&'static str>,
    /// A parked task's question, as in [`Entry`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked: Option<&'static str>,
    /// Lowercase, without `#`.
    pub tags: &'a [String],
}

impl<'a> Entry<'a> {
    /// The full JSON shape of `task`, with its backlinks from `brain`.
    pub fn new(brain: &Brain, task: &'a Task) -> Self {
        let now = now();
        Self {
            id: task.id,
            kind: task.kind.as_str(),
            title: &task.title,
            status: task.is_task().then(|| task.status.as_str()),
            decision: decision(task),
            asked: asked(task),
            tags: &task.tags,
            links: &task.links,
            repo_links: task.repo_links.iter().map(ToString::to_string).collect(),
            linked_from: brain.backlinks(task.id).iter().map(|t| t.id).collect(),
            created: fmt_ts(task.created),
            description: &task.description,
            comments: task.comments.iter().map(EntryComment::new).collect(),
            tracked_minutes: (!task.sessions.is_empty()).then(|| task.total(now).num_minutes()),
            running: task.is_running(),
            path: task.path.as_ref().map(|p| p.display().to_string()),
        }
    }
}

impl<'a> EntryComment<'a> {
    /// The JSON shape of `comment`.
    fn new(comment: &'a Comment) -> Self {
        Self {
            created: fmt_ts(comment.created),
            edited: comment.edited.map(fmt_ts),
            body: &comment.body,
            author: (comment.author != Author::Human).then(|| comment.author.as_str()),
            question: comment.question,
        }
    }
}

impl<'a> Summary<'a> {
    /// The short JSON shape of `task`.
    pub fn new(task: &'a Task) -> Self {
        Self {
            id: task.id,
            kind: task.kind.as_str(),
            title: &task.title,
            status: task.is_task().then(|| task.status.as_str()),
            decision: decision(task),
            asked: asked(task),
            tags: &task.tags,
        }
    }
}

/// A decision's state; `None` for tasks and notes.
fn decision(task: &Task) -> Option<&'static str> {
    (task.kind == Kind::Decision).then(|| task.decision.as_str())
}

/// Where a parked task's question stands; `None` without one.
fn asked(task: &Task) -> Option<&'static str> {
    task.asked().map(|asked| match asked {
        Asked::Waiting(_) => "waiting for an answer",
        Asked::Answered(_) => "answered",
    })
}
