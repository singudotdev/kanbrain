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
use gpui::{ClickEvent, Context, Div, ElementId, div, prelude::*, px, rgb};
use kanbrain_core::model::{fmt_clock, fmt_dur, now, worked};
use kanbrain_core::period::{Period, Unit};

/// A project at a glance: its description, shared by its repositories; each repository with where
/// it is, its role, its tasks per status and what's running; and the time tracked this week.
pub(super) fn draw(board: &Board, src: usize, cx: &mut Context<BrainView>) -> Option<Div> {
    let project = board.brain(src).project.as_ref()?;
    // Rename, Move, Remove and Edit on each repository (`at`), and on the project. Their keys act on the highlighted
    // repository, `src`.
    let manage = |id: &str,
                  at: usize,
                  label: &'static str,
                  key: char,
                  act: fn(&mut Board, usize),
                  cx: &mut Context<BrainView>| {
        let tip =
            if at == src { format!("Shortcut: {key}") } else { format!("Shortcut: {key}, once ↑ ↓ highlight it") };
        small(button(ElementId::Name(format!("{id} {at}").into()), label, Kind::Ghost))
            .tooltip(tooltip(tip))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| act(b, at))))
    };
    let text = |id: String, text: &str, empty: &'static str| {
        if text.is_empty() { dim(empty) } else { markdown::render(id, text) }
    };
    let now = now();
    let (from, to) = Period::current(Unit::Week).bounds();
    let repos = board.project_repos(src);
    let week = worked(repos.iter().flat_map(|&r| &board.brain(r).tasks), from, to, now);
    let repo_blocks: Vec<Div> = repos
        .iter()
        .map(|&repo| {
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
            div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(if repo == src && repos.len() > 1 { theme::ACCENT } else { theme::BORDER }))
                .child(section(
                    info.map_or_else(String::new, |i| i.repo.clone()),
                    [
                        manage("rename-repo", repo, "Rename…", 'r', Board::ask_rename_repo, cx),
                        manage("move-repo", repo, "Move…", 'M', Board::ask_move_repo, cx),
                        manage("remove-repo", repo, "Remove…", 'x', |b, at| b.ask_remove(at, false), cx),
                        manage("edit-notes", repo, "Edit", 'N', |b, at| b.open_editor(Target::RepoNotes(at)), cx),
                    ],
                ))
                .child(dim_small(root))
                .child(div().text_size(px(12.)).child(counts))
                .children(running)
                .child(text(
                    format!("notes {repo}"),
                    info.map_or("", |i| i.notes.as_str()),
                    "No role described: what this repository owns, exposes and consumes.",
                ))
        })
        .collect();
    let content = div()
        .id("manage-project-body")
        .flex()
        .flex_col()
        .gap_3()
        .max_h(px(520.))
        .overflow_y_scroll()
        .child(section(
            "About",
            [
                manage("rename-project", src, "Rename…", 'R', Board::ask_rename_project, cx),
                manage("edit-project", src, "Edit", 'E', |b, at| b.open_editor(Target::Project(at)), cx),
            ],
        ))
        .child(text(
            "description".into(),
            &project.description,
            "Not described yet: what the application is, its services and how they connect, conventions they share.",
        ))
        .child(dim_small(format!("{} tracked this week across its repositories", fmt_dur(week))))
        .child(section(
            format!("Repositories ({})", repos.len()),
            [manage("add-repo", src, "+ Add repository", 'A', Board::add_repo_to, cx)],
        ))
        .children(repo_blocks)
        .child(keys_hint(repos.len() > 1));
    let remove: Stateful<Div> = button("remove-project", "Remove project…", Kind::Danger)
        .tooltip(tooltip("Shortcut: X"))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.ask_remove(src, true))));
    let buttons = vec![remove, action("close-manage-project", "Close", Kind::Primary, KeyCode::Esc, cx)];
    Some(dialog(format!("Manage project {}", project.name), content, buttons).w(px(WIDE_WIDTH)))
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
