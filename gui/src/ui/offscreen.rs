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
    /// The heights in [`Seen::heights`] added up, and how many there are: their average is reckoned for a row not
    /// laid out yet. Of every row known, not only those near the view last, so it hardly changes as the list scrolls:
    /// each change of it moves every row reckoned by it.
    known: (Pixels, u32),
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
        #[expect(clippy::cast_precision_loss, reason = "a list holds far fewer than 2^23 rows")]
        let typical = if seen.known.1 > 0 { seen.known.0 / seen.known.1 as f32 } else { FIRST_GUESS };
        let height = |key: &Option<Key>| key.map_or(typical, |key| seen.heights.get(&key).copied().unwrap_or(typical));
        // Where each item goes, from the heights known or reckoned, in this frame's order.
        let mut y = seen.pad;
        let tops: Vec<_> = order
            .iter()
            .map(|key| {
                let top = y;
                y += height(key) + seen.gap;
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
            let rows = order[..now].iter().zip(was).filter(|(key, _)| key.is_some());
            let moved: Pixels = rows.map(|(key, was)| height(key) - *was).sum();
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
            let (top, key) = (tops[ix], &order[ix]);
            key.is_some() && Some(ix) != follow && (top + height(key) < near.0 || top > near.1)
        };
        // Each run of rows far from the view, one box as tall as they are and the gaps between them.
        let mut slots = Vec::with_capacity(order.len());
        let mut child = 0;
        let mut follow_child = None;
        let mut ix = 0;
        while ix < order.len() {
            if !far(ix) {
                if Some(ix) == follow {
                    follow_child = Some(child);
                }
                slots.push(Slot::Draw);
                (ix, child) = (ix + 1, child + 1);
                continue;
            }
            let run = (ix..order.len()).take_while(|&ix| far(ix)).count();
            #[expect(clippy::cast_precision_loss, reason = "a list holds far fewer than 2^23 children")]
            let gaps = seen.gap * (run - 1) as f32;
            let tall = order[ix..ix + run].iter().map(&height).sum::<Pixels>() + gaps;
            slots.push(Slot::Box(tall));
            slots.extend(std::iter::repeat_n(Slot::Skip, run - 1));
            (ix, child) = (ix + run, child + 1);
        }
        let reckoned = order.iter().map(&height).collect();
        seen.reckoned = reckoned;
        drop(seen);
        // By its place among the children, the boxes standing in for runs of rows.
        if let Some(child) = follow_child {
            self.scroll.scroll_to_item(child);
        }
        Frame { rows: self, order, slots, children: Vec::with_capacity(child), next: 0 }
    }

    /// Learns from the last frame's layout each row's height, the list's padding and gap; and which row was the first
    /// in view, where the view should stay as long as the rows above it are the same.
    fn learn(&self, seen: &mut Seen) -> Option<Key> {
        let view = self.scroll.bounds();
        let bounds: Vec<_> = (0..seen.children.len()).filter_map(|ix| self.scroll.bounds_for_item(ix)).collect();
        let Seen { heights, children, reckoned, pad, gap, known, .. } = seen;
        for (child, bounds) in children.iter().zip(&bounds) {
            if let Child::Drawn(Some(key), ix) = child {
                let height = bounds.size.height;
                if let Some(was) = reckoned.get_mut(*ix) {
                    *was = height;
                }
                // As it was, it's left alone: added and taken out again every frame, the sum would drift.
                match heights.insert(*key, height) {
                    Some(was) if was == height => continue,
                    Some(was) => known.0 -= was,
                    None => known.1 += 1,
                }
                known.0 += height;
            }
        }
        if let Some(first) = bounds.first() {
            *pad = first.top() - view.top();
            *gap = bounds.get(1).map_or(px(0.), |next| next.top() - first.bottom());
        }
        let shown_top = view.top() - self.scroll.offset().y;
        children.iter().zip(&bounds).find_map(|(child, row)| match child {
            Child::Drawn(Some(key), _) if row.bottom() > shown_top => Some(*key),
            _ => None,
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
                Some(div().flex_none().h(height).into_any_element())
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
