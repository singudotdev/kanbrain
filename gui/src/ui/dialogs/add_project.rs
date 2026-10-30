//! Adding a project: the folder picked in the system's dialog, the project it joins or starts, found with a search
//! or named as typed, and what adding it does.

use super::{DIALOG_WIDTH, dialog, labeled, list_box};
use crate::state::{AddProject, Board, ProjectOption};
use crate::text::event::KeyCode;
use crate::ui::dropdown::{list, row};
use crate::ui::pointer::TextBox;
use crate::ui::widgets::{Kind, dim, dim_small, text_field};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px};
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
    let options = board.add_project_options(&add.picker);
    let draw = |index: usize| {
        let option = &options[index];
        let row =
            row(("add-project-option", index), format!("add project option {index}"), index == add.picker.sel, true)
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_add_project(index))),
                );
        match option {
            ProjectOption::New(name) => row
                .child(dim("+ New project").flex_none())
                .child(div().font_weight(FontWeight::MEDIUM).min_w_0().truncate().child(name.clone())),
            ProjectOption::Existing(name) => {
                let repos = counted(board.repos_of(name).count(), "repository", "repositories");
                row.child(div().flex_none().font_weight(FontWeight::MEDIUM).child(name.clone())).child(dim_small(repos))
            }
        }
    };
    let empty = options.is_empty();
    let options =
        list_box(list("add-project-options", options.len(), add.picker.sel, PROJECT_ROWS, scroll, follow, draw))
            .text_size(px(13.))
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
    // From Manage project, it adds a repository to that project, though the name can still take it elsewhere.
    let title = match add.back.and_then(|src| board.sources.get(src)?.project.as_deref()) {
        Some(project) => format!("Add a repository to {project}"),
        None => "Add a project".to_string(),
    };
    dialog(title, content, buttons).w(px(DIALOG_WIDTH))
}
