//! Long lists draw only the rows in view, or near it. GPUI draws the whole window again on every move of it (its
//! `bounds_changed` refreshes the window), and laying out every card of a long column took tens of milliseconds a
//! frame, so on Windows and X11, which report each step of a move, the window lagged behind the pointer. A row out of
//! view stands in as an empty box as tall as it was when last laid out, so the list keeps its length and every row
//! its place: the scrollbar, the wheel and the scroll to a row work as they did.

use crate::state::Key;
use gpui::{AnyElement, Div, Pixels, ScrollHandle, div, prelude::*, px};
use std::cell::RefCell;
use std::collections::HashMap;

/// A scrolling list whose rows out of view aren't drawn.
#[derive(Default)]
pub struct Rows {
    /// The list's scroll, which the element holding the rows tracks.
    pub scroll: ScrollHandle,
    /// What the last frames laid out.
    seen: RefCell<Seen>,
    /// How many frames showed a stand-in in view, for tests to tell a blank from a row drawn.
    #[cfg(test)]
    blanks: std::rc::Rc<std::cell::Cell<usize>>,
}

/// What the list's layout was like, to tell where this frame's rows go before it's laid out.
#[derive(Default)]
struct Seen {
    /// Each row's height, by its entry: rows come and go, and move with a search, so where one was says little.
    heights: HashMap<Key, Pixels>,
    /// The entry each child of the list showed in the last frame, in order: none for a header, a field or a hint.
    order: Vec<Option<Key>>,
    /// Space before the first child, the list's padding.
    pad: Pixels,
    /// Space between two children.
    gap: Pixels,
    /// A child's height on average, for one not laid out yet.
    typical: Pixels,
    /// The children that stood in, in the last frame.
    #[cfg(test)]
    stood_in: Vec<usize>,
}

impl Rows {
    /// This frame's children, the entry each shows in `order` (none for one that isn't a row), the list scrolling
    /// to the child `follow`: which rows are drawn, and which stand in.
    pub fn frame(&self, order: Vec<Option<Key>>, follow: Option<usize>) -> Frame<'_> {
        let mut seen = self.seen.borrow_mut();
        let view = self.scroll.bounds();
        // Laid out since with these children; not before the first frame, which keeps nothing.
        let laid = self.scroll.children_count();
        if laid > 0 && laid == seen.order.len() {
            let bounds: Vec<_> = (0..laid).filter_map(|ix| self.scroll.bounds_for_item(ix)).collect();
            let heights = seen.order.iter().zip(&bounds).filter_map(|(key, row)| Some(((*key)?, row.size.height)));
            seen.heights = heights.collect();
            seen.pad = bounds[0].top() - view.top();
            seen.gap = bounds.get(1).map_or(px(0.), |next| next.top() - bounds[0].bottom());
            #[expect(clippy::cast_precision_loss, reason = "a list holds far fewer than 2^23 children")]
            let typical = bounds.iter().map(|row| row.size.height).sum::<Pixels>() / bounds.len() as f32;
            seen.typical = typical;
        }
        // Where each child goes, from the heights known, in this frame's order.
        let known = |key: &Option<Key>| key.and_then(|key| seen.heights.get(&key).copied());
        let mut y = seen.pad;
        let rows: Vec<_> = order
            .iter()
            .map(|key| {
                let (top, height) = (y, known(key).unwrap_or(seen.typical));
                y += height + seen.gap;
                (top, height)
            })
            .collect();
        let height = view.size.height;
        let top = match follow.and_then(|ix| rows.get(ix)) {
            // Scrolled to, the row shows at the view's top or bottom: either way, within a view of its top.
            Some(&(top, _)) => top,
            // Where GPUI clamps the scroll back to, with fewer rows than the last frame: off by the last gap, and the
            // padding at the end, both well within a view.
            None => (-self.scroll.offset().y).clamp(px(0.), (y - height).max(px(0.))),
        };
        // A view's worth above and below too, so the wheel finds them drawn.
        let near = (top - height, top + height * 2.);
        let stand_ins = order
            .iter()
            .zip(&rows)
            .map(|(key, &(top, tall))| known(key).filter(|_| top + tall < near.0 || top > near.1))
            .collect();
        drop(seen);
        Frame { rows: self, order, stand_ins, next: 0 }
    }
}

/// One frame's children, drawn in order.
pub struct Frame<'a> {
    /// Whose rows.
    rows: &'a Rows,
    /// The entry each child shows: see [`Seen::order`].
    order: Vec<Option<Key>>,
    /// For each child, the height of the box that stands in for it, if one does.
    stand_ins: Vec<Option<Pixels>>,
    /// The child [`Frame::child`] gives next.
    next: usize,
}

impl Frame<'_> {
    /// The next child, as `draw` draws it, or an empty box as tall standing in for a row far from the view.
    pub fn child(&mut self, draw: impl FnOnce() -> AnyElement) -> AnyElement {
        let stand_in = self.stand_ins.get(self.next).copied().flatten();
        self.next += 1;
        match stand_in {
            Some(height) => div().flex_none().h(height).into_any_element(),
            None => draw(),
        }
    }

    /// Ends the frame on `list`, the element that tracks the scroll and holds the children, in order. A stand-in that
    /// lands in view anyway (a row out of view that grew) draws the next frame too, with the row in its place.
    pub fn finish(self, list: Div) -> Div {
        let Frame { rows, order, stand_ins, .. } = self;
        let stand_ins: Vec<_> = stand_ins.iter().enumerate().filter_map(|(ix, s)| s.map(|_| ix)).collect();
        let mut seen = rows.seen.borrow_mut();
        seen.order = order;
        #[cfg(test)]
        seen.stood_in.clone_from(&stand_ins);
        drop(seen);
        if stand_ins.is_empty() {
            return list;
        }
        let scroll = rows.scroll.clone();
        #[cfg(test)]
        let blanks = rows.blanks.clone();
        list.on_children_prepainted(move |_, window, _| {
            if stand_ins.iter().any(|&ix| in_view(&scroll, ix)) {
                #[cfg(test)]
                blanks.set(blanks.get() + 1);
                // Asked for while this frame is drawn, it would be dropped.
                window.on_next_frame(|window, _| window.refresh());
            }
        })
    }
}

/// Whether the list's child `ix` shows in it, as laid out last.
fn in_view(scroll: &ScrollHandle, ix: usize) -> bool {
    let view = scroll.bounds();
    let top = view.top() - scroll.offset().y;
    let bottom = top + view.size.height;
    scroll.bounds_for_item(ix).is_some_and(|row| row.bottom() > top && row.top() < bottom)
}

#[cfg(test)]
impl Rows {
    /// The children that stood in, in the last frame.
    pub fn stood_in(&self) -> Vec<usize> {
        self.seen.borrow().stood_in.clone()
    }

    /// How many frames so far showed a stand-in in view: a blank where a row should be.
    pub fn blank_frames(&self) -> usize {
        self.blanks.get()
    }
}
