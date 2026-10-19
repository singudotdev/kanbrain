//! The new / edit form and the quick-add field: what typing in them and saving them changes.

use super::{
    Board, Changed, Field, Form, FormTarget, Key, Mode, Pending, QuickAdd, Tab, Telling, WhereFor, changed_on_disk,
    listed,
};
use crate::text::editor::LineInput;
use anyhow::Result;
use kanbrain_core::brain::NewEntry;
use kanbrain_core::model::{Kind, Status, Task};
use std::collections::BTreeMap;

impl Board {
    /// The kind of entry the form makes or changes, and the tags entries of that kind already use in
    /// its brain, each with how many use it, to pick from: a task's form offers task tags, not those of notes or
    /// decisions. Those the field holds as chips are left out, and while one is typed, only those containing it
    /// ([`Form::typed_tag`]), as the dropdowns filter.
    pub fn form_tags(&self, form: &Form) -> (Kind, Vec<(String, usize)>) {
        let (src, kind) = match form.target {
            FormTarget::New(src, kind, _) => (src, kind),
            FormTarget::Edit(key) => (key.src, self.task(key).map_or(Kind::Task, |t| t.kind)),
        };
        let typed = form.typed_tag();
        let mut uses = BTreeMap::new();
        for tag in self.brain(src).tasks.iter().filter(|t| t.kind == kind).flat_map(|t| &t.tags) {
            *uses.entry(tag.as_str()).or_insert(0) += 1;
        }
        let tags = uses
            .into_iter()
            .filter(|&(tag, _)| tag.contains(typed.as_str()) && !form.tags.iter().any(|held| held == tag))
            .map(|(tag, n)| (tag.to_string(), n))
            .collect();
        (kind, tags)
    }

    /// Puts `tag`, clicked in the tags field's list, in place of the tag being typed. By name: catching up before the
    /// click acts may move its row, or take it out of the list, and then it's left alone.
    pub fn choose_form_tag(&mut self, tag: &str) {
        let Mode::Form(form) = &self.mode else { return };
        if !self.form_tags(form).1.iter().any(|(listed, _)| listed == tag) {
            return;
        }
        let Mode::Form(form) = &mut self.mode else { return };
        form.put_tag(tag);
    }

    /// What `kb` changed, since the edit form opened, of its entry's title, tags, related entries and status, said for
    /// the status bar; `None` if nothing, or on a new entry's form.
    pub(super) fn form_changed_on_disk(&self, form: &Form) -> Option<String> {
        self.form_changed_but(form, None)
    }

    /// What [`form_changed_on_disk`](Self::form_changed_on_disk) says but for `field`, which a save that stops over
    /// it is about to say.
    fn form_changed_but(&self, form: &Form, but: Option<Changed>) -> Option<String> {
        let (FormTarget::Edit(key), Some(opened)) = (form.target, &form.opened) else { return None };
        let now = self.task(key)?;
        let related = self.related_keys(key);
        let relinked = related.len() != opened.links.len() || related.iter().any(|k| !opened.links.contains(k));
        let fields = [
            (now.title != opened.title, Changed::Title),
            (now.tags != opened.tags, Changed::Tags),
            (relinked, Changed::Related),
            (now.status != opened.status, Changed::Status),
        ];
        changed_on_disk(&self.entry_name(key), &fields.map(|(changed, field)| (changed && Some(field) != but, field)))
    }

    /// Opens the form for changing the selected entry's title and tags.
    pub fn edit_selected(&mut self) {
        if let (Some(key), Some(task)) = (self.selected, self.selected_task()) {
            let related = self.related_keys(key);
            self.mode = Mode::Form(Box::new(Form::edit(key, task, related)));
            self.say_agent_working(key);
        }
    }

    /// Saves the form: creates the entry and selects it, or changes the one being edited.
    /// An empty title keeps the form open.
    pub(super) fn submit_form(&mut self, mut form: Box<Form>, telling: Telling) {
        let title = form.title.text().trim().to_string();
        if title.is_empty() {
            self.notify("the title is empty");
            form.field = Field::Title;
            form.title_missed = true;
            self.mode = Mode::Form(form);
            return;
        }
        let tags = form.tags_to_save();
        match form.target {
            FormTarget::New(src, kind, status) => self.create(src, kind, title, tags, form.links, status),
            FormTarget::Edit(key) => {
                // A save that stops, after the agent's question was answered, asks it again with the note typed.
                if let Telling::Tell(note) = &telling {
                    form.note.clone_from(note);
                }
                // A title `kb` changed meanwhile too isn't replaced unseen: the save stops, the form open, and shows
                // what it is now, to take (`ctrl+t`); saved again with no newer one, the form's replaces it.
                let renamed = match (&form.opened, self.task(key)) {
                    (Some(opened), Some(now))
                        if title != opened.title && now.title != opened.title && title != now.title =>
                    {
                        Some(now.title.clone())
                    }
                    _ => None,
                };
                if let Some(now) = renamed {
                    let name = self.entry_name(key);
                    self.unsay_found(
                        self.form_changed_on_disk(&form),
                        self.form_changed_but(&form, Some(Changed::Title)),
                    );
                    self.notify(format!(
                        "{name}'s title changed meanwhile, to “{now}”: save again to keep yours, ctrl+t to take it"
                    ));
                    if let Some(opened) = &mut form.opened {
                        opened.title.clone_from(&now);
                    }
                    form.theirs_title = Some(now);
                    self.mode = Mode::Form(form);
                    return;
                }
                let related = self.related_keys(key);
                // Only what was changed in the form, over the entry as it is now: `kb` may have changed it since.
                let (title, tags, links) = match (&form.opened, self.task(key)) {
                    (Some(opened), Some(now)) => (
                        if title == opened.title { now.title.clone() } else { title },
                        merged(&opened.tags, &tags, &now.tags),
                        merged(&opened.links, &form.links, &related),
                    ),
                    _ => (title, tags, form.links.clone()),
                };
                // Saved as it was, it's no change, as in the editors: nothing written, nothing said.
                let same_links = related.len() == links.len() && related.iter().all(|k| links.contains(k));
                let (same_title, same_tags) =
                    self.task(key).map_or((false, false), |t| (t.title == title, t.tags == tags));
                if same_title && same_tags && same_links {
                    return;
                }
                if self.asks_to_tell(key, &telling) {
                    return self.ask_to_tell(Pending::Form(form));
                }
                let result = self.brain_mut(key.src).update(key.id, &title, tags);
                let saved = result.is_ok();
                self.report_error(result);
                let result = self.relate_only(key, &links);
                let related = result.is_ok();
                self.report_error(result);
                // As every other edit says it was saved.
                if saved && related {
                    let said = format!("{} saved", self.entry_name(key));
                    let fields =
                        [(!same_title, Changed::Title), (!same_tags, Changed::Tags), (!same_links, Changed::Related)];
                    match (telling, listed(&fields)) {
                        (Telling::Tell(note), Some(changed)) => {
                            self.tell_agent(key, &format!("the {changed}"), &note, &said);
                        }
                        _ => self.notify(said),
                    }
                }
            }
        }
    }

    /// Puts in the title field the title `kb` gave the entry while the form changed it too, which saving then leaves
    /// as it is.
    pub fn take_theirs_title(&mut self) {
        let Mode::Form(form) = &self.mode else { return };
        let Some(title) = form.theirs_title.as_ref().and(form.target.key()).and_then(|key| self.task(key)) else {
            return;
        };
        // As it is now, which a reload since the save stopped shows too: `kb` may have changed it again.
        let title = title.title.clone();
        let Mode::Form(form) = &mut self.mode else { return };
        form.title = LineInput::new(&title);
        if let Some(opened) = &mut form.opened {
            opened.title = title;
        }
        form.theirs_title = None;
        form.go_to(Field::Title);
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
        self.link_options(form.src(), form.target.key(), form.related.search.text(), &form.links)
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

    /// Takes `tag` off the form's entry, from the × on its chip.
    pub fn untag_in_form(&mut self, tag: &str) {
        if let Mode::Form(form) = &mut self.mode {
            form.tags.retain(|held| held != tag);
        }
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
            self.submit_form(form, Telling::Ask);
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
        // A column the task can't start in (one of yours already runs there) leaves it in To do, and says why.
        let refused = (kind == Kind::Task && status != Status::Todo)
            .then(|| self.brain_mut(src).set_status(id, status).err())
            .flatten();
        self.open_entry(key);
        self.drawer = false;
        let name = self.entry_name(key);
        self.notify(match refused {
            Some(e) => format!("created {name}, in To do: {e:#}"),
            None => format!("created {name}"),
        });
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

/// A list the form edited, merged with the changes made to it elsewhere since the form `opened` it: the form's
/// `edited` list, as edited (its order too), without what was taken out elsewhere, and with what was added elsewhere
/// after it. Unedited, it's the list as it is `now`.
fn merged<T: Clone + PartialEq>(opened: &[T], edited: &[T], now: &[T]) -> Vec<T> {
    if edited == opened {
        return now.to_vec();
    }
    let kept = edited.iter().filter(|item| !opened.contains(item) || now.contains(item));
    let added = now.iter().filter(|item| !opened.contains(item) && !edited.contains(item));
    kept.chain(added).cloned().collect()
}
