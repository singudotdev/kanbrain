//! Dropdowns with a search box, hanging from the button that opens them: the project filter and the
//! tag filters. Typing filters the rows, so a long list of projects or tags stays usable.
//!
//! Also [`list`], the scrolling list every dropdown and picker uses: the same number of rows shown
//! everywhere, the highlighted row kept in view, and how many more there are below.

use super::BrainView;
use super::pointer::TextBox;
use super::theme::{self, BORDER, GREEN};
use super::widgets::{self, dim};
use crate::state::{Board, Mode, Picker};
use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, ScrollHandle, Stateful, Styled, anchored, deferred, div,
    prelude::*, px, rgb,
};

/// Height of a row in every list, fixed so a list of `MAX_ROWS` is exactly that many rows tall and
/// the rows out of view can be counted from the scroll offset.
pub const ROW_HEIGHT: f32 = 28.;
/// Rows a dropdown or picker shows before it scrolls.
pub const MAX_ROWS: usize = 8;
/// Height of the "↓ N more" line under a list that scrolls.
const MORE_HEIGHT: f32 = 20.;

/// One row: its content, whether it's the current choice (checked), and whether it's indented
/// under the row before it.
pub struct Row {
    /// What the row shows.
    pub content: AnyElement,
    /// Whether it's what's shown now.
    pub checked: bool,
    /// Whether it belongs to the row above, like a project's repository.
    pub nested: bool,
}

/// A dropdown: its search box (a `picker`), then `rows`. `name` makes the rows' ids (`name 3`) and
/// says what's missing when nothing matches.
pub struct Dropdown<'a> {
    /// What the rows are: `project`, `tag`.
    pub name: &'static str,
    /// The search typed and the highlighted row.
    pub picker: &'a Picker,
    /// Which text box its search is, for the mouse.
    pub search: TextBox,
    /// What's offered.
    pub rows: Vec<Row>,
    /// What a click on row `i` runs.
    pub choose: fn(&mut Board, usize),
    /// How wide it is.
    pub width: f32,
}

impl Dropdown<'_> {
    /// Draws it under the button that opened it; a click outside closes it. `scroll` and `follow`
    /// are its list's (see [`list`]).
    pub fn draw(self, scroll: &ScrollHandle, follow: bool, cx: &mut Context<BrainView>) -> impl IntoElement {
        let Self { name, picker, search, rows, choose, width } = self;
        let (count, empty) = (rows.len(), rows.is_empty());
        // Each drawn once: taken out as it is.
        let mut rows: Vec<Option<Row>> = rows.into_iter().map(Some).collect();
        let draw = |index: usize| {
            let drawn = row((name, index), format!("{name} {index}"), index == picker.sel, true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| choose(b, index))));
            let Some(option) = rows[index].take() else { return drawn };
            drawn
                .when(option.nested, Styled::pl_6)
                .child(div().w(px(14.)).flex_none().text_color(rgb(GREEN)).child(if option.checked {
                    "✓"
                } else {
                    ""
                }))
                .child(div().min_w_0().flex().items_center().child(option.content))
        };
        let list = list((name, usize::MAX), count, picker.sel, MAX_ROWS, scroll, follow, draw)
            .when(empty, |d| d.child(dim(format!("No {name} matches.")).px_2().py_1()));
        let panel: Div = div()
            .occlude()
            .w(px(width))
            .mt_1()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(theme::POPUP))
            .shadow_lg()
            .text_size(px(13.))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if matches!(this.board.mode, Mode::Scopes(_) | Mode::Tags(..)) {
                    this.act(cx, |b| b.mode = Mode::Normal);
                }
            }))
            .child(widgets::text_field(search, &picker.search, true, &format!("Search {name}s"), cx))
            .child(list);
        // Drawn after everything else, so it covers what's under it.
        deferred(anchored().child(panel)).with_priority(1)
    }
}

/// A row of a [`list`]: `ROW_HEIGHT` tall, as the list counts on (the line above it included), and
/// highlighted while it's the list's selection. A `clickable` one shows a pointer and lights up under
/// the mouse; `selector` names it for tests. What it shows and what a click does are the caller's.
pub fn row(id: impl Into<ElementId>, selector: String, highlighted: bool, clickable: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || selector)
        .flex()
        .items_center()
        .gap_2()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .px_2()
        // The line between it and the row above, which `list` draws: as the list view's, darker than the popup.
        .border_color(rgb(theme::BG))
        .when(highlighted, |d| d.bg(rgb(theme::SELECTED)))
        .when(clickable, Styled::cursor_pointer)
        .when(clickable && !highlighted, |d| d.hover(|s| s.bg(rgb(theme::HOVER))))
}

/// A list of `count` rows (each `ROW_HEIGHT` tall, `row` draws the one at an index) showing at most `max_rows` before
/// it scrolls. While the keyboard moved the highlight (`follow`), row `sel` is scrolled into view; while rows are out
/// of view below, a line under the list says how many, since GPUI draws no scroll bar. Only the rows of a view either
/// side of the one shown are drawn, boxes of their height standing in for the rest: the link picker lists every entry,
/// and drawing ten thousand rows took most of a second a frame.
pub fn list(
    id: impl Into<ElementId>,
    count: usize,
    sel: usize,
    max_rows: usize,
    scroll: &ScrollHandle,
    follow: bool,
    mut row: impl FnMut(usize) -> Stateful<Div>,
) -> Div {
    // The offset is negative going down.
    let was = f32::from(-scroll.offset().y);
    // Scrolled here, by the rows' heights, rather than by GPUI's scroll to a child: that measures the child against
    // the list as last laid out, which a list shown for the first time never was, and left it past the rows drawn.
    let scrolled = if follow { followed(count, sel, max_rows, was) } else { was.clamp(0., most(count, max_rows)) };
    scroll.set_offset(gpui::point(px(0.), px(-scrolled)));
    let (top, below) = hidden(count, max_rows, scrolled);
    let (first, end) = (top.saturating_sub(max_rows), (top + 2 * max_rows).min(count));
    let id = id.into();
    let (selector, above, below_rows) = (format!("{id}-list"), format!("{id}-above"), format!("{id}-below"));
    let rows = div()
        .debug_selector(move || selector)
        .id(id)
        .flex()
        .flex_col()
        .max_h(px(rows_height(max_rows)))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(div().debug_selector(move || above).flex_none().h(px(rows_height(first))))
        .children((first..end).map(|index| if index == 0 { row(index) } else { row(index).border_t_1() }))
        .child(div().debug_selector(move || below_rows).flex_none().h(px(rows_height(count - end))));
    // A list that scrolls keeps the line's room even when nothing is below, so reaching the end
    // doesn't change its height: the dialogs are centered, and a jump would shift the rows under the
    // scroll the keyboard just asked for.
    div().flex().flex_col().child(rows).children((count > max_rows).then(|| {
        div()
            // Rows hidden and none below: some are above.
            .debug_selector(move || if below > 0 { "list more" } else { "list above" }.into())
            .h(px(MORE_HEIGHT))
            .px_2()
            .pt_1()
            .text_size(px(12.))
            .text_color(rgb(theme::DIM))
            // Rows above too: a list scrolled down hid its first rows, linked entries say, with nothing to say so.
            .children(match (top, below) {
                (0, 0) => None,
                (above, 0) => Some(format!("↑ {above} above")),
                (0, below) => Some(format!("↓ {below} more · type to filter")),
                (above, below) => Some(format!("↑ {above} above · ↓ {below} more · type to filter")),
            })
    }))
}

/// How many of `count` rows are above the view and below it, with the list `scrolled` this far down: the first row in
/// view is the first more than half shown.
pub(super) fn hidden(count: usize, max_rows: usize, scrolled: f32) -> (usize, usize) {
    // Whole rows: a row is a pixel count small enough to be exact in an `f32`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let top = (((scrolled + ROW_HEIGHT / 2.) / ROW_HEIGHT).floor().max(0.) as usize).min(count);
    (top, count.saturating_sub(top + max_rows))
}

/// How far down a list of `count` rows, showing `max_rows`, scrolled `was` far, scrolls to show row `sel`: just
/// enough, and not past its end.
pub(super) fn followed(count: usize, sel: usize, max_rows: usize, was: f32) -> f32 {
    let (row_top, row_bottom) = (rows_height(sel), rows_height(sel + 1));
    was.min(row_top).max(row_bottom - rows_height(max_rows)).clamp(0., most(count, max_rows))
}

/// How far down a list of `count` rows, showing `max_rows`, scrolls at most.
fn most(count: usize, max_rows: usize) -> f32 {
    (rows_height(count) - rows_height(max_rows)).max(0.)
}

/// How tall `n` rows are; lists are short, so `n` fits a `u16`.
fn rows_height(n: usize) -> f32 {
    ROW_HEIGHT * f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}
