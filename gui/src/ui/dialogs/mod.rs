//! Dialogs drawn over the window, each with its buttons at the bottom:
//!
//! - [`form`]: the new / edit form, with its project, related entries and column.
//! - [`editor`]: the text editor, for descriptions, comments and project text.
//! - [`pickers`]: the link picker, "which project?" and the command palette.
//! - [`project`]: Manage project: a project at a glance, and the buttons that change it.
//! - [`add_project`]: the project a folder picked in the system's dialog joins or starts.
//! - [`manage`]: renaming, moving and removing a project or its repositories.
//!
//! The confirmation of a delete or the update is here.

mod add_project;
mod editor;
mod form;
mod manage;
mod pickers;
mod project;

use crate::state::{Board, Confirm, Mode};
use crate::text::event::KeyCode;
use crate::ui::theme::{self, BORDER, DIM_STYLE, Line, Span, Style};
use crate::ui::widgets::Kind;
use crate::ui::{BrainView, Scrolls, action, truncate, when};
use gpui::{Context, Div, FontWeight, Stateful, div, prelude::*, px, relative, rgb, rgba};
use kanbrain_core::model::Task;

/// Width of the form and delete dialogs.
const DIALOG_WIDTH: f32 = 520.;
/// Width of the link picker and the command palette.
const WIDE_WIDTH: f32 = 620.;

/// The dialog of the current mode, if it has one.
pub fn draw(board: &Board, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Option<Div> {
    let dialog = match &board.mode {
        Mode::Form(form) => form::draw(board, form, scroll, follow, cx),
        Mode::Edit(edit) => editor::draw(board, edit, &scroll.editor, follow, cx),
        Mode::Confirm(confirm) => draw_confirm(board, *confirm, cx)?,
        Mode::Links(picker) => pickers::links(board, picker, &scroll.links, follow, cx)?,
        Mode::Comments(picker) => pickers::comments(board, picker, &scroll.links, follow, cx)?,
        Mode::Where(asked) => pickers::place(board, asked, &scroll.place, follow, cx),
        Mode::ManageProject(src) => project::draw(board, *src, &scroll.manage_project, follow, cx)?,
        Mode::Palette(picker) => pickers::palette(board, picker, &scroll.palette, follow, cx),
        Mode::AddProject(add) => add_project::draw(board, add, &scroll.add_project, follow, cx),
        Mode::Manage(manage) => manage::draw(board, manage, &scroll.add_project, follow, cx)?,
        // The project dropdown hangs from its button in the top bar.
        // The menu is drawn at the click, over everything: see `menu::draw`.
        Mode::Normal | Mode::Search | Mode::QuickAdd(_) | Mode::Scopes(_) | Mode::Tags(..) | Mode::Menu(_) => {
            return None;
        }
    };
    Some(modal(dialog))
}

/// Centers `dialog` over the dimmed window; nothing behind it can be clicked.
fn modal(dialog: Div) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .p_4()
        .bg(rgba(0x0000_0099))
        .occlude()
        .child(dialog.max_w(relative(1.)).max_h(relative(1.)))
}

/// A dialog box: title, content, and buttons at the bottom right.
fn dialog(title: impl Into<String>, content: impl IntoElement, buttons: Vec<Stateful<Div>>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(theme::POPUP))
        .shadow_lg()
        .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(title.into()))
        .child(content)
        .child(div().flex().justify_end().gap_2().children(buttons))
}

/// A label above a field.
fn labeled(label: &str, field: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(label.to_string()))
        .child(field)
}

/// The check column of a row that can be checked: `✓` when it is, blank otherwise, so titles line up.
fn check(checked: bool) -> Div {
    div().w(px(16.)).flex_none().text_color(rgb(theme::GREEN)).child(if checked { "✓" } else { "" })
}

/// A delete dialog, what will be removed with Cancel and Delete buttons; or the update's, with Update.
fn draw_confirm(board: &Board, confirm: Confirm, cx: &mut Context<BrainView>) -> Option<Div> {
    let (title, lines) = match confirm {
        Confirm::Delete(key) => {
            let task = board.task(key)?;
            let consequence = "Its markdown file, with description, comments and history, will be removed.";
            (format!("Delete {}?", task.kind.as_str()), vec![question(task), Line::styled(consequence, DIM_STYLE)])
        }
        Confirm::DeleteComment(key, index) => {
            let comment = board.task(key).and_then(|t| t.comments.get(index))?;
            let preview = comment.body.lines().next().unwrap_or_default();
            let lines = vec![
                Line::raw("This comment will be removed."),
                Line::styled(format!("{} · {}", when(comment.created), truncate(preview, 48)), DIM_STYLE),
            ];
            ("Delete comment?".to_string(), lines)
        }
        Confirm::Update => {
            let version = board.available.as_deref()?;
            let lines = vec![
                Line::raw(format!("kanbrain {version}, and kb beside it, replace {}.", env!("CARGO_PKG_VERSION"))),
                Line::styled(
                    "The app closes and opens again; every project's agent instructions are updated.",
                    DIM_STYLE,
                ),
            ];
            (format!("Update to {version}?"), lines)
        }
    };
    let (confirm_label, confirm_kind) =
        if confirm == Confirm::Update { ("Update", Kind::Primary) } else { ("Delete", Kind::Danger) };
    let content = div().flex().flex_col().gap_1().children(lines.into_iter().map(Line::render));
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("confirm", confirm_label, confirm_kind, KeyCode::Char('y'), cx),
    ];
    Some(dialog(title, content, buttons).w(px(DIALOG_WIDTH)))
}

/// `#id title` in bold: the entry about to be deleted.
fn question(task: &Task) -> Line {
    Line::from(vec![Span::styled(format!("#{} {}", task.id, task.title), Style::new().bold())])
}
