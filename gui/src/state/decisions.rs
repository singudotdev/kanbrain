//! Questions and decisions agents leave for you: which entries wait on you, and answering, accepting or rejecting.

use super::{Board, Key, Target};
use kanbrain_core::model::{DecisionState, Kind, Task};
use std::rc::Rc;

impl Board {
    /// Whether saving `target` answers a question the task waits on.
    pub fn answers(&self, target: Target) -> bool {
        let Target::NewComment(key) = target else { return false };
        self.task(key).is_some_and(Task::waits_for_answer)
    }

    /// The decision `y`, `Y` and `j` act on for the entry `key`: itself if it's a decision, else the first one an
    /// agent proposed about it, which it waits on.
    pub fn decision_for(&self, key: Key) -> Option<Key> {
        match self.task(key)?.kind {
            Kind::Decision => Some(key),
            _ => self.proposed_for(key).first().copied(),
        }
    }

    /// The selected entry's decision (see [`Self::decision_for`]), or why there's none.
    fn selected_decision(&mut self) -> Option<Key> {
        let decision = self.selected.and_then(|key| self.decision_for(key));
        if decision.is_none() && self.selected.is_some() {
            self.notify("only decisions are accepted or rejected");
        }
        decision
    }

    /// Accepts the selected decision, or the one the selected task waits on, which answers the tasks waiting on
    /// it; accepting with a comment and rejecting ask for the text first, in the editor.
    pub fn accept_selected(&mut self) {
        let Some(key) = self.selected_decision() else { return };
        let name = self.entry_name(key);
        match self.brain_mut(key.src).resolve(key.id, DecisionState::Accepted, "") {
            Ok(true) => self.notify(format!("{name} accepted")),
            Ok(false) => self.notify(format!("{name} is already accepted")),
            Err(e) => self.notify_error(&e),
        }
    }

    /// Opens the editor for a comment on accepting the selected decision, or the one the selected task waits on.
    pub fn accept_with_comment(&mut self) {
        let Some(key) = self.selected_decision() else { return };
        match self.task(key).map(|t| t.decision) {
            Some(DecisionState::Accepted) => self.notify(format!("{} is already accepted", self.entry_name(key))),
            Some(_) => self.open_editor(Target::Accept(key)),
            None => {}
        }
    }

    /// Opens the editor for why the selected decision, or the one the selected task waits on, is rejected.
    pub fn reject_selected(&mut self) {
        let Some(key) = self.selected_decision() else { return };
        match self.task(key).map(|t| t.decision) {
            Some(DecisionState::Rejected) => self.notify(format!("{} is already rejected", self.entry_name(key))),
            Some(_) => self.open_editor(Target::Reject(key)),
            None => {}
        }
    }

    /// Tasks waiting for an answer to an agent's question, in every brain shown, in board order.
    pub fn waiting(&self) -> Rc<[Key]> {
        self.showing().waiting.clone()
    }

    /// The decisions an agent proposed about the task `key`, which it waits on until they're accepted or rejected.
    pub fn proposed_for(&self, key: Key) -> Vec<Key> {
        let proposed =
            |k: &Key| self.task(*k).is_some_and(|d| d.kind == Kind::Decision && d.decision == DecisionState::Proposed);
        self.related_keys(key).into_iter().filter(proposed).collect()
    }

    /// Opens the first decision the selected task waits on.
    pub fn open_proposed(&mut self) {
        match self.selected.and_then(|key| self.proposed_for(key).first().copied()) {
            Some(decision) => self.open_entry(decision),
            None => self.notify("it waits on no proposed decision"),
        }
    }

    /// Opens the next task waiting for an answer after the selected one, wrapping around.
    pub fn next_waiting(&mut self) {
        let waiting = self.waiting();
        let after = |k: &&Key| self.selected.is_none_or(|s| (k.src, k.id) > (s.src, s.id));
        match waiting.iter().find(after).or_else(|| waiting.first()) {
            Some(&key) => self.open_entry(key),
            None => self.notify("nothing waits for an answer"),
        }
    }
}
