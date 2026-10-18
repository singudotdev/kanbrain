//! The window saying why the app couldn't open the board. Started from a menu, Finder or a desktop's launcher, the
//! app has no console to print the error to (on Windows none at all), so without it the app just wouldn't open.

use super::keys::key_events;
use super::widgets::{self, Kind};
use super::{theme, window_options};
use crate::text::event::KeyCode;
use gpui::{
    ClickEvent, ClipboardItem, Context, FocusHandle, FontWeight, KeyDownEvent, Window, div, prelude::*, px, rgb, size,
};

/// The window's title and heading: its own, so a smoke check tells it from the board's window, titled `kanbrain`.
const TITLE: &str = "kanbrain couldn't open";

/// The most the error's box takes before it scrolls.
const MESSAGE_HEIGHT: f32 = 320.;

/// Under the error, what to do: each error says what's wrong, and the ones that can, how to fix it.
const WHAT_TO_DO: &str = "Fix what it says, then open kanbrain again.";

/// Says `error` on stderr, as `main` returning it would, then in a window, and exits with 1 once it's closed.
pub fn fail(error: &anyhow::Error) -> ! {
    eprintln!("Error: {error:?}");
    // With no display (over ssh, or under sudo, which drops DISPLAY and WAYLAND_DISPLAY: the root check's own case)
    // GPUI runs headless, where the window opens unseen and its loop never ends: the app would hang, not fail.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if !shows_window(gpui::guess_compositor()) {
        std::process::exit(1);
    }
    let message = message(error);
    gpui_platform::application().run(move |cx| {
        super::load_fonts(cx);
        // Exits here rather than by GPUI's quit, which on macOS ends the process with 0, as if it had worked.
        cx.on_window_closed(|_, _| std::process::exit(1)).detach();
        let opened = cx.open_window(window_options(size(px(560.), px(280.)), cx), |window, cx| {
            window.set_window_title(TITLE);
            cx.new(|cx| StartupError::new(message, window, cx))
        });
        if let Err(e) = opened {
            eprintln!("kanbrain: could not open the window: {e:#}");
            std::process::exit(1);
        }
        cx.activate(true);
    });
    std::process::exit(1)
}

/// Whether GPUI shows a window with `compositor`, which is what it runs on, from [`gpui::guess_compositor`].
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn shows_window(compositor: &str) -> bool {
    compositor != "Headless"
}

/// The error and its causes on a line, as the status bar says an error.
fn message(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// The window's view: the error, a button to copy it (for a bug report) and one to close it. Enter and Esc close it
/// too.
pub struct StartupError {
    /// The error, as [`message`] gives it.
    message: String,
    /// Keyboard focus of the window, so Enter and Esc reach it.
    focus: FocusHandle,
}

impl StartupError {
    /// The view saying `message`, focused so the keys work right away.
    fn new(message: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self { message, focus }
    }
}

impl Render for StartupError {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let copy =
            widgets::button("copy", "Copy", Kind::Secondary).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(this.message.clone()));
            }));
        let close = widgets::button("close", "Close", Kind::Primary)
            .on_click(cx.listener(|_, _: &ClickEvent, window, _| window.remove_window()));
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                if key_events(&event.keystroke).iter().any(|key| matches!(key.code, KeyCode::Enter | KeyCode::Esc)) {
                    cx.stop_propagation();
                    window.remove_window();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(rgb(theme::BG))
            .text_color(rgb(theme::TEXT))
            .font_family(theme::UI_FONT)
            .text_size(px(14.))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(TITLE))
            .child(
                div()
                    .id("startup message")
                    .debug_selector(|| "startup message".into())
                    // The height of its text, up to MESSAGE_HEIGHT, then it scrolls; smaller in a window too short
                    // for it. Not the window's: a tiling desktop or a maximized window gives it the whole screen's.
                    .max_h(px(MESSAGE_HEIGHT))
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .bg(rgb(theme::SURFACE))
                    .text_color(rgb(theme::YELLOW))
                    .child(self.message.clone()),
            )
            .child(widgets::dim(WHAT_TO_DO))
            .child(div().flex().justify_end().gap_2().child(copy).child(close))
    }
}

#[cfg(test)]
#[path = "../../tests/startup.rs"]
mod tests;
