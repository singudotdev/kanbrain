//! The new / edit form: its fields, and what typing in them holds until it's saved.

use super::{Key, Picker};
use crate::text::editor::{LineInput, Pointer};
use kanbrain_core::model::{Kind, Status, Task, parse_tags};

/// The new / edit dialog.
#[derive(Debug)]
pub struct Form {
    /// What saving does.
    pub target: FormTarget,
    /// The title field.
    pub title: LineInput,
    /// The tags the entry is to have, each a chip in the tags field, in the order added.
    pub tags: Vec<String>,
    /// The tags field's box, typing the next tag: a comma or space turns what's before it into chips.
    pub tag_input: LineInput,
    /// The tag highlighted in the tags field's list: `None` until a tag is typed or `↓` moves to one, so `enter`
    /// saves the form rather than picking a tag nobody pointed at. By name, as a reload that adds a tag ahead of it
    /// moves its row.
    pub tag_sel: Option<String>,
    /// The related field's search box, what's typed filtering the entries to link, and the highlighted row of its
    /// dropdown.
    pub related: Picker,
    /// The entries to relate to, in the form's brain or the project's other repositories; may be empty.
    pub links: Vec<Key>,
    /// The project field's search and highlighted row; `None` on an edit or with a single brain, where
    /// there's no other project to pick.
    pub project: Option<Picker>,
    /// The field being typed into.
    pub field: Field,
    /// Saving was tried without a title: the form says, under the field, while it's still empty.
    pub title_missed: bool,
    /// On an edit, the entry as the form opened it, so saving changes only what was changed in the form: what `kb`
    /// changed meanwhile is kept.
    pub opened: Option<Opened>,
    /// On an edit, `kb` changed one of the fields, or the task's status, since it opened: the title says so until
    /// it's closed, as a change put back is still one that happened.
    pub changed_on_disk: bool,
    /// The title `kb` gave the entry while the form changed it too, as the save that stopped over it found it: shown
    /// under the field, to take it (`ctrl+t`) or save again with the form's.
    pub theirs_title: Option<String>,
    /// What the user told the agent working on the entry, as a save that stopped after the question had it.
    pub note: String,
}

/// An edited entry's title, tags and related entries when its form opened (its title as a save that stopped found
/// it). A field equal to it is unchanged in the form, however it got there, and saves what the entry has then.
#[derive(Debug)]
pub struct Opened {
    /// As saved, on one line: the form's title, trimmed, is compared with it.
    pub title: String,
    /// As saved, lowercase and without `#`: what the form saves ([`Form::tags_to_save`]) is compared with them.
    pub tags: Vec<String>,
    /// Naming brains by index, as `Form::links` does: a change to the list of brains remaps both (`Mode::remap`).
    pub links: Vec<Key>,
    /// The task's: an agent may have finished or parked it meanwhile, which the form doesn't change.
    pub status: Status,
}

impl Form {
    /// An empty form for a new entry of `kind` in the brain at `src`; a task starts in `status`.
    /// `pick_project` adds the field to put it in another brain instead.
    pub fn new(src: usize, kind: Kind, status: Status, pick_project: bool) -> Self {
        let form = Self::filled(FormTarget::New(src, kind, status), "", Vec::new(), Vec::new());
        Self { project: pick_project.then(Picker::default), ..form }
    }

    /// The form for changing the title, tags and related entries of `task`, which `key` names;
    /// `related` are the ids it's related to now.
    pub fn edit(key: Key, task: &Task, related: Vec<Key>) -> Self {
        let opened =
            Opened { title: task.title.clone(), tags: task.tags.clone(), links: related.clone(), status: task.status };
        Self { opened: Some(opened), ..Self::filled(FormTarget::Edit(key), &task.title, task.tags.clone(), related) }
    }

    /// A form for `target` with these fields, typing into the title.
    fn filled(target: FormTarget, title: &str, tags: Vec<String>, links: Vec<Key>) -> Self {
        Self {
            target,
            title: LineInput::new(title),
            tags,
            tag_input: LineInput::default(),
            tag_sel: None,
            related: Picker::default(),
            links,
            project: None,
            field: Field::Title,
            title_missed: false,
            opened: None,
            changed_on_disk: false,
            theirs_title: None,
            note: String::new(),
        }
    }

    /// The brain the form's entry is in, by index.
    pub fn src(&self) -> usize {
        match self.target {
            FormTarget::New(src, ..) => src,
            FormTarget::Edit(key) => key.src,
        }
    }

    /// The tag being typed: while the tags field has the keys, what its box holds, as it's saved (lowercase,
    /// without `#`); empty with text selected (which typing replaces), or in another field.
    pub fn typed_tag(&self) -> String {
        if self.field != Field::Tags || self.tag_input.selection().is_some() {
            return String::new();
        }
        self.tag_input.text().trim_start_matches('#').to_lowercase()
    }

    /// Whether the tag being typed is one the entry holds already: neither a new one nor one to pick.
    pub fn typed_tag_held(&self) -> bool {
        let typed = self.typed_tag();
        !typed.is_empty() && self.tags.contains(&typed)
    }

    /// Adds `tag` as a chip in place of the one being typed, unless the entry holds it already, emptying the box to
    /// type or pick the next.
    pub fn put_tag(&mut self, tag: &str) {
        if !self.tags.iter().any(|held| held == tag) {
            self.tags.push(tag.to_string());
        }
        self.tag_input = LineInput::default();
        self.tag_sel = None;
    }

    /// Turns the tags before the box's last separator, typed or pasted, into chips; what follows it is still being
    /// typed, the cursor where it was in it.
    pub(in crate::state) fn split_tags(&mut self) {
        let text = self.tag_input.text();
        let Some((before, after)) = text.rsplit_once(separates) else { return };
        let cut = text.chars().count() - after.chars().count();
        let at = self.tag_input.cursor().saturating_sub(cut);
        let (before, after) = (parse_tags(before), after.to_string());
        for tag in before {
            self.put_tag(&tag);
        }
        self.tag_input = LineInput::new(&after);
        self.tag_input.point(at, Pointer::Place);
    }

    /// The tags saving gives the entry: the chips, then the one still being typed.
    pub fn tags_to_save(&self) -> Vec<String> {
        let mut tags = self.tags.clone();
        tags.extend(parse_tags(self.tag_input.text()).into_iter().filter(|tag| !self.tags.contains(tag)));
        tags
    }

    /// The field being typed into; `None` on the column picker, which takes no text.
    pub fn active_mut(&mut self) -> Option<&mut LineInput> {
        match self.field {
            Field::Title => Some(&mut self.title),
            Field::Tags => Some(&mut self.tag_input),
            Field::Links => Some(&mut self.related.search),
            Field::Project => self.project.as_mut().map(|p| &mut p.search),
            Field::Status => None,
        }
    }

    /// The column a new task starts in; `None` for a note, a decision or an edit, which have none to pick.
    pub fn status(&self) -> Option<Status> {
        match self.target {
            FormTarget::New(_, Kind::Task, status) => Some(status),
            FormTarget::New(..) | FormTarget::Edit(_) => None,
        }
    }

    /// Puts the new task in `status`; does nothing on a form with no column to pick.
    pub fn set_status(&mut self, status: Status) {
        if let FormTarget::New(src, Kind::Task, _) = self.target {
            self.target = FormTarget::New(src, Kind::Task, status);
        }
    }

    /// Puts the new entry in the brain at `src`. Its related entries are dropped, since they're picked
    /// among the brain's own and its project's.
    pub fn set_src(&mut self, src: usize) {
        if let FormTarget::New(_, kind, status) = self.target {
            self.target = FormTarget::New(src, kind, status);
            self.links.clear();
            self.related = Picker::default();
        }
    }

    /// Types into `field`. Reaching the project field opens its dropdown afresh, on the current brain:
    /// with nothing searched, the dropdown lists every brain in order, so its index is the row.
    pub fn go_to(&mut self, field: Field) {
        let src = self.src();
        if field == Field::Project
            && self.field != field
            && let Some(picker) = &mut self.project
        {
            *picker = Picker::at(src);
        }
        self.field = field;
        self.tag_sel = None;
    }

    /// Goes to `field` from the keyboard, its text selected, so typing replaces it rather than adding to it.
    pub fn tab_to(&mut self, field: Field) {
        self.go_to(field);
        if let Some(input) = self.active_mut() {
            input.select_all();
        }
    }

    /// The fields `tab` goes through, in order: the project, if there's one to pick, the title, tags,
    /// related, then the column, if there's one to pick.
    fn fields(&self) -> impl Iterator<Item = Field> {
        let project = self.project.is_some().then_some(Field::Project);
        let status = self.status().is_some().then_some(Field::Status);
        project.into_iter().chain([Field::Title, Field::Tags, Field::Links]).chain(status)
    }

    /// The field after the current one, for `tab`, back to the first after the last.
    pub fn next_field(&self) -> Field {
        let next = self.fields().skip_while(|&f| f != self.field).nth(1);
        next.or_else(|| self.fields().next()).unwrap_or(Field::Title)
    }

    /// The field before the current one, the other way round.
    pub fn previous_field(&self) -> Field {
        let previous = self.fields().take_while(|&f| f != self.field).last();
        previous.or_else(|| self.fields().last()).unwrap_or(Field::Title)
    }
}

/// Whether `c` separates tags, as `parse_tags` splits them.
fn separates(c: char) -> bool {
    c == ',' || c.is_whitespace()
}

/// What the form's Save does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTarget {
    /// Creates an entry of this kind in the brain at this index; a task starts in this status.
    New(usize, Kind, Status),
    /// Changes the title, tags and related entries of this entry.
    Edit(Key),
}

impl FormTarget {
    /// The entry the form changes; none for a new one.
    pub fn key(self) -> Option<Key> {
        match self {
            Self::Edit(key) => Some(key),
            Self::New(..) => None,
        }
    }
}

/// A field of the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The title.
    Title,
    /// The tags.
    Tags,
    /// The related entries: a search box with a dropdown of entries to check.
    Links,
    /// A new task's column, picked with `←` `→`.
    Status,
    /// A new entry's brain: a search box with a dropdown of every brain.
    Project,
}
