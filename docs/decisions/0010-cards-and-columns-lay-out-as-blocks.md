# 10. Cards and columns lay out as blocks, spaced by margins

Date: 2026-10-29. Status: accepted.

## Context

With [decision 9](0009-one-box-per-run-of-rows-heights-kept-and-the-scroll-anchored.md), a frame of the board at
10,000 tasks to do drew about fifty cards, whatever the column's length, and took 13.4 ms in GPUI's test window: 9 ms
with 20 tasks, 1.5 ms with one. A profile of 600 frames with `perf` put 60 % of the time in taffy's flexbox layout and
14 % in hashing the column's keys. Each card was a flex column with a gap between its parts, and so was the column
holding the cards. Laid out as blocks, the same cards took a third of the time: taffy's flexbox measures each child of
a column several times to size it, where its
[block layout](https://docs.rs/taffy/0.13.0/taffy/style/enum.Display.html) lays each child out once, and GPUI lays a
`div` out as a block unless told otherwise
([`Style::default`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/style.rs)).

## Decision

- A card (`ui::board::draw_card`) and a column's body (`ui::board::column`) are blocks. A block has no gap, so each
  part of a card after the first has a top margin of the old gap, and each child of the column a bottom margin of it.
- The boxes standing in for the cards far from the view (`ui::offscreen`) have the bottom margin of the gap learned
  from the layout, so the column keeps its length. A list of one child keeps the gap it learned before: a column a
  search cut to one card learned a gap of none, the boxes were given none, and once the search was cleared, its card
  far down still selected, the column came out 8 px short for each card above.
- The column keeps its padding at the end. GPUI's scroll stops at the children's bounds plus the padding, leaving
  margins out ([`Div::prepaint`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/div.rs)),
  so the last child's margin adds nothing.
- `Rows::frame` looks each row's height up once per frame, not once for each use of it.
- The rows inside a card (its header, its tags, its footer) stay flex rows. As blocks, they saved 0.06 ms of a frame.
- The rows near the view stay those of a view above and two below. Drawing only a quarter of a view either side saved
  0.75 ms of a frame, but a fast wheel or a drag of the scrollbar would show a blank frame before the rows.

## Consequences

- Measured at 10,000 tasks in GPUI's test window, in the release build (`gui/tests/perf.rs`): a frame of the board
  takes 3.1 ms rather than 13.3, a key in the search 27 ms rather than 40, and showing the board 243 ms rather than
  300. Built without LTO, the list's frame takes 2.8 ms and the notes' 1.8 ms; the board's 3.2 ms at 10,000 tasks
  and 2.9 ms with 200, so the frame is nearly flat in the board's size: the rest
  of what grows is `Rows::frame` building its vectors and `Board::column` looking up each task, about 0.03 ms for
  each thousand tasks.
- The cards lay out to the pixel as before: their positions and sizes, compared before and after on a board of fifty.
- The brain's decision #83 chose this before caching each card in its own view. Moving the window
  refreshes it, which skips GPUI's view caches, so only a cheaper card makes that faster. Caching the cards is left
  until a frame needs it.
- A list laid out as a flex column with a gap pays the measuring again. A long list spaces its rows by margins.
