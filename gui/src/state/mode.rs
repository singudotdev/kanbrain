//! What the window is asking for besides the board: a dialog, or a field being typed into.

use super::Key;
use crate::text::editor::{LineInput, TextEditor};
use crate::text::event::{KeyCode, KeyEvent};
use kanbrain_core::model::{Kind, Status, Task};
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
}

impl Mode {
    /// Points every brain the mode names, by index, at where `to` says it now is, after the list of brains
    /// changed; `None` if one is gone, and the mode with it.
    pub fn remap(self, to: impl Fn(usize) -> Option<usize>) -> Option<Self> {
        let key = |key: Key| Some(Key::new(to(key.src)?, key.id));
        Some(match self {
            Self::Form(mut form) => {
                form.target = match form.target {
                    FormTarget::New(src, kind, status) => FormTarget::New(to(src)?, kind, status),
                    FormTarget::Edit(k) => FormTarget::Edit(key(k)?),
                };
                form.links = form.links.into_iter().filter_map(key).collect();
                Self::Form(form)
            }
            Self::QuickAdd(add) => Self::QuickAdd(QuickAdd { src: to(add.src)?, ..add }),
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
                Self::Edit(Edit { target, ..edit })
            }
            Self::Confirm(confirm) => Self::Confirm(match confirm {
                Confirm::Delete(k) => Confirm::Delete(key(k)?),
                Confirm::DeleteComment(k, at) => Confirm::DeleteComment(key(k)?, at),
                Confirm::Update => Confirm::Update,
            }),
            Self::Links(links) => Self::Links(LinkPicker { key: key(links.key)?, ..links }),
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
            | Self::Palette(_)
            | Self::AddProject(_)) => other,
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
    /// The tags field: comma or space separated.
    pub tags: LineInput,
    /// The related field's search box: what's typed filters the entries to link.
    pub links_search: LineInput,
    /// The entries to relate to, in the form's brain or the project's other repositories; may be empty.
    pub links: Vec<Key>,
    /// The highlighted row of the related field's dropdown, while it's typed into.
    pub links_sel: usize,
    /// The project field's search and highlighted row; `None` on an edit or with a single brain, where
    /// there's no other project to pick.
    pub project: Option<Picker>,
    /// The field being typed into.
    pub field: Field,
}

impl Form {
    /// An empty form for a new entry of `kind` in the brain at `src`; a task starts in `status`.
    /// `pick_project` adds the field to put it in another brain instead.
    pub fn new(src: usize, kind: Kind, status: Status, pick_project: bool) -> Self {
        let form = Self::filled(FormTarget::New(src, kind, status), "", "", Vec::new());
        Self { project: pick_project.then(Picker::default), ..form }
    }

    /// The form for changing the title, tags and related entries of `task`, which `key` names;
    /// `related` are the ids it's related to now.
    pub fn edit(key: Key, task: &Task, related: Vec<Key>) -> Self {
        Self::filled(FormTarget::Edit(key), &task.title, &task.tags.join(", "), related)
    }

    /// A form for `target` with these fields, typing into the title.
    fn filled(target: FormTarget, title: &str, tags: &str, links: Vec<Key>) -> Self {
        Self {
            target,
            title: LineInput::new(title),
            tags: LineInput::new(tags),
            links_search: LineInput::default(),
            links,
            links_sel: 0,
            project: None,
            field: Field::Title,
        }
    }

    /// The brain the form's entry is in, by index.
    pub fn src(&self) -> usize {
        match self.target {
            FormTarget::New(src, ..) => src,
            FormTarget::Edit(key) => key.src,
        }
    }

    /// The field being typed into; `None` on the column picker, which takes no text.
    pub fn active(&self) -> Option<&LineInput> {
        match self.field {
            Field::Title => Some(&self.title),
            Field::Tags => Some(&self.tags),
            Field::Links => Some(&self.links_search),
            Field::Project => self.project.as_ref().map(|p| &p.search),
            Field::Status => None,
        }
    }

    /// The field being typed into, to change.
    pub fn active_mut(&mut self) -> Option<&mut LineInput> {
        match self.field {
            Field::Title => Some(&mut self.title),
            Field::Tags => Some(&mut self.tags),
            Field::Links => Some(&mut self.links_search),
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
            self.links_search = LineInput::default();
            self.links_sel = 0;
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
            *picker = Picker { sel: src, ..Picker::default() };
        }
        self.field = field;
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

/// What the form's Save does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTarget {
    /// Creates an entry of this kind in the brain at this index; a task starts in this status.
    New(usize, Kind, Status),
    /// Changes the title, tags and related entries of this entry.
    Edit(Key),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// The entry.
    Delete(Key),
    /// The entry's comment at this index.
    DeleteComment(Key, usize),
    /// Installing the newer release, which closes the app and opens the new one.
    Update,
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
