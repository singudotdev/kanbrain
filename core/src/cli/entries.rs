//! Commands that read or change entries by id, for agents and scripts: `add`, `show`, `search`,
//! `start`/`park`/`done`/`todo`, `comment`, `ask`, `wait`, `propose`, `accept`/`reject`, `describe`, `link`
//! and `context`. Each prints a short confirmation, or with `--json`, the entry (or entries) as JSON.

use crate::api::ApiError;
use crate::api::json::{Entry, Summary};
use crate::api::text::line;
use crate::brain::{Brain, NewEntry};
use crate::model::{Asked, DecisionState, Kind, RepoLink, Status, Task, parse_links, parse_tags};
use crate::projects::ProjectInfo;
use anyhow::{Context, Result, bail};
use std::io::Read;
use std::time::Duration;

/// After the context: how to act on what it lists, for agents that haven't seen the CLI before. Left
/// out in a project, whose agents have the commands in the section `init` wrote.
const CONTEXT_USAGE: &str = "\
Commands: `kb show ID` (everything about an entry), `search WORDS`, \
`add [--note|--decision] [--tag T] [--link ID] [--body TEXT|-] TITLE`, \
`comment ID TEXT|-`, `ask ID TEXT|-`, `wait ID`, `start|park|done|todo ID`, `link ID OTHER…` (`REPO#ID` in another repository). Add `--json` for JSON.";

/// Usage lines for these commands, appended to the main usage text.
pub const USAGE: &str = "\
  add [--note|--decision] [--tag TAG]… [--link ID]… [--body TEXT|-] [--start] TITLE…
            create a task (or note, decision) and print its id; `-` reads the body from stdin
  show ID   print an entry compactly: state, time, description, comments, related, history
  search WORDS…
            find entries by title, tags, description or comments (#tag: tags only)
  start|park|done|todo ID
            change a task's status (start runs its clock and parks the running task)
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
            accept or reject a decision, with why (needed to reject); answers the tasks waiting on it
  describe ID TEXT…|-
            replace the description
  link ID OTHER…
            link an entry to others: an id here, or REPO#ID in another repository of the project
  project [describe|notes TEXT…|-]
            show the project's description (shared by its repositories) and this repository's role,
            or replace one
  context   what's going on: active tasks, decisions and notes, for an agent to read first
            (all but project take --json to print JSON instead)";

/// One of the entry commands, and whether to print JSON.
#[derive(Debug, PartialEq, Eq)]
pub struct EntryArgs {
    /// What to do.
    pub command: EntryCommand,
    /// `--json`: print the result as JSON.
    pub json: bool,
}

/// What to do to which entry.
#[derive(Debug, PartialEq, Eq)]
pub enum EntryCommand {
    /// `add`: create an entry.
    Add(AddArgs),
    /// `show ID`.
    Show(u32),
    /// `search WORDS…`.
    Search(String),
    /// `start|park|done|todo ID`.
    SetStatus(u32, Status),
    /// `comment ID TEXT…|-`.
    Comment(u32, Text),
    /// `ask ID TEXT…|-`: a question to the user, parking the task.
    Ask(u32, Text),
    /// `wait ID [--timeout MINUTES]`: block until a person answers, or the time is up (`None`: never).
    Wait(u32, Option<Duration>),
    /// `propose ID … TITLE…`: a proposed decision for the task.
    Propose(u32, AddArgs),
    /// `accept ID [TEXT…|-]` / `reject ID TEXT…|-`: a decision accepted or rejected, and why.
    Resolve(u32, DecisionState, Option<Text>),
    /// `describe ID TEXT…|-`.
    Describe(u32, Text),
    /// `link ID OTHER…`: ids here, and `REPO#ID` in the project's other repositories.
    Link(u32, Vec<u32>, Vec<RepoLink>),
    /// `context`.
    Context,
    /// `project`: show the project's description and this repository's role; with a part and text,
    /// replace that part.
    Project(Option<(ProjectPart, Text)>),
}

/// What `project` changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPart {
    /// `describe`: the project's description, on the card all its repositories share.
    Description,
    /// `notes`: this repository's role among the project's, in its `.brain/project.md`.
    Notes,
}

/// Text given on the command line, or `-` to read it from stdin (for multi-line markdown).
#[derive(Debug, PartialEq, Eq)]
pub enum Text {
    /// The words after the id, joined by spaces.
    Given(String),
    /// Read stdin to the end.
    Stdin,
}

/// Options of `add`.
#[derive(Debug, PartialEq, Eq, Default)]
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
    /// Parses `name` and the arguments after it, if `name` is one of these commands.
    pub fn parse(name: &str, args: impl Iterator<Item = String>) -> Result<Option<Self>> {
        // `--json` may go anywhere.
        let mut json = false;
        let mut rest = Vec::new();
        for arg in args {
            if arg == "--json" {
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
            ("add", None) => EntryCommand::Add(AddArgs::parse(rest.drain(..))?),
            ("show", None) => EntryCommand::Show(first_id(&mut rest)?),
            ("search", None) => EntryCommand::Search(std::mem::take(&mut rest).join(" ")),
            ("comment", None) => {
                let (id, text) = id_and_text(&mut rest)?;
                EntryCommand::Comment(id, text)
            }
            ("ask", None) => {
                let (id, text) = id_and_text(&mut rest)?;
                EntryCommand::Ask(id, text)
            }
            ("wait", None) => {
                let id = first_id(&mut rest)?;
                EntryCommand::Wait(id, wait_timeout(&mut rest)?)
            }
            ("propose", None) => {
                let id = first_id(&mut rest)?;
                let decision = AddArgs::parse(rest.drain(..))?;
                if decision.kind != Kind::Task || decision.start {
                    bail!("propose makes a decision: leave out --note, --decision and --start");
                }
                EntryCommand::Propose(id, decision)
            }
            ("accept", None) => {
                let id = first_id(&mut rest)?;
                let reason = if rest.is_empty() { None } else { Some(rest_as_text(&mut rest)?) };
                EntryCommand::Resolve(id, DecisionState::Accepted, reason)
            }
            ("reject", None) => {
                let id = first_id(&mut rest)?;
                let reason = rest_as_text(&mut rest).context("reject needs why: TEXT, or - to read it from stdin")?;
                EntryCommand::Resolve(id, DecisionState::Rejected, Some(reason))
            }
            ("describe", None) => {
                let (id, text) = id_and_text(&mut rest)?;
                EntryCommand::Describe(id, text)
            }
            ("link", None) => {
                let id = first_id(&mut rest)?;
                let (others, repo_links) = parse_links(&std::mem::take(&mut rest).join(" "));
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
                let parked = brain.change_status(id, status)?;
                if let (false, Some(parked)) = (json, parked) {
                    println!("{} · parked #{parked}", line(brain.entry(id)?));
                    return Ok(());
                }
                id
            }
            EntryCommand::Comment(id, text) => {
                brain.comment(id, &text.read()?)?;
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
                brain.resolve(id, state, &reason)?;
                id
            }
            EntryCommand::Describe(id, text) => {
                brain.describe(id, &text.read()?)?;
                id
            }
            EntryCommand::Link(id, others, repo_links) => {
                brain.link(id, &others, &repo_links)?;
                id
            }
            EntryCommand::Context if json => bail!("context is markdown only; use search or show with --json"),
            EntryCommand::Project(_) if json => bail!("project is markdown only"),
            EntryCommand::Project(change) => return run_project(&mut brain, change),
            EntryCommand::Context => {
                let usage = if brain.project.is_some() { "" } else { CONTEXT_USAGE };
                println!("{}{usage}", brain.context_markdown());
                return Ok(());
            }
        };
        print_entry(&brain, id, json, show)
    }
}

impl AddArgs {
    /// Parses the options and title words after `add`.
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut add = Self::default();
        let mut title = Vec::new();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--note" => add.kind = Kind::Note,
                "--decision" => add.kind = Kind::Decision,
                "--start" => add.start = true,
                "--tag" => add.tags.extend(parse_tags(&args.next().context("--tag needs a value")?)),
                "--link" => {
                    let (links, repo_links) = parse_links(&args.next().context("--link needs an id")?);
                    add.links.extend(links);
                    add.repo_links.extend(repo_links);
                }
                "--body" => {
                    let body = args.next().context("--body needs text, or - to read stdin")?;
                    add.body = Some(if body == "-" { Text::Stdin } else { Text::Given(body) });
                }
                other if other.starts_with("--") => bail!("unknown add option `{other}`"),
                _ => title.push(arg),
            }
        }
        add.title = title.join(" ");
        if add.title.trim().is_empty() {
            bail!("add needs a title");
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
    /// The text, reading stdin to the end for `-`.
    fn read(self) -> Result<String> {
        match self {
            Text::Given(text) => Ok(text),
            Text::Stdin => {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text).context("reading stdin")?;
                Ok(text)
            }
        }
    }
}

/// The first argument as an entry id (`12` or `#12`).
fn first_id(args: &mut Vec<String>) -> Result<u32> {
    if args.is_empty() {
        bail!("an entry id is needed");
    }
    let arg = args.remove(0);
    arg.trim_start_matches('#').parse().with_context(|| format!("`{arg}` isn't an entry id"))
}

/// An id, then the text: the remaining words, or `-` alone for stdin.
fn id_and_text(args: &mut Vec<String>) -> Result<(u32, Text)> {
    let id = first_id(args)?;
    Ok((id, rest_as_text(args)?))
}

/// The remaining words as one text, or `-` alone for stdin.
fn rest_as_text(args: &mut Vec<String>) -> Result<Text> {
    let words = std::mem::take(args);
    match words.as_slice() {
        [] => bail!("text is needed, or - to read it from stdin"),
        [dash] if dash == "-" => Ok(Text::Stdin),
        _ => Ok(Text::Given(words.join(" "))),
    }
}

/// `describe TEXT…|-` or `notes TEXT…|-` after `project`; nothing, to show them.
fn project_args(args: &mut Vec<String>) -> Result<Option<(ProjectPart, Text)>> {
    if args.is_empty() {
        return Ok(None);
    }
    let part = match args.remove(0).as_str() {
        "describe" => ProjectPart::Description,
        "notes" => ProjectPart::Notes,
        other => bail!("unknown project command `{other}`: describe or notes"),
    };
    Ok(Some((part, rest_as_text(args)?)))
}

/// What `kb project` shows: the project's description and, in a project of several repositories (or
/// with notes written anyway), this repository's role. A single repository has no role to tell apart,
/// so an empty one isn't listed as missing.
fn project_text(project: &ProjectInfo) -> String {
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

/// Shows the project's description and this repository's role, or replaces one of them.
fn run_project(brain: &mut Brain, change: Option<(ProjectPart, Text)>) -> Result<()> {
    let Some(project) = &brain.project else {
        bail!("this isn't a project: `kb init` makes it one, or use --project NAME");
    };
    let name = project.name.clone();
    match change {
        None => print!("{}", project_text(project)),
        Some((ProjectPart::Description, text)) => {
            brain.set_project_description(&text.read()?)?;
            println!("project {name}: description saved");
        }
        Some((ProjectPart::Notes, text)) => {
            brain.set_repo_notes(&text.read()?)?;
            println!("project {name}: repository role saved");
        }
    }
    Ok(())
}

/// How often `wait` reads the brain again. Seconds, as a person takes far longer to answer; the files
/// are re-read rather than watched, so a waiting agent costs no dependency and no model call.
const WAIT_POLL: Duration = Duration::from_secs(1);

/// How long `wait` waits without `--timeout`. Claude Code run headless (`claude -p`) doesn't exit while a
/// background command runs, so a wait nobody answers would hold it forever; past this, an answer is no
/// longer a conversation, and the next session's `context` picks it up. Under the 10 minutes Claude Code
/// gives a tool call at most, so the agent gets this message, which says to stop, rather than a killed
/// command it takes as a hiccup and runs again.
const WAIT_DEFAULT: Duration = Duration::from_secs(8 * 60);

/// `--timeout MINUTES` after `wait`'s id: `None` for 0 (no limit). Without it, `KANBRAIN_WAIT_TIMEOUT`
/// (same minutes), so a run with nobody to answer, such as `e2e/agent.sh`, can make every wait short
/// without changing what agents are told; else the default.
fn wait_timeout(args: &mut Vec<String>) -> Result<Option<Duration>> {
    if args.is_empty() {
        let Some(value) = std::env::var_os("KANBRAIN_WAIT_TIMEOUT") else {
            return Ok(Some(WAIT_DEFAULT));
        };
        return minutes(&value.to_string_lossy()).context("KANBRAIN_WAIT_TIMEOUT must be a number of minutes");
    }
    if args.remove(0) != "--timeout" || args.is_empty() {
        bail!("wait takes an id, then optionally --timeout MINUTES");
    }
    minutes(&args.remove(0))
}

/// A number of minutes as a time limit, `None` for 0 (no limit).
fn minutes(arg: &str) -> Result<Option<Duration>> {
    let minutes: u64 = arg.parse().with_context(|| format!("`{arg}` isn't a number of minutes"))?;
    Ok((minutes > 0).then(|| Duration::from_secs(minutes * 60)))
}

/// Blocks until entry `id` no longer waits on a person, then prints what they said; an error once
/// `timeout` passes without it. An agent runs it in the background after `ask` or `propose`, where its
/// tool wakes it when it exits: one wake-up with the answer, instead of checking by turns.
fn wait(brain: &Brain, id: u32, timeout: Option<Duration>) -> Result<()> {
    let started = std::time::Instant::now();
    loop {
        let (tasks, _) = brain.store.load()?;
        let task = tasks.iter().find(|t| t.id == id).ok_or(ApiError::NotFound(id))?;
        if let Some(said) = settled(task) {
            println!("{said}");
            return Ok(());
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            bail!("no answer on #{id} yet: stop here and don't wait again; once answered, it comes first in `context`");
        }
        std::thread::sleep(WAIT_POLL);
    }
}

/// What `wait` prints once `task` no longer waits on a person, or `None` while it does: a decision that
/// left proposed, with its verdict's comment; a task's answer; or that the task waits on nothing (never
/// asked, or picked up since).
fn settled(task: &Task) -> Option<String> {
    let head = format!("#{} {}", task.id, task.title);
    if task.kind == Kind::Decision {
        if task.decision == DecisionState::Proposed {
            return None;
        }
        let verdict = task.decision.as_str();
        return Some(match task.comments.last() {
            Some(why) => format!("{head}: {verdict}\n\n{}", why.body),
            None => format!("{head}: {verdict}"),
        });
    }
    match task.asked() {
        Some(Asked::Waiting(_)) => None,
        Some(Asked::Answered(answer)) => Some(format!("{head}: answered\n\n{}", answer.body)),
        None => Some(format!("{head}: not waiting for an answer ({})", task.status.as_str())),
    }
}

/// Prints the entry: as one line of JSON, as compact text (`show`), or as a one-line confirmation.
/// JSON is on one line since its readers are programs, and agents pay for the indentation.
fn print_entry(brain: &Brain, id: u32, json: bool, full: bool) -> Result<()> {
    let task = brain.entry(id)?;
    if json {
        println!("{}", serde_json::to_string(&Entry::new(brain, task))?);
    } else if full {
        print!("{}", brain.entry_text(id)?);
    } else {
        println!("{}", line(task));
    }
    Ok(())
}

/// Prints search results, one `- #id title (state) #tag` line each, or every match as JSON.
fn print_search(brain: &Brain, query: &str, json: bool) -> Result<()> {
    if json {
        let summaries: Vec<Summary> = brain.search(query).into_iter().map(Summary::new).collect();
        println!("{}", serde_json::to_string(&summaries)?);
    } else {
        print!("{}", brain.search_text(query));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/cli/entries.rs"]
mod tests;
