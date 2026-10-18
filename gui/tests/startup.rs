use super::*;
use anyhow::anyhow;

#[test]
fn the_message_is_the_error_and_its_causes() {
    let error = anyhow!("Permission denied").context("reading /home/me/.brain");
    assert_eq!(message(&error), "reading /home/me/.brain: Permission denied");
}

/// The window over an error, drawn.
fn window(cx: &mut gpui::TestAppContext) -> &mut gpui::VisualTestContext {
    let (_, cx) = cx.add_window_view(|window, cx| StartupError::new("it broke".into(), window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("startup message").is_some(), "the message is drawn");
    cx
}

#[gpui::test]
fn enter_closes_it(cx: &mut gpui::TestAppContext) {
    let cx = window(cx);
    cx.simulate_keystrokes("a");
    assert_eq!(cx.windows().len(), 1, "other keys leave it open");
    cx.simulate_keystrokes("enter");
    assert_eq!(cx.windows().len(), 0, "it closed");
}

#[gpui::test]
fn esc_closes_it(cx: &mut gpui::TestAppContext) {
    let cx = window(cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(cx.windows().len(), 0, "it closed");
}

#[gpui::test]
fn its_button_closes_it(cx: &mut gpui::TestAppContext) {
    let cx = window(cx);
    let close = cx.debug_bounds("button close").expect("the button is drawn").center();
    cx.simulate_click(close, gpui::Modifiers::none());
    assert_eq!(cx.windows().len(), 0, "it closed");
}

#[gpui::test]
fn its_copy_button_puts_the_message_on_the_clipboard(cx: &mut gpui::TestAppContext) {
    let cx = window(cx);
    let copy = cx.debug_bounds("button copy").expect("the button is drawn").center();
    cx.simulate_click(copy, gpui::Modifiers::none());
    assert_eq!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref(), Some("it broke"));
    assert_eq!(cx.windows().len(), 1, "it stays open");
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn without_a_display_it_shows_no_window() {
    assert!(!shows_window("Headless"), "GPUI would run headless, its loop never ending");
    assert!(shows_window("Wayland") && shows_window("X11"));
}

#[test]
fn the_smoke_check_knows_the_window_by_its_title() {
    // Quoted as PowerShell quotes it, an apostrophe doubled.
    assert!(include_str!("../../e2e/smoke-app.ps1").contains(&format!("'{}'", TITLE.replace('\'', "''"))));
}

#[gpui::test]
fn in_a_tall_window_it_keeps_to_the_height_of_its_text(cx: &mut gpui::TestAppContext) {
    // A tiling desktop or a maximized window gives it the whole screen's height, whatever size it asked for.
    let cx = window(cx);
    cx.simulate_resize(gpui::size(px(1268.), px(1383.)));
    cx.run_until_parked();
    let message = cx.debug_bounds("startup message").expect("the message is drawn");
    let close = cx.debug_bounds("button close").expect("the button is drawn");
    assert!(message.size.height < px(100.), "one line of error, in a box its height: {message:?}");
    assert!(close.top() - message.bottom() < px(100.), "the buttons right below it: {close:?} after {message:?}");
}
