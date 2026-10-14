//! Dialogs that pick from a searchable list: the link picker, "which project?" and the
//! command palette.

use super::{WIDE_WIDTH, check, dialog, list_box};
use crate::state::{Action, Board, CommentPicker, LinkPicker, Picker, Tab, Target, WhereFor, WherePicker};
use crate::text::event::KeyCode;
use crate::ui::dropdown::{MAX_ROWS, list, row};
use crate::ui::pointer::TextBox;
use crate::ui::theme::{self, BORDER};
use crate::ui::widgets::{Kind, dim, dim_small, entry_pill, tag_chips, text_field};
use crate::ui::{BrainView, action, truncate, when};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::model::{Author, Kind as EntryKind};

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
            row(("link", index), selector, index == picker.rows.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.toggle_link(index))))
                .child(check(linked))
                .child(entry_pill(other))
                .child(div().flex_1().min_w_0().truncate().child(format!(
                    "{} {}",
                    board.link_name(picker.key.src, key),
                    other.title
                )))
                .child(tag_chips(board, &other.tags).flex_nowrap().flex_none())
        })
        .collect();
    let empty = candidates.is_empty();
    let list = list("links", rows, picker.rows.sel, MAX_ROWS, scroll, follow)
        .when(empty, |d| d.child(dim("Nothing matches.")));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(text_field(TextBox::LinkSearch, &picker.rows.search, true, "Search titles, or #tag", cx))
        .child(dim_small("Click an entry (or Enter) to link or unlink it. Links show on both entries."))
        .child(list_box(list));
    let buttons = vec![action("links-done", "Done", Kind::Primary, KeyCode::Esc, cx)];
    let title = format!("Related to {} {}", board.entry_name(picker.key), truncate(&task.title, 40));
    Some(dialog(title, content, buttons).w(px(WIDE_WIDTH)))
}

/// An entry's comments, to edit one (`enter`, or a click) or delete it (`x`).
pub(super) fn comments(
    board: &Board,
    picker: &CommentPicker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Option<Div> {
    let task = board.task(picker.key)?;
    let key = picker.key;
    let rows: Vec<_> = task
        .comments
        .iter()
        .enumerate()
        .map(|(index, comment)| {
            let first = comment.body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
            let by = if comment.author == Author::Agent { " · agent" } else { "" };
            row(("comment", index), format!("comment {index}"), index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.act(cx, |b| b.open_editor(Target::Comment(key, index)));
                }))
                .child(dim_small(format!("{}{by}", when(comment.created))).flex_none())
                .child(div().flex_1().min_w_0().truncate().child(first.to_string()))
        })
        .collect();
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(dim_small("Enter (or a click) edits the highlighted comment; x deletes it, after asking."))
        .child(list_box(list("comments", rows, picker.sel, MAX_ROWS, scroll, follow)));
    let buttons = vec![action("comments-done", "Close", Kind::Primary, KeyCode::Esc, cx)];
    let title = format!("Comments on {} {}", board.entry_name(key), truncate(&task.title, 40));
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
        .child(list_box(list));
    let buttons = vec![
        action("where-cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("where-choose", "Choose", Kind::Primary, KeyCode::Enter, cx),
    ];
    let title = match asked.then {
        WhereFor::QuickAdd(_) => format!("New {} in which project?", EntryKind::Task.as_str()),
        WhereFor::Move(key) => {
            let name = board.task(key).map_or_else(String::new, |t| format!(" {}", truncate(&t.title, 40)));
            format!("Move {}{name} to which project?", board.entry_name(key))
        }
    };
    let content = match asked.then {
        WhereFor::Move(_) => content.child(dim_small("It gets a new number there and keeps everything but its links.")),
        WhereFor::QuickAdd(_) => content,
    };
    dialog(title, content, buttons).w(px(WIDE_WIDTH))
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
        .iter()
        .enumerate()
        .map(|(index, &&Action { shown, does, runs, .. })| {
            // The first row of each section names it, its line above lighter than the others: a heading row would
            // break the list's even rows, which its scrolling counts on.
            let section = commands[index].section();
            let starts = index == 0 || commands[index - 1].section() != section;
            row(("command", index), format!("command {index}"), index == picker.sel, runs)
                .gap_3()
                .when(starts, |d| d.border_color(rgb(BORDER)))
                .when(runs, |d| {
                    d.on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_command(index))),
                    )
                })
                .child(div().flex_1().min_w_0().truncate().when(!runs, |d| d.text_color(rgb(theme::DIM))).child(does))
                .children(starts.then(|| dim_small(section.title()).flex_none()))
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
                        .child(shown),
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
        .child(list_box(list));
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
