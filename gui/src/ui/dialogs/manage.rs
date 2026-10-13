//! Changing a project or one of its repositories, from Manage project: renaming either, moving a repository
//! to another project, and taking a repository or the whole project out, keeping their tasks or deleting them.

use super::{DIALOG_WIDTH, dialog, labeled};
use crate::state::{Board, Manage, Picker, ProjectOption};
use crate::text::event::KeyCode;
use crate::ui::dropdown::{list, row};
use crate::ui::pointer::TextBox;
use crate::ui::theme::{self, BORDER};
use crate::ui::widgets::{Kind, dim, dim_small, text_field};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px, rgb};

/// Rows of projects shown before the list scrolls, as in the add-project dialog.
const PROJECT_ROWS: usize = 5;

/// The dialog for `manage`; `None` if its repository is gone.
pub(super) fn draw(
    board: &Board,
    manage: &Manage,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Option<Div> {
    let src = manage.src();
    let project = board.sources.get(src)?.project.clone()?;
    let repo = board.brain(src).project.as_ref().map(|p| p.repo.clone()).unwrap_or_default();
    Some(match manage {
        Manage::RenameProject { name, .. } => rename(
            format!("Rename project {project}"),
            text_field(TextBox::ManageName, &name.search, true, "The project's new name", cx),
            "Its repositories and its card follow: each repository's .brain/project.md says the new name.",
            cx,
        ),
        Manage::RenameRepo { name, .. } => rename(
            format!("Rename repository {repo}"),
            text_field(TextBox::ManageName, &name.search, true, "The repository's new name, one word", cx),
            "Kept in its .brain/project.md; the project's other repositories' links to it are rewritten \
             (api#3 becomes the new name's #3).",
            cx,
        ),
        Manage::MoveRepo { picker, .. } => move_dialog(board, src, &project, &repo, picker, scroll, follow, cx),
        Manage::Remove { whole, .. } => remove_dialog(board, &project, &repo, *whole, cx),
    })
}

/// A rename dialog: the name field and what renaming does, with Cancel and Rename.
fn rename(title: String, field: impl IntoElement, does: &'static str, cx: &mut Context<BrainView>) -> Div {
    let content = div().flex().flex_col().gap_3().child(labeled("New name", field)).child(dim_small(does));
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("rename", "Rename", Kind::Primary, KeyCode::Enter, cx),
    ];
    dialog(title, content, buttons).w(px(DIALOG_WIDTH))
}

/// Moving the repository at `src`: the project it goes to, found with the picker's search or named as typed.
#[expect(clippy::too_many_arguments, reason = "the dialog's state and the window's, as the other dialogs take them")]
fn move_dialog(
    board: &Board,
    src: usize,
    project: &str,
    repo: &str,
    picker: &Picker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let rows: Vec<_> = board
        .move_options(src, picker)
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            let row = row(("move-option", index), format!("move option {index}"), index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.apply_manage(index))));
            match option {
                ProjectOption::New(name) => row
                    .child(dim("+ New project").flex_none())
                    .child(div().font_weight(FontWeight::MEDIUM).min_w_0().truncate().child(name)),
                ProjectOption::Existing(name) => row.child(div().font_weight(FontWeight::MEDIUM).child(name)),
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
        .child(list("move-options", rows, picker.sel, PROJECT_ROWS, scroll, follow))
        .when(empty, |d| d.child(dim("Type the project's name.").px_2()));
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labeled("Repository", dim(format!("{project}/{repo}"))))
        .child(labeled(
            "To project",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(text_field(TextBox::ManageName, &picker.search, true, "Search projects, or name a new one", cx))
                .child(options),
        ))
        .child(dim_small("Its links to this project's other repositories are dropped, on both sides."));
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("move-repo", "Move", Kind::Primary, KeyCode::Enter, cx),
    ];
    dialog(format!("Move repository {repo}"), content, buttons).w(px(DIALOG_WIDTH))
}

/// Taking the repository (or, `whole`, every repository of the project) out: unlinking, or deleting the tasks too.
fn remove_dialog(board: &Board, project: &str, repo: &str, whole: bool, cx: &mut Context<BrainView>) -> Div {
    let (title, what) = if whole {
        let repos = board.repos_of(project).count();
        let what = if repos == 1 { "its repository".to_string() } else { format!("its {repos} repositories") };
        (format!("Remove project {project}?"), what)
    } else {
        (format!("Remove repository {repo} from {project}?"), "the repository".to_string())
    };
    let content = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().child(format!(
            "Unlinking takes {what} out of the project: unregistered, without the \
                                    agents' section and hooks, and without the links other repositories \
                                    have to it. The project's card goes with its last repository."
        )))
        .child(dim_small("Unlink keeps each .brain/ folder, its tasks, notes and decisions, to add it again later."))
        .child(dim_small("Unlink and delete tasks removes each .brain/ folder too: only git can bring it back."));
    let buttons = vec![
        action("cancel", "Cancel", Kind::Secondary, KeyCode::Esc, cx),
        action("remove-keep", "Unlink", Kind::Primary, KeyCode::Char('y'), cx),
        action("remove-purge", "Unlink and delete tasks", Kind::Danger, KeyCode::Char('d'), cx),
    ];
    dialog(title, content, buttons).w(px(DIALOG_WIDTH))
}
