//! Commands that read or change entries by id, for agents and scripts: `add`, `show`, `search`,
//! `start`/`park`/`done`/`todo`, `comment`, `ask`, `wait`, `propose`, `accept`/`reject`, `describe`, `rename`,
//! `tag`, `link` and `context`. Each prints a short confirmation, or with `--json`, the entry (or entries) as JSON.

mod args;
mod project;
mod show;
mod tags;
mod wait;

use crate::api::text::line;
use crate::brain::{Brain, NewEntry};
use crate::model::{DecisionState, Kind, RepoLink, Status, one_line, parse_tags};
use crate::store;
use anyhow::{Context, Result, bail};
use args::{first_id, id_and_text, new_title, rest_as_text, search_words, typed_links};
use project::{project_args, run_project};
use show::{print_entry, print_search};
use std::io::Read;
use std::time::Duration;
use tags::{one_tag, rename_tag, retag, tag_changes};
use wait::{wait, wait_timeout};

/// After the context: how to act on what it lists, for agents that haven't seen the CLI before. Left
/// out in a project, whose agents have the commands in the section `init` wrote.
const CONTEXT_USAGE: &str = "\
Commands: `kb show ID` (everything about an entry), `search WORDS`, \
`add [--note|--decision] [--tag T] [--link ID] [--body TEXT|-] TITLE`, \
`comment ID TEXT|-`, `ask ID TEXT|-`, `wait ID`, `start|park|done|todo ID`, \
`link ID OTHER…` (`REPO#ID` in another repository). Add `--json` for JSON.";

/// Usage lines for these commands, appended to the main usage text.
pub const USAGE: &str = "\
  add [--note|--decision] [--tag TAG]… [--link ID]… [--body TEXT|-] [--start] TITLE…
            create a task (or note, decision) and print its id; `-` reads the body from stdin
  show ID   print an entry compactly: state, time, description, comments, related, history
  search WORDS…
            find entries by title, tags, description or comments (#tag: tags only)
  start|park|done|todo ID
            change a task's status (start runs its clock; a person runs one task at a time)
  comment ID TEXT…|-
            add a comment, e.g. progress notes
  ask ID TEXT…|-
            ask the user a question on a task and park it until a person answers with a comment
  wait ID [--timeout MINUTES]
            block until a person answers what task ID waits on (or decision ID is accepted or
            rejected), then print the answer; gives up after 8 minutes (0: never)
  propose ID [--tag TAG]… [--link ID]… [--body TEXT|-] TITLE…
            propose a decision for a task: linked to it, which asks about it and is parked
  accept ID [TEXT…|-]
  reject ID TEXT…|-
            accept or reject a decision, with why (needed to reject); answers a question that names it (#ID)
  describe ID TEXT…|-
            replace the description
  rename ID TITLE…
            replace the title (and its file's name)
  tag ID +TAG|-TAG…
            add tags to an entry (+TAG) or remove them (-TAG), in order
  tag OLD NEW
            rename a tag in every entry that has it
  link ID OTHER…
            link an entry to others: an id here, or REPO#ID in another repository of the project
  project [describe|notes TEXT…|-]
            show the project's description (shared by its repositories) and this repository's role,
            or replace one
  project rename NAME… | repo NAME | move PROJECT… | remove [--purge]
            rename the project, or this repository in it; move this repository to another project;
            take every repository of the project out (--purge: and delete their tasks)
  context   what's going on: active tasks, decisions and notes, for an agent to read first";

/// One of the entry commands, and whether to print JSON.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EntryArgs {
    /// What to do.
    pub command: EntryCommand,
    /// `--json`: print the result as JSON.
    pub json: bool,
}

/// What to do to which entry.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryCommand {
    /// `add`: create an entry.
    #[non_exhaustive]
    Add(AddArgs),
    /// `show ID`.
    #[non_exhaustive]
    Show(u32),
    /// `search WORDS…`.
    #[non_exhaustive]
    Search(String),
    /// `start|park|done|todo ID`.
    #[non_exhaustive]
    SetStatus(u32, Status),
    /// `comment ID TEXT…|-`.
    #[non_exhaustive]
    Comment(u32, Text),
    /// `ask ID TEXT…|-`: a question to the user, parking the task.
    #[non_exhaustive]
    Ask(u32, Text),
    /// `wait ID [--timeout MINUTES]`: block until a person answers, or the time is up (`None`: never).
    #[non_exhaustive]
    Wait(u32, Option<Duration>),
    /// `propose ID … TITLE…`: a proposed decision for the task.
    #[non_exhaustive]
    Propose(u32, AddArgs),
    /// `accept ID [TEXT…|-]` / `reject ID TEXT…|-`: a decision accepted or rejected, and why.
    #[non_exhaustive]
    Resolve(u32, DecisionState, Option<Text>),
    /// `describe ID TEXT…|-`.
    #[non_exhaustive]
    Describe(u32, Text),
    /// `rename ID TITLE…`.
    #[non_exhaustive]
    Rename(u32, String),
    /// `tag ID +TAG|-TAG…`: tags added and removed, in order.
    #[non_exhaustive]
    Tag(u32, Vec<TagChange>),
    /// `tag OLD NEW`: a tag renamed in every entry; true when OLD was written `#OLD`, which no id is.
    #[non_exhaustive]
    RenameTag(String, String, bool),
    /// `link ID OTHER…`: ids here, and `REPO#ID` in the project's other repositories.
    #[non_exhaustive]
    Link(u32, Vec<u32>, Vec<RepoLink>),
    /// `context`.
    #[non_exhaustive]
    Context,
    /// `project`: show the project's description and this repository's role, or change the project.
    #[non_exhaustive]
    Project(Option<ProjectChange>),
}

/// What `project` changes.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectChange {
    /// `describe|notes TEXT…|-`: replace one of its texts.
    #[non_exhaustive]
    Text(ProjectPart, Text),
    /// `rename NAME…`: the project's name; `true` when given after `--`, as meant whatever it starts with.
    #[non_exhaustive]
    Rename(String, bool),
    /// `repo NAME`: this repository's name in its project.
    #[non_exhaustive]
    Repo(String),
    /// `move PROJECT…`: this repository to another project, created if new.
    #[non_exhaustive]
    Move(String),
    /// `remove [--purge]`: take every repository of the project out of it; `true` also deletes their tasks.
    #[non_exhaustive]
    Remove(bool),
}

/// What `project` changes in one of its texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectPart {
    /// `describe`: the project's description, on the card all its repositories share.
    #[non_exhaustive]
    Description,
    /// `notes`: this repository's role among the project's, in its `.brain/project.md`.
    #[non_exhaustive]
    Notes,
}

/// A change `tag` makes to an entry's tags.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagChange {
    /// `+TAG`: added after its others, unless it has it.
    #[non_exhaustive]
    Add(String),
    /// `-TAG`: removed, if it has it.
    #[non_exhaustive]
    Remove(String),
}

/// Text given on the command line, or `-` to read it from stdin (for multi-line markdown).
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Text {
    /// The words after the id, joined by spaces.
    #[non_exhaustive]
    Given(String),
    /// Read stdin to the end.
    #[non_exhaustive]
    Stdin,
}

/// Options of `add`.
#[derive(Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct AddArgs {
    /// `--note` / `--decision`; a task otherwise.
    pub kind: Kind,
    /// The remaining words.
    pub title: String,
    /// `--tag`, repeatable.
    pub tags: Vec<String>,
    /// `--link`, repeatable: ids here.
    pub links: Vec<u32>,
    /// `--link REPO#ID`: entries of the project's other repositories.
    pub repo_links: Vec<RepoLink>,
    /// `--body`.
    pub body: Option<Text>,
    /// `--start`.
    pub start: bool,
}

impl EntryArgs {
    /// Reads the text given as `-` from stdin, if any.
    pub fn read_stdin(&mut self) -> Result<()> {
        let text = match &mut self.command {
            EntryCommand::Add(args) | EntryCommand::Propose(_, args) => args.body.as_mut(),
            EntryCommand::Comment(_, text)
            | EntryCommand::Ask(_, text)
            | EntryCommand::Describe(_, text)
            | EntryCommand::Project(Some(ProjectChange::Text(_, text))) => Some(text),
            EntryCommand::Resolve(_, _, reason) => reason.as_mut(),
            EntryCommand::Show(_)
            | EntryCommand::Search(_)
            | EntryCommand::SetStatus(..)
            | EntryCommand::Wait(..)
            | EntryCommand::Rename(..)
            | EntryCommand::Tag(..)
            | EntryCommand::RenameTag(..)
            | EntryCommand::Link(..)
            | EntryCommand::Context
            | EntryCommand::Project(_) => None,
        };
        text.map_or(Ok(()), Text::read_stdin)
    }

    /// Parses `name` and the arguments after it, if `name` is one of these commands.
    pub fn parse(name: &str, args: impl Iterator<Item = String>) -> Result<Option<Self>> {
        // `--json` may go anywhere before a `--` (see `cli::options`).
        let mut json = false;
        let mut rest = Vec::new();
        let mut text = false;
        for arg in args {
            text |= arg == "--";
            if arg == "--json" && !text {
                json = true;
            } else {
                rest.push(arg);
            }
        }
        let status = match name {
            "start" => Some(Status::Doing),
            "park" => Some(Status::Parked),
            "done" => Some(Status::Done),
            "todo" => Some(Status::Todo),
            _ => None,
        };
        let command = match (name, status) {
            (_, Some(status)) => EntryCommand::SetStatus(first_id(&mut rest)?, status),
            ("add", None) => EntryCommand::Add(AddArgs::parse(rest.drain(..), "add needs a title")?),
            ("show", None) => EntryCommand::Show(first_id(&mut rest)?),
            ("search", None) => {
                match search_words(std::mem::take(&mut rest)) {
                    // Every entry would match: `list` is for that.
                    words if words.trim().is_empty() => {
                        bail!("search needs words to look for, or a #tag; `kb list` lists them all")
                    }
                    words => EntryCommand::Search(words),
                }
            }
            ("comment", None) => {
                let (id, text) = id_and_text(&mut rest, "comment needs its text")?;
                EntryCommand::Comment(id, text)
            }
            ("ask", None) => {
                let (id, text) = id_and_text(&mut rest, "ask needs the question")?;
                EntryCommand::Ask(id, text)
            }
            ("wait", None) => {
                let id = first_id(&mut rest)?;
                EntryCommand::Wait(id, wait_timeout(&mut rest)?)
            }
            ("propose", None) => {
                let id = first_id(&mut rest)?;
                let decision = AddArgs::parse(rest.drain(..), "propose needs the decision's title")?;
                if decision.kind != Kind::Task || decision.start {
                    bail!("propose makes a decision: leave out --note, --decision and --start");
                }
                EntryCommand::Propose(id, decision)
            }
            ("accept", None) => {
                let id = first_id(&mut rest)?;
                let reason =
                    if rest.is_empty() { None } else { Some(rest_as_text(&mut rest, "accept needs its comment")?) };
                EntryCommand::Resolve(id, DecisionState::Accepted, reason)
            }
            ("reject", None) => {
                let id = first_id(&mut rest)?;
                let reason = rest_as_text(&mut rest, "reject needs why")?;
                EntryCommand::Resolve(id, DecisionState::Rejected, Some(reason))
            }
            ("describe", None) => {
                let (id, text) = id_and_text(&mut rest, "describe needs the description")?;
                EntryCommand::Describe(id, text)
            }
            ("rename", None) => {
                let id = first_id(&mut rest)?;
                EntryCommand::Rename(id, new_title(std::mem::take(&mut rest))?)
            }
            ("tag", None) => match std::mem::take(&mut rest).as_slice() {
                // A word after the first that isn't a change names the tag's new name.
                words @ [old, new] if !new.starts_with(['+', '-']) => {
                    let (old, new) = (one_tag(old)?, one_tag(new)?);
                    if old == new {
                        bail!("#{old} is already its name");
                    }
                    EntryCommand::RenameTag(old, new, words[0].starts_with('#'))
                }
                words => {
                    let mut words = words.to_vec();
                    let id = first_id(&mut words)?;
                    EntryCommand::Tag(id, tag_changes(words)?)
                }
            },
            ("link", None) => {
                let id = first_id(&mut rest)?;
                let (others, repo_links) = typed_links(&std::mem::take(&mut rest).join(" "))?;
                if others.is_empty() && repo_links.is_empty() {
                    bail!("link needs the ids to link to");
                }
                EntryCommand::Link(id, others, repo_links)
            }
            ("context", None) => EntryCommand::Context,
            ("project", None) => EntryCommand::Project(project_args(&mut rest)?),
            _ => return Ok(None),
        };
        if let Some(extra) = rest.first() {
            bail!("unexpected argument `{extra}`");
        }
        Ok(Some(Self { command, json }))
    }

    /// The entry the command reads and changes alone, if it does: a comment, a question, a description, a move to a
    /// status other than doing (which looks for room among the others), or waiting on it. Said as a line, never as JSON,
    /// which names the entries linked to it.
    pub(crate) fn alone(&self) -> Option<u32> {
        match self.command {
            _ if self.json => None,
            EntryCommand::Comment(id, _)
            | EntryCommand::Ask(id, _)
            | EntryCommand::Describe(id, _)
            | EntryCommand::Wait(id, _) => Some(id),
            EntryCommand::SetStatus(id, status) if status != Status::Doing => Some(id),
            _ => None,
        }
    }

    /// Runs the command against `brain` and prints the result.
    pub fn run(self, mut brain: Brain) -> Result<()> {
        let json = self.json;
        let show = matches!(self.command, EntryCommand::Show(_));
        let id = match self.command {
            EntryCommand::Add(args) => brain.add(args.into_new_entry()?)?,
            EntryCommand::Show(id) => {
                brain.entry(id)?;
                id
            }
            EntryCommand::Search(query) => return print_search(&brain, &query, json),
            EntryCommand::SetStatus(id, status) => {
                // On stderr, so stdout stays the entry's line for scripts.
                if !brain.set_status(id, status)? {
                    eprintln!("#{id} was already {}: nothing changed", status.as_str());
                }
                id
            }
            EntryCommand::Comment(id, text) => {
                brain.add_comment(id, &text.read()?)?;
                id
            }
            EntryCommand::Ask(id, text) => {
                brain.ask(id, &text.read()?)?;
                id
            }
            EntryCommand::Wait(..) if json => bail!("wait prints the answer as text only"),
            EntryCommand::Wait(id, timeout) => return wait(&brain, id, timeout),
            EntryCommand::Propose(task, args) => {
                let decision = brain.propose(task, args.into_new_entry()?)?;
                if !json {
                    println!("{} · {}", line(brain.entry(decision)?), line(brain.entry(task)?));
                    return Ok(());
                }
                decision
            }
            EntryCommand::Resolve(id, state, reason) => {
                let reason = reason.map(Text::read).transpose()?.unwrap_or_default();
                if !brain.resolve(id, state, &reason)? {
                    eprintln!("#{id} was already {}: nothing changed", state.as_str());
                }
                id
            }
            EntryCommand::Describe(id, text) => {
                brain.set_description(id, &text.read()?)?;
                id
            }
            EntryCommand::Rename(id, title) => {
                if !retitle(&mut brain, id, &title)? {
                    eprintln!("#{id} already had that title: nothing changed");
                }
                id
            }
            EntryCommand::Tag(id, changes) => {
                if !retag(&mut brain, id, &changes)? {
                    eprintln!("#{id}'s tags were already so: nothing changed");
                }
                id
            }
            EntryCommand::RenameTag(old, new, hashed) => return rename_tag(&mut brain, &old, &new, hashed, json),
            EntryCommand::Link(id, others, repo_links) => {
                brain.link(id, &others, &repo_links)?;
                id
            }
            EntryCommand::Context if json => bail!("context is markdown only; use search or show with --json"),
            EntryCommand::Project(_) if json => bail!("project is markdown only"),
            EntryCommand::Project(change) => return run_project(&mut brain, change),
            EntryCommand::Context => {
                // A release that adds a hook reaches the project with its next session. Never in the context's way:
                // a session starts all the same without it.
                let dir = brain.store.dir();
                if let Some(root) = dir.parent().filter(|_| dir.file_name() == Some(store::PROJECT_DIR.as_ref())) {
                    let _ = super::claude::update_hooks(root);
                }
                let usage = if brain.project.is_some() { "" } else { CONTEXT_USAGE };
                println!("{}{usage}", brain.context_markdown());
                return Ok(());
            }
        };
        print_entry(&brain, id, json, show)
    }
}

impl AddArgs {
    /// Parses the options and title words after `add` or `propose ID`; `no_title` is the error without a title.
    fn parse(mut args: impl Iterator<Item = String>, no_title: &str) -> Result<Self> {
        let mut add = Self::default();
        let mut title = Vec::new();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--note" => add.kind = Kind::Note,
                "--decision" => add.kind = Kind::Decision,
                "--start" => add.start = true,
                "--tag" => add.tags.extend(parse_tags(&args.next().context("--tag needs a value")?)),
                "--link" => {
                    let (links, repo_links) = typed_links(&args.next().context("--link needs an id")?)?;
                    add.links.extend(links);
                    add.repo_links.extend(repo_links);
                }
                "--body" => {
                    let body = args.next().context("--body needs text, or - to read stdin")?;
                    add.body = Some(if body == "-" { Text::Stdin } else { Text::Given(body) });
                }
                // Every word after it is the title's, however it starts.
                "--" => title.extend(args.by_ref()),
                other if other.starts_with("--") => bail!("unknown add option `{other}`"),
                _ => title.push(arg),
            }
        }
        add.title = title.join(" ");
        if add.title.trim().is_empty() {
            bail!("{no_title}");
        }
        if add.start && add.kind != Kind::Task {
            bail!("only tasks can be started");
        }
        Ok(add)
    }

    /// The entry to create, reading the body from stdin if asked to.
    fn into_new_entry(self) -> Result<NewEntry> {
        Ok(NewEntry {
            kind: self.kind,
            title: self.title,
            tags: self.tags,
            links: self.links,
            repo_links: self.repo_links,
            description: self.body.map(Text::read).transpose()?,
            start: self.start,
        })
    }
}

impl Text {
    /// Reads stdin to the end for `-`, so the text is given from then on.
    fn read_stdin(&mut self) -> Result<()> {
        if *self == Text::Stdin {
            *self = Text::Given(stdin_text()?);
        }
        Ok(())
    }

    /// The text, reading stdin to the end for `-`.
    fn read(self) -> Result<String> {
        match self {
            Text::Given(text) => Ok(text),
            Text::Stdin => stdin_text(),
        }
    }
}

/// Everything on stdin, for a `-`. Nothing there is refused: it's a heredoc left out (an agent's shell tool gives an
/// empty stdin) far more often than a wish to clear the text, and `describe` would save it over the description.
fn stdin_text() -> Result<String> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).context("reading stdin")?;
    if text.trim().is_empty() {
        bail!("nothing came on stdin for `-`: give the text there, e.g. with a heredoc (<<'EOF' … EOF)");
    }
    Ok(text)
}

/// Saves `title`, on one line, as entry `id`'s, as the kanban app's form does (its file is renamed after it); returns
/// whether it changed, as nothing is written when it didn't.
fn retitle(brain: &mut Brain, id: u32, title: &str) -> Result<bool> {
    let entry = brain.entry(id)?;
    let title = one_line(title);
    if title == entry.title {
        return Ok(false);
    }
    let tags = entry.tags.clone();
    brain.update(id, &title, tags)?;
    Ok(true)
}

#[cfg(test)]
#[path = "../../../tests/cli/entries.rs"]
mod tests;
