//! Adding a project: the folder picked in the system's dialog, the project it joins or starts, found with a search
//! or named as typed, and what adding it does.

use super::{DIALOG_WIDTH, dialog, labeled};
use crate::state::{AddProject, Board, ProjectOption};
use crate::text::event::KeyCode;
use crate::ui::dropdown::{list, row};
use crate::ui::pointer::TextBox;
use crate::ui::theme::{self, BORDER};
use crate::ui::widgets::{Kind, dim, dim_small, text_field};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::model::counted;

/// Rows of projects shown before the list scrolls: few, since it's inside the dialog.
const PROJECT_ROWS: usize = 5;

/// The folder, the project's search box with the matching projects below it (a new one first, named as typed),
/// and what adding it does.
pub(super) fn draw(
    board: &Board,
    add: &AddProject,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let rows: Vec<_> = board
        .add_project_options(&add.picker)
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            let row = row(
                ("add-project-option", index),
                format!("add project option {index}"),
                index == add.picker.sel,
                true,
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_add_project(index))));
            match option {
                ProjectOption::New(name) => row
                    .child(dim("+ New project").flex_none())
                    .child(div().font_weight(FontWeight::MEDIUM).min_w_0().truncate().child(name)),
                ProjectOption::Existing(name) => {
                    let repos = counted(board.repos_of(&name).count(), "repository", "repositories");
                    row.child(div().flex_none().font_weight(FontWeight::MEDIUM).child(name)).child(dim_small(repos))
                }
            }
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
        .child(list("add-project-options", rows, add.picker.sel, PROJECT_ROWS, scroll, follow))
        .when(empty, |d| d.child(dim("Type the new project's name.").px_2()));
    let project = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(text_field(TextBox::ProjectName, &add.picker.search, true, "Search projects, or name a new one", cx))
        .child(options)
        .child(dim_small("Pick a project you already have to add this folder to it, or start a new one."));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labeled("Folder", dim(add.dir.display().to_string()).truncate()))
        .child(labeled("Project", project))
        .child(labeled(
            "Adding it",
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(dim_small("• creates .brain/ in the folder, for the project's tasks, notes and decisions"))
                .child(dim_small("• tells its coding agents to use kanbrain, in CLAUDE.md or AGENTS.md")),
        ));
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("add-project", "Add", Kind::Primary, KeyCode::Enter, cx),
    ];
    dialog("Add a project", content, buttons).w(px(DIALOG_WIDTH))
}
