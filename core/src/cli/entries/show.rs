//! Printing an entry (`show`, and each command's confirmation) and search results.

use crate::api::json::{Entry, Summary};
use crate::api::text::line;
use crate::brain::Brain;
use crate::model::{RepoLink, Task};
use anyhow::Result;
use serde::Serialize;

/// An entry's JSON as `show --json` prints it: [`Entry`], and the entries of the project's other repositories
/// linking to it, as `web#1`, which `show` lists under Related along with its own `repo_links`.
#[derive(Serialize)]
struct Shown<'a> {
    /// Its fields first, at the top level.
    #[serde(flatten)]
    entry: Entry<'a>,
    /// Left out when empty, as `Entry`'s links are.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    repo_linked_from: Vec<String>,
}

impl<'a> Shown<'a> {
    /// Reads the project's other repositories for the links to `task`.
    fn new(brain: &Brain, task: &'a Task) -> Self {
        let here = brain.project.as_ref().map(|p| RepoLink { repo: p.repo.clone(), id: task.id });
        let repo_linked_from = brain
            .repo_related(task.id)
            .into_iter()
            .filter(|(_, other)| here.as_ref().is_some_and(|here| other.repo_links.contains(here)))
            .map(|(repo, other)| RepoLink { repo, id: other.id }.to_string())
            .collect();
        Self { entry: Entry::new(brain, task), repo_linked_from }
    }
}

/// Prints the entry: as one line of JSON, as compact text (`show`), or as a one-line confirmation.
/// JSON is on one line since its readers are programs, and agents pay for the indentation.
pub(super) fn print_entry(brain: &Brain, id: u32, json: bool, full: bool) -> Result<()> {
    let task = brain.entry(id)?;
    if json {
        println!("{}", serde_json::to_string(&Shown::new(brain, task))?);
    } else if full {
        print!("{}", brain.entry_text(id)?);
    } else {
        println!("{}", line(task));
    }
    Ok(())
}

/// Prints search results, one `- #id title (state) #tag` line each, or every match as JSON.
pub(super) fn print_search(brain: &Brain, query: &str, json: bool) -> Result<()> {
    if json {
        let summaries: Vec<Summary> = brain.search(query).into_iter().map(Summary::new).collect();
        println!("{}", serde_json::to_string(&summaries)?);
    } else {
        print!("{}", brain.search_text(query));
    }
    Ok(())
}
