//! What the window is asking for besides the board: a dialog, or a field being typed into.

mod edit;
mod form;

pub use edit::{Edit, Target};
pub use form::{Field, Form, FormTarget};

use super::Key;
use crate::text::editor::LineInput;
use crate::text::event::{KeyCode, KeyEvent};
use kanbrain_core::model::{Comment, Status};
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
    /// Choosing which projects to show, from a list filtered by a search.
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
