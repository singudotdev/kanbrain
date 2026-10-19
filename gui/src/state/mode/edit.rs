//! The text editor on a description, a comment, a verdict's text or a project's, and where saving it writes.

use super::Key;
use crate::text::editor::TextEditor;
use crate::text::merge::Merge;
use kanbrain_core::model::{Comment, DecisionState, Status};

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
