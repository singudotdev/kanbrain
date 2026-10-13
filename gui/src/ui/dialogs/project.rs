//! A project at a glance, and where it's managed: its description, its repositories and the week's time, with
//! buttons to rename the project or a repository, move or remove a repository, add one, and remove the project.

use super::{WIDE_WIDTH, dialog};
use crate::state::{Board, COLUMNS, Target};
use crate::text::event::KeyCode;
use crate::ui::markdown;
use crate::ui::theme::{self};
use crate::ui::widgets::{Kind, button, dim, dim_small, section, small};
use crate::ui::{BrainView, action};
use gpui::Stateful;
use gpui::{ClickEvent, Context, Div, ElementId, div, prelude::*, px, rgb};
use kanbrain_core::model::{fmt_clock, fmt_dur, now, worked};
use kanbrain_core::period::{Period, Unit};

/// A project at a glance: its description, shared by its repositories; each repository with where
/// it is, its role, its tasks per status and what's running; and the time tracked this week.
pub(super) fn draw(board: &Board, src: usize, cx: &mut Context<BrainView>) -> Option<Div> {
    let project = board.brain(src).project.as_ref()?;
    let edit = |id: String, target: Target, cx: &mut Context<BrainView>| {
        small(button(ElementId::Name(id.into()), "Edit", Kind::Ghost))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_editor(target))))
    };
    // Rename, Move and Remove on each repository (`at`), and on the project.
    let manage = |id: &str, at: usize, label: &'static str, act: fn(&mut Board, usize), cx: &mut Context<BrainView>| {
        small(button(ElementId::Name(format!("{id} {at}").into()), label, Kind::Ghost))
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
                .border_color(rgb(theme::BORDER))
                .child(section(
                    info.map_or_else(String::new, |i| i.repo.clone()),
                    [
                        manage("rename-repo", repo, "Rename…", Board::ask_rename_repo, cx),
                        manage("move-repo", repo, "Move…", Board::ask_move_repo, cx),
                        manage("remove-repo", repo, "Remove…", |b, at| b.ask_remove(at, false), cx),
                        edit(format!("edit-notes {repo}"), Target::RepoNotes(repo), cx),
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
        .id("project-info-body")
        .flex()
        .flex_col()
        .gap_3()
        .max_h(px(520.))
        .overflow_y_scroll()
        .child(section(
            "About",
            [
                manage("rename-project", src, "Rename…", Board::ask_rename_project, cx),
                edit("edit-project".into(), Target::Project(src), cx),
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
            [manage("add-repo", src, "+ Add repository", Board::add_repo_to, cx)],
        ))
        .children(repo_blocks);
    let remove: Stateful<Div> = button("remove-project", "Remove project…", Kind::Danger)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.ask_remove(src, true))));
    let buttons = vec![remove, action("close-project-info", "Close", Kind::Primary, KeyCode::Esc, cx)];
    Some(dialog(format!("Project {}", project.name), content, buttons).w(px(WIDE_WIDTH)))
}
