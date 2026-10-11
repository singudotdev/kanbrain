//! A project at a glance: its description, its repositories and the week's time.

use super::{WIDE_WIDTH, dialog};
use crate::state::{Board, COLUMNS, Target};
use crate::text::event::KeyCode;
use crate::ui::markdown;
use crate::ui::theme::{self};
use crate::ui::widgets::{Kind, button, dim, dim_small, section, small};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, ElementId, div, prelude::*, px, rgb};
use kanbrain_core::model::{fmt_clock, fmt_dur, now};
use kanbrain_core::period::{Period, Unit};

/// A project at a glance: its description, shared by its repositories; each repository with where
/// it is, its role, its tasks per status and what's running; and the time tracked this week.
pub(super) fn draw(board: &Board, src: usize, cx: &mut Context<BrainView>) -> Option<Div> {
    let project = board.brain(src).project.as_ref()?;
    let edit = |id: String, target: Target, cx: &mut Context<BrainView>| {
        small(button(ElementId::Name(id.into()), "Edit", Kind::Ghost))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_editor(target))))
    };
    let text = |id: String, text: &str, empty: &'static str| {
        if text.is_empty() { dim(empty) } else { markdown::render(id, text) }
    };
    let now = now();
    let (from, to) = Period::current(Unit::Week).bounds();
    let repos = board.project_repos(src);
    let week: chrono::Duration =
        repos.iter().flat_map(|&r| &board.brain(r).tasks).map(|t| t.time_between(from, to, now)).sum();
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
                div().text_color(rgb(theme::GREEN)).child(format!(
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
                    [edit(format!("edit-notes {repo}"), Target::RepoNotes(repo), cx)],
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
        .child(section("About", [edit("edit-project".into(), Target::Project(src), cx)]))
        .child(text(
            "description".into(),
            &project.description,
            "Not described yet: what the application is, its services and how they connect, conventions they share.",
        ))
        .child(dim_small(format!("{} tracked this week across its repositories", fmt_dur(week))))
        .child(section(format!("Repositories ({})", repos.len()), []))
        .children(repo_blocks);
    let buttons = vec![action("close-project-info", "Close", Kind::Primary, KeyCode::Esc, cx)];
    Some(dialog(format!("Project {}", project.name), content, buttons).w(px(WIDE_WIDTH)))
}
