//! A project at a glance, and where it's managed: its description, its repositories and the week's time, with
//! buttons to rename the project or a repository, move or remove a repository, add one, and remove the project.

use super::{WIDE_WIDTH, dialog};
use crate::state::{Board, COLUMNS, Target};
use crate::text::event::KeyCode;
use crate::ui::markdown;
use crate::ui::theme::{self};
use crate::ui::widgets::{Kind, button, dim, dim_small, section, small, tooltip};
use crate::ui::{BrainView, action};
use gpui::Stateful;
use gpui::{ClickEvent, Context, Div, ElementId, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::model::{fmt_clock, fmt_dur, now, worked};
use kanbrain_core::period::{Period, Unit};

/// A project at a glance: its description, shared by its repositories; each repository with where
/// it is, its role, its tasks per status and what's running; and the time tracked this week. The highlighted
/// repository, `src`, scrolls into view as the arrows move it, and the keys stay in view under the list.
pub(super) fn draw(
    board: &Board,
    src: usize,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Option<Div> {
    let project = board.brain(src).project.as_ref()?;
    let now = now();
    let (from, to) = Period::current(Unit::Week).bounds();
    let repos = board.project_repos(src);
    let week = worked(repos.iter().flat_map(|&r| &board.brain(r).tasks), from, to, now);
    let several = repos.len() > 1;
    let blocks: Vec<Div> = repos.iter().map(|&repo| repo_block(board, repo, src, several, now, cx)).collect();
    // The blocks follow the four children before them.
    if follow && let Some(at) = repos.iter().position(|&r| r == src) {
        scroll.scroll_to_item(4 + at);
    }
    let body = div()
        .id("manage-project-body")
        .flex()
        .flex_col()
        .gap_3()
        .max_h(px(480.))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(section(
            "About",
            [
                manage_button("rename-project", src, src, "Rename…", 'R', Board::ask_rename_project, cx),
                manage_button(
                    "edit-project",
                    src,
                    src,
                    "Edit",
                    'E',
                    |b, at| b.open_editor_from(Target::Project(at), Some(at)),
                    cx,
                ),
            ],
        ))
        .child(described(
            "description".into(),
            &project.description,
            "Not described yet: what the application is, its services and how they connect, conventions they share.",
        ))
        .child(dim_small(format!("{} tracked this week across its repositories", fmt_dur(week))))
        .child(section(
            format!("Repositories ({})", repos.len()),
            [manage_button("add-repo", src, src, "+ Add repository", 'A', Board::add_repo_to, cx)],
        ))
        .children(blocks);
    let content = div().flex().flex_col().gap_3().child(body).child(keys_hint(several));
    let remove: Stateful<Div> = button("remove-project", "Remove project…", Kind::Danger)
        .tooltip(tooltip("Shortcut: X"))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.ask_remove(src, true))));
    let buttons = vec![remove, action("close-manage-project", "Close", Kind::Primary, KeyCode::Esc, cx)];
    Some(dialog(format!("Manage project {}", project.name), content, buttons).w(px(WIDE_WIDTH)))
}

/// One of the project's repositories: its name with its buttons, where it is, its tasks per status, what's running
/// and its role; outlined when it's the highlighted one of `several`.
fn repo_block(
    board: &Board,
    repo: usize,
    src: usize,
    several: bool,
    now: chrono::DateTime<chrono::Local>,
    cx: &mut Context<BrainView>,
) -> Div {
    let brain = board.brain(repo);
    let info = brain.project.as_ref();
    let counts = COLUMNS
        .iter()
        .map(|&status| format!("{} {}", brain.column(status).len(), theme::status_label(status).to_lowercase()))
        .collect::<Vec<_>>()
        .join(" · ");
    let root = brain.store.dir().parent().unwrap_or(brain.store.dir()).display().to_string();
    let running = brain.running().map(|t| {
        div().text_color(rgb(theme::running_color(t))).child(format!(
            "▶ #{} {} {}",
            t.id,
            t.title,
            fmt_clock(t.running_for(now).unwrap_or_default())
        ))
    });
    let notes = |b: &mut Board, at: usize| b.open_editor_from(Target::RepoNotes(at), Some(at));
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(if repo == src && several { theme::ACCENT } else { theme::BORDER }))
        .child(section(
            info.map_or_else(String::new, |i| i.repo.clone()),
            [
                manage_button("rename-repo", repo, src, "Rename…", 'r', Board::ask_rename_repo, cx),
                manage_button("move-repo", repo, src, "Move…", 'M', Board::ask_move_repo, cx),
                manage_button("remove-repo", repo, src, "Remove…", 'x', |b, at| b.ask_remove(at, false), cx),
                manage_button("edit-notes", repo, src, "Edit", 'N', notes, cx),
            ],
        ))
        .child(dim_small(root))
        .child(div().text_size(px(12.)).child(counts))
        .children(running)
        .child(described(
            format!("notes {repo}"),
            info.map_or("", |i| i.notes.as_str()),
            "No role described: what this repository owns, exposes and consumes.",
        ))
}

/// A button that runs `act` on the repository `at` (or the project, through one of its brains). Its key acts on the
/// highlighted repository, `src`, which the tooltip says.
fn manage_button(
    id: &str,
    at: usize,
    src: usize,
    label: &'static str,
    key: char,
    act: fn(&mut Board, usize),
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let tip =
        if at == src { format!("Shortcut: {key}") } else { format!("Shortcut: {key}, once ↑ ↓ highlight it") };
    small(button(ElementId::Name(format!("{id} {at}").into()), label, Kind::Ghost))
        .tooltip(tooltip(tip))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| act(b, at))))
}

/// `text` as markdown, or `empty` dimmed when there's none.
fn described(id: String, text: &str, empty: &'static str) -> Div {
    if text.is_empty() { dim(empty) } else { markdown::render(id, text) }
}

/// The dialog's keys, under its repositories: with `several`, the arrows choose which one they act on.
fn keys_hint(several: bool) -> Div {
    dim_small(if several {
        "↑ ↓ highlight a repository for r rename, M move, x remove and N its role; R renames the project, E describes \
         it, A adds a repository and X removes it."
    } else {
        "r renames the repository, M moves it, x removes it and N edits its role; R renames the project, E describes \
         it, A adds a repository and X removes it."
    })
}
