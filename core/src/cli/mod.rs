//! The command line: which command to run, with its options.
//!
//! - [`entries`]: commands that read or change entries by id, for agents and scripts.
//! - [`mod@init`]: a project's `.brain/`, and the section telling its agents about it; `deinit` undoes it.
//! - [`claude`]: the Claude Code hooks `init` adds: the context at each session start, and [`hook`]'s checks.
//! - [`hook`]: `kb hook prompt|stop|edit`, which hold an agent to describing an undescribed project first, to
//!   commenting on the tasks it started, and to `kb` for the brain's files.
//! - [`completions`]: `kb completions SHELL`, tab completion, and the ids, projects and tags it offers.
//! - [`update`]: `kb update` and the app's, the latest release over this one, through the installer, once no
//!   agent is working.

mod claude;
mod completions;
mod entries;
mod hook;
mod init;
mod manage;
pub mod update;

pub use entries::{AddArgs, EntryArgs, EntryCommand, ProjectChange, ProjectPart, Text};
pub use init::init;
pub use manage::{move_repo, remove_project, remove_repo, rename_project, rename_repo};

use crate::api::json::Summary;
use crate::brain::{Brain, Group, ReportView};
use crate::model::{Asked, Author, Kind, Status, Task, fmt_clock, now, parse_tags};
use crate::period::{Period, Unit};
use crate::store::{self, Store};
use crate::{export, platform, projects};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local};
use std::fmt::Write;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

/// The binary running the command line, named in the usage text and `--version`.
#[derive(Debug, Clone, Copy)]
pub struct Program {
    /// The command's name.
    pub name: &'static str,
}

impl Program {
    /// Parses the arguments after the program name and runs the command.
    /// A command-line error comes with the usage of the command it's about.
    pub fn main(self, args: impl IntoIterator<Item = String>) -> Result<()> {
        let (project, args) = take_project(args.into_iter().collect()).map_err(|e| self.misused(&e, None))?;
        let name = args.first().cloned();
        // `kb add --help` and `kb help add`: that command's usage, as an error after it would show.
        // Right after the command, or `project`'s subcommand, only: in a comment's words (`kb comment 2 pass -h to
        // it`), it's text.
        let help = |flag: &String| flag == "--help" || flag == "-h";
        let asked = match args.as_slice() {
            [word, command] if word == "help" => Some(command.as_str()),
            [command, flag, ..] if help(flag) => Some(command.as_str()),
            [command, _, flag, ..] if command == "project" && help(flag) => Some(command.as_str()),
            _ => None,
        };
        if let Some(command) = asked {
            let lines = self.usage_of(command);
            if lines.is_empty() {
                bail!("unknown command `{command}`\n\n`{} --help` lists every command", self.name);
            }
            println!("usage:\n{}", lines.join("\n"));
            return Ok(());
        }
        let command = Command::parse(args).map_err(|e| self.misused(&e, name.as_deref()))?;
        command.run(self, project.as_deref())
    }

    /// `error`, then the usage lines of `command` (when it's one), and where the rest is: the whole usage would bury
    /// the one line that matters.
    fn misused(self, error: &anyhow::Error, command: Option<&str>) -> anyhow::Error {
        let lines = command.map(|c| self.usage_of(c)).unwrap_or_default();
        let help = format!("`{} --help` lists every command", self.name);
        if lines.is_empty() {
            anyhow!("{error:#}\n\n{help}")
        } else {
            anyhow!("{error:#}\n\nusage:\n{}\n\n{help}", lines.join("\n"))
        }
    }

    /// The usage lines of `command`, from the whole usage; none for what isn't one.
    fn usage_of(self, command: &str) -> Vec<String> {
        let usage = usage(self);
        let header = format!("  {} ", self.name);
        let mut lines = Vec::new();
        let (mut inside, mut bare) = (false, false);
        for line in usage.lines() {
            if let Some(rest) = line.strip_prefix(&header) {
                let names = rest.split_whitespace().next().unwrap_or_default();
                // `accept` shares the description under `reject`'s line: a matched line with none yet, on it or
                // under it, takes the next.
                inside = names.split('|').any(|n| n == command) || bare;
                bare = inside && !bare && !rest.contains("   ");
            } else if !(inside && line.starts_with("   ")) {
                inside = false;
            } else {
                bare = false;
            }
            if inside {
                lines.push(line.to_string());
            }
        }
        lines
    }
}

/// Takes `--project NAME` out of the arguments, wherever it is before a `--` (see [`options`]): it picks the brain for
/// any command.
fn take_project(mut args: Vec<String>) -> Result<(Option<String>, Vec<String>)> {
    let Some(at) = options(&args).iter().position(|a| a == "--project") else { return Ok((None, args)) };
    if at + 1 >= args.len() {
        bail!("--project needs a project name (or `{}`)", projects::CENTRAL);
    }
    let name = args.remove(at + 1);
    args.remove(at);
    Ok((Some(name), args))
}

/// The arguments that may be options: those before a `--`, after which every word is text (`kb comment 3 -- use
/// --json`), as most command lines have it.
pub(crate) fn options(args: &[String]) -> &[String] {
    args.iter().position(|a| a == "--").map_or(args, |at| &args[..at])
}

/// Printed by `--help` and after a command-line error. The status, period and group names come
/// from `Status`, `Unit` and `Group`, so they're only written there.
pub fn usage(program: Program) -> String {
    let name = program.name;
    let pad = " ".repeat(name.len());
    format!(
        "{name} — project memory for you and your coding agents: tasks, notes and decisions in markdown files

usage:
  {name} list [{statuses}|all] [notes] [decisions] [--tag TAG] [--json]
  {pad}           print tasks as markdown, one section per status, then notes and decisions
  {pad}           (default: {default}; several can be given; notes or decisions alone
  {pad}           list only those)
  {name} init [NAME] [--repo REPO]
  {pad}           make this folder a project: its own .brain/, registered as NAME (default: the
  {pad}           folder's name) with a card for the application, and a section telling its agents
  {pad}           about it in CLAUDE.md / AGENTS.md (run again to update them); in a project with
  {pad}           several, the repository is named REPO (default: the folder's, made unique)
  {name} init --all
  {pad}           update the agents' section and hooks in every registered project to this version's
  {pad}           (the installer and update run it)
  {name} deinit [--purge]
  {pad}           stop this folder (or --project NAME) being a project: unregister it, remove its
  {pad}           section from CLAUDE.md / AGENTS.md, the other repositories' links to it and, with
  {pad}           its last repository, the project's card; its .brain/ is kept unless --purge
  {name} hook prompt|stop|edit
  {pad}           what the Claude Code hooks `init` adds run: they hold an agent to describing an
  {pad}           undescribed project first, to commenting on the tasks it started, and to `kb` for
  {pad}           the brain's files (they read the hook's JSON on stdin)
  {name} status    print the running tasks, in whichever project, and how many wait for an answer
  {pad}           (for status bars)
  {name} report [{units}] [--group {groups}] [--tag TAG] [--save]
  {pad}           print the current period's tracked time as markdown
  {pad}           (default: {unit}, grouped by {group}); --save also writes it to reports/
{entries}
  {name} path      print the data directory in use (a project's .brain/ inside it)
  {name} update    install the latest release over this one (and the other app, if it's beside it)
  {name} completions bash|zsh|fish
  {pad}           print a tab completion script for the shell (commands, ids, projects, tags)
  {name} --version print the version

--project NAME, anywhere on the line, runs a command on a registered project's brain (or `central`).
--json prints JSON instead, for list, add, show, search, start|park|done|todo, comment, ask, propose, accept,
reject, describe and link.
-- ends a text's options: every word after it is the text (`{name} comment 3 -- use --json`).
Data lives in the nearest .brain/ folder here or above (see init); else in the central brain:
$KANBRAIN_DIR, else the platform data dir (e.g. ~/.local/share/kanbrain).
The kanban board, `kanbrain` without arguments, shows the same data.

kanbrain by singudotdev (https://github.com/singudotdev/kanbrain). AGPL-3.0-only with the additional terms in
NOTICE; no warranty.",
        statuses = Status::names("|"),
        default = DEFAULT_LIST.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
        units = Unit::names("|"),
        groups = Group::names("|"),
        unit = DEFAULT_REPORT_UNIT.as_str(),
        group = Group::default().as_str(),
        entries = entries::USAGE
            .lines()
            .map(|line| {
                // Commands start at the margin and get the program's name; descriptions under them are
                // indented further and get blanks instead, so they stay aligned.
                let rest = line.strip_prefix("  ").unwrap_or(line);
                format!("  {} {rest}", if rest.starts_with(' ') { &pad } else { name })
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// What `kb list` shows unless statuses are given: everything that isn't done.
const DEFAULT_LIST: &[Status] = &[Status::Doing, Status::Todo, Status::Parked];

/// The period `kb report` shows unless another one is given.
const DEFAULT_REPORT_UNIT: Unit = Unit::Week;

/// What to do, as chosen on the command line.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `list`: print tasks as markdown.
    List(ListArgs),
    /// `init [NAME] [--repo REPO]`: make the current directory a project, named `NAME` or after the folder,
    /// and the repository `REPO` in it.
    Init(Option<String>, Option<String>),
    /// `init --all`: bring every registered repository's agent instructions up to this version's.
    InitAll,
    /// `deinit [--purge]`: undo `init` for the current directory's project, or the one `--project` names;
    /// `true` also deletes its `.brain/`.
    Deinit(bool),
    /// `hook prompt|stop|edit`: what Claude Code's hooks run (see [`hook`]).
    Hook(hook::HookEvent),
    /// `status`: print the running tasks for status bars.
    Status,
    /// `report`: print a time report as markdown.
    Report(ReportArgs),
    /// `add`, `show`, `search`, `start`, `comment`, …: entries by id, for agents and scripts.
    Entries(EntryArgs),
    /// `path`: print the data folder.
    Path,
    /// `update`: install the latest release.
    Update,
    /// `completions SHELL`: print a tab completion script.
    Completions(completions::Shell),
    /// `complete ids|projects|tags …`: what the completion scripts offer, one per line. The words after
    /// it are the command line being completed, ignored but for its `--project`.
    Complete(completions::Candidates),
    /// `--version` / `-V`.
    Version,
    /// `--help` / `-h`.
    Help,
}

/// Options of `kb list`.
#[derive(Debug, PartialEq, Eq)]
pub struct ListArgs {
    /// Task statuses to show, in list order.
    pub statuses: Vec<Status>,
    /// Notes and/or decisions to show after the tasks, in that order.
    pub kinds: Vec<Kind>,
    /// Only entries with this tag.
    pub tag: Option<String>,
    /// Print a JSON array instead of markdown.
    pub json: bool,
}

/// Options of `kb report`.
#[derive(Debug, PartialEq, Eq)]
pub struct ReportArgs {
    /// Period length; the current one is reported.
    pub unit: Unit,
    /// Rows per task or per tag.
    pub group: Group,
    /// Only count tasks with this tag.
    pub tag: Option<String>,
    /// Also write the report to `reports/`.
    pub save: bool,
}

impl Command {
    /// Parses the arguments after the program name.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let command = match args.next().as_deref() {
            Some("list") => Command::List(ListArgs::parse(args.by_ref())?),
            Some("init") => {
                let words: Vec<String> = args.by_ref().collect();
                match words.as_slice() {
                    [all] if all == "--all" => Command::InitAll,
                    _ => init_args(words)?,
                }
            }
            Some("deinit") => Command::Deinit(match args.next() {
                None => false,
                Some(purge) if purge == "--purge" => true,
                Some(other) => bail!("unexpected argument `{other}`"),
            }),
            Some("hook") => {
                let name =
                    args.next().with_context(|| format!("hook needs one of {}", hook::HookEvent::names(", ")))?;
                Command::Hook(
                    hook::HookEvent::parse(&name)
                        .with_context(|| format!("unknown hook `{name}`: {}", hook::HookEvent::names(", ")))?,
                )
            }
            Some("status") => Command::Status,
            Some("report") => Command::Report(ReportArgs::parse(args.by_ref())?),
            Some("path") => Command::Path,
            Some("update") => Command::Update,
            Some("completions") => {
                let name = args
                    .next()
                    .with_context(|| format!("completions needs one of {}", completions::Shell::names(", ")))?;
                Command::Completions(
                    completions::Shell::parse(&name)
                        .with_context(|| format!("unknown shell `{name}`: {}", completions::Shell::names(", ")))?,
                )
            }
            Some("complete") => {
                let name = args
                    .next()
                    .with_context(|| format!("complete needs one of {}", completions::Candidates::names(", ")))?;
                let what = completions::Candidates::parse(&name).with_context(|| {
                    format!("unknown candidates `{name}`: {}", completions::Candidates::names(", "))
                })?;
                args.by_ref().for_each(drop);
                Command::Complete(what)
            }
            Some("-V" | "--version") => Command::Version,
            None | Some("-h" | "--help" | "help") => Command::Help,
            Some(other) => match EntryArgs::parse(other, args.by_ref())? {
                Some(entries) => Command::Entries(entries),
                None => bail!("unknown command `{other}`"),
            },
        };
        if let Some(extra) = args.next() {
            bail!("unexpected argument `{extra}`");
        }
        Ok(command)
    }

    /// Runs the command on the brain of `project` (by name), else the one found from here.
    /// Everything except `--version` and `--help` refuses to run as root.
    pub fn run(self, program: Program, project: Option<&str>) -> Result<()> {
        match self {
            Command::Version => {
                println!("{} {}", program.name, env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Command::Help => {
                println!("{}", usage(program));
                return Ok(());
            }
            Command::Completions(shell) => {
                print!("{}", completions::script(shell, program.name));
                return Ok(());
            }
            Command::Update => {
                platform::ensure_not_root()?;
                return update::update(&projects::central_dir()?);
            }
            Command::Init(name, repo) => {
                platform::ensure_not_root()?;
                if project.is_some() {
                    bail!("init works on the current directory; leave out --project");
                }
                let root = std::env::current_dir().context("reading the current directory")?;
                init::init(&root, name.as_deref(), repo.as_deref(), &projects::central_dir()?)?
                    .iter()
                    .for_each(|line| println!("{line}"));
                return Ok(());
            }
            Command::InitAll => {
                platform::ensure_not_root()?;
                init::init_all(&projects::central_dir()?)?.iter().for_each(|line| println!("{line}"));
                return Ok(());
            }
            Command::Hook(event) => {
                // Claude Code sends the event as JSON on stdin; typed at a terminal, there's none to wait for.
                let mut input = String::new();
                if !std::io::stdin().is_terminal() {
                    std::io::stdin().read_to_string(&mut input).context("reading the hook's input")?;
                }
                let here = std::env::current_dir().context("reading the current directory")?;
                if let Some(out) = hook::respond(event, &input, &projects::central_dir()?, &here) {
                    println!("{out}");
                }
                return Ok(());
            }
            Command::Deinit(purge) => {
                platform::ensure_not_root()?;
                let central = projects::central_dir()?;
                let dir = deinit_dir(&central, project)?;
                manage::remove_repo(&central, &dir, purge)?.iter().for_each(|line| println!("{line}"));
                return Ok(());
            }
            _ => platform::ensure_not_root()?,
        }
        let store = Store::open_named(project)?;
        let central = projects::central_dir()?;
        match self {
            Command::List(args) => {
                // Opened as the entry commands open it, repairs and all, so the ids it lists are the ones they take:
                // a sync conflict copy listed under its original's id would be ambiguous.
                let _lock = store.lock()?;
                let brain = Brain::open_in(store, &central)?;
                say_skipped(&brain);
                let tasks = &brain.tasks;
                if args.json {
                    println!("{}", list_json(tasks, &args)?);
                } else {
                    match list_markdown(tasks, &args) {
                        // As `search` says it, rather than nothing a reader could take for a failure.
                        empty if empty.is_empty() => println!("nothing to list"),
                        listed => print!("{listed}"),
                    }
                }
                Ok(())
            }
            Command::Status => {
                println!("{}", status_line(&central, now())?);
                Ok(())
            }
            Command::Entries(args) => run_entries(store, &central, args),
            Command::Report(args) => print_report(store, &args),
            Command::Path => {
                println!("{}", store.dir().display());
                Ok(())
            }
            Command::Complete(what) => {
                completions::candidates(what, store, &central)?.iter().for_each(|line| println!("{line}"));
                Ok(())
            }
            // Handled above, before touching the data folder.
            Command::Version
            | Command::Help
            | Command::Completions(_)
            | Command::Update
            | Command::Init(..)
            | Command::InitAll
            | Command::Deinit(_)
            | Command::Hook(_) => Ok(()),
        }
    }
}

impl ListArgs {
    /// Parses the options after `list`.
    fn parse(args: &mut impl Iterator<Item = String>) -> Result<Self> {
        let mut statuses = Vec::new();
        let mut kinds = Vec::new();
        let mut tag = None;
        let mut json = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--json" => json = true,
                "notes" | "note" => kinds.push(Kind::Note),
                "decisions" | "decision" => kinds.push(Kind::Decision),
                "--tag" => {
                    let value = args.next().context("--tag needs a value")?;
                    // One tag: a second would be dropped without a word.
                    let mut tags = parse_tags(&value).into_iter();
                    tag = tags.next();
                    if tags.next().is_some() {
                        bail!("--tag takes one tag, not `{value}`");
                    }
                }
                "all" => statuses.extend(Status::ALL),
                // Anything else must be a status name.
                other => match Status::parse(other) {
                    Some(status) => statuses.push(status),
                    None => bail!("unknown list option `{other}`"),
                },
            }
        }
        // Asking only for notes or decisions lists only those.
        if statuses.is_empty() && kinds.is_empty() {
            statuses.extend(DEFAULT_LIST);
        }
        // List order, without repeats.
        statuses.sort_by_key(|s| s.position());
        statuses.dedup();
        kinds.sort_by_key(|k| k.position());
        kinds.dedup();
        Ok(Self { statuses, kinds, tag, json })
    }
}

impl ReportArgs {
    /// Parses the options after `report`.
    fn parse(args: &mut impl Iterator<Item = String>) -> Result<Self> {
        let mut report = ReportArgs { unit: DEFAULT_REPORT_UNIT, group: Group::default(), tag: None, save: false };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--group" => {
                    let groups = Group::names(" or ");
                    let value = args.next().with_context(|| format!("--group needs {groups}"))?;
                    report.group = Group::parse(&value).with_context(|| format!("--group must be {groups}"))?;
                }
                "--tag" => {
                    let value = args.next().context("--tag needs a value")?;
                    report.tag = parse_tags(&value).into_iter().next();
                }
                "--save" => report.save = true,
                // Anything else must be a period name.
                other => match Unit::parse(other) {
                    Some(unit) => report.unit = unit,
                    None => bail!("unknown report option `{other}`"),
                },
            }
        }
        Ok(report)
    }
}

/// Environment variables coding agents set in the commands they run, so `kb` can tell them from a
/// person at a terminal: the generic `AGENT` and `AI_AGENT`, then Claude Code's, Codex's and Gemini CLI's.
const AGENT_VARIABLES: &[&str] =
    &["AGENT", "AI_AGENT", "CLAUDECODE", "CODEX_SANDBOX", "CODEX_CI", "CODEX_THREAD_ID", "GEMINI_CLI"];

/// `init`'s arguments: an optional project name, and `--repo REPO` anywhere.
fn init_args(words: Vec<String>) -> Result<Command> {
    let mut words = words.into_iter();
    let (mut name, mut repo) = (None, None);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--repo" => repo = Some(words.next().context("--repo needs the repository's name")?),
            other if other.starts_with("--") => bail!("unknown init option `{other}`"),
            _ if name.is_none() => name = Some(word),
            _ => bail!("unexpected argument `{word}`"),
        }
    }
    Ok(Command::Init(name, repo))
}

/// Who this command runs for, recorded on the comments and status changes it makes: `KANBRAIN_AUTHOR`
/// (`human` or `agent`, in any case) if set and not empty, as the agents' variables are read, else an agent if one
/// of [`AGENT_VARIABLES`] is set, else a person. `var` reads a variable, as `std::env::var_os` does for the command
/// line.
fn author(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Result<Author> {
    if let Some(value) = var("KANBRAIN_AUTHOR").filter(|v| !v.is_empty()) {
        let value = value.to_string_lossy();
        return Author::parse(&value)
            .with_context(|| format!("KANBRAIN_AUTHOR must be {}, not `{value}`", Author::names(" or ")));
    }
    let agent = AGENT_VARIABLES.iter().any(|name| var(name).is_some_and(|v| !v.is_empty()));
    Ok(if agent { Author::Agent } else { Author::Human })
}

/// One line for status bars: `#id title HH:MM:SS` for each running session, or `idle`, then how many tasks wait for an
/// answer, if any. Looks in every brain, wherever it's run from, since each runs its own tasks; a project's task says
/// which project, and several are joined: `shop/api #3 Fix login 00:42:17 · blog #1 Post 00:05:00 · 2 waiting`. A brain
/// it can't read (one a newer kanbrain wrote, say) is left out rather than the whole line lost.
fn status_line(central: &Path, now: DateTime<Local>) -> Result<String> {
    let mut brains = vec![(None, central.to_path_buf())];
    let repos = projects::load(central)?;
    brains.extend(repos.iter().map(|r| (Some(r.label(&repos)), r.dir.clone())));
    let mut running = Vec::new();
    let mut waiting = 0;
    for (name, dir) in brains.into_iter().filter(|(_, dir)| dir.is_dir()) {
        let Ok((tasks, _)) = Store::at(&dir).and_then(|store| store.load()) else { continue };
        let project = name.map(|n| format!("{n} ")).unwrap_or_default();
        for (task, clock) in tasks.iter().filter_map(|t| Some((t, t.running_for(now)?))) {
            running.push(format!("{project}#{} {} {}", task.id, task.title, fmt_clock(clock)));
        }
        waiting += tasks.iter().filter(|t| matches!(t.asked(), Some(Asked::Waiting(_)))).count();
    }
    if running.is_empty() {
        running.push("idle".to_string());
    }
    if waiting > 0 {
        running.push(format!("{waiting} waiting"));
    }
    Ok(running.join(" · "))
}

/// The `.brain/` `deinit` takes out: `project`'s, else the one here. Not through `Store::open_named`, which refuses a
/// deleted repository: this is how it's taken out.
fn deinit_dir(central: &Path, project: Option<&str>) -> Result<PathBuf> {
    let central_refused = || anyhow!("the central brain isn't a project: there's nothing to deinit");
    match project {
        Some(projects::CENTRAL) => Err(central_refused()),
        Some(label) => match projects::find(central, label) {
            Ok(repo) => Ok(repo.dir),
            Err(_) if label.eq_ignore_ascii_case(projects::CENTRAL) => Err(central_refused()),
            Err(e) => Err(e),
        },
        None => {
            let here = std::env::current_dir().context("reading the current directory")?;
            store::find_project(&here)
                .context("no .brain/ here or above: run deinit inside a project, or name one with --project")
        }
    }
}

/// Runs one of the entry commands on the brain in `store`.
fn run_entries(store: Store, central: &Path, mut args: EntryArgs) -> Result<()> {
    let author = author(|name| std::env::var_os(name))?;
    // Before the lock: `-` waits for stdin, maybe for someone typing, and every other `kb` and the app would wait
    // meanwhile.
    args.read_stdin()?;
    // A project change writes the central brain's registry or cards too, under its lock: taken first, in the order
    // the app takes them, or this and the app could each hold one lock and wait forever for the other's.
    let _central = match args.command {
        EntryCommand::Project(Some(_)) => Some(Store::at(central)?.lock()?),
        _ => None,
    };
    // Held from reading the files to writing them, so parallel commands each see the others' changes; opening may
    // write too, as it repairs. `wait` lets go once open, not to hold it while it waits.
    let lock = store.lock()?;
    let brain = Brain { author, ..Brain::open_in(store, central)? };
    say_skipped(&brain);
    let _lock = (!matches!(args.command, EntryCommand::Wait(..))).then_some(lock);
    args.run(brain)
}

/// Says on stderr which entries' files couldn't be read: left out silently, an agent told `no entry #7` would add
/// the task again, though its file is there. A file is an entry's when it's broken as only one is (a bad history
/// line, say), or named as one (`0007-title.md`); markdown of other names, or dated (`2024-01-15.md`), with no id is
/// left unsaid: a central brain kept in a notes vault holds many.
fn say_skipped(brain: &Brain) {
    let entry_file = |warning: &&String| store::entry_named(warning) || !warning.contains(crate::model::MISSING_ID);
    for warning in brain.warnings.iter().filter(entry_file) {
        eprintln!("kb: skipped {warning}; fix the file or remove it");
    }
}

/// Prints the current period's report as markdown, and saves it with `--save`.
fn print_report(store: Store, args: &ReportArgs) -> Result<()> {
    // Opening may repair files, and `--save` writes one.
    let _lock = store.lock()?;
    let brain = Brain::open(store)?;
    let view = ReportView { group: args.group, tag: args.tag.clone(), ..ReportView::new(Period::current(args.unit)) };
    print!("{}", export::to_markdown(&brain.report(&view), &view, now()));
    if args.save {
        eprintln!("saved {}", brain.export_report(&view)?.display());
    }
    Ok(())
}

/// What `list` prints, in order: one section per status (tasks only), then notes, then decisions,
/// each entry carrying `args.tag` if one was given.
fn list_sections<'a>(tasks: &'a [Task], args: &ListArgs) -> Vec<(&'static str, Vec<&'a Task>)> {
    let tagged = |t: &&Task| args.tag.as_ref().is_none_or(|tag| t.tags.contains(tag));
    let mut sections = Vec::new();
    for &status in &args.statuses {
        let mut matching: Vec<&Task> =
            tasks.iter().filter(|t| t.is_task() && t.status == status).filter(tagged).collect();
        matching.sort_by(|a, b| b.status_since().cmp(&a.status_since()).then(b.id.cmp(&a.id)));
        sections.push((status.as_str(), matching));
    }
    for &kind in &args.kinds {
        let mut matching: Vec<&Task> = tasks.iter().filter(|t| t.kind == kind).filter(tagged).collect();
        matching.sort_by_key(|t| std::cmp::Reverse(t.id));
        sections.push((kind_heading(kind), matching));
    }
    sections.retain(|(_, entries)| !entries.is_empty());
    sections
}

/// The `list` word and section heading for notes and decisions.
fn kind_heading(kind: Kind) -> &'static str {
    match kind {
        Kind::Task => "tasks",
        Kind::Note => "notes",
        Kind::Decision => "decisions",
    }
}

/// One `## heading` section per non-empty section, each entry as `- #id title #tag …`, then ` · ` what its section
/// doesn't say (`waiting for an answer`, `proposed`…).
fn list_markdown(tasks: &[Task], args: &ListArgs) -> String {
    let mut out = String::new();
    for (heading, entries) in list_sections(tasks, args) {
        if !out.is_empty() {
            out.push('\n');
        }
        writeln!(out, "## {heading}\n").expect("writing to a String cannot fail");
        for task in entries {
            out.push_str("- ");
            crate::api::text::head(&mut out, task).expect("writing to a String cannot fail");
            // As `context` says it: a parked task's question, a decision not in force.
            if let Some(note) = crate::api::text::state_note(task) {
                write!(out, " · {note}").expect("writing to a String cannot fail");
            }
            out.push('\n');
        }
    }
    out
}

/// The same entries as `list_markdown`, in the same order, as a JSON array of summaries on one line, as every
/// command's JSON is.
fn list_json(tasks: &[Task], args: &ListArgs) -> Result<String> {
    let entries: Vec<Summary> = list_sections(tasks, args).into_iter().flat_map(|(_, e)| e).map(Summary::new).collect();
    Ok(serde_json::to_string(&entries)?)
}

#[cfg(test)]
#[path = "../../tests/cli.rs"]
mod tests;
