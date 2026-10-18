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
//! ## Activity
//!
//! - 2026-09-24T10:50:00+02:00 title (agent)
//!   - from: Fix login
//!   - to: Fix login 500
//! - 2026-09-24T10:51:00+02:00 tags: +prod -bug
//! - 2026-09-24T10:52:00+02:00 related (agent): +5 -shop-api#3
//! - 2026-09-24T10:53:00+02:00 description
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
//! `## Activity` and `## Sessions` are only written once there's something in them, so files stay short. Notes and
//! decisions have `- kind: note` / `- kind: decision` instead of `- status:`, and no History or Sessions, and a
//! decision that isn't in force has `- decision: proposed|rejected`. What a person wrote or did carries no mark; what
//! an agent did says `agent`. `- links:` is only written when there are links. `shop-api#3` is entry 3 of the project's
//! `shop-api` repository. Other `- key:` lines, link words and `##` sections are kept and written back (see
//! [`Unknown`]).

use super::text::{infallible, one_line, parse_tags};
use super::time::{fmt_dur, fmt_ts, now, parse_ts};
use super::{
    Activity, Author, Changed, Comment, DecisionState, Kind, RepoLink, Session, Status, StatusChange, Task, Unknown,
};
use chrono::{DateTime, Local};
use std::fmt::Write;

impl Task {
    /// The task as a markdown file (see the format at the top of this module).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        infallible(self.write_markdown(&mut out));
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

        self.write_activity(out)?;
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

    /// Writes the Activity section, if there's any: one item a change, what this version can't read last.
    fn write_activity(&self, out: &mut String) -> std::fmt::Result {
        if self.activity.is_empty() && self.unknown.activity.is_empty() {
            return Ok(());
        }
        writeln!(out, "## Activity\n")?;
        for activity in &self.activity {
            write!(out, "- {} ", fmt_ts(activity.at))?;
            let by = by_mark(activity.by);
            match &activity.change {
                // A title may say anything, ` -> ` and `(agent)` too: it has lines of its own.
                Changed::Title { from, to } => writeln!(out, "title{by}\n  - from: {from}\n  - to: {to}")?,
                Changed::Tags { added, removed } => writeln!(out, "tags{by}: {}", signed(added, removed))?,
                Changed::Related { added, removed } => writeln!(out, "related{by}: {}", signed(added, removed))?,
                Changed::Description => writeln!(out, "description{by}")?,
            }
        }
        for item in &self.unknown.activity {
            writeln!(out, "{item}")?;
        }
        writeln!(out)
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
        // A title with line breaks, which versions before 0.4.0 wrote as typed: the lines right under it, up to the
        // blank line kanbrain writes before the keys, continue it rather than read as a section that hides the id
        // and every key after it. Never a `- key:` line, which a hand edit may put first.
        let lines: Vec<&str> = text.lines().collect();
        // Only a title above the id: a `# ` line further down is another section's text.
        let id_at = lines.iter().position(|l| l.trim_start().starts_with("- id:")).unwrap_or_default();
        let continued = match lines[..id_at].iter().position(|l| l.starts_with("# ")) {
            Some(title) => {
                let is_key = |l: &&&str| l.trim_start().strip_prefix("- ").is_some_and(|rest| rest.contains(':'));
                let run = lines[title + 1..id_at].iter().take_while(|l| !l.trim().is_empty() && !is_key(l)).count();
                title + 1..title + 1 + run
            }
            None => 0..0,
        };
        // Read line by line; each `## ` heading switches the section the next lines belong to.
        for (at, line) in text.lines().enumerate() {
            let trimmed = line.trim_end();
            if continued.contains(&at) {
                if let Some(title) = &mut parsed.title
                    && !trimmed.trim().is_empty()
                {
                    title.push(' ');
                    title.push_str(&one_line(trimmed));
                }
                continue;
            }
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
                Section::Activity => parsed.activity_line(trimmed),
                Section::History => parsed.history_line(trimmed)?,
                Section::Sessions => parsed.session_line(trimmed)?,
                Section::Other => parsed.other_line(line),
            }
        }
        parsed.into_task()
    }
}

/// [`Task::from_markdown`]'s error for a file with no id: what tells markdown of other kinds, which a central brain
/// kept in a notes vault holds, from a broken entry.
pub(crate) const MISSING_ID: &str = "missing `- id:` line";

/// The `##` section a line belongs to.
#[derive(Clone, Copy)]
enum Section {
    /// Title and `- key: value` lines, before the first `##`.
    Meta,
    /// `## Description`: free text.
    Description,
    /// `## Comments`: `### <timestamp>` headings, each followed by the comment.
    Comments,
    /// `## Activity`: one `- <time> <what>` item per change, a title's with its lines.
    Activity,
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
            "Activity" => Section::Activity,
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
    /// The `## Activity` items, each with its lines, read at the end.
    activity: Vec<String>,
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
            // A lone `\r` older versions kept in it is a line break too, as markdown reads it.
            self.title = Some(one_line(title));
            return;
        }
        let Some((key, value)) = line.trim_start().strip_prefix("- ").and_then(|rest| rest.split_once(':')) else {
            // Text of a hand edit among the keys: kept, written back after them.
            if self.title.is_some() && !line.trim().is_empty() {
                self.unknown.meta.push(line.trim().to_string());
            }
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
            // A second line, from a hand edit or a merge, adds to the first rather than replace it.
            "links" => {
                let (links, repo_links) = parse_links(value);
                for link in links {
                    if !self.links.contains(&link) {
                        self.links.push(link);
                    }
                }
                for link in repo_links {
                    if !self.repo_links.contains(&link) {
                        self.repo_links.push(link);
                    }
                }
                self.unknown.links.extend(link_words(value).filter(|w| parse_link(w).is_none()).map(String::from));
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

    /// A `- ` line at the start starts an `## Activity` item; any other line is part of the last one, blank ones
    /// too, so text someone wrote there reads back as it was. Text before the first item is an item of its own, which
    /// this version can't read and keeps.
    fn activity_line(&mut self, line: &str) {
        match self.activity.last_mut() {
            Some(item) if !line.starts_with("- ") => {
                item.push('\n');
                item.push_str(line);
            }
            _ if line.trim().is_empty() => {}
            _ => self.activity.push(line.to_string()),
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
        let id = self.id.ok_or(MISSING_ID)?;
        self.history.sort_by_key(|c| c.at);
        let (mut activity, mut unread) = (Vec::new(), Vec::new());
        for item in self.activity {
            match activity_item(&item) {
                Some(read) => activity.push(read),
                None => unread.push(item.trim_end().to_string()),
            }
        }
        activity.sort_by_key(|a: &Activity| a.at);
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
            description: without_blank_ends(&self.description),
            comments: self
                .comments
                .into_iter()
                .map(|(comment, body)| Comment { body: without_blank_ends(&body), ..comment })
                .collect(),
            history: self.history,
            activity,
            sessions: self.sessions,
            path: None,
            unknown: Unknown {
                sections: self.unknown.sections.into_iter().map(|s| s.trim_end().to_string()).collect(),
                activity: unread,
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

/// The first word of `value` that isn't a link (an id, `#12`, or `REPO#ID`): typed on the command line, it's a
/// mistake to say, where a file's `- links:` line keeps such words as they are.
pub fn not_a_link(value: &str) -> Option<&str> {
    link_words(value).find(|word| parse_link(word).is_none())
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

/// Free text without the blank lines around it, its first line keeping its indentation (an indented code block).
fn without_blank_ends(lines: &[&str]) -> String {
    let blank = |line: &&&str| line.trim().is_empty();
    let start = lines.iter().position(|line| !blank(&line)).unwrap_or(lines.len());
    let end = lines.iter().rposition(|line| !blank(&line)).map_or(start, |at| at + 1);
    lines[start..end].join("\n").trim_end().to_string()
}

/// `+added -removed`, the words of an `## Activity` list's change.
fn signed(added: &[String], removed: &[String]) -> String {
    let added = added.iter().map(|word| format!("+{word}"));
    added.chain(removed.iter().map(|word| format!("-{word}"))).collect::<Vec<_>>().join(" ")
}

/// An `## Activity` item, as [`Task::write_activity`] writes it; `None` for one this version can't read, which is
/// kept as written rather than lose the file: what this section says is only a record.
fn activity_item(item: &str) -> Option<Activity> {
    let mut lines = item.lines().filter(|line| !line.trim().is_empty());
    let (at, rest) = lines.next()?.strip_prefix("- ")?.split_once(' ')?;
    let at = parse_ts(at)?;
    let (head, words) = match rest.split_once(':') {
        Some((head, words)) => (head, Some(words)),
        None => (rest, None),
    };
    let (what, by) = match head.trim_end().strip_suffix(')').and_then(|head| head.rsplit_once(" (")) {
        Some((what, by)) => (what, Author::parse(by)?),
        None => (head.trim_end(), Author::Human),
    };
    let mut value = |key: &str| lines.next()?.trim_start().strip_prefix("- ")?.strip_prefix(key).map(str::trim);
    let change = match (what, words) {
        ("title", None) => Changed::Title { from: value("from:")?.to_string(), to: value("to:")?.to_string() },
        ("tags", Some(words)) => {
            let (added, removed) = signed_words(words)?;
            Changed::Tags { added, removed }
        }
        ("related", Some(words)) => {
            let (added, removed) = signed_words(words)?;
            Changed::Related { added, removed }
        }
        ("description", None) => Changed::Description,
        _ => return None,
    };
    lines.next().is_none().then(|| Activity::new(at, by, change))
}

/// The `+word` and `-word` words of an `## Activity` list's change; `None` when one is neither, or there's none.
fn signed_words(words: &str) -> Option<(Vec<String>, Vec<String>)> {
    let (mut added, mut removed) = (Vec::new(), Vec::new());
    for word in words.split_whitespace() {
        let (list, word) = match word.split_at_checked(1)? {
            ("+", word) => (&mut added, word),
            ("-", word) => (&mut removed, word),
            _ => return None,
        };
        if word.is_empty() {
            return None;
        }
        list.push(word.to_string());
    }
    (!added.is_empty() || !removed.is_empty()).then_some((added, removed))
}

/// ` (agent)` after what an agent did; nothing after what a person did, so their files read as before.
fn by_mark(by: Author) -> &'static str {
    match by {
        Author::Human => "",
        Author::Agent => " (agent)",
    }
}

/// What a comment's header says in parentheses after its time: `agent`, `question` or `answer`, words this version
/// doesn't know, then `edited <time>`.
fn comment_attributes(comment: &Comment) -> Vec<String> {
    let author = (comment.author != Author::Human).then(|| comment.author.as_str().to_string());
    let question = comment.question.then(|| "question".to_string());
    let answer = comment.answer.then(|| "answer".to_string());
    let edited = comment.edited.map(|at| format!("edited {}", fmt_ts(at)));
    author.into_iter().chain(question).chain(answer).chain(comment.unknown.iter().cloned()).chain(edited).collect()
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
        } else if word == "answer" {
            comment.answer = true;
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
