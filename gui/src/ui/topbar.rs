//! The top of the window: tabs, the search box and the window's buttons; under them, the task whose
//! clock is running (time is always tracked), with today's total.

use super::dropdown::{Dropdown, Row};
use super::pointer::TextBox;
use super::theme::{self, BORDER, YELLOW};
use super::widgets::{self, Kind, dim, dim_small, segment, segmented, small, tag_chips};
use super::{BrainView, Scrolls, action, tag_filter};
use crate::state::{Board, Key, Layout, Mode, Picker, Scope, Tab, Target};
use crate::text::event::KeyCode;
use gpui::{
    ClickEvent, Context, Div, ElementId, FontWeight, Image, ImageFormat, ScrollHandle, Stateful, div, img, prelude::*,
    px, rgb,
};
use kanbrain_core::model::{Kind as EntryKind, Status, Task, fmt_clock, fmt_dur, now, worked};
use kanbrain_core::period::{Period, Unit};

/// Width of the project dropdown.
const SCOPE_DROPDOWN_WIDTH: f32 = 300.;

/// Narrowest the search box gets, as the window narrows, before the bar's buttons wrap to a second line.
const SEARCH_MIN: f32 = 160.;
/// Widest the search box gets: enough for a few words and tags.
const SEARCH_MAX: f32 = 260.;

/// Side of the logo that starts the bar: the height of the tabs beside it.
const LOGO_SIZE: f32 = 28.;

/// The logo, in place of the app's name. One image for the whole run, since GPUI caches its render by the image's id.
static LOGO: std::sync::LazyLock<std::sync::Arc<Image>> = std::sync::LazyLock::new(|| {
    std::sync::Arc::new(Image::from_bytes(ImageFormat::Svg, include_bytes!("../../../docs/images/logo.svg").to_vec()))
});

/// Tabs, search, new entry and the window's buttons; the projects filter, with the shown project's
/// card; then the running tasks.
pub fn draw(board: &Board, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(toolbar(board, scroll, follow, cx))
        .child(scope_bar(board, scroll, follow, cx))
        .child(running_bar(board, cx))
}

/// Which brains are shown: a button naming the scope (all, the central brain, a project, or one
/// repository of a project with several) that opens a dropdown with a search box, so it works with
/// many projects; for a project, what it's about, with buttons to describe it; and a button to add a
/// project. While there are none, only that button, with what a project is.
fn scope_bar(board: &Board, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let add = action("add-project", "+ Add project", Kind::Ghost, KeyCode::Char('A'), cx)
        .debug_selector(|| "add-project-button".into());
    let bar = div().flex().items_start().gap_3().px_3().py_1p5().border_b_1().border_color(rgb(BORDER));
    if board.sources.len() < 2 {
        let hint = "No projects yet: add a code folder to keep its tasks, notes and decisions next to its code.";
        return bar.items_center().child(add).child(dim_small(hint));
    }
    let name = board.scopes().into_iter().find(|(s, _)| *s == board.scope).map_or_else(String::new, |(_, n)| n);
    let button = action("scope", format!("Project: {name}  ▾"), Kind::Secondary, KeyCode::Char('P'), cx);
    let picker = div().flex().flex_col().child(button).children(match &board.mode {
        Mode::Scopes(picker) => Some(scope_dropdown(board, picker, &scroll.dropdown, follow, cx)),
        _ => None,
    });
    bar.child(picker)
        .child(project_card(board, cx).unwrap_or_else(div).flex_1().min_w_0().pt_1())
        .children(board.project_src().map(|_| {
            // Named for what it's mostly opened for, and drawn as a button: as "Project info", without a border,
            // it read as a label, and nothing said the project is managed there.
            action("manage-project", "Manage project", Kind::Secondary, KeyCode::Char('I'), cx)
                .debug_selector(|| "manage-project-button".into())
        }))
        .child(add)
}

/// The dropdown under the project button: every scope matching its search, a project's repositories
/// indented under it.
fn scope_dropdown(
    board: &Board,
    picker: &Picker,
    scroll: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> impl IntoElement {
    let rows = board
        .scope_candidates(picker)
        .into_iter()
        .map(|(scope, name)| Row {
            checked: board.scope == scope,
            nested: matches!(scope, Scope::Repo(_)),
            content: div().truncate().child(name).into_any_element(),
        })
        .collect();
    Dropdown {
        name: "project",
        picker,
        search: TextBox::ScopeSearch,
        rows,
        choose: Board::choose_scope,
        width: SCOPE_DROPDOWN_WIDTH,
    }
    .draw(scroll, follow, cx)
}

/// The shown project's description, shared by its repositories, and the shown repository's role,
/// each with an Edit button.
fn project_card(board: &Board, cx: &mut Context<BrainView>) -> Option<Div> {
    let src = match &board.scope {
        Scope::Repo(src) => *src,
        Scope::Project(_) => board.scoped().next()?.0,
        Scope::All | Scope::Central => return None,
    };
    let project = board.brain(src).project.as_ref()?;
    let first = |text: &str| text.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().to_string();
    let line = |label: String, text: &str, button: Stateful<Div>| {
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(12.))
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(label))
            .child(if text.is_empty() { dim("Not described yet.") } else { dim(first(text)) }.min_w_0().truncate())
            .child(small(button))
    };
    let edit = |id: &'static str, target: Target, cx: &mut Context<BrainView>| {
        widgets::button(id, "Edit", Kind::Ghost)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_editor(target))))
    };
    let card = div().flex().flex_col().gap_0p5().child(line(
        project.name.clone(),
        &project.description,
        edit("edit-project", Target::Project(src), cx),
    ));
    let repo = matches!(board.scope, Scope::Repo(_))
        .then(|| line(project.repo.clone(), &project.notes, edit("edit-repo-notes", Target::RepoNotes(src), cx)));
    Some(card.children(repo))
}

/// The tabs with their counts, the search box, and New / Reload / Shortcuts.
fn toolbar(board: &Board, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let entries = || board.scoped().flat_map(|(_, brain)| &brain.tasks);
    let count = |kind: EntryKind| entries().filter(|t| t.kind == kind).count();
    let active = entries().filter(|t| t.is_task() && t.status != Status::Done).count();
    let tabs: Vec<_> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(index, &tab)| {
            let label = match tab {
                Tab::Board => format!("Board  {active}"),
                Tab::Notes => format!("Notes  {}", count(EntryKind::Note)),
                Tab::Decisions => format!("Decisions  {}", count(EntryKind::Decision)),
                Tab::Report | Tab::Calendar => tab.label().to_string(),
            };
            segment(("tab", index), label, board.tab == tab)
                .tooltip(widgets::tooltip(format!("Shortcut: {}", index + 1)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.switch_tab(tab))))
        })
        .collect();
    let searching = matches!(board.mode, Mode::Search);
    // It gives up width first, down to SEARCH_MIN, so a narrower window keeps the bar on one line.
    let search = widgets::text_field(TextBox::Search, &board.search, searching, "Search titles, or #tag", cx)
        .flex_1()
        .min_w(px(SEARCH_MIN))
        .max_w(px(SEARCH_MAX));
    let filtered = !board.search.is_empty();
    let clear = filtered.then(|| small(action("clear-search", "Clear", Kind::Ghost, KeyCode::Esc, cx)));
    let new_label = match board.tab {
        Tab::Notes => "+ New note",
        Tab::Decisions => "+ New decision",
        Tab::Board | Tab::Report | Tab::Calendar => "+ New task",
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .bg(rgb(theme::SURFACE))
        .child(img(LOGO.clone()).size(px(LOGO_SIZE)).flex_none().mr_2())
        .child(segmented(tabs))
        .children((board.tab == Tab::Board).then(|| layout_switch(board, cx)))
        // Text and tag filter every tab, like the project filter under them.
        .child(search)
        .child(tag_filter(board, scroll, follow, cx))
        .children(clear)
        // One group, pushed right: in a window too narrow for the bar it wraps whole, still on the right.
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .ml_auto()
                .children(waiting_button(board, cx))
                .child(action("new", new_label, Kind::Primary, KeyCode::Char('n'), cx))
                .child(action("reload", "Reload", Kind::Ghost, KeyCode::Char('R'), cx))
                .child(action("commands", "Commands", Kind::Ghost, KeyCode::Char('?'), cx)),
        )
}

/// How many tasks wait for the user's answer, when any do; each click opens the next one.
fn waiting_button(board: &Board, cx: &mut Context<BrainView>) -> Option<Stateful<Div>> {
    let waiting = board.waiting().len();
    let label = if waiting == 1 { "1 waiting for you".to_string() } else { format!("{waiting} waiting for you") };
    (waiting > 0).then(|| {
        widgets::button("waiting", label, Kind::Secondary)
            .tooltip(widgets::tooltip("Tasks an agent asked you about · Shortcut: w"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::next_waiting)))
    })
}

/// Kanban or List, for the board tab.
fn layout_switch(board: &Board, cx: &mut Context<BrainView>) -> Div {
    let segments = [(Layout::Kanban, "Kanban"), (Layout::List, "List")].map(|(layout, label)| {
        segment(("layout", layout as usize), label, board.layout == layout)
            .tooltip(widgets::tooltip("Shortcut: v"))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.layout = layout)))
    });
    segmented(segments)
}

/// Each running task with its clock, or "idle"; today's tracked total; Park and Park & quit.
fn running_bar(board: &Board, cx: &mut Context<BrainView>) -> Div {
    let now = now();
    let (from, to) = Period::current(Unit::Day).bounds();
    // By the clock: tasks that ran at once (yours beside an agent's) count that time once.
    let today = worked(board.sources.iter().flat_map(|s| &s.brain.tasks), from, to, now);
    let bar = div()
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .px_3()
        .py_1p5()
        .border_b_1()
        .border_color(rgb(BORDER))
        .text_size(px(13.));
    let running = board.running();
    let bar = if running.is_empty() {
        bar.child(div().text_color(rgb(YELLOW)).font_weight(FontWeight::SEMIBOLD).child("■ Idle"))
            .child(dim("Nothing is running: drag a card to Doing, or press s on it."))
    } else {
        // Each brain runs its own task, so there can be several: one per project repository.
        let items = running.iter().map(|&(key, task)| running_item(board, key, task, now, cx));
        bar.child(div().flex().flex_wrap().items_center().gap_x_4().gap_y_1().min_w_0().children(items))
    };
    bar.child(div().flex_1()).child(dim(format!("Today {}", fmt_dur(today))).flex_none()).children(
        (!running.is_empty()).then(|| small(action("park-quit", "Park & quit", Kind::Ghost, KeyCode::Char('Q'), cx))),
    )
}

/// One running task: where it is, its title and tags, its clock and total, and a Park button.
fn running_item(
    board: &Board,
    key: Key,
    task: &Task,
    now: chrono::DateTime<chrono::Local>,
    cx: &mut Context<BrainView>,
) -> Div {
    let place = board.sources[key.src].label.as_ref().map(|l| format!("{l} ")).unwrap_or_default();
    div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(div().text_color(rgb(theme::running_color(task))).font_weight(FontWeight::SEMIBOLD).child("▶"))
        .child(
            div()
                .id(ElementId::Name(format!("running {}/{}", key.src, key.id).into()))
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .hover(|s| s.text_color(rgb(theme::ACCENT)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_entry(key))))
                .child(format!("{place}#{} {}", task.id, task.title)),
        )
        .child(tag_chips(board.brain(key.src), &task.tags).flex_nowrap().flex_none())
        .child(
            div()
                .text_color(rgb(theme::running_color(task)))
                .font_family(theme::EDITOR_FONT)
                .child(fmt_clock(task.running_for(now).unwrap_or_default())),
        )
        .child(dim_small(format!("total {}", fmt_dur(task.total(now)))))
        .child(
            small(widgets::button(
                ElementId::Name(format!("park {}/{}", key.src, key.id).into()),
                "Park",
                Kind::Secondary,
            ))
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.set_status(key, Status::Parked))),
            ),
        )
}
