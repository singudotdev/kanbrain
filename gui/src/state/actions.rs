//! Changes to entries, through the brain: each one says in the status bar what it did, or why not.

use super::{
    Board, Confirm, Edit, Form, Key, LinkPicker, Mode, Picker, QuickAdd, Request, Tab, Target, WhereFor, WherePicker,
};
use crate::text::editor::{LineInput, TextEditor};
use anyhow::Result;
use chrono::{NaiveDate, NaiveTime};
use kanbrain_core::cli::update;
use kanbrain_core::model::{DecisionState, Kind, Status};
use kanbrain_core::period::{Period, Unit};
use kanbrain_core::store::{self, Change};

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
        match self.brain_mut(key.src).set_status(key.id, status) {
            Ok(true) => {
                let title = self.task(key).map(|t| t.title.clone()).unwrap_or_default();
                self.notify(format!("#{} {title} → {}", key.id, status.as_str()));
            }
            Ok(false) => self.notify(format!("#{} is already {}", key.id, status.as_str())),
            Err(e) => self.notify_error(&e),
        }
        self.keep_selection();
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
        self.notify(format!("moved #{} to {} as #{id}", key.id, self.brain_name(to)));
    }

    /// Opens the text editor on a description or comment of an existing entry, or a project's card.
    pub fn open_editor(&mut self, target: Target) {
        let text = match target {
            Target::Description(key) => self.task(key).map(|t| t.description.as_str()),
            Target::NewComment(key) | Target::Reject(key) | Target::Accept(key) => self.task(key).map(|_| ""),
            Target::Comment(key, index) => {
                self.task(key).map(|t| t.comments.get(index).map_or("", |c| c.body.as_str()))
            }
            Target::Project(src) => self.brain(src).project.as_ref().map(|p| p.description.as_str()),
            Target::RepoNotes(src) => self.brain(src).project.as_ref().map(|p| p.notes.as_str()),
        };
        let Some(text) = text else { return };
        self.mode = Mode::Edit(Edit { editor: TextEditor::new(text), target });
    }

    /// Writes the editor's text to its target, if it changed.
    pub(super) fn save_edit(&mut self, edit: &Edit) {
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
                _ if edit.editor.modified => self.notify("nothing added: the comment was empty"),
                _ => {}
            };
        }
        if !edit.editor.modified {
            return;
        }
        let saved = match edit.target {
            Target::Description(key) => {
                self.brain_mut(key.src).set_description(key.id, &text).map(|d| (d, "description saved"))
            }
            Target::NewComment(key) => self.brain_mut(key.src).add_comment(key.id, &text).map(|d| (d, "comment added")),
            Target::Comment(key, index) => {
                self.brain_mut(key.src).update_comment(key.id, index, &text).map(|d| (d, "comment saved"))
            }
            Target::Reject(key) => self
                .brain_mut(key.src)
                .resolve(key.id, DecisionState::Rejected, &text)
                .map(|_| (false, "decision rejected")),
            Target::Accept(key) => self
                .brain_mut(key.src)
                .resolve(key.id, DecisionState::Accepted, &text)
                .map(|_| (false, "decision accepted")),
            Target::Project(src) => {
                self.save_project_description(src, &text).map(|()| (false, "project description saved"))
            }
            Target::RepoNotes(src) => {
                self.brain_mut(src).set_repo_notes(&text).map(|()| (false, "repository role saved"))
            }
        };
        match saved {
            Ok((true, what)) => self.notify(format!("{what} (headings were made smaller)")),
            Ok((false, what)) => self.notify(what),
            Err(e) => self.notify_error(&e),
        }
    }

    /// Edits the description of the project shown, else the selected entry's, which its repositories share.
    pub fn edit_project_description(&mut self) {
        if let Some(src) = self.project_to_act_on() {
            self.open_editor(Target::Project(src));
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
            Confirm::Delete(key) => self
                .brain_mut(key.src)
                .delete(key.id)
                .and_then(|()| self.drop_links_from_other_repos(key))
                .map(|()| format!("deleted #{}", key.id)),
            Confirm::DeleteComment(key, index) => {
                self.brain_mut(key.src).delete_comment(key.id, index).map(|()| "comment deleted".into())
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
    }

    /// Asks the window to open the selected entry's file in `$EDITOR`.
    pub fn open_file(&mut self) {
        self.request = self.selected_task().and_then(|t| t.path.clone()).map(Request::Edit);
    }

    /// Re-reads every file and says so.
    pub fn reload_from_disk(&mut self) {
        self.message = None;
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
                self.notify("undone; u again redoes");
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
