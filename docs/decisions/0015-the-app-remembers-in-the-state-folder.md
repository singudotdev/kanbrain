# 15. The app remembers in the state folder, not in a brain

Date: 2026-11-19. Status: accepted.

## Context

Since 0.7.0 the details beside the board or list are resized by dragging their left edge, and the width lasted only
as long as the app ran. Remembering it between sessions takes somewhere per person and per computer: a project's
`.brain/` goes to the team and its agents through git, and the central brain's folder holds brains, which `kb` reads.
A screen's width is neither's to keep.

## Decision

- The app keeps what it remembers for whoever runs it in a `kanbrain-app` folder (`remembered::folder`):
  - in the state folder, `$XDG_STATE_HOME` or `~/.local/state` on Linux
    ([dirs 7.0.0: `state_dir`](https://docs.rs/dirs/7.0.0/dirs/fn.state_dir.html)), which the
    [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/0.8/) gives to "state data
    that should persist between (application) restarts, but that is not important or portable enough", naming "view,
    layout" among it;
  - where there's none (`state_dir` is none on macOS and Windows), in the local data folder
    ([`data_local_dir`](https://docs.rs/dirs/7.0.0/dirs/fn.data_local_dir.html): `~/Library/Application Support`,
    `AppData\Local`, which doesn't roam between Windows computers, as a screen's width shouldn't).
- `kanbrain-app`, the crate's name, not `kanbrain`: on macOS the local data folder is the data folder, where
  `kanbrain` is the central brain.
- One file per value: `drawer-width`, the width in pixels as text. Written when a drag of the edge ends (the button let
  go, seen or not, or the details closed meanwhile) or a double-click puts it back to 420 px, unless it's the width
  last remembered; off the window's thread, into a temporary file renamed over the last, so an app stopped mid-write
  leaves the last whole (not synced to disk: a power cut can lose it, which costs only the 420 px default).
- Writes run one after another (`BrainView::remembering`, each waiting for the one before): the background executor
  runs tasks on a pool of threads, where two writes at once would share the temporary file and could leave the older
  width, mixed bytes, or a rename of a file already gone.
- A write that fails says so in the status bar and remembers none, so the same width is written again next time.
- The installers' purge deletes the folder's files, and the folder once empty, as it does kanbrain's cache.
- dirs is already in the tree, for kanbrain-core's central brain: no new dependency.

## Consequences

- A width unreadable, not finite or not positive reads as none, so the details open 420 px wide; one too wide for the
  window is narrowed when drawn, as a dragged one, and given back in a wider window.
- Each person's width is theirs, on each computer; nothing reaches a brain, git or `kb`.
- Tests never reach the real folder: in them there's none, so nothing is remembered, unless a test names its own
  (`remembered::remember_in`, per thread). The `folder` built for the app is never compiled into a test, so its
  mutants are excluded in `.cargo/mutants.toml`; the end-to-end scripts unset `XDG_STATE_HOME`, so the app and
  installers they run look in their own home.
