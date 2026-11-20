//! `kb context`: what an agent should know when it starts working, as short markdown.
//! Titles and ids, plus the latest comment on tasks in progress so work can be picked up where it
//! stopped; `show` gives the rest. Every section is capped, so it stays cheap however big the brain gets.

use super::text::{head, state_note};
use crate::brain::Brain;
use crate::model::{Asked, Comment, DecisionState, Kind, Status, Task, fmt_age, fmt_clock, infallible, now};
use crate::projects::ProjectInfo;
use std::fmt::Write;

/// Entries listed per section; the rest are counted, to keep the context short.
const SECTION_LIMIT: usize = 20;
/// Recently finished tasks listed, so an agent doesn't redo what was just done.
const DONE_LIMIT: usize = 5;
/// Longest excerpt of a latest comment, in characters.
const EXCERPT_CHARS: usize = 120;
/// Notes and decisions in force listed under a task in progress; the rest are counted.
const RELATED_LIMIT: usize = 3;

impl Brain {
    /// Active tasks by status, the latest done, then decisions and notes, newest first, each as
    /// `- #id title #tag`.
    pub fn context_markdown(&self) -> String {
        let mut out = String::new();
        infallible(self.write_context(&mut out));
        out
    }

    /// Writes the context into `out`.
    fn write_context(&self, out: &mut String) -> std::fmt::Result {
        let scope = self.store.project().map_or_else(|| "central brain".to_string(), |p| format!("project {p}"));
        writeln!(out, "# kanbrain · {scope}\n")?;
        if let Some(project) = &self.project {
            write_project(out, project)?;
        }
        // First, since the user answered them: the work to pick up before anything new.
        let answered = self.newest(|t| matches!(t.asked(), Some(Asked::Answered(_))));
        self.section(out, "Answered: pick these up first", &answered, SECTION_LIMIT)?;
        for status in [Status::Doing, Status::Parked, Status::Todo] {
            let tasks =
                self.newest(|t| t.is_task() && t.status == status && !matches!(t.asked(), Some(Asked::Answered(_))));
            self.section(out, &format!("Tasks: {}", status.as_str()), &tasks, SECTION_LIMIT)?;
        }
        let mut done = self.newest(|t| t.is_task() && t.status == Status::Done);
        done.sort_by_key(|t| std::cmp::Reverse(t.status_since()));
        self.section(out, "Tasks: done lately", &done, DONE_LIMIT)?;
        self.section(out, "Decisions", &self.newest(|t| t.kind == Kind::Decision), SECTION_LIMIT)?;
        self.section(out, "Notes", &self.newest(|t| t.kind == Kind::Note), SECTION_LIMIT)
    }

    /// `  related: #12 title · #94 title · +2 more`: the notes and decisions in force linked to `task`, which is
    /// what it has to know, so an agent doesn't have to guess which of the newest twenty bear on it.
    fn write_related(&self, out: &mut String, task: &Task) -> std::fmt::Result {
        let related: Vec<&Task> = self
            .related(task.id)
            .into_iter()
            .filter(|t| t.kind == Kind::Note || (t.kind == Kind::Decision && t.decision == DecisionState::Accepted))
            .collect();
        if related.is_empty() {
            return Ok(());
        }
        out.push_str("  related: ");
        for (i, entry) in related.iter().take(RELATED_LIMIT).enumerate() {
            if i > 0 {
                out.push_str(" · ");
            }
            head(out, entry)?;
        }
        if let Some(more) = related.len().checked_sub(RELATED_LIMIT).filter(|&n| n > 0) {
            write!(out, " · +{more} more")?;
        }
        out.push('\n');
        Ok(())
    }

    /// Entries matching `keep`, newest first.
    fn newest(&self, keep: impl Fn(&Task) -> bool) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.iter().filter(|t| keep(t)).collect();
        tasks.sort_by_key(|t| std::cmp::Reverse(t.id));
        tasks
    }
}

impl Brain {
    /// A `## title` section listing up to `limit` entries; empty sections are left out. Tasks in doing
    /// or parked also show their latest comment, which usually says where the work stopped, or the
    /// question they wait on, or its answer; those in doing, the notes and decisions in force linked to them; an
    /// accepted decision, what it decided.
    fn section(&self, out: &mut String, title: &str, tasks: &[&Task], limit: usize) -> std::fmt::Result {
        if tasks.is_empty() {
            return Ok(());
        }
        writeln!(out, "## {title}\n")?;
        let now = now();
        for task in tasks.iter().take(limit) {
            out.push_str("- ");
            head(out, task)?;
            if let Some(running) = task.running_for(now) {
                write!(out, " · running {}", fmt_clock(running))?;
            }
            if let Some(note) = state_note(task) {
                write!(out, " · {note}")?;
            }
            out.push('\n');
            match task.asked() {
                Some(Asked::Waiting(question)) => excerpt(out, "question", question, now)?,
                Some(Asked::Answered(answer)) => excerpt(out, "answer", answer, now)?,
                None if matches!(task.status, Status::Doing | Status::Parked) && task.is_task() => {
                    if let Some(comment) = task.comments.last() {
                        excerpt(out, "last comment", comment, now)?;
                    }
                }
                None => {}
            }
            if task.is_task() && task.status == Status::Doing {
                self.write_related(out, task)?;
            }
            if let Some(decided) = decided(task) {
                writeln!(out, "  decided: {decided}")?;
            }
        }
        if let Some(more) = tasks.len().checked_sub(limit).filter(|&n| n > 0) {
            writeln!(out, "- … {more} more: `kb search` finds them")?;
        }
        out.push('\n');
        Ok(())
    }
}

/// What the latest acceptance of an accepted decision said, as an excerpt: the title often names the options, and
/// this says which. `resolve` writes it as a comment, `Accepted: REASON`; one with no reason says nothing more.
fn decided(task: &Task) -> Option<String> {
    if task.kind != Kind::Decision || task.decision != DecisionState::Accepted {
        return None;
    }
    task.comments.iter().rev().find_map(|c| c.body.trim_start().strip_prefix("Accepted: ")).map(cut)
}

/// `first line…`: a text's first line that isn't blank, cut to [`EXCERPT_CHARS`], with `…` when more follows.
fn cut(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let mut cut: String = first.chars().take(EXCERPT_CHARS).collect();
    if cut.len() < first.len() || text.trim() != first {
        cut.push('…');
    }
    cut
}

/// `  last comment (2h ago): first line…`, under a task, for `comment` and what it is.
fn excerpt(out: &mut String, what: &str, comment: &Comment, now: jiff::Timestamp) -> std::fmt::Result {
    writeln!(out, "  {what} ({} ago): {}", fmt_age(now - comment.created), cut(&comment.body))
}

/// What the application is about (or that it isn't described yet) and, for one of several repositories, this one's role
/// (or that it isn't described yet) and where the others are: an agent knows the project without exploring it, and can
/// reach the other services' tasks with `--project`.
fn write_project(out: &mut String, project: &ProjectInfo) -> std::fmt::Result {
    writeln!(out, "## Project {}\n", project.name)?;
    // Every session starts from the description, so an agent that finds none is told, in one line, to
    // write it before anything else: asked only to "keep it up to date", agents left it empty.
    if project.description.is_empty() {
        writeln!(
            out,
            concat!(
                "Not described yet. Before other work, describe the application from the README and manifests: \
                 `kb project describe -` (",
                crate::projects::description_scope!(),
                ").\n"
            )
        )?;
    } else {
        writeln!(out, "{}\n", project.description)?;
    }
    if project.siblings.is_empty() {
        if !project.notes.is_empty() {
            writeln!(out, "{}\n", project.notes)?;
        }
        return Ok(());
    }
    writeln!(out, "This is its `{}` repository.", project.repo)?;
    // Only a project of several repositories has roles to tell apart, and work that crosses between them:
    // said here rather than in the section every repository's agents read.
    if project.notes.is_empty() {
        writeln!(
            out,
            "\nIts role isn't described yet. Before other work, describe it from the README and manifests: \
             `kb project notes -` (what it owns, exposes and consumes)."
        )?;
    } else {
        writeln!(out, "\n{}", project.notes)?;
    }
    writeln!(
        out,
        "\nIts other repositories. Work one of them needs, or a problem found in it, is a task there, not a note \
         here: `kb --project {0}/NAME add TITLE`, then `kb link ID NAME#ID` relates it to the task here. \
         `kb --project {0}/NAME search WORDS` finds its decisions and notes.\n",
        project.name
    )?;
    for (name, dir) in &project.siblings {
        let root = dir.parent().unwrap_or(dir);
        writeln!(out, "- {name}: {}", root.display())?;
    }
    out.push('\n');
    Ok(())
}
