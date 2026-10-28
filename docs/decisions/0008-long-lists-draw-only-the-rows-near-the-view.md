# 8. Long lists draw only the rows near the view

Date: 2026-10-28. Status: accepted.

## Context

Moving the kanban app's window lagged on Windows and on KDE, where other apps' windows moved smoothly. GPUI (Zed's
v1.23.2, which the app pins, and Zed's main since) refreshes the whole window on every move: the platform's move
event calls [`Window::bounds_changed`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/window.rs),
which calls `Window::refresh`, and a refresh bypasses the views' cache. Windows (`WM_MOVE`) and X11 report each step
of a move, so every one redrew the board; a Wayland client isn't told where its window is. Building the board's
elements took half a millisecond; laying them out took the rest, every card included, the ones out of view too. In
GPUI's test window, a frame took 0.85 ms with no tasks, 8 ms with 20 and 75 ms with 200 to do.

Three ways out: GPUI's [`list`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/list.rs)
(the pinned tag has no versioned documentation page), which lays out only the items in view and keeps the scroll in
its own `ListState`; making each card lighter, which only divides the cost by what it
saves; and a GPUI fork that doesn't refresh on a move that leaves the size and scale as they were, a fork of Zed to
keep up to date.

## Decision

- A long list's rows out of view and of the view's height above and below it stand in as an empty box as tall as the
  row was when last laid out (`ui::offscreen`). The board's columns, its list layout and the notes and decisions lists
  draw their rows through it.
- The list keeps its element, its `ScrollHandle` and every child: the scrollbar, the wheel and the scroll to the
  selected row (`scroll_to_item`, by index) work as before. `list` would have changed each of them, and the app's
  scrollbar is drawn from a `ScrollHandle`.
- Where each row goes is reckoned before it's laid out, in this frame's order: each row's height when last laid out
  (`ScrollHandle::bounds_for_item`), kept by entry, the list's padding and gap, and the average height for a child
  not laid out yet. So a search, a reload or rows moving between columns put the rows where they'll be, not where they
  were. A row not laid out yet is always drawn.
- The rows drawn are those within a view's height of the view, or of the row the list scrolls to (the selected row,
  or the quick-add field), and with fewer rows than before, of where GPUI clamps the scroll back.
- A stand-in that ends up in view anyway (a row drawn for the first time much taller than reckoned, pushing others
  down) draws the next frame too, with the row in its place.

## Consequences

- With 200 tasks to do, a frame takes 10 ms rather than 75 in the test window; it now grows with the rows in view.
  With 2000 it takes 30 ms: the stand-ins are elements too.
- A row that changed height out of view keeps its old height until it comes near the view; drawn above the view, it
  then moves what's in view by the difference, as nothing anchors the scroll. The first frame a list is ever drawn
  draws every row; shown again, it keeps what it knew.
- Moving the window still redraws it; frames cost what the rows in view cost.
