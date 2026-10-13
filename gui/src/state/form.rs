//! The new / edit form and the quick-add field: what typing in them and saving them changes.

use super::{Board, Field, Form, FormTarget, Key, Mode, QuickAdd, Tab, WhereFor};
use crate::text::editor::LineInput;
use anyhow::Result;
use kanbrain_core::brain::NewEntry;
use kanbrain_core::model::{Kind, Status, Task, parse_tags};

impl Board {
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
            form.title_missed = true;
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
        self.link_options(form.src(), own, form.related.search.text(), &form.links)
    }

    /// Checks or unchecks the related field's option at `index`.
    pub fn toggle_form_link(&mut self, index: usize) {
        let Mode::Form(form) = &self.mode else { return };
        let Some(other) = self.form_link_options(form).get(index).map(|&(key, _)| key) else { return };
        let Mode::Form(form) = &mut self.mode else { return };
        form.related.sel = index;
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
}
