//! Changes to entries, through the brain: each one says in the status bar what it did, or why not.

use super::{
    Board, Confirm, Edit, Field, Form, FormTarget, Key, LinkPicker, Mode, Picker, QuickAdd, Request, Tab, Target,
    WhereFor, WherePicker,
};
use crate::text::editor::{LineInput, TextEditor};
use anyhow::Result;
use chrono::{NaiveDate, NaiveTime};
use kanbrain_core::brain::NewEntry;
use kanbrain_core::cli::update;
use kanbrain_core::model::{Asked, DecisionState, Kind, Status, Task, parse_tags};
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
    fn ask_where(&mut self, then: WhereFor) {
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

    /// The kind of entry the form makes or changes, and the tags entries of that kind already use in
    /// its brain, to reuse: a task's form suggests task tags, not those of notes or decisions.
    pub fn form_tags(&self, form: &Form) -> (Kind, Vec<String>) {
        let (src, kind) = match form.target {
            FormTarget::New(src, kind, _) => (src, kind),
            FormTarget::Edit(key) => (key.src, self.task(key).map_or(Kind::Task, |t| t.kind)),
        };
        let mut tags: Vec<String> =
            self.brain(src).tasks.iter().filter(|t| t.kind == kind).flat_map(|t| t.tags.iter().cloned()).collect();
        tags.sort();
        tags.dedup();
        (kind, tags)
    }

    /// Opens the form for changing the selected entry's title and tags.
    pub fn edit_selected(&mut self) {
        if let (Some(key), Some(task)) = (self.selected, self.selected_task()) {
            let related = self.related_keys(key);
            self.mode = Mode::Form(Box::new(Form::edit(key, task, related)));
        }
    }

    /// Saves the form: creates the entry and selects it, or changes the one being edited.
    /// An empty title keeps the form open.
    pub(super) fn submit_form(&mut self, mut form: Box<Form>) {
        let title = form.title.text().trim().to_string();
        if title.is_empty() {
            self.notify("the title is empty");
            form.field = Field::Title;
            self.mode = Mode::Form(form);
            return;
        }
        let tags = parse_tags(form.tags.text());
        match form.target {
            FormTarget::New(src, kind, status) => self.create(src, kind, title, tags, form.links, status),
            FormTarget::Edit(key) => {
                let result = self.brain_mut(key.src).update(key.id, &title, tags);
                self.report_error(result);
                let result = self.relate_only(key, &form.links);
                self.report_error(result);
            }
        }
    }

    /// Makes `key`'s related entries exactly `links`: links what isn't related yet, unlinks what's
    /// no longer checked, whichever side stores the link.
    fn relate_only(&mut self, key: Key, links: &[Key]) -> Result<()> {
        let related = self.related_keys(key);
        let unchecked = related.iter().filter(|k| !links.contains(k));
        for &other in unchecked.chain(links.iter().filter(|k| !related.contains(k))) {
            self.toggle_link_between(key, other)?;
        }
        Ok(())
    }

    /// The entries the form can relate its entry to: the others of its brain and of its project's
    /// other repositories matching the related field's search (see `Task::matches`), the checked ones
    /// first, then its brain's, then the newest.
    pub fn form_link_options(&self, form: &Form) -> Vec<(Key, &Task)> {
        let own = match form.target {
            FormTarget::Edit(key) => Some(key),
            FormTarget::New(..) => None,
        };
        let src = form.src();
        let mut options: Vec<(Key, &Task)> =
            self.linkable(src).filter(|&(key, t)| Some(key) != own && t.matches(form.links_search.text())).collect();
        options.sort_by_key(|&(key, t)| (!form.links.contains(&key), key.src != src, std::cmp::Reverse(t.id)));
        options
    }

    /// Checks or unchecks the related field's option at `index`.
    pub fn toggle_form_link(&mut self, index: usize) {
        let Mode::Form(form) = &self.mode else { return };
        let Some(other) = self.form_link_options(form).get(index).map(|&(key, _)| key) else { return };
        let Mode::Form(form) = &mut self.mode else { return };
        form.links_sel = index;
        if let Some(at) = form.links.iter().position(|&l| l == other) {
            form.links.remove(at);
        } else {
            form.links.push(other);
        }
    }

    /// Puts the form's new task in `status`, from a click on the column picker.
    pub fn set_form_status(&mut self, status: Status) {
        if let Mode::Form(form) = &mut self.mode {
            form.set_status(status);
            form.field = Field::Status;
        }
    }

    /// Puts the form's new entry in the project field's option at `index`, and goes back to its title.
    pub fn choose_form_project(&mut self, index: usize) {
        let Mode::Form(form) = &self.mode else { return };
        let Some(&(src, _)) = self.form_projects(form).get(index) else { return };
        let Mode::Form(form) = &mut self.mode else { return };
        form.set_src(src);
        form.go_to(Field::Title);
    }

    /// Unchecks a related entry, from the × on its chip.
    pub fn unlink_in_form(&mut self, other: Key) {
        if let Mode::Form(form) = &mut self.mode {
            form.links.retain(|&l| l != other);
        }
    }

    /// Saves the form, from its Save button: `enter` checks a related entry while that field has
    /// the keys, so the button doesn't go through it.
    pub fn save_form(&mut self) {
        if let Mode::Form(form) = std::mem::replace(&mut self.mode, Mode::Normal) {
            self.submit_form(form);
        }
    }

    /// Creates an entry in the brain at `src`, related to `links` (there, or in the project's other
    /// repositories), puts a task in `status`, and selects it.
    fn create(&mut self, src: usize, kind: Kind, title: String, tags: Vec<String>, links: Vec<Key>, status: Status) {
        let (here, elsewhere): (Vec<Key>, Vec<Key>) = links.into_iter().partition(|k| k.src == src);
        let links = here.into_iter().map(|k| k.id).collect();
        let repo_links = elsewhere.into_iter().map(|k| self.repo_link(k)).collect();
        let new = NewEntry { kind, title, tags, links, repo_links, ..NewEntry::default() };
        let id = match self.brain_mut(src).add(new) {
            Ok(id) => id,
            Err(e) => return self.notify_error(&e),
        };
        let key = Key::new(src, id);
        if kind == Kind::Task && status != Status::Todo {
            self.set_status(key, status);
        }
        self.open_entry(key);
        self.drawer = false;
        let place = self.source_label(src).map(|l| format!(" in {l}")).unwrap_or_default();
        self.notify(format!("created #{id}{place}"));
    }

    /// Starts typing a new task at the top of a column, in the brain the scope shows (see
    /// [`Self::ask_where`]).
    pub fn quick_add(&mut self, status: Status) {
        self.show_tab(Tab::Board);
        self.ask_where(WhereFor::QuickAdd(status));
    }

    /// Creates the task typed at the top of a column, and keeps the field open for the next one.
    pub(super) fn submit_quick_add(&mut self, add: &QuickAdd) {
        let title = add.title.text().trim().to_string();
        if title.is_empty() {
            return;
        }
        self.create(add.src, Kind::Task, title, Vec::new(), Vec::new(), add.status);
        self.mode = Mode::QuickAdd(QuickAdd { src: add.src, status: add.status, title: LineInput::default() });
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

    /// Whether saving `target` answers a question the task waits on.
    pub fn answers(&self, target: Target) -> bool {
        let Target::NewComment(key) = target else { return false };
        self.task(key).is_some_and(|t| matches!(t.asked(), Some(Asked::Waiting(_))))
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
        match self.brain_mut(key.src).resolve(key.id, DecisionState::Accepted, "") {
            Ok(true) => self.notify(format!("#{} accepted", key.id)),
            Ok(false) => self.notify(format!("#{} is already accepted", key.id)),
            Err(e) => self.notify_error(&e),
        }
    }

    /// Opens the editor for a comment on accepting the selected decision, or the one the selected task waits on.
    pub fn accept_with_comment(&mut self) {
        let Some(key) = self.selected_decision() else { return };
        match self.task(key).map(|t| t.decision) {
            Some(DecisionState::Accepted) => self.notify(format!("#{} is already accepted", key.id)),
            Some(_) => self.open_editor(Target::Accept(key)),
            None => {}
        }
    }

    /// Opens the editor for why the selected decision, or the one the selected task waits on, is rejected.
    pub fn reject_selected(&mut self) {
        let Some(key) = self.selected_decision() else { return };
        match self.task(key).map(|t| t.decision) {
            Some(DecisionState::Rejected) => self.notify(format!("#{} is already rejected", key.id)),
            Some(_) => self.open_editor(Target::Reject(key)),
            None => {}
        }
    }

    /// Tasks waiting for an answer to an agent's question, in every brain shown, in board order.
    pub fn waiting(&self) -> Vec<Key> {
        let mut waiting: Vec<Key> = self
            .sources
            .iter()
            .enumerate()
            .filter(|&(src, _)| self.in_scope(src))
            .flat_map(|(src, source)| {
                source
                    .brain
                    .tasks
                    .iter()
                    .filter(|t| matches!(t.asked(), Some(Asked::Waiting(_))))
                    .map(move |t| Key::new(src, t.id))
            })
            .collect();
        waiting.sort_by_key(|k| (k.src, k.id));
        waiting
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

    /// Edits the description of the project shown, else the selected entry's, which its repositories share.
    pub fn edit_project_description(&mut self) {
        match self.project_here() {
            Some(src) => self.open_editor(Target::Project(src)),
            None => self.notify("no project here: select one of its cards, or show it with P"),
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
