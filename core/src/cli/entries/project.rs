//! `project`: its arguments, showing the project, and changing it.

use super::super::manage;
use super::super::manage::Step;
use super::args::rest_as_text;
use super::{ProjectChange, ProjectPart};
use crate::brain::Brain;
use crate::projects::ProjectInfo;
use anyhow::{Context, Result, bail};

/// What follows `project`: nothing, to show it; else what to change.
pub(super) fn project_args(args: &mut Vec<String>) -> Result<Option<ProjectChange>> {
    if args.is_empty() {
        return Ok(None);
    }
    let command = args.remove(0);
    // A project's name may have spaces; a repository's is one word, which `project::check_repo_name` checks.
    let words = |args: &mut Vec<String>, what: &str| match std::mem::take(args).join(" ") {
        name if name.trim().is_empty() => bail!("project {command} needs {what}"),
        name => Ok(name),
    };
    Ok(Some(match command.as_str() {
        "describe" => {
            ProjectChange::Text(ProjectPart::Description, rest_as_text(args, "project describe needs the description")?)
        }
        "notes" => ProjectChange::Text(ProjectPart::Notes, rest_as_text(args, "project notes needs the notes")?),
        "rename" => {
            let literal = args.first().is_some_and(|a| a == "--");
            if literal {
                args.remove(0);
            }
            ProjectChange::Rename(words(args, "the new name")?, literal)
        }
        "repo" => ProjectChange::Repo(words(args, "the repository's new name")?),
        "move" => ProjectChange::Move(words(args, "the project to move to")?),
        "remove" => {
            let purge = args.first().is_some_and(|a| a == "--purge");
            if purge {
                args.remove(0);
            }
            ProjectChange::Remove(purge)
        }
        other => bail!("unknown project command `{other}`: describe, notes, rename, repo, move or remove"),
    }))
}

/// What `kb project` shows: the project's description and, in a project of several repositories (or
/// with notes written anyway), this repository's role. A single repository has no role to tell apart,
/// so an empty one isn't listed as missing.
pub(super) fn project_text(project: &ProjectInfo) -> String {
    /// The text, or that there's none yet.
    fn shown(text: &str) -> &str {
        if text.is_empty() { "(empty)" } else { text }
    }
    let card = format!("## Project {}\n\n{}\n", project.name, shown(&project.description));
    if project.siblings.is_empty() && project.notes.is_empty() {
        return card;
    }
    format!("{card}\n## Repository {}\n\n{}\n", project.repo, shown(&project.notes))
}

/// Shows the project's description and this repository's role, or changes the project.
pub(super) fn run_project(brain: &mut Brain, change: Option<ProjectChange>) -> Result<()> {
    let Some(project) = &brain.project else {
        bail!("this isn't a project: `kb init` makes it one, or use --project NAME");
    };
    let name = project.name.clone();
    let central = brain.central.clone().context("the central brain isn't known here")?;
    let dir = brain.store.dir().to_path_buf();
    let done = match change {
        None => {
            print!("{}", project_text(project));
            return Ok(());
        }
        Some(ProjectChange::Text(ProjectPart::Description, text)) => {
            brain.set_project_description(&text.read()?)?;
            vec![Step::other(format!("project {name}: description saved"))]
        }
        Some(ProjectChange::Text(ProjectPart::Notes, text)) => {
            brain.set_repo_notes(&text.read()?)?;
            vec![Step::other(format!("project {name}: repository role saved"))]
        }
        // `rename OLD NEW`, as other tools have it, would name the project "OLD NEW", names having spaces.
        Some(ProjectChange::Rename(new, false))
            if new.strip_prefix(&name).is_some_and(|rest| rest.starts_with(' ')) =>
        {
            let rest = new[name.len()..].trim();
            bail!(
                "`project rename` takes the new name only, for this project: `kb project rename {rest}`; to name it \
                 `{new}`, `kb project rename -- {new}`"
            )
        }
        Some(ProjectChange::Rename(new, _)) => manage::rename_project_steps(&central, &name, &new)?,
        Some(ProjectChange::Repo(new)) => manage::rename_repo_steps(&central, &dir, &new)?,
        Some(ProjectChange::Move(to)) => manage::move_repo_steps(&central, &dir, &to)?,
        Some(ProjectChange::Remove(purge)) => manage::remove_project_steps(&central, &name, purge)?,
    };
    for line in &done {
        println!("{line}");
    }
    Ok(())
}
