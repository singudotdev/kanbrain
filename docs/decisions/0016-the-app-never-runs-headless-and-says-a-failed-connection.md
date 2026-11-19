# 16. The app never runs headless, and says a failed connection as an error

Date: 2026-11-19. Status: accepted.

## Context

GPUI from Zed 1.29.2 changed two things the app's start relied on:

- [`gpui::guess_compositor`](https://github.com/zed-industries/zed/blob/v1.29.2/crates/gpui/src/platform.rs) says
  `"headless"` with no display, where 1.23.2 said `"Headless"`. `display::ensure_display` matched the old spelling, so
  over ssh or under `sudo -u other` the app ran headless instead: the window opened unseen and its loop never ended
  (the Linux smoke check timed out, exit 124).
- On Linux, `gpui_platform::application()` no longer connects to the window system: `LinuxPlatform::run` does, in
  [`connect_initially`](https://github.com/zed-industries/zed/blob/v1.29.2/crates/gpui_linux/src/linux/platform.rs),
  which panics when the connection fails. The app caught a panic around `application()` only, so a refused connection
  would end it with 101 and Rust's panic message.

## Decision

- `ensure_display` matches `"headless"`, and its test feeds it what GPUI itself says with no display
  (`GraphicalEnvironment::default().guess_compositor()`), so a spelling change there fails a test, not the app.
- On Linux the app is made with
  [`gpui_platform::linux(WindowingModes::WAYLAND | WindowingModes::X11)`](https://github.com/zed-industries/zed/blob/v1.29.2/crates/gpui_platform/src/gpui_platform.rs):
  with no display server allowed GPUI panics rather than starts headless, so a case `ensure_display` misses fails
  instead of hanging.
- `ui::launch` runs both making the app and its `run` under `catch_unwind`. The `run` closure sets a flag first
  thing; a panic before it is the window system's (said as `couldn't connect to the window system: …`, exit 1, the
  panic hook silent), one after it is a bug in the board and unwinds on as before, hook and all.

## Consequences

- No display, a dead one, or one that refuses: exit 1 with the reason on stderr, as before 1.29.2.
- The panic hook is replaced while the app runs (it calls the previous one once the app launched), and put back after.
- `ZED_HEADLESS` set makes GPUI's guess `"headless"` too, so the app refuses to start with it.
