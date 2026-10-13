//! The new / edit form: the project a new entry goes in, its title and tags, related entries and column.

use super::{DIALOG_WIDTH, check, dialog, labeled};
use crate::state::{Board, COLUMNS, Field, Form, FormTarget, Picker};
use crate::text::editor::LineInput;
use crate::text::event::KeyCode;
use crate::ui::dropdown::{list, row};
use crate::ui::pointer::TextBox;
use crate::ui::theme::{self, ACCENT, BORDER};
use crate::ui::widgets::{self, Kind, button, dim, dim_small, entry_pill, tag_chips, text_field};
use crate::ui::{BrainView, Scrolls, action, truncate};
use gpui::{ClickEvent, Context, Div, ElementId, FontWeight, ScrollHandle, Stateful, div, prelude::*, px, rgb};
use kanbrain_core::model::Status;

/// Rows the form's related field shows before it scrolls: fewer than other lists, since it's inside
/// the form, under its other fields.
const FORM_LINK_ROWS: usize = 6;

/// The new / edit form: the project a new entry goes in, title and tags, and the tags already in use.
pub(super) fn draw(board: &Board, form: &Form, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let (title, submit) = match form.target {
        FormTarget::New(_, kind, _) => (format!("New {}", kind.as_str()), "Create"),
        FormTarget::Edit(key) => (format!("Edit #{}", key.id), "Save"),
    };
    let field = |which: Field, input: &LineInput, text_box: TextBox, placeholder: &str, cx: &mut Context<BrainView>| {
        text_field(text_box, input, form.field == which, placeholder, cx)
    };
    let (kind, known) = board.form_tags(form);
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .children(form.project.as_ref().map(|picker| {
            let search = field(Field::Project, &picker.search, TextBox::Project, "Search projects", cx);
            labeled("Project", project_field(board, form, picker, search, (&scroll.form_projects, follow), cx))
        }))
        .child(labeled("Title", field(Field::Title, &form.title, TextBox::Title, "What is it about?", cx)))
        .child(labeled("Tags", field(Field::Tags, &form.tags, TextBox::Tags, "e.g. prod, support", cx)))
        .child(dim_small("Separate tags with commas or spaces. Tab switches fields."))
        .when(!known.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(dim_small(format!("Tags in use in {}s", kind.as_str())))
                    .child(tag_chips(board, &known)),
            )
        })
        .child(labeled(
            "Related",
            related_field(
                board,
                form,
                field(Field::Links, &form.related.search, TextBox::Links, "Search to link, or leave empty", cx),
                (&scroll.form_links, follow),
                cx,
            ),
        ))
        .children(
            form.status().map(|status| labeled("Column", column_picker(status, form.field == Field::Status, cx))),
        );
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        // Not `enter`'s action: in the related field, enter checks an entry.
        button("submit", submit, Kind::Primary)
            .debug_selector(|| "form-save".into())
            .tooltip(widgets::tooltip("Shortcut: Enter (outside the related field)"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::save_form))),
    ];
    dialog(title, content, buttons).w(px(DIALOG_WIDTH))
}

/// Where a new task starts: To do · Doing · Parked · Done, the one picked highlighted. Clicked, or with
/// `←` `→` once `tab` reaches it, which `active` marks with a ring.
fn column_picker(status: Status, active: bool, cx: &mut Context<BrainView>) -> Div {
    let segments = COLUMNS.map(|column| {
        widgets::segment(("form-status", column as usize), theme::status_label(column), column == status)
            .debug_selector(move || format!("form-status {}", column.as_str()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.set_form_status(column))))
    });
    widgets::segmented(segments).self_start().when(active, |d| d.border_color(rgb(ACCENT)))
}

/// The project field: the brain the new entry goes in, which a click or `tab` turns into a search box
/// with every brain dropping down below, the current one checked.
fn project_field(
    board: &Board,
    form: &Form,
    picker: &Picker,
    search: Stateful<Div>,
    (scroll, follow): (&ScrollHandle, bool),
    cx: &mut Context<BrainView>,
) -> Div {
    let src = form.src();
    if form.field != Field::Project {
        let dir = board.brain(src).store.dir().display().to_string();
        let current = div()
            .id("form-project")
            .flex()
            .items_center()
            .gap_3()
            .h(px(32.))
            .px_2()
            .rounded_md()
            .bg(rgb(theme::INPUT))
            .border_1()
            .border_color(rgb(BORDER))
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, |b| b.focus_field(Field::Project))))
            .child(div().flex_none().child(board.brain_name(src).to_string()))
            .child(dim_small(dir).min_w_0().truncate())
            .child(div().flex_1())
            .child(dim("▾"));
        return div().child(current);
    }
    let rows: Vec<_> = board
        .form_projects(form)
        .into_iter()
        .enumerate()
        .map(|(index, (option, name))| {
            let dir = board.brain(option).store.dir().display().to_string();
            row(("project-option", index), format!("project option {index}"), index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.act(cx, |b| b.choose_form_project(index));
                }))
                .child(check(option == src))
                .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(name))
                .child(dim_small(dir).min_w_0().truncate())
        })
        .collect();
    let empty = rows.is_empty();
    let options = div()
        .p_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(theme::POPUP))
        .text_size(px(13.))
        .child(list("project-options", rows, picker.sel, FORM_LINK_ROWS, scroll, follow))
        .when(empty, |d| d.child(dim("No project matches.").px_2()));
    div().flex().flex_col().gap_1().child(search).child(options)
}

/// The related field: the entries checked, each a chip with ×, then the search box; while it has the
/// keys, the entries matching it drop down below, checked or not.
fn related_field(
    board: &Board,
    form: &Form,
    search: Stateful<Div>,
    (scroll, follow): (&ScrollHandle, bool),
    cx: &mut Context<BrainView>,
) -> Div {
    let src = form.src();
    let chips = form.links.iter().filter_map(|&other| Some((other, board.task(other)?))).map(|(other, task)| {
        div()
            .id(ElementId::Name(format!("linked {}/{}", other.src, other.id).into()))
            .flex()
            .items_center()
            .gap_1()
            .px_1p5()
            .rounded_sm()
            .bg(rgb(theme::SELECTED))
            .text_size(px(12.))
            .child(format!("{} {}", board.link_name(src, other), truncate(&task.title, 32)))
            .child(
                div()
                    .id(ElementId::Name(format!("unlink {}/{}", other.src, other.id).into()))
                    .cursor_pointer()
                    .text_color(rgb(theme::DIM))
                    .hover(|s| s.text_color(rgb(theme::TEXT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.unlink_in_form(other))))
                    .child("×"),
            )
    });
    let open = form.field == Field::Links;
    let options = open.then(|| {
        let rows: Vec<_> = board
            .form_link_options(form)
            .into_iter()
            .enumerate()
            .map(|(index, (key, other))| {
                let checked = form.links.contains(&key);
                row(("link-option", index), format!("link option {index}"), index == form.related.sel, true)
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.toggle_form_link(index))),
                    )
                    .child(check(checked))
                    .child(entry_pill(other))
                    .child(div().min_w_0().truncate().child(format!("{} {}", board.link_name(src, key), other.title)))
            })
            .collect();
        let empty = rows.is_empty();
        div()
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(theme::POPUP))
            .text_size(px(13.))
            .child(list("link-options", rows, form.related.sel, FORM_LINK_ROWS, scroll, follow))
            .when(empty, |d| d.child(dim("Nothing matches.").px_2()))
    });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .when(!form.links.is_empty(), |d| d.child(div().flex().flex_wrap().gap_1().children(chips)))
        .child(search)
        .children(options)
}
