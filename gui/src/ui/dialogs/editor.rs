//! The multi-line editor for a description, a comment or a project's text.

use super::{dialog, titled};
use crate::state::{Board, Edit, Target};
use crate::text::event::KeyCode;
use crate::ui::caret::caret;
use crate::ui::pointer::{Hit, TextBox, pointable};
use crate::ui::theme::{self, ACCENT};
use crate::ui::widgets::{Kind, button, dim_small, tooltip};
use crate::ui::{BrainView, action, byte_at, edited, when};
use gpui::{ClickEvent, Context, Div, ScrollHandle, div, prelude::*, px, relative, rgb};

/// Widest the text editor gets.
const EDITOR_WIDTH: f32 = 860.;

/// The multi-line editor for a description or a comment.
pub(super) fn draw(
    board: &Board,
    edit: &Edit,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    if let Some(merge) = &edit.merge {
        return super::merge::draw(board, edit, merge, (EDITOR_WIDTH, scroll), follow, cx);
    }
    let name = edited_name(board, edit.target);
    let what = match edit.target {
        Target::Description(_) => "Description".to_string(),
        Target::NewComment(_) if board.answers(edit.target) => "Answer".to_string(),
        Target::NewComment(_) => "New comment".to_string(),
        Target::Reject(_) => "Why it's rejected".to_string(),
        Target::Accept(_) => "Accepted, with a comment".to_string(),
        // As it opened: another may be at its index now.
        Target::Comment(_, index) => edit
            .comments
            .get(index)
            .map_or_else(|| "Comment".to_string(), |c| format!("Comment of {}", when(c.created))),
        Target::Project(_) => "Project description".to_string(),
        Target::RepoNotes(_) => "This repository's role".to_string(),
    };
    let changed = board.changed(edit);

    // Long lines wrap; the cursor's line is kept in view while typing.
    let (row, col) = edit.editor.cursor();
    if follow {
        scroll.scroll_to_item(row);
    }
    let selection = edit.editor.selection();
    let mut lines = Vec::with_capacity(edit.editor.lines().len());
    let mut hits = Vec::with_capacity(lines.capacity());
    for (index, line) in edit.editor.lines().iter().enumerate() {
        // The part of this line inside the selection, if any.
        let selected = selection.and_then(|((r1, c1), (r2, c2))| {
            let len = line.chars().count();
            (r1..=r2).contains(&index).then_some((if index == r1 { c1 } else { 0 }, if index == r2 { c2 } else { len }))
        });
        let text = edited(line, selected).render();
        let cursor = (index == row).then(|| caret(&text, byte_at(line, col), None));
        hits.push(Hit::new(&text, line.chars().count()));
        lines.push(div().child(text).children(cursor));
    }
    let text = div()
        .id(TextBox::Editor.id())
        .flex_1()
        .min_h(px(160.))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .p_3()
        .rounded_md()
        .bg(rgb(theme::INPUT))
        .border_1()
        .border_color(rgb(ACCENT))
        .font_family(theme::EDITOR_FONT)
        .text_size(px(13.))
        .children(lines);
    let text = pointable(text, TextBox::Editor, hits, cx);
    let content = div().flex_1().min_h_0().flex().flex_col().gap_2().child(dim_small(name)).child(text).child(
        dim_small(if changed {
            "Markdown is fine. Esc or Ctrl+Enter saves and closes; Ctrl+D discards the changes."
        } else {
            "Markdown is fine. Esc closes."
        }),
    );
    // Until something's typed there's nothing to save, and the button says so: as "Save", closing an answer
    // never written read as sending it.
    let buttons = if changed {
        let save = match edit.target {
            Target::NewComment(_) if board.answers(edit.target) => "Answer",
            Target::NewComment(_) => "Add comment",
            Target::Reject(_) => "Reject",
            Target::Accept(_) => "Accept",
            _ => "Save",
        };
        vec![
            button("discard", "Discard changes", Kind::Secondary)
                .tooltip(tooltip("Shortcut: Ctrl+D"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::discard_edit))),
            action("save", save, Kind::Primary, KeyCode::Esc, cx),
        ]
    } else {
        vec![action("save", "Close", Kind::Secondary, KeyCode::Esc, cx)]
    };
    dialog(title(what, changed, edit.changed_on_disk), content, buttons).w(px(EDITOR_WIDTH)).h(relative(0.8))
}

/// The editor's title: what it edits, the mark that `kb` changed it, then the dot for unsaved changes, kept last as
/// the one that comes and goes while typing.
pub(super) fn title(what: String, modified: bool, changed_on_disk: bool) -> String {
    let title = titled(what, changed_on_disk);
    if modified { format!("{title} •") } else { title }
}

/// What the editor's text belongs to: the entry, the project or the repository.
pub(super) fn edited_name(board: &Board, target: Target) -> String {
    let project = |src: usize| board.brain(src).project.as_ref();
    match target {
        Target::Project(src) => project(src).map(|p| format!("project {}, shared by its repositories", p.name)),
        Target::RepoNotes(src) => project(src).map(|p| format!("repository {} of project {}", p.repo, p.name)),
        _ => target
            .key()
            .zip(target.key().and_then(|key| board.task(key)))
            .map(|(key, t)| format!("{} {}", board.entry_name(key), t.title)),
    }
    .unwrap_or_default()
}
