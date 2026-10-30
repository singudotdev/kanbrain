//! `wait`: blocking until an entry no longer waits on a person, then printing what they said.

use crate::api::ApiError;
use crate::brain::Brain;
use crate::model::{Asked, DecisionState, Kind, Task};
use crate::store::Store;
use anyhow::{Context, Result, bail};
use std::time::Duration;

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
pub(super) fn wait_timeout(args: &mut Vec<String>) -> Result<Option<Duration>> {
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
pub(super) fn minutes(arg: &str) -> Result<Option<Duration>> {
    let minutes: u64 = arg.parse().with_context(|| format!("`{arg}` isn't a number of minutes"))?;
    let seconds = minutes.checked_mul(60).with_context(|| format!("`{arg}` minutes is longer than any wait"))?;
    Ok((minutes > 0).then(|| Duration::from_secs(seconds)))
}

/// Blocks until entry `id` no longer waits on a person, then prints what they said; an error once
/// `timeout` passes without it. An agent runs it in the background after `ask` or `propose`, where its
/// tool wakes it when it exits: one wake-up with the answer, instead of checking by turns.
pub(super) fn wait(brain: &Brain, id: u32, timeout: Option<Duration>) -> Result<()> {
    let started = std::time::Instant::now();
    let mut watch = Watch::default();
    loop {
        if let Some(said) = watch.look(id, read_for(&brain.store, id)) {
            println!("{}", said?);
            return Ok(());
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            bail!("no answer on #{id} yet: stop here and don't wait again; once answered, it comes first in `context`");
        }
        std::thread::sleep(WAIT_POLL);
    }
}

/// One look's read of the brain for entry `id`: its file, rather than every entry's each second; every entry when no
/// file named for it holds it (renamed by hand, or gone, which [`Watch::look`] tells apart by looking again).
pub(super) fn read_for(store: &Store, id: u32) -> Result<Vec<Task>> {
    let (tasks, _) = store.load_named(id)?;
    if tasks.iter().any(|task| task.id == id) {
        return Ok(tasks);
    }
    Ok(store.load_entries()?.0)
}

/// What `wait` keeps from one look at the brain to the next, apart from the clock so a test can play
/// a run of looks without sleeping through them.
#[derive(Default)]
pub(super) struct Watch {
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
    pub(super) fn look(&mut self, id: u32, read: Result<Vec<Task>>) -> Option<Result<String>> {
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
pub(super) fn settled(task: &Task, asked_at: Option<chrono::DateTime<chrono::Local>>) -> Option<String> {
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
