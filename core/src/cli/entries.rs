//! Commands that read or change entries by id, for agents and scripts: `add`, `show`, `search`,
//! `start`/`park`/`done`/`todo`, `comment`, `ask`, `wait`, `propose`, `accept`/`reject`, `describe`, `link`
//! and `context`. Each prints a short confirmation, or with `--json`, the entry (or entries) as JSON.

use super::manage;
use crate::api::ApiError;
use crate::api::json::{Entry, Summary};
use crate::api::text::line;
use crate::brain::{Brain, NewEntry};
use crate::model::{Asked, DecisionState, Kind, RepoLink, Status, Task, not_a_link, parse_links, parse_tags};
use crate::projects::ProjectInfo;
use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use std::io::Read;
use std::time::Duration;

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
    /// `project`: show the project's description and this repository's role, or change the project.
    Project(Option<ProjectChange>),
}

/// What `project` changes.
#[derive(Debug, PartialEq, Eq)]
pub enum ProjectChange {
    /// `describe|notes TEXT…|-`: replace one of its texts.
    Text(ProjectPart, Text),
    /// `rename NAME…`: the project's name; `true` when given after `--`, as meant whatever it starts with.
    Rename(String, bool),
    /// `repo NAME`: this repository's name in its project.
    Repo(String),
    /// `move PROJECT…`: this repository to another project, created if new.
    Move(String),
    /// `remove [--purge]`: take every repository of the project out of it; `true` also deletes their tasks.
    Remove(bool),
}

/// What `project` changes in one of its texts.
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

/// The words `search` looks for, without the first `--`, which only ends the options: `kb search -- --` looks for
/// `--`.
fn search_words(mut words: Vec<String>) -> String {
    if let Some(at) = words.iter().position(|w| w == "--") {
        words.remove(at);
    }
    words.join(" ")
}

/// The links typed in `value`; a word that isn't one is refused rather than left out.
fn typed_links(value: &str) -> Result<(Vec<u32>, Vec<RepoLink>)> {
    if let Some(word) = not_a_link(value) {
        bail!("`{word}` isn't an entry id (12 or #12) or another repository's entry (REPO#ID)");
    }
    Ok(parse_links(value))
}

/// The first argument as an entry id (`12` or `#12`).
fn first_id(args: &mut Vec<String>) -> Result<u32> {
    if args.is_empty() {
        bail!("an entry id is needed");
    }
    let arg = args.remove(0);
    // The parser's own words ("invalid digit found in string") say nothing more to whoever typed it.
    arg.trim_start_matches('#').parse().map_err(|_| anyhow!("`{arg}` isn't an entry id"))
}

/// An id, then the text: the remaining words, or `-` alone for stdin; `needs` says what's missing without it.
fn id_and_text(args: &mut Vec<String>, needs: &str) -> Result<(u32, Text)> {
    let id = first_id(args)?;
    Ok((id, rest_as_text(args, needs)?))
}

/// The remaining words as one text, or `-` alone for stdin; `needs` says what's missing without them.
fn rest_as_text(args: &mut Vec<String>, needs: &str) -> Result<Text> {
    let mut words = std::mem::take(args);
    // Only says where options end (see `cli::options`); the text is what follows.
    if let Some(at) = words.iter().position(|w| w == "--") {
        words.remove(at);
    }
    match words.as_slice() {
        [] => bail!("{needs}: TEXT, or - to read it from stdin"),
        [dash] if dash == "-" => Ok(Text::Stdin),
        _ => Ok(Text::Given(words.join(" "))),
    }
}

/// What follows `project`: nothing, to show it; else what to change.
fn project_args(args: &mut Vec<String>) -> Result<Option<ProjectChange>> {
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

/// Shows the project's description and this repository's role, or changes the project.
fn run_project(brain: &mut Brain, change: Option<ProjectChange>) -> Result<()> {
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
            vec![format!("project {name}: description saved")]
        }
        Some(ProjectChange::Text(ProjectPart::Notes, text)) => {
            brain.set_repo_notes(&text.read()?)?;
            vec![format!("project {name}: repository role saved")]
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
        Some(ProjectChange::Rename(new, _)) => manage::rename_project(&central, &name, &new)?,
        Some(ProjectChange::Repo(new)) => manage::rename_repo(&central, &dir, &new)?,
        Some(ProjectChange::Move(to)) => manage::move_repo(&central, &dir, &to)?,
        Some(ProjectChange::Remove(purge)) => manage::remove_project(&central, &name, purge)?,
    };
    for line in &done {
        println!("{line}");
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
    let seconds = minutes.checked_mul(60).with_context(|| format!("`{arg}` minutes is longer than any wait"))?;
    Ok((minutes > 0).then(|| Duration::from_secs(seconds)))
}

/// Blocks until entry `id` no longer waits on a person, then prints what they said; an error once
/// `timeout` passes without it. An agent runs it in the background after `ask` or `propose`, where its
/// tool wakes it when it exits: one wake-up with the answer, instead of checking by turns.
fn wait(brain: &Brain, id: u32, timeout: Option<Duration>) -> Result<()> {
    let started = std::time::Instant::now();
    let mut watch = Watch::default();
    loop {
        if let Some(said) = watch.look(id, brain.store.load().map(|(tasks, _)| tasks)) {
            println!("{}", said?);
            return Ok(());
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            bail!("no answer on #{id} yet: stop here and don't wait again; once answered, it comes first in `context`");
        }
        std::thread::sleep(WAIT_POLL);
    }
}

/// What `wait` keeps from one look at the brain to the next, apart from the clock so a test can play
/// a run of looks without sleeping through them.
#[derive(Default)]
struct Watch {
    /// The question it found waiting: answered and then moved before the next look, the answer is still what's said.
    asked_at: Option<chrono::DateTime<chrono::Local>>,
    /// Whether it looked before: the first look fails at once on a brain it can't read or without the entry, as
    /// waiting on either would only wait out the time limit.
    looked: bool,
    /// Whether the entry was missing from the last look: its file renamed for a new title between listing the
    /// folder and reading it is back by the next one, so only missing twice in a row it was deleted.
    missed: bool,
}

impl Watch {
    /// Takes one look's read of the brain: `Some` ends the wait, with what to print or why it can't go on;
    /// `None` looks again.
    fn look(&mut self, id: u32, read: Result<Vec<Task>>) -> Option<Result<String>> {
        let first = !std::mem::replace(&mut self.looked, true);
        // After the first look, a read that fails (the folder caught mid-change) is looked again at next time.
        match read {
            Ok(tasks) => match tasks.into_iter().find(|t| t.id == id) {
                None if first || self.missed => Some(Err(ApiError::NotFound(id).into())),
                None => {
                    self.missed = true;
                    None
                }
                Some(task) => {
                    self.missed = false;
                    if let Some(Asked::Waiting(question)) = task.asked() {
                        self.asked_at.get_or_insert(question.created);
                    }
                    settled(&task, self.asked_at).map(Ok)
                }
            },
            Err(e) if first => Some(Err(e)),
            Err(_) => None,
        }
    }
}

/// What `wait` prints once `task` no longer waits on a person, or `None` while it does: a decision that
/// left proposed, with its verdict's comment; a task's answer, also when it was moved after it (the question
/// `wait` found waiting was asked at `asked_at`); or that the task waits on nothing (never asked, or picked up since).
fn settled(task: &Task, asked_at: Option<chrono::DateTime<chrono::Local>>) -> Option<String> {
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
        None if task.is_task() => {
            let answers = |c: &&crate::model::Comment| c.author == crate::model::Author::Human || c.answer;
            let answer = asked_at
                .and_then(|at| task.comments.iter().rev().find(|c| c.created >= at && !c.question && answers(c)));
            Some(match answer {
                Some(answer) => format!("{head}: answered, now {}\n\n{}", task.status.as_str(), answer.body),
                None => format!("{head}: not waiting for an answer ({})", task.status.as_str()),
            })
        }
        // A note has no status to name.
        None => Some(format!("{head}: a note, not waiting for an answer")),
    }
}

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
fn print_entry(brain: &Brain, id: u32, json: bool, full: bool) -> Result<()> {
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
