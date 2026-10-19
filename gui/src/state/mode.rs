//! What the window is asking for besides the board: a dialog, or a field being typed into.

use super::Key;
use crate::text::editor::{LineInput, Pointer, TextEditor};
use crate::text::event::{KeyCode, KeyEvent};
use crate::text::merge::Merge;
use kanbrain_core::model::{Comment, DecisionState, Kind, Status, Task, parse_tags};
use std::path::PathBuf;

/// The dialog or field that takes the keys; `Normal` leaves them to the board.
#[derive(Debug)]
pub enum Mode {
    /// Keys move around the board and act on the selected entry.
    Normal,
    /// Typing into the search box; the board filters live.
    Search,
    /// The new / edit dialog: title and tags.
    Form(Box<Form>),
    /// Typing a new task's title at the top of a column.
    QuickAdd(QuickAdd),
    /// The text editor: a description, a comment or an answer, a decision's reason, or a project's text.
    Edit(Edit),
    /// A delete, or the update, that waits for a yes.
    Confirm(Confirm),
    /// Choosing the entries related to one, by searching for them.
    Links(LinkPicker),
    /// Choosing one of an entry's comments to edit or delete.
    Comments(CommentPicker),
    /// Choosing which project to show, from a list filtered by a search.
    Scopes(Picker),
    /// Choosing which brain a quick-added task goes in, when the scope shows several, or which an entry
    /// moves to.
    Where(WherePicker),
    /// The menu a right click opened on an entry.
    Menu(super::MenuPicker),
    /// Choosing the tag every tab is filtered by, from a list filtered by a search.
    Tags(Picker),
    /// What a project is and how it's doing: its description, its repositories and their tasks.
    /// The index is one of its repositories' brains.
    ManageProject(usize),
    /// The command palette: every action with its key, filtered by a search.
    Palette(Picker),
    /// Naming the project a folder picked in the system's dialog becomes.
    AddProject(AddProject),
    /// Renaming, moving or removing a project or one of its repositories, from Manage project.
    Manage(Manage),
    /// Before a save on a task an agent is working on: whether to tell it, with a note, as the user's comment.
    Tell(Box<Tell>),
}

impl Mode {
    /// The entry the mode acts on, if it's one: its form, editor, confirmation, pickers or menu.
    pub fn entry(&self) -> Option<Key> {
        match self {
            Self::Form(form) => form.target.key(),
            Self::Edit(edit) => edit.target.key(),
            Self::Tell(tell) => tell.pending.key(),
            Self::Confirm(Confirm::Delete(key) | Confirm::DeleteComment(key, _))
            | Self::Links(LinkPicker { key, .. })
            | Self::Comments(CommentPicker { key, .. })
            | Self::Where(WherePicker { then: WhereFor::Move(key), .. })
            | Self::Menu(super::MenuPicker { key, .. }) => Some(*key),
            Self::Normal
            | Self::Search
            | Self::QuickAdd(_)
            | Self::Confirm(Confirm::Update)
            | Self::Where(_)
            | Self::Scopes(_)
            | Self::Tags(_)
            | Self::ManageProject(_)
            | Self::Palette(_)
            | Self::AddProject(_)
            | Self::Manage(_) => None,
        }
    }

    /// Points every brain the mode names, by index, at where `to` says it now is, after the list of brains
    /// changed; `None` if one is gone, and the mode with it.
    pub fn remap(self, to: impl Fn(usize) -> Option<usize>) -> Option<Self> {
        self.remap_by(&to)
    }

    /// [`Self::remap`], through a reference: [`Tell`] remaps the form or editor it holds with it.
    fn remap_by(self, to: &dyn Fn(usize) -> Option<usize>) -> Option<Self> {
        let key = |key: Key| Some(Key::new(to(key.src)?, key.id));
        Some(match self {
            Self::Form(mut form) => {
                form.target = match form.target {
                    FormTarget::New(src, kind, status) => FormTarget::New(to(src)?, kind, status),
                    FormTarget::Edit(k) => FormTarget::Edit(key(k)?),
                };
                form.links = form.links.into_iter().filter_map(key).collect();
                if let Some(opened) = &mut form.opened {
                    opened.links = std::mem::take(&mut opened.links).into_iter().filter_map(key).collect();
                }
                // With nothing searched its rows are the brains in order, so the row follows its brain; a search's
                // rows shift in ways only the board knows, so it goes back to the first.
                if let Some(picker) = &mut form.project {
                    picker.sel = if picker.search.text().trim().is_empty() { to(picker.sel).unwrap_or(0) } else { 0 };
                }
                Self::Form(form)
            }
            Self::QuickAdd(add) => Self::QuickAdd(QuickAdd { src: to(add.src)?, ..add }),
            // The save waiting is remapped as it would be open.
            Self::Tell(mut tell) => {
                tell.pending = match tell.pending {
                    Pending::Form(form) => {
                        let Self::Form(form) = Self::Form(form).remap_by(to)? else { return None };
                        Pending::Form(form)
                    }
                    Pending::Edit(edit) => {
                        let Self::Edit(edit) = Self::Edit(*edit).remap_by(to)? else { return None };
                        Pending::Edit(Box::new(edit))
                    }
                };
                Self::Tell(tell)
            }
            Self::Edit(edit) => {
                let target = match edit.target {
                    Target::Description(k) => Target::Description(key(k)?),
                    Target::NewComment(k) => Target::NewComment(key(k)?),
                    Target::Comment(k, at) => Target::Comment(key(k)?, at),
                    Target::Reject(k) => Target::Reject(key(k)?),
                    Target::Accept(k) => Target::Accept(key(k)?),
                    Target::Project(src) => Target::Project(to(src)?),
                    Target::RepoNotes(src) => Target::RepoNotes(to(src)?),
                };
                let back = match edit.back {
                    Some(src) => Some(to(src)?),
                    None => None,
                };
                Self::Edit(Edit { target, back, ..edit })
            }
            Self::Confirm(confirm) => Self::Confirm(match confirm {
                Confirm::Delete(k) => Confirm::Delete(key(k)?),
                Confirm::DeleteComment(k, comment) => Confirm::DeleteComment(key(k)?, comment),
                Confirm::Update => Confirm::Update,
            }),
            Self::Links(links) => Self::Links(LinkPicker { key: key(links.key)?, ..links }),
            Self::Comments(comments) => Self::Comments(CommentPicker { key: key(comments.key)?, ..comments }),
            Self::Where(WherePicker { picker, then: WhereFor::Move(k) }) => {
                Self::Where(WherePicker { picker, then: WhereFor::Move(key(k)?) })
            }
            Self::Menu(menu) => Self::Menu(super::MenuPicker { key: key(menu.key)?, ..menu }),
            Self::ManageProject(src) => Self::ManageProject(to(src)?),
            Self::Manage(manage) => Self::Manage(match manage {
                Manage::RenameProject { src, name } => Manage::RenameProject { src: to(src)?, name },
                Manage::RenameRepo { src, name } => Manage::RenameRepo { src: to(src)?, name },
                Manage::MoveRepo { src, picker } => Manage::MoveRepo { src: to(src)?, picker },
                Manage::Remove { src, whole } => Manage::Remove { src: to(src)?, whole },
            }),
            other @ (Self::Normal
            | Self::Search
            | Self::Where(_)
            | Self::Scopes(_)
            | Self::Tags(_)
            | Self::Palette(_)) => other,
            Self::AddProject(add) => {
                let back = match add.back {
                    Some(src) => Some(to(src)?),
                    None => None,
                };
                Self::AddProject(AddProject { back, ..add })
            }
        })
    }
}

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
    pub(super) fn split_tags(&mut self) {
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

/// A task being added at the top of a column.
#[derive(Debug)]
pub struct QuickAdd {
    /// The brain it goes in, by index.
    pub src: usize,
    /// The column, and the new task's status.
    pub status: Status,
    /// What's typed so far.
    pub title: LineInput,
}

/// A folder about to become a project's repository, as `kb init` makes it, and the project it joins or starts.
#[derive(Debug)]
pub struct AddProject {
    /// The folder picked, the repository's root.
    pub dir: PathBuf,
    /// The name typed, which also searches the projects there are, and the highlighted row of
    /// [`Board::add_project_options`](super::Board::add_project_options).
    pub picker: Picker,
    /// The repository (by brain) whose Manage project it was added from: its project is the one joined, and Manage
    /// project, on it, is where cancelling goes back to.
    pub back: Option<usize>,
}

/// A change to a project or one of its repositories, asked from Manage project. Each `src` is one of the
/// project's repositories' brains, by index in `Board::sources`.
#[derive(Debug)]
pub enum Manage {
    /// The project's new name.
    RenameProject {
        /// One of the project's repositories.
        src: usize,
        /// The name typed, in the search box of a picker that lists nothing.
        name: Picker,
    },
    /// The repository's new name in its project.
    RenameRepo {
        /// The repository.
        src: usize,
        /// The name typed, likewise.
        name: Picker,
    },
    /// The project the repository moves to.
    MoveRepo {
        /// The repository.
        src: usize,
        /// The project typed: one there is, found with it, or a new one named so.
        picker: Picker,
    },
    /// Taking a repository, or a whole project, out: unlinking it and keeping its tasks, or deleting them too.
    Remove {
        /// The repository, or one of the project's.
        src: usize,
        /// Every repository of the project, not only this one.
        whole: bool,
    },
}

impl Manage {
    /// The repository it was asked from, whose Manage project it goes back to.
    pub fn src(&self) -> usize {
        match self {
            Self::RenameProject { src, .. }
            | Self::RenameRepo { src, .. }
            | Self::MoveRepo { src, .. }
            | Self::Remove { src, .. } => *src,
        }
    }

    /// Its text field and dropdown, if it has one.
    pub fn picker_mut(&mut self) -> Option<&mut Picker> {
        match self {
            Self::RenameProject { name, .. } | Self::RenameRepo { name, .. } => Some(name),
            Self::MoveRepo { picker, .. } => Some(picker),
            Self::Remove { .. } => None,
        }
    }
}

/// A row of the add-project dialog's dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectOption {
    /// A new project, named as typed.
    New(String),
    /// A project there is: the folder becomes another of its repositories.
    Existing(String),
}

impl ProjectOption {
    /// The project's name.
    pub fn name(&self) -> &str {
        match self {
            Self::New(name) | Self::Existing(name) => name,
        }
    }
}

/// The text editor and what saving it changes.
#[derive(Debug)]
pub struct Edit {
    /// The text being edited.
    pub editor: TextEditor,
    /// Where the text goes.
    pub target: Target,
    /// Manage project, on this brain, when it was opened from there: where closing it goes back to.
    pub back: Option<usize>,
    /// The text the editor's and the target's both started from: the target's when the editor opened, or the one a
    /// merge's conflicts were picked over. Another text there is a change made meanwhile (`kb describe`), which a save
    /// merges with the editor's.
    pub base: String,
    /// For a comment, the entry's comments when the editor opened: one deleted, moved or edited meanwhile may put
    /// another at its index, which comments written in one second, with one text, don't tell apart.
    pub comments: Vec<Comment>,
    /// For a verdict, where the decision stood when the editor opened, or as the last save that stopped found it:
    /// `kb` deciding meanwhile isn't overturned unseen.
    pub decision: Option<DecisionState>,
    /// The entry's status when the editor opened: an agent may have finished or parked the task meanwhile.
    pub status: Option<Status>,
    /// `kb` changed the text, the entry's status or the verdict being given since the editor opened: the title says
    /// so until it's closed, as a change put back is still one that happened.
    pub changed_on_disk: bool,
    /// Lines both the editor and `kb` changed, which a save that stopped over them shows in place of the text, to
    /// pick in each conflict the editor's, `kb`'s or both; `None` while typing.
    pub merge: Option<Merge>,
    /// What the user told the agent working on the entry, as a save that stopped after the question had it: asked
    /// again, the question starts from it.
    pub note: String,
}

impl Edit {
    /// Ends the merge, its conflicts picked or merged again cleanly: `text` goes in the editor, to look over and save,
    /// and `base` is what a save merges with from now on, the text the merge was made over.
    pub fn settle(&mut self, base: String, text: &str) {
        self.base = base;
        self.merge = None;
        self.editor = TextEditor::new(text);
    }
}

/// What the text editor edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The entry's description.
    Description(Key),
    /// A new comment on the entry.
    NewComment(Key),
    /// The entry's comment at this index.
    Comment(Key, usize),
    /// Why the decision is rejected: saving it rejects the decision.
    Reject(Key),
    /// A comment on accepting the decision: saving it accepts the decision with it.
    Accept(Key),
    /// The description of the project the brain at this index belongs to, shared by its repositories.
    Project(usize),
    /// The notes about itself of the repository whose brain is at this index.
    RepoNotes(usize),
}

impl Target {
    /// The entry the text belongs to; `None` for a project's card.
    pub fn key(self) -> Option<Key> {
        match self {
            Self::Description(key)
            | Self::NewComment(key)
            | Self::Comment(key, _)
            | Self::Reject(key)
            | Self::Accept(key) => Some(key),
            Self::Project(_) | Self::RepoNotes(_) => None,
        }
    }
}

/// A delete, or the update, waiting for confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    /// The entry.
    Delete(Key),
    /// The entry's comment, as it was when asked about: one deleted meanwhile puts another at its index, which is
    /// not the one to delete.
    DeleteComment(Key, Comment),
    /// Installing the newer release, which closes the app and opens the new one.
    Update,
}

/// An entry's comments, oldest first as the details show them, one of them highlighted (`sel`): `enter` edits it,
/// `x` deletes it.
#[derive(Debug)]
pub struct CommentPicker {
    /// The entry.
    pub key: Key,
    /// The highlighted comment's index.
    pub sel: usize,
}

/// The link picker: every other entry, filtered by a search, to relate to the entry or unrelate.
#[derive(Debug)]
pub struct LinkPicker {
    /// The entry whose related entries are being chosen; candidates come from its brain.
    pub key: Key,
    /// Filters the candidates (see `Task::matches`), and its row is an index into `Board::link_candidates`.
    pub rows: Picker,
}

/// A dropdown with a search box: projects or tags, filtered as you type, so a long list stays usable.
#[derive(Debug, Default)]
pub struct Picker {
    /// Filters the rows by name.
    pub search: LineInput,
    /// The highlighted row.
    pub sel: usize,
}

/// "Which project?": the brains an entry can go in, filtered by a search, and what happens in the one
/// chosen: a quick-add field opens there, or an entry moves there.
#[derive(Debug)]
pub struct WherePicker {
    /// The search and the highlighted row.
    pub picker: Picker,
    /// What happens once a brain is chosen.
    pub then: WhereFor,
}

/// What the "which project?" picker is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhereFor {
    /// A new task: the field at the top of this column.
    QuickAdd(Status),
    /// Moving this entry to another brain.
    Move(Key),
}

/// What a key did to a dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKey {
    /// `esc`: close it, choosing nothing.
    Close,
    /// `enter`: choose the highlighted row.
    Choose,
    /// Moved the highlight or typed in the search box: it stays open.
    Stay,
}

impl Picker {
    /// An empty search, its row `sel` highlighted.
    pub fn at(sel: usize) -> Self {
        Self { sel, ..Self::default() }
    }

    /// A name field holding `name`, selected so typing replaces it.
    pub fn named(name: &str) -> Self {
        let mut search = LineInput::new(name);
        search.select_all();
        Self { search, sel: 0 }
    }

    /// Whether `name` contains every word of the search, ignoring case: how every dropdown filters.
    pub fn matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.search.text().to_lowercase().split_whitespace().all(|word| name.contains(word))
    }

    /// Whether nothing is typed in the search.
    pub fn is_empty(&self) -> bool {
        self.search.text().trim().is_empty()
    }

    /// Handles a key, with `count` rows shown: arrows move, typing filters (and goes back to the top).
    pub fn key(&mut self, key: KeyEvent, count: usize) -> PickerKey {
        match key.code {
            KeyCode::Esc => return PickerKey::Close,
            KeyCode::Enter => return PickerKey::Choose,
            KeyCode::Up => self.sel = self.sel.saturating_sub(1),
            KeyCode::Down => self.sel = (self.sel + 1).min(count.saturating_sub(1)),
            _ => {
                if self.search.on_key(key) {
                    self.sel = 0;
                }
            }
        }
        PickerKey::Stay
    }
}

/// The question a save on a task an agent is working on asks first: what it changes may cross what the agent writes
/// next, and the agent hears of a change only through a comment (`kb hook tool` tells it of one).
#[derive(Debug)]
pub struct Tell {
    /// The form or editor saving, as it was: `esc` goes back to it, either answer saves it.
    pub pending: Pending,
    /// What the user adds for the agent, after what changed; may stay empty.
    pub note: LineInput,
}

/// A save waiting for [`Tell`]'s answer.
#[derive(Debug)]
pub enum Pending {
    /// The edit form's.
    Form(Box<Form>),
    /// The description's or a comment's editor.
    Edit(Box<Edit>),
}

impl Pending {
    /// The entry being saved.
    pub fn key(&self) -> Option<Key> {
        match self {
            Self::Form(form) => form.target.key(),
            Self::Edit(edit) => edit.target.key(),
        }
    }

    /// What the save changes, as the question names it.
    pub fn what(&self) -> &'static str {
        match self {
            Self::Form(_) => "its title, tags or related entries",
            Self::Edit(edit) => match edit.target {
                Target::Comment(..) => "a comment",
                _ => "its description",
            },
        }
    }

    /// What the user told the agent before this save stopped, to ask with again.
    pub fn note(&self) -> &str {
        match self {
            Self::Form(form) => &form.note,
            Self::Edit(edit) => &edit.note,
        }
    }

    /// The form or editor again, as it was.
    pub fn into_mode(self) -> Mode {
        match self {
            Self::Form(form) => Mode::Form(form),
            Self::Edit(edit) => Mode::Edit(*edit),
        }
    }
}

/// What a save does about an agent working on the entry: asks first, or was answered.
#[derive(Debug, PartialEq, Eq)]
pub enum Telling {
    /// Asks, if an agent is working on it ([`Tell`]); else just saves.
    Ask,
    /// Saves without telling.
    Save,
    /// Saves, then tells the agent what changed, with this note, as the user's comment.
    Tell(String),
}
