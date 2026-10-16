# 3. Entries log their changes in an `## Activity` section, as additions and removals

Date: 2026-10-16. Status: accepted.

## Context

An entry's file says who commented and who moved it (`## Comments`, `## History`), but not who changed its title,
tags, related entries or description, nor when. With agents editing entries through `kb` while the user edits them
on the board, "who changed this, and from what?" had no answer. GitHub and GitLab answer it with a line in the
issue's thread for each change, among the comments.

Every released version keeps a `##` section it doesn't know and writes it back (`Unknown::sections`), so a new section
needs no format step or migration: an older `kb` or app saving the entry keeps it, and only doesn't add to it. A line
an older version can't parse in a section it knows, such as `## History`, would make it skip the whole file.

`model::Task` was a public struct with every field public: adding a field breaks `kanbrain-core`'s API
(`constructible_struct_adds_field`), which under [ADR 1](0001-kanbrain-core-follows-cargo-semver.md) takes the breaking
slot. Keeping the log as text inside `Unknown::sections` would have avoided that, but leaves the model's "unknown"
holding what this version knows, parsed on every read; the user chose the field, and v0.6.0.

## Decision

- `Task::activity` holds `model::Activity` items (when, who, what `Changed`), written in `## Activity` after the
  comments, and only once there's one. A title has its old and new value on lines of their own (a title may say
  anything, ` -> ` and `(agent)` too); tags and related entries record what was added and removed (`+ux -bug`,
  `+5 -shop-api#3`), so a relation reads the same on both entries and a renamed tag says so in each; a description
  records only that it changed, since one can be long.
- An item this version can't read is kept as written (`Unknown::activity`), and so is text before the first item, as
  an `## Activity` of someone's own from before 0.6.0 has: the section is a record, never a reason to lose a file.
- The latest change, when the same author made it in the same second to the same thing, takes in the next
  (`Task::log`), and one that ends up changing nothing goes: the app saves a form's related entries one at a time.
  Any other change between them keeps them apart, so the order stays what happened. A change logged with an earlier
  time (another computer's clock) goes in its place.
- Logged where a person or an agent edits an entry: `Brain::update`, `set_description`, `toggle_link` and `link`
  (both entries, whichever keeps the link), `toggle_repo_link` and `link` to another repository (only the entry whose
  `- links:` line changes, as writing the other repository's files would take its lock too), and `rename_tag`
  (`kb tag OLD NEW`, new; a tag named as an entry's id is renamed only when written `#5`, as `kb tag 5 ui` is more
  likely `kb tag 5 +ui` mistyped, and a rename that wasn't meant changes every entry). Not what follows from
  something else: deleting or moving an entry, or loading dropping links to one deleted by hand.
- `Task::thread` interleaves comments and changes by time, a change first within its second (the app's **Save and
  tell** comments after the change it tells of); the app's details list it, `kb show` its latest 10 changes (the rest
  counted, as History's), and `--json` all of `activity`.
- `Task`, `Comment`, `StatusChange`, `Session`, `Unknown`, `Activity`, `Changed` (not its variants, which other crates
  build for `Task::log`), the JSON shapes and `api::ApiError` are `#[non_exhaustive]` from v0.6.0, so a field or
  variant added later isn't breaking. `Kind`, `Status`, `DecisionState`, `period::Unit` and `brain::Group` stay
  exhaustive: the app matches on them, and should be told when one grows.

## Consequences

- v0.6.0 breaks `kanbrain-core`'s API only; `kb`, the app and the data need nothing migrated, and the data format
  stays 1: a file without the section reads as one with nothing logged, and a `migrate::FORMAT` step would only have
  older versions refuse the whole brain (a teammate's, read from git) over a section they already keep. An older
  version on the same brain keeps it (written last in the file) and doesn't log its own changes; `e2e/upgrade.sh`
  checks both ways from every release.
- Outside the crate, these types are built through their constructors (`Task::new`, `Comment::new`, `Activity::new`),
  status changes and sessions only by `Task::set_status` and `start_session`, and entries changed through their
  fields or `Task::set_title`, `set_tags`, `set_description`, `relate` and `log`, which log.
- Setting a field directly logs nothing: what changes an entry for someone goes through those, or `Brain`'s actions.
