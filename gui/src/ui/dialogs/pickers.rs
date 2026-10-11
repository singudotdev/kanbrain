//! Dialogs that pick from a searchable list: the link picker, "which project?", "what's next?" and the
//! command palette.

use super::{WIDE_WIDTH, check, dialog};
use crate::state::{Board, Command, LinkPicker, NextChoice, NextPicker, Picker, Tab, WhereFor, WherePicker};
use crate::text::event::KeyCode;
use crate::ui::dropdown::{MAX_ROWS, list, row};
use crate::ui::pointer::TextBox;
use crate::ui::theme::{self, BORDER};
use crate::ui::widgets::{self, Kind, button, dim, dim_small, entry_pill, tag_chips, text_field};
use crate::ui::{BrainView, action, truncate};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::model::Kind as EntryKind;

/// Rows the command palette shows before it scrolls: more than other lists, since it's the list of
/// every shortcut too.
const PALETTE_ROWS: usize = 12;

/// The link picker: a search box, then every other entry matching it; clicking a row links or unlinks it.
pub(super) fn links(
    board: &Board,
    picker: &LinkPicker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Option<Div> {
    let task = board.task(picker.key)?;
    let related = board.related_keys(picker.key);
    let candidates = board.link_candidates(picker);
    let rows: Vec<_> = candidates
        .iter()
        .enumerate()
        .map(|(index, &(key, other))| {
            let linked = related.contains(&key);
            let selector = if key.src == picker.key.src {
                format!("link {}", key.id)
            } else {
                format!("link {}/{}", key.src, key.id)
            };
            row(("link", index), selector, index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.toggle_link(index))))
                .child(check(linked))
                .child(entry_pill(other))
                .child(div().flex_1().min_w_0().truncate().child(format!(
                    "{} {}",
                    board.link_name(picker.key.src, key),
                    other.title
                )))
                .child(tag_chips(board.brain(key.src), &other.tags).flex_nowrap().flex_none())
        })
        .collect();
    let empty = candidates.is_empty();
    let list =
        list("links", rows, picker.sel, MAX_ROWS, scroll, follow).when(empty, |d| d.child(dim("Nothing matches.")));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(text_field(TextBox::LinkSearch, &picker.search, true, "Search titles, or #tag", cx))
        .child(dim_small("Click an entry (or Enter) to link or unlink it. Links show on both entries."))
        .child(list);
    let buttons = vec![action("links-done", "Done", Kind::Primary, KeyCode::Esc, cx)];
    let title = format!("Related to #{} {}", task.id, truncate(&task.title, 40));
    Some(dialog(title, content, buttons).w(px(WIDE_WIDTH)))
}

/// "Which project?": for a quick-added task when the scope shows several brains (the selected entry's
/// highlighted), or to move an entry; the brains offered, filtered by a search.
pub(super) fn place(
    board: &Board,
    asked: &WherePicker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let picker = &asked.picker;
    let candidates = board.where_candidates(picker, asked.then);
    let empty = candidates.is_empty();
    let rows: Vec<_> = candidates
        .into_iter()
        .enumerate()
        .map(|(index, (src, name))| {
            let dir = board.brain(src).store.dir().display().to_string();
            row(("where", index), format!("where {index}"), index == picker.sel, true)
                .gap_3()
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_where(index))))
                .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(name))
                .child(dim_small(dir).min_w_0().truncate())
        })
        .collect();
    let list = list("where", rows, picker.sel, MAX_ROWS, scroll, follow)
        .when(empty, |d| d.child(dim("No project matches.").px_2().py_1()));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(text_field(TextBox::WhereSearch, &picker.search, true, "Search projects", cx))
        .child(list);
    let buttons = vec![
        action("where-cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("where-choose", "Choose", Kind::Primary, KeyCode::Enter, cx),
    ];
    let title = match asked.then {
        WhereFor::QuickAdd(_) => format!("New {} in which project?", EntryKind::Task.as_str()),
        WhereFor::Move(key) => {
            let name = board.task(key).map_or_else(String::new, |t| format!(" {}", truncate(&t.title, 40)));
            format!("Move #{}{name} to which project?", key.id)
        }
    };
    let content = match asked.then {
        WhereFor::Move(_) => content.child(dim_small("It gets a new number there and keeps everything but its links.")),
        WhereFor::QuickAdd(_) => content,
    };
    dialog(title, content, buttons).w(px(WIDE_WIDTH))
}

/// "What's next?" for a brain left with nothing running: its parked tasks (the latest first, to
/// resume after an interruption), its to-do tasks, a new task, or nothing.
pub(super) fn next(
    board: &Board,
    picker: &NextPicker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let brain = board.brain(picker.src);
    let rows: Vec<_> = board
        .next_choices(picker)
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            let row = row(("next", index), format!("next {index}"), index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_next(index))));
            match choice {
                NextChoice::Start(key) => {
                    let Some(task) = board.task(key) else { return row };
                    row.child(entry_pill(task))
                        .child(div().flex_1().min_w_0().truncate().child(format!("#{} {}", task.id, task.title)))
                        .child(tag_chips(brain, &task.tags).flex_nowrap().flex_none())
                }
                NextChoice::New => row.child(div().text_color(rgb(theme::ACCENT)).child("+ New task, started")),
                NextChoice::Nothing => row.child(dim("■ Nothing for now")),
            }
        })
        .collect();
    let list = list("next-list", rows, picker.sel, MAX_ROWS, scroll, follow);
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(text_field(TextBox::NextSearch, &picker.search, true, "Search titles, or #tag", cx))
        .child(dim_small("Parked tasks first: the latest is usually the one to get back to."))
        .child(list);
    let buttons = vec![
        action("next-nothing", "Nothing for now", Kind::Secondary, KeyCode::Esc, cx),
        // Not `n`'s action: once something is searched, n is a letter of it.
        button("next-new", "New task", Kind::Secondary)
            .tooltip(widgets::tooltip("Shortcut: n (with nothing searched)"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::next_new))),
        action("next-start", "Start", Kind::Primary, KeyCode::Enter, cx),
    ];
    let place = board.sources[picker.src].label.clone().unwrap_or_else(|| "the central brain".to_string());
    dialog(format!("Nothing running in {place}: what's next?"), content, buttons).w(px(WIDE_WIDTH))
}

/// The command palette: every action of the tab with its key, filtered by a search; a click or `enter`
/// runs one. Rows that only say how to do something, like moving around, are dimmed.
pub(super) fn palette(
    board: &Board,
    picker: &Picker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let commands = board.commands(picker);
    let empty = commands.is_empty();
    let rows: Vec<_> = commands
        .into_iter()
        .enumerate()
        .map(|(index, Command { key, does, press, .. })| {
            row(("command", index), format!("command {index}"), index == picker.sel, press.is_some())
                .gap_3()
                .when(press.is_some(), |d| {
                    d.on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_command(index))),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(press.is_none(), |d| d.text_color(rgb(theme::DIM)))
                        .child(does),
                )
                .child(
                    div()
                        .flex_none()
                        .px_1p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(theme::INPUT))
                        .text_size(px(12.))
                        .font_family(theme::EDITOR_FONT)
                        .child(key),
                )
        })
        .collect();
    let list = list("commands", rows, picker.sel, PALETTE_ROWS, scroll, follow)
        .when(empty, |d| d.child(dim("No command matches.").px_2().py_1()));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(text_field(TextBox::PaletteSearch, &picker.search, true, "Search commands or keys", cx))
        .child(list);
    let buttons = vec![
        action("palette-close", "Close", Kind::Secondary, KeyCode::Esc, cx),
        action("palette-run", "Run", Kind::Primary, KeyCode::Enter, cx),
    ];
    let title = match board.tab {
        Tab::Report => "Commands · report",
        Tab::Calendar => "Commands · calendar",
        Tab::Board | Tab::Notes | Tab::Decisions => "Commands",
    };
    dialog(title, content, buttons).w(px(WIDE_WIDTH))
}
