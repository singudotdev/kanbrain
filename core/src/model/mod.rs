//! Entries (tasks, notes and decisions), their links, status history, time-tracking sessions and comments.
//!
//! - [`markdown`]: reading and writing the task file format.
//! - [`text`]: tags, file-name slugs and heading rules for user text.
//! - [`time`]: timestamps, ages and durations.

mod markdown;
mod text;
mod time;

pub use markdown::{not_a_link, parse_links};
pub use text::{counted, one_line, parse_tags, sanitize_comment, sanitize_description, slugify};
#[cfg(test)]
pub use time::parse_ts;
pub use time::{covered, fmt_age, fmt_clock, fmt_dur, fmt_ts, now};

use crate::enums::named_enum;
use chrono::{DateTime, Duration, Local};
use std::path::PathBuf;

named_enum! {
    /// Where a task stands. Declared in the order the task list shows them, so
    /// `position()` is the sort key; the names are what task files contain.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Status {
        /// Being worked on.
        Doing => "doing",
        /// Not started yet.
        Todo => "todo",
        /// Started, then set aside for now.
        Parked => "parked",
        /// Finished; hidden from the list by default.
        Done => "done",
    }
}

named_enum! {
    /// Who made a change: the user, or a coding agent working through `kb`. Comments and status changes
    /// record it, so the board tells them apart, and so a question an agent asked waits for a person.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum Author {
        /// The user: in the kanban app, or `kb` typed at a terminal.
        #[default]
        Human => "human",
        /// A coding agent, as `kb` finds from its environment.
        Agent => "agent",
    }
}

named_enum! {
    /// Where a decision stands. An agent records the decisions within its task as accepted, and proposes
    /// those it shouldn't take alone (architecture, dependencies, storage) for a person to accept or reject.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum DecisionState {
        /// In force.
        #[default]
        Accepted => "accepted",
        /// Waiting for a person to accept or reject it.
        Proposed => "proposed",
        /// Turned down: kept so the same approach isn't proposed again.
        Rejected => "rejected",
    }
}

named_enum! {
    /// What an entry is. Only tasks have a status, history and sessions; notes and decisions are
    /// documents that tasks and each other link to. Declared in the order `tab` cycles the list.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum Kind {
        /// Something to do.
        #[default]
        Task => "task",
        /// Something to remember: a finding, how something works, a snippet.
        Note => "note",
        /// Something decided, with its context and consequences (an ADR).
        Decision => "decision",
    }
}

/// One status change, as kept in the task's history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusChange {
    /// When the status changed.
    pub at: DateTime<Local>,
    /// The status before.
    pub from: Status,
    /// The status after.
    pub to: Status,
    /// Who moved it.
    pub by: Author,
}

/// One continuous stretch of tracked work on a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// When the session started.
    pub start: DateTime<Local>,
    /// `None` while the session is still running.
    pub end: Option<DateTime<Local>>,
}

impl Session {
    /// How long the session lasted, or has lasted so far if it's running.
    pub fn duration(&self, now: DateTime<Local>) -> Duration {
        (self.end.unwrap_or(now) - self.start).max(Duration::zero())
    }

    /// Part of this session that falls inside `[from, to)`.
    pub fn overlap(&self, from: DateTime<Local>, to: DateTime<Local>, now: DateTime<Local>) -> Duration {
        let (start, end) = self.span(from, to, now);
        (end - start).max(Duration::zero())
    }

    /// Where this session starts and ends within `[from, to)`, a running one ending `now`; the end is
    /// before the start when it falls outside.
    pub fn span(
        &self,
        from: DateTime<Local>,
        to: DateTime<Local>,
        now: DateTime<Local>,
    ) -> (DateTime<Local>, DateTime<Local>) {
        (self.start.max(from), self.end.unwrap_or(now).min(to))
    }
}

/// A timestamped comment on a task, like on a GitHub/GitLab issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// When the comment was written.
    pub created: DateTime<Local>,
    /// Set when the body was changed after posting.
    pub edited: Option<DateTime<Local>>,
    /// The comment text (markdown).
    pub body: String,
    /// Who wrote it.
    pub author: Author,
    /// A question to the user: the task waits for a person's answer (see [`Task::asked`]).
    pub question: bool,
    /// The answer to the question before it, whoever wrote it: a verdict on the decision it asked about, which an
    /// agent may give on the user's word.
    pub answer: bool,
    /// Words of the header's parentheses this version doesn't know, written back as they were.
    pub unknown: Vec<String>,
}

impl Comment {
    /// A plain comment by `author`.
    pub fn new(created: DateTime<Local>, body: String, author: Author) -> Self {
        Self { created, edited: None, body, author, question: false, answer: false, unknown: Vec::new() }
    }
}

/// Where a question an agent asked on a task stands, while the task is parked waiting on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked<'a> {
    /// Nobody has answered the question yet.
    Waiting(&'a Comment),
    /// A person answered (the latest answer): the task can be picked up again.
    Answered(&'a Comment),
}

impl Asked<'_> {
    /// Where it stands, as the command line's text and JSON say it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting(_) => "waiting for an answer",
            Self::Answered(_) => "answered",
        }
    }
}

/// An entry in another repository of the same project, written `shop-api#3`: ids repeat across
/// repositories, so the repository's name goes with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoLink {
    /// The repository's name in the project, as `kb init` registered it.
    pub repo: String,
    /// The entry's id there.
    pub id: u32,
}

impl std::fmt::Display for RepoLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.repo, self.id)
    }
}

/// An entry: a task, note or decision. What it is, how it's tagged and linked, and for tasks,
/// where it stands and how it got there.
#[derive(Clone, Debug)]
pub struct Task {
    /// Unique number across all kinds, also the start of the file name.
    pub id: u32,
    /// Task, note or decision.
    pub kind: Kind,
    /// One-line title.
    pub title: String,
    /// Lowercase, without the leading `#`.
    pub tags: Vec<String>,
    /// Ids of related entries, in the order they were added. Deleting an entry removes the links to it.
    pub links: Vec<u32>,
    /// Related entries in the project's other repositories, in the order they were added. Nothing checks
    /// them once written: the other repository may be elsewhere, or the entry deleted there since.
    pub repo_links: Vec<RepoLink>,
    /// Where the task stands.
    pub status: Status,
    /// Decisions only: whether it's in force, proposed or rejected.
    pub decision: DecisionState,
    /// When the task was created.
    pub created: DateTime<Local>,
    /// Free text describing the task.
    pub description: String,
    /// Oldest first.
    pub comments: Vec<Comment>,
    /// Every status change, oldest first.
    pub history: Vec<StatusChange>,
    /// Tracked work, oldest first; only the last one can be open, and only while the task is `doing`.
    pub sessions: Vec<Session>,
    /// File this task was loaded from / saved to.
    pub path: Option<PathBuf>,
    /// What the file holds that this version doesn't understand, written back as it was.
    pub unknown: Unknown,
}

/// Fields, link forms and sections a newer version wrote, kept as written: otherwise an older `kb` or app
/// saving the entry, for a comment or a status change, would drop them from the file.
#[derive(Clone, Debug, Default)]
pub struct Unknown {
    /// `- key: value` lines before the first section, in file order.
    pub meta: Vec<String>,
    /// Words of `- links:` that are neither an id nor `REPO#ID`.
    pub links: Vec<String>,
    /// Other `##` sections, each from its heading line, in file order.
    pub sections: Vec<String>,
}

impl Task {
    /// A new `todo` task.
    pub fn new(id: u32, title: String, now: DateTime<Local>) -> Self {
        Self {
            id,
            kind: Kind::Task,
            title,
            tags: Vec::new(),
            links: Vec::new(),
            repo_links: Vec::new(),
            unknown: Unknown::default(),
            status: Status::Todo,
            decision: DecisionState::Accepted,
            created: now,
            description: String::new(),
            comments: Vec::new(),
            history: Vec::new(),
            sessions: Vec::new(),
            path: None,
        }
    }

    /// Moves the task to `status` and records the change, made `by` someone. Returns false if it already
    /// was there. Leaving `doing` closes the open session, so a clock never keeps running on a task that
    /// isn't being worked on.
    pub fn set_status(&mut self, status: Status, now: DateTime<Local>, by: Author) -> bool {
        if self.status == status {
            return false;
        }
        if status != Status::Doing {
            self.stop_session(now);
        }
        self.history.push(StatusChange { at: now, from: self.status, to: status, by });
        self.status = status;
        true
    }

    /// Whether the last session is still open.
    pub fn is_running(&self) -> bool {
        self.sessions.last().is_some_and(|s| s.end.is_none())
    }

    /// How long the open session has been running, if any.
    pub fn running_for(&self, now: DateTime<Local>) -> Option<Duration> {
        self.sessions.last().filter(|s| s.end.is_none()).map(|s| s.duration(now))
    }

    /// Opens a session now, unless one is already running. The caller moves the task to `doing`.
    pub fn start_session(&mut self, now: DateTime<Local>) {
        if !self.is_running() {
            self.sessions.push(Session { start: now, end: None });
        }
    }

    /// Closes the open session, if any.
    pub fn stop_session(&mut self, now: DateTime<Local>) {
        if let Some(session) = self.sessions.last_mut().filter(|s| s.end.is_none()) {
            session.end = Some(now.max(session.start));
        }
    }

    /// Time tracked in all sessions.
    pub fn total(&self, now: DateTime<Local>) -> Duration {
        self.sessions.iter().map(|s| s.duration(now)).sum()
    }

    /// Time tracked inside `[from, to)`: sessions are clipped to the range. For several tasks, see [`worked`].
    pub fn time_between(&self, from: DateTime<Local>, to: DateTime<Local>, now: DateTime<Local>) -> Duration {
        self.sessions.iter().map(|s| s.overlap(from, to, now)).sum()
    }

    /// Who last moved the task to `doing`: a person or an agent. A task in `doing` with no such change (from
    /// an older version, or edited by hand) counts as a person's.
    pub fn started_by(&self) -> Author {
        self.history.iter().rev().find(|c| c.to == Status::Doing).map_or(Author::Human, |c| c.by)
    }

    /// When the status last changed (or the task was created, if never); used for sorting.
    pub fn status_since(&self) -> DateTime<Local> {
        self.history.last().map_or(self.created, |c| c.at)
    }

    /// The question this parked task waits on, or its answer. A question is the latest comment marked as
    /// one; a person's comment after it answers it, as does a verdict on the decision it asked about. Starting the
    /// task after either means it was picked up, so it's neither once the task is parked again for another reason.
    pub fn asked(&self) -> Option<Asked<'_>> {
        if !self.is_task() || self.status != Status::Parked {
            return None;
        }
        let at = self.comments.iter().rposition(|c| c.question)?;
        let question = &self.comments[at];
        let answer = self.comments[at + 1..].iter().rev().find(|c| c.author == Author::Human || c.answer);
        // Strictly after: times are whole seconds, and an agent that starts a task and asks at once does
        // both in the same second, which isn't picking it up again.
        let picked_up = self.history.iter().any(|c| c.to == Status::Doing && c.at > question.created);
        match answer {
            _ if picked_up => None,
            Some(answer) => Some(Asked::Answered(answer)),
            None => Some(Asked::Waiting(question)),
        }
    }

    /// Whether this entry has a status, history and sessions (only tasks do).
    pub fn is_task(&self) -> bool {
        self.kind == Kind::Task
    }

    /// Search: every whitespace-separated term must match. `#term` matches a tag prefix;
    /// anything else matches the title or a tag. Case-insensitive.
    pub fn matches(&self, query: &str) -> bool {
        query.split_whitespace().all(|term| self.has_term(&term.to_lowercase(), false))
    }

    /// Whether the entry has `term`, already lowercase: `#tag` (written as a tag is typed anywhere) a tag that
    /// starts with it; anything else the title or a tag, and with `everywhere`, the description or a comment too.
    pub(crate) fn has_term(&self, term: &str, everywhere: bool) -> bool {
        match term.strip_prefix('#') {
            Some("") => true,
            Some(tag) => text::parse_tags(tag).first().is_some_and(|tag| self.tags.iter().any(|t| t.starts_with(tag))),
            None => {
                self.title.to_lowercase().contains(term)
                    || self.tags.iter().any(|t| t.contains(term))
                    || everywhere
                        && (self.description.to_lowercase().contains(term)
                            || self.comments.iter().any(|c| c.body.to_lowercase().contains(term)))
            }
        }
    }
}

/// Time worked on any of `tasks` inside `[from, to)`: time two of them ran at once counts once (see
/// [`covered`]), so agents' tasks running in parallel don't make a day longer than it was.
pub fn worked<'a>(
    tasks: impl IntoIterator<Item = &'a Task>,
    from: DateTime<Local>,
    to: DateTime<Local>,
    now: DateTime<Local>,
) -> Duration {
    covered(tasks.into_iter().flat_map(|t| &t.sessions).map(|s| s.span(from, to, now)))
}

#[cfg(test)]
#[path = "../../tests/model.rs"]
mod tests;
