# 4. The edit hook takes for an entry what `kb` reads as one

Date: 2026-10-15 (recorded 2026-10-16). Status: accepted.

## Context

`kb hook edit` (`PreToolUse`, on the tools that write files) refuses an agent's write to a brain's file and sends it
to `kb`, which takes the brain's lock, numbers new entries and keeps their format. In a repository that's simple:
everything in `.brain/` is kanbrain's. The central brain isn't a folder of kanbrain's own: it may be a notes vault
(Obsidian, a dated journal), and may hold repositories, whose files are the agent's to write. Refusing the whole
folder stopped agents from working there; refusing nothing there let them edit central entries by hand.

So the hook has to tell, file by file, what in the central brain is kanbrain's. A file name pattern alone
(`0007-title.md`) misses entries `kb` reads under another name, and a parser of its own in the hook would drift from
the store's, so that what the hook lets through and what `kb` then loads disagree.

## Decision

- In the central brain, the hook refuses `tags.md`, `projects.md`, `last-id`, `format`, anything in the project
  cards' and reports' folders, and an entry: a top-level `.md` file named as one (`store::entry_named`: four digits
  or more, `-`, a lowercase letter, so a vault's `2026-10-15.md` isn't one), or one that `Task::from_markdown`, the
  store's own parser, reads as an entry or as a broken one (any error but `MISSING_ID`), as `Store::load` would
  load or report it (`hook::central_owned`).
- Only a regular file is read: a pipe, or a link to a device, would hold the hook.
- A link counts through the nearest of the file's folders that exists, so a write through a link, or to a file not
  created yet, is judged where it lands (`hook::in_brain`, `real_path`).

## Consequences

- What the hook refuses and what `kb` loads come from the same code: a change to the format moves both.
- A vault's other notes, and repositories kept in the central brain, stay the agent's to write.
- The hook reads a file only when it's at the central brain's top level and isn't one of kanbrain's by name.
