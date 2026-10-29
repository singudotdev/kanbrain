//! Long lists draw only the rows in view, or near it. GPUI draws the whole window again on every move of it (its
//! `bounds_changed` refreshes the window), and laying out every card of a long column took tens of milliseconds a
//! frame, so on Windows and X11, which report each step of a move, the window lagged behind the pointer. Each run of
//! rows out of view stands in as one empty box as tall as they were when last laid out (or, never laid out, as rows
//! are on average), so a frame lays out about a view's worth of children however long the list is, the list keeps its
//! length and every row its place: the scrollbar, the wheel and the scroll to a row work as they did.

use crate::state::Key;
use gpui::{AnyElement, Div, Pixels, ScrollHandle, div, prelude::*, px};
use std::cell::RefCell;
use std::collections::HashMap;

/// A row's height reckoned before any was laid out: about a card of one line and a gap.
const FIRST_GUESS: Pixels = px(48.);
/// The view's height reckoned before the list was laid out: no screen draws much more of one list.
const FIRST_VIEW: Pixels = px(1000.);

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
    /// The most children a frame laid out, for tests to tell a list laid out whole from one drawn near the view.
    #[cfg(test)]
    most: std::cell::Cell<usize>,
}

/// What the list's layout was like, to tell where this frame's rows go before it's laid out.
#[derive(Default)]
struct Seen {
    /// Each row's height, by its entry, as last laid out: rows come and go, and move with a search, so where one was
    /// says little. Kept while it's out of the list, so a search cleared lays out no more than the view again.
    heights: HashMap<Key, Pixels>,
    /// What each child of the list was in the last frame, in order.
    children: Vec<Child>,
    /// The entry each item showed in the last frame: see [`Rows::frame`].
    order: Vec<Option<Key>>,
    /// How tall the last frame reckoned each item, in [`Seen::order`]'s order; a row it drew, as tall as it was laid
    /// out. The scroll anchors on what changed of these, not on where the items were laid out: GPUI snaps boxes to the
    /// pixel grid, and the padding, the gap and the items that aren't rows are reckoned, so what's reckoned and what's
    /// laid out differ by as much every frame. A row drawn showed as tall as it is the frame it was laid out: the view
    /// moved then, and moving it for that again would move it twice.
    reckoned: Vec<Pixels>,
    /// Space before the first child, the list's padding.
    pad: Pixels,
    /// Space between two children.
    gap: Pixels,
    /// The rows' width when the heights known were laid out: wider or narrower, rows wrap onto other lines, so the
    /// heights known then say nothing of those now.
    width: Pixels,
    /// The children that stood in, in the last frame.
    #[cfg(test)]
    stood_in: Vec<usize>,
}

/// One child of the list, as a frame laid it out.
#[derive(Clone, Copy)]
enum Child {
    /// Drawn: the row of this entry, or none for a header, a field or a hint; and its place in the frame's order.
    Drawn(Option<Key>, usize),
    /// An empty box standing in for rows out of view, as tall as they are with the gaps between them.
    Box,
}

/// What a frame does with one of the list's items.
#[derive(Clone, Copy)]
enum Slot {
    /// Draws it.
    Draw,
    /// Stands in for it and the items after it out of view with a box this tall.
    Box(Pixels),
    /// Leaves it to the box before it.
    Skip,
}

impl Rows {
    /// This frame's items, the entry each shows in `order` (none for one that isn't a row, always drawn), the list
    /// scrolling to the item `follow`: which rows are drawn, and which a box stands in for.
    pub fn frame(&self, order: Vec<Option<Key>>, follow: Option<usize>) -> Frame<'_> {
        let mut seen = self.seen.borrow_mut();
        let view = self.scroll.bounds();
        // Laid out since with these children; not before the first frame, which keeps nothing.
        let laid = self.scroll.children_count();
        #[cfg(test)]
        self.most.set(self.most.get().max(laid));
        let anchor = if laid == seen.children.len() { self.learn(&mut seen) } else { None };
        // Of every row known, not only those near the view last, so it hardly changes as the list scrolls: each change
        // of it moves every row reckoned by it. Added up anew each frame: a few microseconds at 10,000 rows, and a sum
        // kept by what changed would drift.
        #[expect(clippy::cast_precision_loss, reason = "a list holds far fewer than 2^23 rows")]
        let typical = match seen.heights.len() {
            0 => FIRST_GUESS,
            known => seen.heights.values().copied().sum::<Pixels>() / known as f32,
        };
        // Each item's height, known or reckoned, in this frame's order: looked up once, as hashing the keys of 10,000
        // rows again for each use of their heights took a tenth of a frame.
        let heights: Vec<_> = order
            .iter()
            .map(|key| key.map_or(typical, |key| seen.heights.get(&key).copied().unwrap_or(typical)))
            .collect();
        // Where each item goes.
        let mut y = seen.pad;
        let tops: Vec<_> = heights
            .iter()
            .map(|&height| {
                let top = y;
                y += height + seen.gap;
                top
            })
            .collect();
        let at_end =
            self.scroll.max_offset().y > px(0.) && -self.scroll.offset().y >= self.scroll.max_offset().y - px(0.5);
        // Scrolled to its end, the list stays there however long its rows turn out: GPUI clamps the scroll to it.
        if follow.is_none() && at_end {
            self.scroll.scroll_to_bottom();
        }
        // Rows above the view laid out at last, or reckoned anew, are taller or shorter than they were: the view stays
        // on the row it showed, as GPUI keeps the scroll in pixels. Not when rows came or went above it, which moves
        // the view's rows as it did before rows stood in.
        else if follow.is_none()
            && let Some(key) = anchor
            && let Some(now) = order.iter().position(|k| *k == Some(key))
            && let Some(then) = seen.order.iter().position(|k| *k == Some(key))
            && order[..now] == seen.order[..then]
            && let Some(was) = seen.reckoned.get(..then)
        {
            // Only the rows' heights: the items that aren't rows are laid out as they were, whatever was reckoned.
            let rows = order[..now].iter().zip(&heights).zip(was).filter(|((key, _), _)| key.is_some());
            let moved: Pixels = rows.map(|((_, height), was)| *height - *was).sum();
            if moved != px(0.) {
                let mut offset = self.scroll.offset();
                offset.y = (offset.y - moved).min(px(0.));
                self.scroll.set_offset(offset);
            }
        }
        let shown = if view.size.height > px(0.) { view.size.height } else { FIRST_VIEW };
        let top = match follow.and_then(|ix| tops.get(ix)) {
            // Scrolled to, the row shows at the view's top or bottom: either way, within a view of its top.
            Some(&top) => top,
            None if at_end => (y - shown).max(px(0.)),
            // Where GPUI clamps the scroll back to, with fewer rows than the last frame: off by the last gap, and the
            // padding at the end, both well within a view.
            None => (-self.scroll.offset().y).clamp(px(0.), (y - shown).max(px(0.))),
        };
        // A view's worth above and below too, so the wheel finds them drawn.
        let near = (top - shown, top + shown * 2.);
        let far = |ix: usize| {
            let top = tops[ix];
            order[ix].is_some() && Some(ix) != follow && (top + heights[ix] < near.0 || top > near.1)
        };
        // Each run of rows far from the view, one box as tall as they are and the gaps between them.
        let mut slots = Vec::with_capacity(order.len());
        let mut child = 0;
        let mut follow_child = None;
        // Where the box for the run of far rows so far is, among the slots.
        let mut run = None;
        for (ix, &height) in heights.iter().enumerate() {
            if !far(ix) {
                run = None;
                if Some(ix) == follow {
                    follow_child = Some(child);
                }
                slots.push(Slot::Draw);
                child += 1;
            } else if let Some(Slot::Box(tall)) = run.and_then(|at| slots.get_mut(at)) {
                *tall += seen.gap + height;
                slots.push(Slot::Skip);
            } else {
                run = Some(slots.len());
                slots.push(Slot::Box(height));
                child += 1;
            }
        }
        seen.reckoned = heights;
        let gap = seen.gap;
        drop(seen);
        // By its place among the children, the boxes standing in for runs of rows.
        if let Some(child) = follow_child {
            self.scroll.scroll_to_item(child);
        }
        Frame { rows: self, order, slots, children: Vec::with_capacity(child), next: 0, gap }
    }

    /// Learns from the last frame's layout each row's height, the list's padding and gap; and which row was the first
    /// drawn, where the view should stay as long as the rows above it are the same. Drawn near the view, it's in view
    /// or above it, and the rows between it and the view were drawn too: as tall as laid out, they move nothing.
    fn learn(&self, seen: &mut Seen) -> Option<Key> {
        let view = self.scroll.bounds();
        let bounds: Vec<_> = (0..seen.children.len()).filter_map(|ix| self.scroll.bounds_for_item(ix)).collect();
        let Seen { heights, children, reckoned, pad, gap, width, .. } = seen;
        // The first child's width: the rows', which the scrollbar's gutter narrows as the list gets long.
        let now = bounds.first().map_or(*width, |first| first.size.width);
        if now != *width {
            heights.clear();
            *width = now;
        }
        for (child, bounds) in children.iter().zip(&bounds) {
            if let Child::Drawn(Some(key), ix) = child {
                let height = bounds.size.height;
                if let Some(was) = reckoned.get_mut(*ix) {
                    *was = height;
                }
                heights.insert(*key, height);
            }
        }
        if let Some(first) = bounds.first() {
            *pad = first.top() - view.top();
            // Kept from before with one child, which shows none: a list a search cut to one row learned a gap of none,
            // and the boxes, spaced by the gap learned, kept teaching it once the search was cleared.
            if let Some(next) = bounds.get(1) {
                *gap = next.top() - first.bottom();
            }
        }
        children.iter().find_map(|child| match child {
            Child::Drawn(Some(key), _) => Some(*key),
            Child::Drawn(None, _) | Child::Box => None,
        })
    }
}

/// One frame's children, drawn in order.
pub struct Frame<'a> {
    /// Whose rows.
    rows: &'a Rows,
    /// The entry each item shows: see [`Rows::frame`].
    order: Vec<Option<Key>>,
    /// What the frame does with each item.
    slots: Vec<Slot>,
    /// What each child given so far is.
    children: Vec<Child>,
    /// The item [`Frame::child`] gives next.
    next: usize,
    /// Space between two children, as last laid out.
    gap: Pixels,
}

impl Frame<'_> {
    /// The next item, as `draw` draws it; or the box standing in for it and the rows after it far from the view; or
    /// nothing, the box before it standing in for it too.
    pub fn child(&mut self, draw: impl FnOnce() -> AnyElement) -> Option<AnyElement> {
        let ix = self.next;
        self.next += 1;
        match self.slots.get(ix).copied().unwrap_or(Slot::Draw) {
            Slot::Draw => {
                self.children.push(Child::Drawn(self.order.get(ix).copied().flatten(), ix));
                Some(draw())
            }
            Slot::Box(height) => {
                self.children.push(Child::Box);
                // Spaced from the next child as the rows are, for a list that spaces them by their margins.
                Some(div().flex_none().h(height).mb(self.gap).into_any_element())
            }
            Slot::Skip => None,
        }
    }

    /// Ends the frame on `list`, the element that tracks the scroll and holds the children, in order. A box that
    /// lands in view anyway (rows out of view that grew, or reckoned shorter than they are) draws the next frame too,
    /// with the rows in their place.
    pub fn finish(self, list: Div) -> Div {
        let Frame { rows, order, children, .. } = self;
        let boxes: Vec<_> =
            children.iter().enumerate().filter_map(|(ix, c)| matches!(c, Child::Box).then_some(ix)).collect();
        let mut seen = rows.seen.borrow_mut();
        seen.children = children;
        seen.order = order;
        #[cfg(test)]
        seen.stood_in.clone_from(&boxes);
        drop(seen);
        if boxes.is_empty() {
            return list;
        }
        let scroll = rows.scroll.clone();
        #[cfg(test)]
        let blanks = rows.blanks.clone();
        list.on_children_prepainted(move |_, window, _| {
            if boxes.iter().any(|&ix| in_view(&scroll, ix)) {
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

    /// The most children a frame laid out since last asked.
    pub fn take_most_laid_out(&self) -> usize {
        self.most.take().max(self.scroll.children_count())
    }

    /// How many frames so far showed a stand-in in view: a blank where a row should be.
    pub fn blank_frames(&self) -> usize {
        self.blanks.get()
    }
}
