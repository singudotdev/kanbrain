//! The task file format:
//!
//! ```markdown
//! # Title
//!
//! - id: 2
//! - status: doing
//! - tags: prod, support
//! - links: 5, 9, shop-api#3
//! - created: 2026-09-24T10:12:00+02:00
//!
//! ## Description
//!
//! free text
//!
//! ## Comments
//!
//! ### 2026-09-24T10:40:00+02:00 (edited 2026-09-24T10:52:00+02:00)
//!
//! comment text
//!
//! ### 2026-09-24T10:45:00+02:00 (agent, question)
//!
//! a question an agent asked
//!
//! ## History
//!
//! - 2026-09-24T10:57:00+02:00 todo -> doing (agent)
//!
//! ## Sessions
//!
//! - 2026-09-24T10:57:00+02:00 -> 2026-09-24T11:42:00+02:00 (45m)
//! - 2026-09-25T09:30:00+02:00 -> running
//! ```
//!
//! `## Sessions` is only written once the task has been tracked, so untracked files stay short.
//! Notes and decisions have `- kind: note` / `- kind: decision` instead of `- status:`, and no
//! History or Sessions, and a decision that isn't in force has `- decision: proposed|rejected`. What a person
//! wrote or did carries no mark; what an agent did says `agent`. `- links:` is only written when there are links. `shop-api#3` is entry 3 of the
//! project's `shop-api` repository. Other `- key:` lines, link words and `##` sections are kept and
//! written back (see [`Unknown`]).

use super::text::parse_tags;
use super::time::{fmt_dur, fmt_ts, now, parse_ts};
use super::{Author, Comment, DecisionState, Kind, RepoLink, Session, Status, StatusChange, Task, Unknown};
use chrono::{DateTime, Local};
use std::fmt::Write;

impl Task {
    /// The task as a markdown file (see the format at the top of this module).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        self.write_markdown(&mut out).expect("writing to a String cannot fail");
        out
    }

    /// Writes the file contents into `out`.
    fn write_markdown(&self, out: &mut String) -> std::fmt::Result {
        writeln!(out, "# {}\n", self.title)?;
        writeln!(out, "- id: {}", self.id)?;
        if self.is_task() {
            writeln!(out, "- status: {}", self.status.as_str())?;
        } else {
            writeln!(out, "- kind: {}", self.kind.as_str())?;
        }
        if self.kind == Kind::Decision && self.decision != DecisionState::Accepted {
            writeln!(out, "- decision: {}", self.decision.as_str())?;
        }
        writeln!(out, "- tags: {}", self.tags.join(", "))?;
        if !self.links.is_empty() || !self.repo_links.is_empty() || !self.unknown.links.is_empty() {
            let local = self.links.iter().map(u32::to_string);
            let links: Vec<String> = local
                .chain(self.repo_links.iter().map(RepoLink::to_string))
                .chain(self.unknown.links.iter().cloned())
                .collect();
            writeln!(out, "- links: {}", links.join(", "))?;
        }
        for line in &self.unknown.meta {
            writeln!(out, "{line}")?;
        }
        writeln!(out, "- created: {}\n", fmt_ts(self.created))?;

        writeln!(out, "## Description\n")?;
        if !self.description.is_empty() {
            writeln!(out, "{}\n", self.description)?;
        }

        writeln!(out, "## Comments\n")?;
        for comment in &self.comments {
            write!(out, "### {}", fmt_ts(comment.created))?;
            let attributes = comment_attributes(comment);
            if !attributes.is_empty() {
                write!(out, " ({})", attributes.join(", "))?;
            }
            writeln!(out, "\n")?;
            if !comment.body.is_empty() {
                writeln!(out, "{}\n", comment.body)?;
            }
        }

        if self.is_task() {
            self.write_tracking(out)?;
        }
        for section in &self.unknown.sections {
            if !out.ends_with("\n\n") {
                out.push('\n');
            }
            writeln!(out, "{section}\n")?;
        }
        Ok(())
    }

    /// Writes the History and Sessions sections.
    fn write_tracking(&self, out: &mut String) -> std::fmt::Result {
        writeln!(out, "## History\n")?;
        for change in &self.history {
            write!(out, "- {} {} -> {}", fmt_ts(change.at), change.from.as_str(), change.to.as_str())?;
            writeln!(out, "{}", by_mark(change.by))?;
        }

        if !self.sessions.is_empty() {
            writeln!(out, "\n## Sessions\n")?;
        }
        for session in &self.sessions {
            match session.end {
                Some(end) => {
                    let duration = fmt_dur(end - session.start);
                    writeln!(out, "- {} -> {} ({duration})", fmt_ts(session.start), fmt_ts(end))?;
                }
                None => writeln!(out, "- {} -> running", fmt_ts(session.start))?,
            }
        }
        Ok(())
    }

    /// Parses a task file. Unknown `- key:` lines, link words and `##` sections are kept as written;
    /// open sessions that can't be running are closed (see `Parsed::into_task`).
    pub fn from_markdown(text: &str) -> Result<Task, String> {
        let mut parsed = Parsed::default();
        let mut section = Section::Meta;
        // Read line by line; each `## ` heading switches the section the next lines belong to.
        for line in text.lines() {
            let trimmed = line.trim_end();
            if let Some(heading) = trimmed.strip_prefix("## ") {
                section = Section::from_heading(heading);
                if matches!(section, Section::Other) {
                    parsed.unknown.sections.push(trimmed.to_string());
                }
                continue;
            }
            match section {
                Section::Meta => parsed.meta_line(trimmed),
                // Free text keeps its original indentation, so pass the untrimmed line.
                Section::Description => parsed.description.push(line),
                Section::Comments => parsed.comment_line(line),
                Section::History => parsed.history_line(trimmed)?,
                Section::Sessions => parsed.session_line(trimmed)?,
                Section::Other => parsed.other_line(line),
            }
        }
        parsed.into_task()
    }
}

/// The `##` section a line belongs to.
#[derive(Clone, Copy)]
enum Section {
    /// Title and `- key: value` lines, before the first `##`.
    Meta,
    /// `## Description`: free text.
    Description,
    /// `## Comments`: `### <timestamp>` headings, each followed by the comment.
    Comments,
    /// `## History`: one `- <time> <from> -> <to>` line per status change.
    History,
    /// `## Sessions`: one `- <start> -> <end> (duration)` or `- <start> -> running` line each.
    Sessions,
    /// Any other `##` section, kept as written.
    Other,
}

impl Section {
    /// The section a `## <heading>` starts.
    fn from_heading(heading: &str) -> Self {
        match heading.trim() {
            "Description" => Section::Description,
            "Comments" => Section::Comments,
            "History" => Section::History,
            "Sessions" => Section::Sessions,
            _ => Section::Other,
        }
    }
}

/// Everything read from a file so far, before it becomes a `Task`.
#[derive(Default)]
struct Parsed<'a> {
    /// From the `# Title` line.
    title: Option<String>,
    /// From `- id:`; required.
    id: Option<u32>,
    /// From `- kind:`; a task if missing.
    kind: Option<Kind>,
    /// From `- links:`, the ids in this brain.
    links: Vec<u32>,
    /// From `- links:`, the entries of other repositories.
    repo_links: Vec<RepoLink>,
    /// From `- status:`; `todo` if missing.
    status: Option<Status>,
    /// From `- decision:`; accepted if missing.
    decision: DecisionState,
    /// From `- tags:`.
    tags: Vec<String>,
    /// From `- created:`.
    created: Option<DateTime<Local>>,
    /// Lines of the description, joined at the end.
    description: Vec<&'a str>,
    /// Each comment with its body lines, joined at the end.
    comments: Vec<(Comment, Vec<&'a str>)>,
    /// In file order; sorted at the end.
    history: Vec<StatusChange>,
    /// In file order; sorted at the end.
    sessions: Vec<Session>,
    /// Whatever the lines above don't take.
    unknown: Unknown,
}

impl<'a> Parsed<'a> {
    /// The title line or a `- key: value` line before the first section.
    fn meta_line(&mut self, line: &str) {
        if self.title.is_none()
            && let Some(title) = line.strip_prefix("# ")
        {
            self.title = Some(title.trim().to_string());
            return;
        }
        let Some((key, value)) = line.trim_start().strip_prefix("- ").and_then(|rest| rest.split_once(':')) else {
            return;
        };
        match key.trim() {
            "id" => self.id = value.trim().parse().ok(),
            "status" => self.status = Status::parse(value),
            "kind" => self.kind = Kind::parse(value),
            "decision" => match DecisionState::parse(value) {
                Some(decision) => self.decision = decision,
                None => self.unknown.meta.push(line.trim().to_string()),
            },
            "links" => {
                (self.links, self.repo_links) = parse_links(value);
                self.unknown.links = link_words(value).filter(|w| parse_link(w).is_none()).map(String::from).collect();
            }
            "tags" => self.tags = parse_tags(value),
            "created" => self.created = parse_ts(value),
            _ => self.unknown.meta.push(line.trim().to_string()),
        }
    }

    /// A line of a section this version doesn't know, added to it with blank lines at its end dropped
    /// when the file is written back.
    fn other_line(&mut self, line: &str) {
        if let Some(section) = self.unknown.sections.last_mut() {
            section.push('\n');
            section.push_str(line);
        }
    }

    /// `### <created> [(<attributes>)]` starts a comment; other lines belong to the current one.
    fn comment_line(&mut self, line: &'a str) {
        match comment_header(line.trim_end()) {
            Some(comment) => self.comments.push((comment, Vec::new())),
            None => {
                // Text before the first comment header has nowhere to go.
                if let Some((_, body)) = self.comments.last_mut() {
                    body.push(line);
                }
            }
        }
    }

    /// `- <time> <from> -> <to> [(agent)]`. The time may contain a space when typed by hand. Who moved it
    /// is only a record, so a name this version doesn't know reads as a person rather than losing the file.
    fn history_line(&mut self, line: &str) -> Result<(), String> {
        let Some(rest) = line.trim_start().strip_prefix("- ") else { return Ok(()) };
        let bad = || format!("bad history line: {rest}");
        let (left, right) = rest.split_once("->").ok_or_else(bad)?;
        let (at, from) = left.trim().rsplit_once(' ').ok_or_else(bad)?;
        let at = parse_ts(at).ok_or_else(|| format!("bad timestamp: {}", at.trim()))?;
        let from = Status::parse(from).ok_or_else(bad)?;
        let (to, by) = right.split_once('(').unwrap_or((right, ""));
        let to = Status::parse(to).ok_or_else(bad)?;
        let by = Author::parse(by.trim().trim_end_matches(')')).unwrap_or_default();
        self.history.push(StatusChange { at, from, to, by });
        Ok(())
    }

    /// `- <start> -> <end> (duration)` or `- <start> -> running`; the duration is informational.
    fn session_line(&mut self, line: &str) -> Result<(), String> {
        let Some(rest) = line.trim_start().strip_prefix("- ") else { return Ok(()) };
        let (start, end) = rest.split_once("->").ok_or_else(|| format!("bad session line: {rest}"))?;
        let start = parse_ts(start).ok_or_else(|| format!("bad timestamp: {}", start.trim()))?;
        let end = end.split('(').next().unwrap_or_default().trim();
        let end =
            if end == "running" { None } else { Some(parse_ts(end).ok_or_else(|| format!("bad timestamp: {end}"))?) };
        self.sessions.push(Session { start, end });
        Ok(())
    }

    /// Builds the task. The `- status:` line decides where the task stands, so open sessions
    /// are closed where they can't be running: any but the last at the next one's start, and the
    /// last at the latest status change if the task isn't `doing`.
    fn into_task(mut self) -> Result<Task, String> {
        let id = self.id.ok_or("missing `- id:` line")?;
        self.history.sort_by_key(|c| c.at);
        self.sessions.sort_by_key(|s| s.start);
        let next_starts: Vec<_> = self.sessions.iter().skip(1).map(|s| s.start).collect();
        for (session, next_start) in self.sessions.iter_mut().zip(next_starts) {
            session.end.get_or_insert(next_start);
        }
        let status = self.status.unwrap_or(Status::Todo);
        if status != Status::Doing
            && let Some(last) = self.sessions.last_mut()
            && last.end.is_none()
        {
            let changed = self.history.last().map_or(last.start, |c| c.at);
            last.end = Some(changed.max(last.start));
        }
        Ok(Task {
            id,
            kind: self.kind.unwrap_or_default(),
            links: self.links,
            repo_links: self.repo_links,
            title: self.title.unwrap_or_else(|| "untitled".into()),
            tags: self.tags,
            status,
            decision: self.decision,
            created: self.created.or_else(|| self.history.first().map(|c| c.at)).unwrap_or_else(now),
            description: self.description.join("\n").trim().to_string(),
            comments: self
                .comments
                .into_iter()
                .map(|(comment, body)| Comment { body: body.join("\n").trim().to_string(), ..comment })
                .collect(),
            history: self.history,
            sessions: self.sessions,
            path: None,
            unknown: Unknown {
                sections: self.unknown.sections.into_iter().map(|s| s.trim_end().to_string()).collect(),
                ..self.unknown
            },
        })
    }
}

/// `5, 9` or `#5 #9`: ids of linked entries in this brain; `shop-api#3`: an entry of another repository.
/// Without repeats; anything else is skipped.
pub fn parse_links(value: &str) -> (Vec<u32>, Vec<RepoLink>) {
    let (mut links, mut repo_links) = (Vec::new(), Vec::new());
    for link in link_words(value).filter_map(parse_link) {
        match link {
            Link::Here(id) if !links.contains(&id) => links.push(id),
            Link::Repo(link) if !repo_links.contains(&link) => repo_links.push(link),
            Link::Here(_) | Link::Repo(_) => {}
        }
    }
    (links, repo_links)
}

/// The words of a `- links:` value, split at commas and spaces.
fn link_words(value: &str) -> impl Iterator<Item = &str> {
    value.split([',', ' ']).map(str::trim).filter(|w| !w.is_empty())
}

/// One link word, read.
enum Link {
    /// `5` or `#5`: an entry of this brain.
    Here(u32),
    /// `shop-api#3`: an entry of another repository of the project.
    Repo(RepoLink),
}

/// The link a word names; `None` for a form this version doesn't know.
fn parse_link(word: &str) -> Option<Link> {
    match word.rsplit_once('#') {
        Some((repo, id)) if !repo.is_empty() => {
            Some(Link::Repo(RepoLink { repo: repo.to_string(), id: id.parse().ok()? }))
        }
        _ => word.trim_start_matches('#').parse().ok().map(Link::Here),
    }
}

/// ` (agent)` after what an agent did; nothing after what a person did, so their files read as before.
fn by_mark(by: Author) -> &'static str {
    match by {
        Author::Human => "",
        Author::Agent => " (agent)",
    }
}

/// What a comment's header says in parentheses after its time: `agent`, `question`, words this version
/// doesn't know, then `edited <time>`.
fn comment_attributes(comment: &Comment) -> Vec<String> {
    let author = (comment.author != Author::Human).then(|| comment.author.as_str().to_string());
    let question = comment.question.then(|| "question".to_string());
    let edited = comment.edited.map(|at| format!("edited {}", fmt_ts(at)));
    author.into_iter().chain(question).chain(comment.unknown.iter().cloned()).chain(edited).collect()
}

/// Parses `### <created>` or `### <created> (<attributes>)`, the attributes as [`comment_attributes`]
/// writes them, in any order. A `###` line whose time doesn't parse isn't a header, but part of a comment.
fn comment_header(line: &str) -> Option<Comment> {
    let header = line.strip_prefix("### ")?;
    let (created, attributes) = match header.split_once('(') {
        Some((created, attributes)) => (created, attributes.trim_end().trim_end_matches(')')),
        None => (header, ""),
    };
    let mut comment = Comment::new(parse_ts(created)?, String::new(), Author::Human);
    for word in attributes.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        if word == "question" {
            comment.question = true;
        } else if let Some(author) = Author::parse(word) {
            comment.author = author;
        } else if let Some(at) = word.strip_prefix("edited ") {
            comment.edited = parse_ts(at);
        } else {
            comment.unknown.push(word.to_string());
        }
    }
    Some(comment)
}

#[cfg(test)]
#[path = "../../tests/model/markdown.rs"]
mod tests;
