//! The text cursor, as in desktop apps: a thin line over the text, blinking while nothing is typed.
//! It's drawn over the text instead of into it, so the text doesn't shift when it moves.

use super::theme::TEXT;
use gpui::{Bounds, Global, Pixels, ScrollHandle, StyledText, Window, canvas, fill, point, prelude::*, px, rgb, size};
use std::time::Duration;

/// How long the cursor stays shown, then hidden, while blinking.
pub const BLINK: Duration = Duration::from_millis(530);
/// Width of the cursor line, in pixels.
const WIDTH: f32 = 1.;
/// How far inside a one-line field's edges the cursor is kept: the field's padding (`px_2`).
const INSET: Pixels = px(8.);
/// How far past that the cursor may be and the field not scroll: layout rounding, so it doesn't redraw forever.
const SLACK: Pixels = px(0.5);
/// Height of the cursor line, relative to the font size: about a glyph's ascent plus descent.
/// The line height is taller (GPUI's default is about 1.6 font sizes), so the cursor is centered in it.
const HEIGHT: f32 = 1.2;

/// Whether the blinking cursor is in its shown half: one phase for every field, set by the root view.
pub struct Blink(pub bool);

impl Global for Blink {}

/// The cursor before byte `index` of `text`, drawn once the text is laid out. Hidden in the blink's off half
/// and while the window isn't focused. In a one-line field, whose scroll is `field`, the text scrolls sideways
/// to keep it in view: a line longer than the field would otherwise be typed past its edge, unseen.
pub fn caret(text: &StyledText, index: usize, field: Option<ScrollHandle>) -> impl IntoElement + use<> {
    let layout = text.layout().clone();
    canvas(
        |_, _, _| {},
        move |_, (), window, cx| {
            let Some(at) = layout.position_for_index(index) else { return };
            if let Some(field) = &field {
                keep_in_view(field, at.x, window);
            }
            let shown = cx.try_global::<Blink>().is_none_or(|blink| blink.0);
            if !shown || !window.is_window_active() {
                return;
            }
            // The canvas is inside the text's element, so this is the text's own size.
            let font_size = window.text_style().font_size.to_pixels(window.rem_size());
            let line_height = layout.line_height();
            let height = (font_size * HEIGHT).min(line_height);
            let top = at + point(px(0.), (line_height - height) / 2.);
            window.paint_quad(fill(Bounds::new(top, size(px(WIDTH), height)), rgb(TEXT)));
        },
    )
    .absolute()
    .size_0()
}

/// Scrolls one-line `field` sideways, as little as needed, for the cursor at `x` to show inside it; the frame
/// after this one draws it there. No further than the field scrolls: at the end of a long line its border leaves the
/// cursor a pixel short of `INSET`, and asking for more, clamped back every frame, redrew it for ever.
fn keep_in_view(field: &ScrollHandle, x: Pixels, window: &Window) {
    let bounds = field.bounds();
    let (left, right) = (bounds.left() + INSET, bounds.right() - INSET);
    let shift = if x > right {
        right - x
    } else if x < left {
        left - x
    } else {
        return;
    };
    if shift.abs() <= SLACK {
        return;
    }
    let offset = field.offset();
    let to = (offset.x + shift).clamp(-field.max_offset().x, px(0.));
    if (to - offset.x).abs() <= SLACK {
        return;
    }
    field.set_offset(point(to, offset.y));
    // Drawn with the new offset in a frame of its own: a refresh while this one is drawn is dropped.
    window.on_next_frame(|window, _| window.refresh());
}
