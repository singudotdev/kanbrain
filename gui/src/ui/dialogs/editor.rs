//! The multi-line editor for a description, a comment or a project's text.

use super::dialog;
use crate::state::{Board, Edit, Target};
use crate::text::event::KeyCode;
use crate::ui::caret::caret;
use crate::ui::pointer::{Hit, TextBox, pointable};
use crate::ui::theme::{self, ACCENT};
use crate::ui::widgets::{Kind, button, dim_small};
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
    let task = edit.target.key().and_then(|key| board.task(key));
    let project = |src: usize| board.brain(src).project.as_ref().map(|p| (p.name.clone(), p.repo.clone()));
    let name = match edit.target {
        Target::Project(src) => project(src).map(|(name, _)| format!("project {name}, shared by its repositories")),
        Target::RepoNotes(src) => project(src).map(|(name, repo)| format!("repository {repo} of project {name}")),
        _ => task.map(|t| format!("#{} {}", t.id, t.title)),
    }
    .unwrap_or_default();
    let what = match edit.target {
        Target::Description(_) => "Description".to_string(),
        Target::NewComment(_) if board.answers(edit.target) => "Answer".to_string(),
        Target::NewComment(_) => "New comment".to_string(),
        Target::Reject(_) => "Why it's rejected".to_string(),
        Target::Comment(_, index) => task
            .and_then(|t| t.comments.get(index))
            .map_or_else(|| "Comment".to_string(), |c| format!("Comment of {}", when(c.created))),
        Target::Project(_) => "Project description".to_string(),
        Target::RepoNotes(_) => "This repository's role".to_string(),
    };
    let modified = if edit.editor.modified { " •" } else { "" };

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
        let cursor = (index == row).then(|| caret(&text, byte_at(line, col)));
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
        dim_small(if edit.editor.modified {
            "Markdown is fine. Esc or Ctrl+Enter saves and closes."
        } else {
            "Markdown is fine. Esc closes."
        }),
    );
    // Until something's typed there's nothing to save, and the button says so: as "Save", closing an answer
    // never written read as sending it.
    let buttons = if edit.editor.modified {
        let save = match edit.target {
            Target::NewComment(_) if board.answers(edit.target) => "Answer",
            Target::NewComment(_) => "Add comment",
            Target::Reject(_) => "Reject",
            _ => "Save",
        };
        vec![
            button("discard", "Discard changes", Kind::Secondary)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::discard_edit))),
            action("save", save, Kind::Primary, KeyCode::Esc, cx),
        ]
    } else {
        vec![action("save", "Close", Kind::Secondary, KeyCode::Esc, cx)]
    };
    dialog(format!("{what}{modified}"), content, buttons).w(px(EDITOR_WIDTH)).h(relative(0.8))
}
