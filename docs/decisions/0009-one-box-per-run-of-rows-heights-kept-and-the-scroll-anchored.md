# 9. One box per run of rows, heights kept, and the scroll anchored

Date: 2026-10-28. Status: accepted. Supersedes part of [8](0008-long-lists-draw-only-the-rows-near-the-view.md).

## Context

[Decision 8](0008-long-lists-draw-only-the-rows-near-the-view.md) drew only the rows near the view, but each row out
of view was still an element, a box of its height, and a row never laid out was always drawn. With 10,000 tasks to
do, the board's index (step 2 of the performance work) built a frame in 1.6 ms, but laying out 10,000 boxes took about
65 ms. Opening the board, or clearing a search, laid out every card at once: 8.6 s to show the board and 5.3 s for a
key in the search. The heights were also replaced every frame by those of the rows laid out, so a search forgot the
heights of the rows it left out.

## Decision

- Each run of rows far from the view stands in as one empty box, as tall as the rows and the gaps between them
  (`ui::offscreen`). A frame lays out about a view's worth of children, plus a box above and below, however long the
  list is. The list's `ScrollHandle` finds a row by its index among the children, so `Rows::frame` scrolls to the row
  it follows itself, by its place among the boxes.
- A row never laid out is reckoned at the average height of the rows known, not drawn. The heights are kept by entry,
  merged frame after frame, and never forgotten while the app runs. The average is that of every row known, not of the
  last frame's rows, so it hardly moves as the list scrolls. Before any row was laid out, a row is reckoned at 48 px
  and the view at 1,000 px.
- When the heights reckoned for the rows above the first row in view change from one frame to the next (one laid out
  at last, or the average moving), the scroll moves by the difference so that row stays where it was. This is what
  GPUI's [`list`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/list.rs) does with its
  logical scroll top. It happens only when the entries above that row are the same as in the last frame; when entries
  came or went above it, the view moves as it did before.
- The comparison is between the heights reckoned in two frames, row by row, never with where the rows were laid out.
  GPUI snaps each box to the pixel grid, and the padding, the gap and the items that aren't rows (a section's header,
  the quick-add field) are only reckoned. So the reckoning and the layout differ by the same amount every frame, and an
  anchor on the layout moved the scroll by that much on every frame that changed nothing.
- A row the last frame drew counts as tall as it was laid out, not as it was reckoned: drawn, it already moved what's
  in view that frame. Counting its reckoned height made the next frame move the scroll by the same difference again,
  so the row in view landed a card's height off and stayed there.
- A list scrolled to its end stays at its end, through
  [`ScrollHandle::scroll_to_bottom`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/div.rs).
  GPUI clamps the offset to the content's height when it lays the list out.
- The rows' positions are added up anew each frame, in one pass over the list's entries. The Fenwick tree planned for
  them isn't needed: at 10,000 rows, the pass takes 0.33 ms.

## Consequences

- Measured at 10,000 tasks to do in GPUI's test window: showing the board takes 317 ms rather than 8.6 s, a search key
  41 ms rather than 3 s, and a frame 13.6 ms rather than 169. A frame now costs about the same at 1,000 tasks
  (13.2 ms), and 9.3 ms with 20. The view's own render is 0.65 ms of it; the rest is GPUI laying out and painting the
  rows in view.
- A row reckoned shorter than it is, brought into view in a frame planned for a smaller view (the window made taller),
  shows a blank for one frame, then draws in its place, as decided in 8.
- A row above the view drawn for the first time, or one drawn there that grows or shrinks, moves what's in view by
  the difference from its last height, as in any GPUI list without an anchor. The view then stays where it is.
- The scrollbar's thumb is as long as the reckoned length, which changes slightly as rows are laid out for the first
  time.
