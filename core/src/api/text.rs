//! Entries as compact text for agents: what `show`, `search` and the write commands print. Short on purpose, since
//! agents pay for every token they read: dates without seconds or offsets, empty sections left out, sessions as one
//! total, and only the latest history. The full record stays in the entry's file, whose path `show` gives.

use super::ApiError;
use crate::brain::Brain;
use crate::model::{Author, Comment, DecisionState, Task, fmt_age, fmt_dur, now};
use chrono::{DateTime, Local};
use std::fmt::Write;

/// Status changes `show` lists, latest last; the rest are counted.
const HISTORY_LIMIT: usize = 5;
/// Results `search` lists, best first; the rest are counted.
pub const SEARCH_LIMIT: usize = 20;

/// One line naming an entry: `#4 Fix login (doing) #auth`, with the kind instead of a status for
/// notes and decisions. A question it waits on, and a decision not in force, are said too:
/// `(parked, waiting for an answer)`, `(decision, proposed)`.
pub fn line(task: &Task) -> String {
    let mut state = if task.is_task() { task.status.as_str() } else { task.kind.as_str() }.to_string();
    if let Some(also) = state_note(task) {
        write!(state, ", {also}").expect("writing to a String cannot fail");
    }
    let mut line = format!("#{} {} ({state})", task.id, task.title);
    for tag in &task.tags {
        write!(line, " #{tag}").expect("writing to a String cannot fail");
    }
    line
}

/// `#id title #tag…`, how `context` and `list` name an entry.
pub fn head(out: &mut String, task: &Task) -> std::fmt::Result {
    write!(out, "#{} {}", task.id, task.title)?;
    task.tags.iter().try_for_each(|tag| write!(out, " #{tag}"))
}

/// What an entry's status alone doesn't say: that a parked task waits for an answer or got one, or that a
/// decision is proposed or rejected.
pub fn state_note(task: &Task) -> Option<&'static str> {
    if let Some(asked) = task.asked() {
        return Some(asked.label());
    }
    match task.decision {
        DecisionState::Accepted => None,
        state => Some(state.as_str()),
    }
}

/// A comment's heading in `show`: when, and who and what when it's an agent's or a question.
fn comment_heading(comment: &Comment) -> String {
    let mut heading = when(comment.created);
    if comment.author == Author::Agent {
        heading.push_str(" · agent");
    }
    if comment.question {
        heading.push_str(" · question");
    }
    if let Some(edited) = comment.edited {
        write!(heading, " (edited {})", when(edited)).expect("writing to a String cannot fail");
    }
    heading
}

/// A time as `2026-10-08 10:40`: minutes are precise enough to act on, and the files keep the rest.
fn when(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d %H:%M").to_string()
}

impl Brain {
    /// The entry for an agent to read: state, description, comments, related entries, the latest
    /// status changes and the time tracked.
    pub fn entry_text(&self, id: u32) -> Result<String, ApiError> {
        let task = self.entry(id)?;
        let mut out = String::new();
        self.write_entry(&mut out, task).expect("writing to a String cannot fail");
        Ok(out)
    }

    /// Search results as lines, best first, at most `SEARCH_LIMIT`, then how many more matched.
    pub fn search_text(&self, query: &str) -> String {
        let found = self.search(query);
        if found.is_empty() {
            return "nothing found\n".to_string();
        }
        let mut out = String::new();
        for task in found.iter().take(SEARCH_LIMIT) {
            writeln!(out, "- {}", line(task)).expect("writing to a String cannot fail");
        }
        if let Some(more) = found.len().checked_sub(SEARCH_LIMIT).filter(|&n| n > 0) {
            writeln!(out, "- … {more} more: add words to narrow it down").expect("writing to a String cannot fail");
        }
        out
    }

    /// Writes `task` for `entry_text`.
    fn write_entry(&self, out: &mut String, task: &Task) -> std::fmt::Result {
        let now = now();
        writeln!(out, "# {}", line(task))?;
        let mut facts = vec![format!("created {}", when(task.created))];
        if task.is_task() && !task.history.is_empty() {
            facts.push(format!("{} for {}", task.status.as_str(), fmt_age(now - task.status_since())));
        }
        if !task.sessions.is_empty() {
            let running = task.running_for(now).map(|r| format!(", running for {}", fmt_dur(r))).unwrap_or_default();
            facts.push(format!("tracked {} in {} session(s){running}", fmt_dur(task.total(now)), task.sessions.len()));
        }
        writeln!(out, "{}", facts.join(" · "))?;
        if let Some(path) = &task.path {
            writeln!(out, "file: {}", path.display())?;
        }
        if !task.description.trim().is_empty() {
            writeln!(out, "\n## Description\n\n{}", task.description.trim())?;
        }
        if !task.comments.is_empty() {
            writeln!(out, "\n## Comments")?;
            for comment in &task.comments {
                writeln!(out, "\n### {}\n\n{}", comment_heading(comment), comment.body.trim())?;
            }
        }
        let related = self.related(task.id);
        let elsewhere = self.repo_related(task.id);
        if !related.is_empty() || !elsewhere.is_empty() {
            writeln!(out, "\n## Related\n")?;
            for other in related {
                writeln!(out, "- {}", line(other))?;
            }
            for (repo, other) in &elsewhere {
                writeln!(out, "- {repo}{}", line(other))?;
            }
        }
        if !task.history.is_empty() {
            let skipped = task.history.len().saturating_sub(HISTORY_LIMIT);
            let heading = if skipped > 0 {
                format!("History (latest {HISTORY_LIMIT} of {})", task.history.len())
            } else {
                "History".to_string()
            };
            writeln!(out, "\n## {heading}\n")?;
            for change in &task.history[skipped..] {
                let by = if change.by == Author::Agent { " (agent)" } else { "" };
                writeln!(out, "- {} {} -> {}{by}", when(change.at), change.from.as_str(), change.to.as_str())?;
            }
        }
        Ok(())
    }
}
