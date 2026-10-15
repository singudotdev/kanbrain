//! The question a save on a task an agent is working on asks first: tell the agent, or just save.

use super::{DIALOG_WIDTH, dialog};
use crate::state::{Board, Tell};
use crate::text::event::KeyCode;
use crate::ui::pointer::TextBox;
use crate::ui::widgets::{Kind, button, dim_small, text_field, tooltip};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, div, prelude::*, px};

/// Whether to tell the agent working on the entry what the save changes, with a note.
pub(super) fn draw(board: &Board, tell: &Tell, cx: &mut Context<BrainView>) -> Div {
    let name = tell.pending.key().map(|key| board.entry_name(key)).unwrap_or_default();
    let what = tell.pending.what();
    let content = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().child(format!("An agent is working on {name}: changing {what} may cross what it writes next.")))
        .child(dim_small("Telling it adds your comment, which it gets with its next step."))
        .child(text_field(TextBox::TellNote, &tell.note, true, "A note for the agent (optional)", cx))
        .child(dim_small("Enter saves and tells it · Ctrl+Enter just saves · Esc goes back."));
    let buttons = vec![
        action("back", "Back", Kind::Secondary, KeyCode::Esc, cx),
        button("just-save", "Just save", Kind::Secondary)
            .tooltip(tooltip("Shortcut: Ctrl+Enter"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::just_save))),
        action("tell", "Save and tell", Kind::Primary, KeyCode::Enter, cx),
    ];
    dialog("Tell the agent?".to_string(), content, buttons).w(px(DIALOG_WIDTH))
}
