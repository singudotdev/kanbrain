# 13. Rows above the view are laid out out of sight first

Date: 2026-10-30. Status: accepted. Amends [9](0009-one-box-per-run-of-rows-heights-kept-and-the-scroll-anchored.md).

## Context

With [decision 9](0009-one-box-per-run-of-rows-heights-kept-and-the-scroll-anchored.md), a row never laid out stands
in at the mean height of those that were, and the scroll stays on the row it shows while rows above it are reckoned
anew. A row drawn for the first time, though, is as tall as it is that very frame, and nothing can be known of it before
the layout. The v0.6.8 pre-release review found that wheeling up through cards never laid out, of one, two or three
lines, moved the view by 23 to 100 px for a wheel of 60. GPUI sets a scroll after the layout only to bring a child into
view, or to put its top at the view's
([`ScrollHandle::scroll_to_item`, `scroll_to_top_of_item`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/div.rs)),
not to keep a row where it showed.

The same review found that a list shown for the first time on a row far down (`v` with the last card selected) didn't
show it: GPUI scrolls to a child only once the list was laid out and found to scroll, and puts a child at the top
against the list's bounds as last laid out, which a list never laid out has none of.

## Decision

- A row above the view, never laid out at the rows' width, that a frame would draw (a view above the one shown) is
  laid out out of sight instead (`ui::offscreen::Frame::finish`): after the list's other children, in an absolute box
  as wide as the rows and where they start, hidden, so it's neither painted nor in the way of the others' places. Taffy
  places an absolute box from the list's border, not inside its padding
  ([`compute_block_layout`](https://docs.rs/taffy/0.13.0/src/taffy/compute/block.rs.html)), and GPUI scrolls over every
  child's bounds: one in the padding made what a column scrolls over 8 px longer and wider
  ([`Visibility::Hidden`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/elements/div.rs) skips the
  paint, and so the mouse listeners). Its height is read when the box's children are prepainted. In the list, a box of
  its reckoned height stands in for it meanwhile. The next frame draws it as tall as it is, and the scroll stays on
  the row shown, as for any row reckoned anew.
- The row the scroll stays on is the first drawn that shows, no longer the first drawn: rows above it, between it and
  the view, may now be the ones laid out out of sight.
- Not while the list goes to a row, nor at its end: GPUI sets the scroll after the layout then.
- A list shown for the first time on the row it goes to draws no row above it and sets the scroll to put it at the
  view's top as reckoned (`Rows::frame`), then goes to it again the next frame, laid out, as GPUI does: the headers
  above it are reckoned, not known. Once only, so a list still not laid out then asks for no more frames.
- Every call of `Frame::finish` is given the list's children, so the rows laid out out of sight come after them and
  each child keeps its index, which the layout's bounds are read by.

## Consequences

- Wheeling up through rows never laid out moves the view by the wheel, within a pixel (the test
  `wheeling_up_through_cards_never_laid_out_moves_the_view_by_the_wheel`). A row the wheel brings in is laid out twice,
  once out of sight; a frame costs a row or two more.
- A wheel of more than a view at once, or a drag of the scrollbar, still lands on rows never laid out: they're drawn
  as they are, with nothing shown before them to keep in place.
- The list layout, the board's columns and the notes and decisions lists show the selected row the first time they're
  shown (the test `a_list_first_shown_on_a_card_far_down_shows_it`), the second frame placing it exactly.
