# 7. The editor merges what `kb` changed, and asks only about lines both changed

Date: 2026-10-16. Status: accepted.

## Context

While the kanban app's editor or form is open, an agent may change the same text with `kb`. Since v0.5.1 a save over
such a change stopped, with the editor open, and saving again replaced what the agent wrote: the user chose between
their whole text and the agent's, even when the two had touched different lines. The form already merged the fields
only one side changed, and the tags and related entries, which are sets; only a title both changed stopped it. The
activity log (ADR 3) says who changed what, and when.

A merge needs a line diff and a three-way merge. Nothing in the tree had one. The candidates were writing one (Myers'
diff and diff3, a few hundred lines to keep and test), [diffy](https://docs.rs/diffy), whose `merge` returns only the
text with `<<<<<<<` markers in it, which a text holding such a line breaks, and
[similar](https://docs.rs/similar/3.2.0/similar/merge/index.html), whose `TextMerge` gives each region (unchanged,
ours, theirs, both, conflict) as data.

## Decision

- Saving the editor over a text `kb` changed since it opened (`Edit::base`) merges the three line by line
  (`text::merge`, on similar 3.2's `TextMerge`, with no default features: nothing of its own to build). Lines only
  one side changed take that change, and the save goes on. Lines both changed, differently, stop it.
- `similar`, like `git merge`, takes changes that only touch (a line edited, and one added right after it) for one
  conflict. Those are applied both (`untangled`): an agent adding a line under the one you edit is the usual case, and
  no line of the text both changed. Lines added at one place on both sides, or within lines the other side changed,
  stay a conflict.
- Keys and buttons name what they do with the conflicts: `esc`, which saves in the editor, takes yours where nothing's
  picked, keeping what `kb` changed elsewhere, rather than going back to your whole text over theirs. A text `kb`
  changes again while the conflicts are picked is merged again.
- The conflicts show in the editor's place, inside the same dialog, not in a window of their own: an agent writing
  every few seconds would leave a separate view stale, and most conflicts here are a line or a title. Each shows yours
  beside theirs, to take yours, theirs or both; the merged text then goes back into the editor, to look over and save.
  Who changed the text, and when, comes from the activity log.
- The form keeps merging fields, and shows a title both changed under its field, to take or to keep yours.
- Every line ends in a break for the merge, so a text's last line, which has none, is the same line as it is with
  one after it.

## Consequences

- A dependency, `similar`, in the app only (Apache-2.0, which `deny.toml` allows; `Cargo.lock` lists `bstr`, one of
  its optional features, which isn't built); `kanbrain-core` and `kb` don't change, nor does the data, so nothing
  needs migrating.
- Saving back to back with what an agent wrote keeps both, which `save again to replace it` didn't.
- A conflict is per region of lines: two changes on one line, even at its two ends, are one conflict.
