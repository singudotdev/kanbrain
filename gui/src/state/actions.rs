//! Changes to entries, through the brain: each one says in the status bar what it did, or why not.

use super::{
    Board, CommentPicker, Confirm, Edit, Form, Key, LinkPicker, Mode, NextUndo, Pending, Picker, QuickAdd, Request,
    Tab, Target, Tell, Telling, WhereFor, WherePicker, changed_on_disk,
};
use crate::text::editor::{LineInput, TextEditor};
use crate::text::merge::{Merged, merge};
use anyhow::Result;
use chrono::{NaiveDate, NaiveTime};
use kanbrain_core::cli::update;
use kanbrain_core::model::{Author, Comment, DecisionState, Kind, Status};
use kanbrain_core::period::{Period, Unit};
use kanbrain_core::store::{self, Change};
use std::path::Path;

impl Board {
    /// Shows `result`'s error in the status bar, if it failed.
    pub(super) fn report_error(&mut self, result: Result<()>) {
        if let Err(e) = result {
            self.notify_error(&e);
        }
    }

    /// Moves a task to `status`. Starting one while the brain already runs a task you started is refused,
    /// saying which: nothing is parked for it. Agents' tasks don't count.
    pub fn set_status(&mut self, key: Key, status: Status) {
        let name = self.entry_name(key);
        match self.brain_mut(key.src).set_status(key.id, status) {
            Ok(true) => {
                let title = self.task(key).map(|t| t.title.clone()).unwrap_or_default();
                self.notify(format!("{name} {title} → {}", status.as_str()));
            }
            Ok(false) => self.notify(format!("{name} is already {}", status.as_str())),
            // The refusal names the task already running (`#2 is in doing`), in the same brain.
            Err(e) => {
                let at = self.source_label(key.src).map(|label| format!("{label} ")).unwrap_or_default();
                self.notify(format!("{at}{e:#}"));
            }
        }
        self.keep_selection();
    }

    /// A card dragged from the brain in `dir` dropped on `status`'s column, as [`Board::drop_card`]: its brain found
    /// by its folder, wherever a reload while it was dragged put it in the list; nothing if it's gone.
    pub fn drop_dragged(&mut self, dir: &Path, id: u32, status: Status) {
        if let Some(src) = self.src_at(dir) {
            self.drop_card(Key::new(src, id), status);
        }
    }

    /// A card dropped on `status`'s column: it moves there and is selected. Dropping it back on its own
    /// column only selects it, rather than saying it's already there.
    pub fn drop_card(&mut self, key: Key, status: Status) {
        self.selected = Some(key);
        if self.task(key).is_some_and(|t| t.status != status) {
            self.set_status(key, status);
        }
    }

    /// Shows an entry's details from the calendar, staying on it.
    pub fn show_entry(&mut self, key: Key) {
        self.select(key);
        self.drawer = self.selected == Some(key);
    }

    /// Shows one day of the calendar, from a click on it in the month.
    pub fn calendar_day(&mut self, date: NaiveDate) {
        let midnight = date.and_time(NaiveTime::MIN);
        self.calendar.period = Period::containing(Unit::Day, midnight);
        self.keep_selection();
    }

    /// Opens the form for a new entry of the current tab's kind: a task starts in `status`. It goes in
    /// the brain the scope shows, or, when it shows several, the selected entry's (see [`Self::home`]);
    /// the form's project field puts it in another.
    pub fn new_entry(&mut self, status: Status) {
        let kind = self.tab.kind().unwrap_or(Kind::Task);
        if self.tab.kind().is_none() {
            self.show_tab(Tab::Board);
        }
        self.mode = Mode::Form(Box::new(Form::new(self.home(), kind, status, self.sources.len() > 1)));
    }

    /// Opens `then` in the brain the scope shows, or, when it shows several, asks which one first: a
    /// new entry goes where the filter points, never somewhere the window doesn't say. The selected
    /// entry's brain is highlighted, as the likeliest.
    pub(super) fn ask_where(&mut self, then: WhereFor) {
        let picker = Picker::default();
        let candidates = self.where_candidates(&picker, then);
        if let [(src, _)] = candidates[..] {
            return self.open_new_in(src, then);
        }
        let selected = self.selected.map(|k| k.src);
        let sel = candidates.iter().position(|&(src, _)| Some(src) == selected).unwrap_or(0);
        self.mode = Mode::Where(WherePicker { picker: Picker { sel, ..picker }, then });
    }

    /// Opens what the "which project?" picker was asked for, in its candidate at `index`.
    pub fn choose_where(&mut self, index: usize) {
        let Mode::Where(asked) = &self.mode else { return };
        let Some(&(src, _)) = self.where_candidates(&asked.picker, asked.then).get(index) else { return };
        let then = asked.then;
        self.open_new_in(src, then);
    }

    /// Opens the column's field for a new task in the brain at `src`, or moves the entry there.
    fn open_new_in(&mut self, src: usize, then: WhereFor) {
        self.mode = match then {
            WhereFor::QuickAdd(status) => Mode::QuickAdd(QuickAdd { src, status, title: LineInput::default() }),
            WhereFor::Move(_) => Mode::Normal,
        };
        if let WhereFor::Move(key) = then {
            self.move_entry(key, src);
        }
    }

    /// Asks which project to move the selected entry to: every other brain, even with one, since a
    /// move renumbers the entry and drops its links.
    pub fn ask_move(&mut self) {
        let Some(key) = self.selected.filter(|k| self.task(*k).is_some()) else { return };
        if self.sources.len() < 2 {
            return self.notify("there's no other project to move it to");
        }
        self.mode = Mode::Where(WherePicker { picker: Picker::default(), then: WhereFor::Move(key) });
    }

    /// Moves `key`'s entry to the brain at `to`. It stays selected if the scope shows it there; else the
    /// selection goes to what's shown, and the status bar says where it went.
    fn move_entry(&mut self, key: Key, to: usize) {
        let from = self.entry_name(key);
        // Two brains of `sources` at once, one to take from and one to give to.
        let moved = if key.src < to {
            let (left, right) = self.sources.split_at_mut(to);
            left[key.src].brain.move_to(key.id, &mut right[0].brain)
        } else {
            let (left, right) = self.sources.split_at_mut(key.src);
            right[0].brain.move_to(key.id, &mut left[to].brain)
        };
        let id = match moved {
            Ok(id) => id,
            Err(e) => return self.notify_error(&e),
        };
        let result = self.drop_links_from_other_repos(key);
        self.report_error(result);
        let new = Key::new(to, id);
        if self.in_scope(to) {
            self.open_entry(new);
        } else {
            self.keep_selection();
        }
        self.notify(format!("moved {from} to {} as #{id}", self.brain_name(to)));
    }

    /// Opens the text editor on a description or comment of an existing entry, or a project's card.
    pub fn open_editor(&mut self, target: Target) {
        self.open_editor_from(target, None);
    }

    /// Opens the editor on `target` from Manage project on `back`, which closing it goes back to.
    pub fn open_editor_from(&mut self, target: Target, back: Option<usize>) {
        let Some(text) = self.text_of(target) else { return };
        let (editor, base) = (TextEditor::new(text), text.to_string());
        let comments = match target {
            Target::Comment(key, _) => self.task(key).map(|t| t.comments.clone()).unwrap_or_default(),
            _ => Vec::new(),
        };
        let decision = match target {
            Target::Reject(key) | Target::Accept(key) => self.task(key).map(|t| t.decision),
            _ => None,
        };
        let status = target.key().and_then(|key| self.task(key)).map(|t| t.status);
        self.mode = Mode::Edit(Edit {
            editor,
            target,
            back,
            base,
            comments,
            decision,
            status,
            changed_on_disk: false,
            merge: None,
            note: String::new(),
        });
        // A new comment crosses nothing the agent writes: it's how to tell it.
        if let Target::Description(key) | Target::Comment(key, _) = target {
            self.say_agent_working(key);
        }
    }

    /// Says, on opening the form or an editor on a task an agent has in doing, that it's being worked on: what's
    /// saved may cross what the agent writes next. It only warns, since the user may be correcting the agent.
    pub(super) fn say_agent_working(&mut self, key: Key) {
        if self.agent_working(key) {
            self.notify(format!("{} is being worked on by an agent", self.entry_name(key)));
        }
    }

    /// Whether `key` is a task an agent has in doing.
    pub(super) fn agent_working(&self, key: Key) -> bool {
        self.task(key).is_some_and(|t| t.doing_by(Author::Agent))
    }

    /// Whether a save on `key` asks [`Tell`]'s question first: an agent is working on it, and it wasn't answered.
    pub(super) fn asks_to_tell(&self, key: Key, telling: &Telling) -> bool {
        *telling == Telling::Ask && self.agent_working(key)
    }

    /// Asks [`Tell`]'s question about the save `pending`, which waits for the answer.
    pub(super) fn ask_to_tell(&mut self, pending: Pending) {
        let note = LineInput::new(pending.note());
        self.mode = Mode::Tell(Box::new(Tell { pending, note }));
    }

    /// Saves the form or editor [`Tell`] holds, telling the agent unless `just_save`.
    pub(super) fn answer_tell(&mut self, tell: Tell, just_save: bool) {
        let telling = if just_save { Telling::Save } else { Telling::Tell(tell.note.text().trim().to_string()) };
        match tell.pending {
            Pending::Form(form) => self.submit_form(form, telling),
            Pending::Edit(edit) => {
                let back = edit.back;
                self.save_edit(*edit, telling);
                self.back_to_manage(back);
            }
        }
    }

    /// Saves without telling the agent, from [`Tell`]'s button.
    pub fn just_save(&mut self) {
        if let Mode::Tell(tell) = std::mem::replace(&mut self.mode, Mode::Normal) {
            self.answer_tell(*tell, true);
        }
    }

    /// Tells the agent working on `key` what the user changed (`changed`, as "the title and tags"), and `note`, as the
    /// user's comment: what `kb hook tool` brings to it with its next step. Adds to `said`, the save's message.
    pub(super) fn tell_agent(&mut self, key: Key, changed: &str, note: &str, said: &str) {
        // Finished or parked while the question was open: nobody would read it before the task is picked up again.
        if !self.agent_working(key) {
            return self.notify(format!("{said}; not told: no agent is working on it now"));
        }
        let comment =
            if note.is_empty() { format!("Changed {changed}.") } else { format!("Changed {changed}: {note}") };
        match self.brain_mut(key.src).add_comment(key.id, &comment) {
            Ok(_) => self.notify(format!("{said}; the agent is told")),
            Err(e) => self.notify(format!("{said}, but the agent isn't told: {e:#}")),
        }
    }

    /// Whether the comment at the editor's index is still the one it opened on: the others are as they were, but
    /// for any written after them, and the one there was written when it was, by the same author. Anything but a
    /// comment is.
    pub(super) fn same_comment(&self, edit: &Edit) -> bool {
        fn others(comments: &[Comment], index: usize) -> impl Iterator<Item = &Comment> {
            comments.iter().enumerate().filter(move |&(at, _)| at != index).map(|(_, comment)| comment)
        }
        let Target::Comment(key, index) = edit.target else { return true };
        let (Some(task), Some(then)) = (self.task(key), edit.comments.get(index)) else { return false };
        task.comments.get(index).is_some_and(|now| now.created == then.created && now.author == then.author)
            && others(&edit.comments, index).eq(others(&task.comments, index).take(edit.comments.len() - 1))
    }

    /// What `kb` changed, since the editor opened, of what it edits: its text (or which comment is at its index), the
    /// entry's status and a decision's verdict being given, said for the status bar; `None` if nothing.
    pub(super) fn edit_changed_on_disk(&self, edit: &Edit) -> Option<String> {
        self.edit_changed_but(edit, "")
    }

    /// What [`edit_changed_on_disk`](Self::edit_changed_on_disk) says but for `field`, which a save that stops over
    /// it is about to say.
    fn edit_changed_but(&self, edit: &Edit, field: &str) -> Option<String> {
        // Another comment edited isn't what it edits, unlike for a save (`same_comment`): one put at its index is.
        let moved = match edit.target {
            Target::Comment(key, index) => {
                let at = |comments: &[Comment]| comments.get(index).map(|c| (c.created, c.author));
                self.task(key).map(|t| at(&t.comments)) != Some(at(&edit.comments))
            }
            _ => false,
        };
        let word = text_word(edit.target);
        let text = word != field && (moved || self.text_of(edit.target) != Some(edit.base.as_str()));
        let Some(key) = edit.target.key() else {
            return text.then(|| format!("{} changed on disk", self.text_name(edit.target)));
        };
        let status = field != "status" && self.task(key).map(|t| t.status) != edit.status;
        // Only a verdict's editor has one, which its save gives.
        let verdict =
            field != "verdict" && edit.decision.is_some() && self.task(key).map(|t| t.decision) != edit.decision;
        changed_on_disk(&self.entry_name(key), &[(text, word), (status, "status"), (verdict, "verdict")])
    }

    /// What the editor's text is, to say what changed meanwhile.
    pub(super) fn text_name(&self, target: Target) -> String {
        match target {
            Target::Description(key) => format!("{}'s description", self.entry_name(key)),
            Target::NewComment(key) | Target::Comment(key, _) | Target::Reject(key) | Target::Accept(key) => {
                format!("{}'s comment", self.entry_name(key))
            }
            Target::Project(_) => "the project's description".into(),
            Target::RepoNotes(_) => "this repository's role".into(),
        }
    }

    /// Whether the editor holds a change of the user's: its text differs from the one it started from (`Edit::base`)
    /// and from what its target holds. Typed and put back as it was is no change, to save, to say was saved, or to
    /// mark in its title, even once `kb` changed the target meanwhile.
    pub fn changed(&self, edit: &Edit) -> bool {
        let text = edit.editor.text();
        text != edit.base && self.text_of(edit.target) != Some(text.as_str())
    }

    /// What `target` holds now: the text the editor starts from, and saving the same is no change.
    pub(super) fn text_of(&self, target: Target) -> Option<&str> {
        match target {
            Target::Description(key) => self.task(key).map(|t| t.description.as_str()),
            Target::NewComment(key) | Target::Reject(key) | Target::Accept(key) => self.task(key).map(|_| ""),
            Target::Comment(key, index) => {
                self.task(key).map(|t| t.comments.get(index).map_or("", |c| c.body.as_str()))
            }
            Target::Project(src) => self.brain(src).project.as_ref().map(|p| p.description.as_str()),
            Target::RepoNotes(src) => self.brain(src).project.as_ref().map(|p| p.notes.as_str()),
        }
    }

    /// Writes the editor's text to its target, if it changed.
    pub(super) fn save_edit(&mut self, mut edit: Edit, telling: Telling) {
        let answers = self.answers(edit.target);
        let text = edit.editor.text();
        // A comment or a reason is something said: closing with none says so, rather than closing as if it was
        // sent, since an answer that was never written leaves the agent waiting.
        if matches!(edit.target, Target::NewComment(_) | Target::Reject(_) | Target::Accept(_))
            && text.trim().is_empty()
        {
            return match edit.target {
                Target::Reject(_) => self.notify("not rejected: say why it's rejected"),
                Target::Accept(_) => self.notify("not accepted: write the comment, or accept without one (y)"),
                _ if answers => self.notify("nothing answered: the question still waits"),
                // Only blank text left typed; typed and taken back again is no comment tried. The text is trimmed, so
                // it's the lines that tell them apart.
                _ if edit.editor.lines().iter().any(|line| !line.is_empty()) => {
                    self.notify("nothing added: the comment was empty");
                }
                _ => {}
            };
        }
        if !self.changed(&edit) {
            return;
        }
        // A save that stops, after the agent's question was answered, asks it again with the note typed.
        if let Telling::Tell(note) = &telling {
            edit.note.clone_from(note);
        }
        // Another comment put at this one's place isn't saved over: the save stops for good.
        if let Target::Comment(key, _) = edit.target
            && !self.same_comment(&edit)
        {
            let name = self.entry_name(key);
            self.unsay_found(self.edit_changed_on_disk(&edit), self.edit_changed_but(&edit, text_word(edit.target)));
            self.notify(format!("{name}'s comments changed around this one meanwhile: copy your text, then ctrl+d"));
            self.mode = Mode::Edit(edit);
            return;
        }
        // A conflict stops the save, the editor put back to pick in it.
        let Some((text, merged)) = self.merge_meanwhile(&mut edit, text) else {
            if edit.merge.is_some() {
                self.mode = Mode::Edit(edit);
            }
            return;
        };
        // Said before the verdict it stops over is taken in.
        let found = (self.edit_changed_on_disk(&edit), self.edit_changed_but(&edit, "verdict"));
        if let Some(stop) = self.verdict_meanwhile(&mut edit) {
            self.unsay_found(found.0, found.1);
            self.notify(stop);
            self.mode = Mode::Edit(edit);
            return;
        }
        // What the agent working on the entry is told, if it's one it may be writing too.
        let told = match edit.target {
            Target::Description(key) => Some((key, "the description")),
            Target::Comment(key, _) => Some((key, "a comment")),
            Target::NewComment(_)
            | Target::Reject(_)
            | Target::Accept(_)
            | Target::Project(_)
            | Target::RepoNotes(_) => None,
        };
        if told.is_some_and(|(key, _)| self.asks_to_tell(key, &telling)) {
            return self.ask_to_tell(Pending::Edit(Box::new(edit)));
        }
        let saved = match edit.target {
            Target::Description(key) => {
                self.brain_mut(key.src).set_description(key.id, &text).map(|d| (d, "description saved"))
            }
            Target::NewComment(key) => {
                let said = if answers { "answer sent: the agent picks it up" } else { "comment added" };
                self.brain_mut(key.src).add_comment(key.id, &text).map(|d| (d, said))
            }
            Target::Comment(key, index) => {
                self.brain_mut(key.src).update_comment(key.id, index, &text).map(|d| (d, "comment saved"))
            }
            Target::Reject(key) => {
                self.brain_mut(key.src).resolve(key.id, DecisionState::Rejected, &text).map(|_| (false, "rejected"))
            }
            Target::Accept(key) => {
                self.brain_mut(key.src).resolve(key.id, DecisionState::Accepted, &text).map(|_| (false, "accepted"))
            }
            Target::Project(src) => {
                self.save_project_description(src, &text).map(|()| (false, "project description saved"))
            }
            Target::RepoNotes(src) => {
                self.brain_mut(src).set_repo_notes(&text).map(|()| (false, "repository role saved"))
            }
        };
        // A verdict names its decision, as `y` does: "shop/api #11 accepted".
        let what = |what: &str| match edit.target {
            Target::Accept(key) | Target::Reject(key) => format!("{} {what}", self.entry_name(key)),
            _ => what.to_string(),
        };
        let said = match saved {
            Ok((true, said)) => format!("{} (headings were made smaller)", what(said)),
            Ok((false, said)) => what(said),
            Err(e) => return self.notify_error(&e),
        };
        let said = if merged { format!("{said}, merged with the change made meanwhile") } else { said };
        match (told, telling) {
            (Some((key, changed)), Telling::Tell(note)) => self.tell_agent(key, changed, &note, &said),
            _ => self.notify(said),
        }
    }

    /// `text`, the editor's, merged with what `kb` wrote to its target since the editor opened (`Edit::base`): lines
    /// only one side changed take its change. Returns the text to save, and whether it merged anything; `None` if
    /// there's nothing to save, or, with `edit`'s `merge` set, if the save stops over lines both changed, for the user
    /// to pick.
    fn merge_meanwhile(&mut self, edit: &mut Edit, text: String) -> Option<(String, bool)> {
        let Some(now) = self.text_of(edit.target).filter(|&now| now != edit.base).map(str::to_string) else {
            return Some((text, false));
        };
        // What the read found of the text is what this says now, merged or stopped over.
        let found = self.edit_changed_but(edit, text_word(edit.target));
        self.unsay_found(self.edit_changed_on_disk(edit), found);
        match merge(&edit.base, &text, &now) {
            Merged::Clean(both) if both == now => {
                let name = self.text_name(edit.target);
                self.notify(format!("nothing to save: {name} has your changes already"));
                None
            }
            Merged::Clean(both) => Some((both, true)),
            Merged::Conflicted(conflicts) => {
                self.notify(format!(
                    "{} changed meanwhile: pick yours or theirs where you both changed it",
                    self.text_name(edit.target)
                ));
                edit.merge = Some(conflicts);
                None
            }
        }
    }

    /// For a verdict's editor, why its save stops: `kb` decided meanwhile, which a save doesn't overturn unseen.
    /// Where the decision stands is kept in `edit`, so a change after this stops the next save again.
    fn verdict_meanwhile(&self, edit: &mut Edit) -> Option<String> {
        let (key, verb, verdict, word) = match edit.target {
            Target::Reject(key) => (key, "reject", DecisionState::Rejected, "reason"),
            Target::Accept(key) => (key, "accept", DecisionState::Accepted, "comment"),
            Target::Description(_)
            | Target::NewComment(_)
            | Target::Comment(..)
            | Target::Project(_)
            | Target::RepoNotes(_) => return None,
        };
        let now = self.task(key)?.decision;
        if Some(now) == edit.decision && now != verdict {
            return None;
        }
        edit.decision = Some(now);
        let was = match now {
            DecisionState::Proposed => "proposed again",
            now => now.as_str(),
        };
        let name = self.entry_name(key);
        // Given the verdict the user was writing, `resolve` does nothing: the text would be lost while saying it's done.
        Some(if now == verdict {
            format!("{name} was {was} meanwhile: copy your {word}, then ctrl+d")
        } else {
            format!("{name} was {was} meanwhile: save again to {verb} it anyway, ctrl+d to leave it")
        })
    }

    /// Edits the description of the project shown, else the selected entry's, which its repositories share.
    pub fn edit_project_description(&mut self) {
        if let Some(src) = self.project_to_act_on() {
            self.open_editor(Target::Project(src));
        }
    }

    /// Lists the selected entry's comments to edit or delete one, the latest highlighted.
    pub fn open_comments(&mut self) {
        let Some(key) = self.selected else { return };
        match self.task(key).map(|t| t.comments.len().checked_sub(1)) {
            Some(Some(last)) => self.mode = Mode::Comments(CommentPicker { key, sel: last }),
            // Rather than nothing, which reads as a key that doesn't work.
            Some(None) => self.notify("no comments yet: c adds one"),
            None => {}
        }
    }

    /// Opens the editor on the role of [`Board::repo_here`], or says how to get one.
    pub fn edit_repo_notes(&mut self) {
        match self.repo_here() {
            Some(src) => self.open_editor(Target::RepoNotes(src)),
            None => self.notify("no repository of a project here: select one of its cards, or show it with P"),
        }
    }

    /// Asks to install the newer release, unless there's none or an agent is working on a task: an update
    /// rewrites agents' instructions and may migrate their data, which mustn't change mid-session.
    pub fn ask_update(&mut self) {
        let Some(version) = &self.available else {
            self.notify(format!("no newer release than {} found", env!("CARGO_PKG_VERSION")));
            return;
        };
        let version = version.clone();
        match update::check_no_agent_working(&self.central) {
            Ok(()) => self.mode = Mode::Confirm(Confirm::Update),
            Err(e) => self.notify(format!("can't update to {version} yet: {e:#}")),
        }
    }

    /// Runs a confirmed delete, or asks the window for the update.
    pub(super) fn confirm(&mut self, confirm: Confirm) {
        let result = match confirm {
            Confirm::Delete(key) => {
                let name = self.entry_name(key);
                self.brain_mut(key.src)
                    .delete(key.id)
                    .and_then(|()| self.drop_links_from_other_repos(key))
                    .map(|()| format!("deleted {name}"))
            }
            Confirm::DeleteComment(key, comment) => {
                // Two the same are one to delete as much as the other.
                match self.task(key).and_then(|t| t.comments.iter().position(|c| *c == comment)) {
                    Some(index) => {
                        self.brain_mut(key.src).delete_comment(key.id, index).map(|()| "comment deleted".into())
                    }
                    None => {
                        Ok(format!("not deleted: {}'s comment was edited or deleted meanwhile", self.entry_name(key)))
                    }
                }
            }
            Confirm::Update => {
                self.request = Some(Request::Update);
                return;
            }
        };
        match result {
            Ok(msg) => self.notify(msg),
            Err(e) => self.notify_error(&e),
        }
        self.keep_selection();
    }

    /// Asks whether to delete comment `index` of `key`'s entry, as it is now.
    pub fn ask_delete_comment(&mut self, key: Key, index: usize) {
        if let Some(comment) = self.task(key).and_then(|t| t.comments.get(index)) {
            self.mode = Mode::Confirm(Confirm::DeleteComment(key, comment.clone()));
        }
    }

    /// Opens the link picker for the selected entry.
    pub fn open_links(&mut self) {
        if let Some(key) = self.selected {
            self.mode = Mode::Links(LinkPicker { key, rows: Picker::default() });
        }
    }

    /// Links or unlinks the picker's entry and the candidate at `index`.
    pub fn toggle_link(&mut self, index: usize) {
        let Mode::Links(picker) = &self.mode else { return };
        let key = picker.key;
        let Some(other) = self.link_candidates(picker).get(index).map(|&(k, _)| k) else { return };
        if let Mode::Links(picker) = &mut self.mode {
            picker.rows.sel = index;
        }
        let name = self.link_name(key.src, other);
        match self.toggle_link_between(key, other) {
            Ok(true) => self.notify(format!("linked {name}")),
            Ok(false) => self.notify(format!("unlinked {name}")),
            Err(e) => self.notify_error(&e),
        }
        // Linked entries list first, so the row moves: the highlight goes with it rather than onto another.
        let Mode::Links(picker) = &self.mode else { return };
        if let Some(at) = self.link_candidates(picker).iter().position(|&(k, _)| k == other)
            && let Mode::Links(picker) = &mut self.mode
        {
            picker.rows.sel = at;
        }
    }

    /// Asks the window to open the selected entry's file in `$EDITOR`.
    pub fn open_file(&mut self) {
        self.request = self.selected_task().and_then(|t| t.path.clone()).map(Request::Edit);
    }

    /// Re-reads every file and says so.
    pub fn reload_from_disk(&mut self) {
        self.message = None;
        // Asked for, it says again what's still skipped.
        self.warned.clear();
        let result = self.reload();
        self.report_error(result);
        if self.message.is_none() {
            self.notify("reloaded from disk");
        }
    }

    /// A click on an entry's card or row: one selects it, two (a double-click) open its details.
    pub fn click_entry(&mut self, key: Key, clicks: usize) {
        if clicks >= 2 {
            self.open_entry(key);
        } else {
            self.select(key);
        }
    }

    /// Runs `act`, recording the files it writes in every brain, for `u` to put back. Input that writes
    /// nothing, like typing or moving the selection, leaves the last change to undo as it was.
    pub fn recorded(&mut self, act: impl FnOnce(&mut Self)) {
        for source in &self.sources {
            source.brain.store.record();
        }
        act(self);
        let changes: Vec<_> = self.sources.iter().flat_map(|s| s.brain.store.recorded()).collect();
        if !changes.is_empty() {
            self.last_change = changes;
            self.next_undo = NextUndo::Undo;
        }
    }

    /// Puts the files the last change wrote back as they were, and re-reads them; undoing again redoes
    /// it. Refused if another program changed one of them since: that would be lost.
    pub fn undo(&mut self) {
        if self.last_change.is_empty() {
            self.notify("nothing to undo");
            return;
        }
        let changes = std::mem::take(&mut self.last_change);
        match store::undo(&changes) {
            Ok(()) => {
                let result = self.reload();
                self.report_error(result);
                let (said, next) = match self.next_undo {
                    NextUndo::Undo => ("undone; u again redoes", NextUndo::Redo),
                    NextUndo::Redo => ("redone; u again undoes", NextUndo::Undo),
                };
                self.notify(said);
                self.next_undo = next;
                self.last_change =
                    changes.into_iter().map(|c| Change { before: c.after, after: c.before, ..c }).collect();
            }
            Err(e) => {
                self.notify_error(&e);
                self.last_change = changes;
            }
        }
    }

    /// Parks every running task you started, in every brain, and closes the window, for the end of the day.
    /// Agents' tasks keep running: they may still be at work.
    pub fn park_and_quit(&mut self) {
        for src in 0..self.sources.len() {
            let result = self.brain_mut(src).park_people_running();
            self.report_error(result);
        }
        self.quit = true;
    }

    /// Writes the project's shared description, then re-reads the project's other repositories,
    /// which show it too.
    fn save_project_description(&mut self, src: usize, text: &str) -> Result<()> {
        self.brain_mut(src).set_project_description(text)?;
        let project = self.sources[src].project.clone();
        for source in self.sources.iter_mut().filter(|s| s.project == project) {
            source.brain.reload()?;
        }
        Ok(())
    }

    /// Writes the report tab's report to `reports/` as markdown.
    pub fn export_report(&mut self) {
        match self.write_report() {
            Ok(path) => self.notify(format!("saved {}", path.display())),
            Err(e) => self.notify_error(&e),
        }
    }
}

/// The text `target` edits, as what changed on disk names it.
fn text_word(target: Target) -> &'static str {
    if matches!(target, Target::Description(_)) { "description" } else { "comment" }
}
