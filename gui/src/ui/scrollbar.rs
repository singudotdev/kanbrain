//! Scrollbars, for an area that scrolls with the wheel to show where it is and how much more there is, and to
//! drag. GPUI has none: a bar is a strip beside the area, its thumb drawn once the area is laid out, from the
//! same frame's scroll.

use super::theme::{BORDER, DIM, TEXT};
use gpui::{
    Along, Axis, Bounds, DispatchPhase, Div, ElementId, HitboxBehavior, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, ScrollHandle, ScrollWheelEvent, canvas, div, fill, prelude::*, px, rgb,
};
use std::cell::Cell;

/// Shortest a thumb gets, to stay big enough to grab however long what scrolls.
pub const MIN_THUMB: Pixels = px(24.);
/// Space left on each side of the thumb, across the strip.
const INSET: Pixels = px(1.);
/// Less overflow than this is layout rounding, not something to scroll to.
const SLACK: Pixels = px(1.);

/// A bar along `axis` for the area whose scroll is `scroll`, filling the strip the caller sizes and places it in.
/// It draws nothing, and takes no presses, while what the area holds fits in it. `id` keeps where the thumb was
/// grabbed from one frame to the next, so it must be unique among the bars drawn.
pub fn bar(scroll: &ScrollHandle, axis: Axis, id: impl Into<ElementId>) -> Div {
    let (scroll, id) = (scroll.clone(), id.into());
    let drawn = canvas(
        move |bounds, window, cx| {
            // Before anything returns: state no frame asks for is dropped, which would end a drag.
            let grab = window.use_keyed_state(id, cx, |_, _| Cell::new(None::<Pixels>));
            let track = bounds.size.along(axis);
            let shown = scroll.bounds().size.along(axis);
            let Some((start, len)) = thumb(track, shown, scroll.max_offset().along(axis), scroll.offset().along(axis))
            else {
                // Gone, it hears no more of the mouse: a drag it had would be picked up again by the next press
                // anywhere, once it's back.
                grab.read(cx).set(None);
                return None;
            };
            // Not blocking what's under it: a card dropped on a column's bar lands in the column.
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            Some((hitbox, grab, start, len, scroll))
        },
        move |bounds, prepainted, window, cx| {
            let Some((hitbox, grab, start, len, scroll)) = prepainted else { return };
            let (strip, thumb, radius) = painted(bounds, axis, start, len);
            let hovered = hitbox.is_hovered(window);
            window.paint_quad(fill(strip, rgb(BORDER)).corner_radii(radius));
            window
                .paint_quad(fill(thumb, rgb(thumb_color(grab.read(cx).get().is_some(), hovered))).corner_radii(radius));

            // GPUI draws nothing again when only what's under the pointer changes; as its `hover` styles do, this
            // asks for it. Either phase will do: drawing again is asked once a frame however often.
            let entered = hitbox.clone();
            window.on_mouse_event(move |_: &MouseMoveEvent, _, window, _| {
                if entered.is_hovered(window) != hovered {
                    window.refresh();
                }
            });
            // Below the area it scrolls (the board's, under the columns), the wheel would scroll nothing: it scrolls
            // the area from here, its own way, or the other for a wheel that only turns that way.
            // A line as tall as the board's text, where GPUI's own scrolling measures it, not the window's default.
            let wheeled = (hitbox.clone(), scroll.clone(), window.line_height());
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, _| {
                let (hitbox, scroll, line_height) = &wheeled;
                if bubbling(phase) && hitbox.should_handle_scroll(window) && !scroll.bounds().contains(&event.position)
                {
                    let delta = event.delta.pixel_delta(*line_height);
                    let by = if delta.along(axis) == px(0.) { delta.along(axis.invert()) } else { delta.along(axis) };
                    let to = (scroll.offset().along(axis) + by).clamp(-scroll.max_offset().along(axis), px(0.));
                    scroll_to(scroll, axis, Some(to));
                    window.refresh();
                }
            });

            let track = (bounds.origin.along(axis), bounds.size.along(axis));
            let scroll_ = scroll.clone();
            let pressed = grab.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if bubbling(phase) && event.button == MouseButton::Left && hitbox.is_hovered(window) {
                    let at = event.position.along(axis) - track.0;
                    // Grabbed where pressed; pressed beside the thumb, it jumps there, centered, and is dragged on.
                    let held = if (start..start + len).contains(&at) { at - start } else { len / 2. };
                    pressed.read(cx).set(Some(held));
                    scroll_to(&scroll_, axis, offset_at(track.1, len, scroll_.max_offset().along(axis), at - held));
                    // Before the column's own listeners: a press on its bar starts no card's drag.
                    cx.stop_propagation();
                    window.refresh();
                }
            });
            let dragged = grab.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if bubbling(phase)
                    && let Some(held) = dragged.read(cx).get()
                {
                    if event.dragging() {
                        let at = event.position.along(axis) - track.0 - held;
                        scroll_to(&scroll, axis, offset_at(track.1, len, scroll.max_offset().along(axis), at));
                        window.refresh();
                    } else {
                        // The button came up where the window couldn't see it.
                        dragged.read(cx).set(None);
                    }
                }
            });
            // Either phase: whichever comes first lets go, and the other finds nothing held.
            window.on_mouse_event(move |_: &MouseUpEvent, _, window, cx| {
                if grab.read(cx).take().is_some() {
                    window.refresh();
                }
            });
        },
    )
    .size_full();
    div().child(drawn)
}

/// Whether a window-wide mouse listener is called in the phase the bar acts in: once per event, as an element's
/// own listeners are, from the topmost element down.
fn bubbling(phase: DispatchPhase) -> bool {
    phase == DispatchPhase::Bubble
}

/// The thumb's color: lit while it's held or the pointer is on the bar.
pub fn thumb_color(held: bool, hovered: bool) -> u32 {
    if held || hovered { TEXT } else { DIM }
}

/// The track and the thumb as drawn in a bar's `bounds` along `axis`, the thumb `len` long from `start`: both
/// [`INSET`] inside its edges across it, so they show apart from what's beside the bar, and the radius that rounds
/// their ends.
pub fn painted(
    bounds: Bounds<Pixels>,
    axis: Axis,
    start: Pixels,
    len: Pixels,
) -> (Bounds<Pixels>, Bounds<Pixels>, Pixels) {
    let across = axis.invert();
    let strip = Bounds::new(
        bounds.origin.apply_along(across, |o| o + INSET),
        bounds.size.apply_along(across, |s| s - INSET * 2.),
    );
    let thumb = Bounds::new(strip.origin.apply_along(axis, |o| o + start), strip.size.apply_along(axis, |_| len));
    (strip, thumb, strip.size.along(across) / 2.)
}

/// Where the thumb starts along a `track` long bar, and how long it is, for an area showing `shown` of what it holds,
/// scrolled by `offset` out of `max` (GPUI's offsets go negative as it scrolls on); none while it all fits.
pub fn thumb(track: Pixels, shown: Pixels, max: Pixels, offset: Pixels) -> Option<(Pixels, Pixels)> {
    if max < SLACK {
        return None;
    }
    let len = (track * (shown / (shown + max))).max(MIN_THUMB).min(track);
    Some(((track - len) * (-offset / max).clamp(0., 1.), len))
}

/// The scroll offset that puts a `len` long thumb at `start` along a `track` long bar, for an area that scrolls `max`;
/// none for a thumb as long as its bar (one shorter than [`MIN_THUMB`]), which has nowhere to go.
pub fn offset_at(track: Pixels, len: Pixels, max: Pixels, start: Pixels) -> Option<Pixels> {
    let travel = track - len;
    (travel > px(0.)).then(|| -max * (start / travel).clamp(0., 1.))
}

/// Scrolls `scroll` to `offset` along `axis`, if there's one, leaving the other way as it is.
fn scroll_to(scroll: &ScrollHandle, axis: Axis, offset: Option<Pixels>) {
    if let Some(offset) = offset {
        scroll.set_offset(scroll.offset().apply_along(axis, |_| offset));
    }
}
