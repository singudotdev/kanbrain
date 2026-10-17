# Data format

Everything kanbrain knows is stored in folders of plain markdown files: the central brain, and one `.brain/`
per project repository. You can read them, edit them, grep them and version them.

```
~/.local/share/kanbrain/       ← the central brain
├── 0001-feature-export-csv.md     ← one file per task, note or decision
├── 0002-fix-login-500.md
├── tags.md                        ← tag colors
├── last-id                        ← the highest id given so far
├── format                         ← the data format's version (a brain made before 0.4.0 gets it once migrated)
├── projects.md                    ← the registered repositories, by project
├── projects/
│   └── shop.md                    ← project shop's card, shared by its repositories (named after the project:
│                                    a name other than its own slug adds a hash of itself, `acme-corp-1a2b3c4d.md`
│                                    for Acme Corp, so names that slug alike don't share one)
└── reports/                       ← markdown report exports (never read back)

~/code/shop-api/.brain/            ← a repository of project shop, made by `kb init shop`
├── 0001-rate-limit-login.md       ← its own entries, numbered from 1
├── tags.md
├── last-id
├── format
├── project.md                     ← which project it belongs to, and this repository's role in it
└── reports/                       ← `kb report --save` run in the repository (committed with `.brain/` unless
                                     ignored)
```

## Projects

The central brain's `projects.md` lists the repositories `init` registered: the project's name, then the path of
the repository's `.brain/`. Several lines with the same name are one project's repositories.

```markdown
# Projects

- shop: /home/you/code/shop-api/.brain
- shop: /home/you/code/shop-web/.brain
- blog: /home/you/code/blog/.brain
```

A project's card, `projects/<name>.md` in the central brain, holds the application's horizontal context: what it is,
its services and how they connect, the conventions they share. A name that isn't its own slug (`My_App`, `日記`) adds
a hash of itself (`projects/my-app-1a2b3c4d.md`), so names that slug alike keep a card each; its `# name` title says
whose it is. Every repository of the project shares it:

```markdown
# shop

Online shop: catalog, cart and Stripe payments. shop-api (Rust, axum + Postgres) serves the REST API; shop-web
(React) is the store and only talks to shop-api, over `/api/v1` with a JWT. Prices are in cents everywhere.
```

A repository's `.brain/project.md` names its project as its title and, in a project of several repositories, says
what this one's role is among them (how to build or run it belongs in its `CLAUDE.md`). Right under the title, a
`- repo:` line gives the repository's name in the project when it isn't its folder's (see
[`kb init`](cli.md#kb-init)):

```markdown
# shop
- repo: web

The store: UI only; every piece of data comes from shop-api, nothing is kept but the local cart.
```

All three can be edited by hand, from the kanban app, or with `kb project describe` / `project notes`. A
registered folder that's gone is skipped, and a new repository can take its name.

## Task files

Named `NNNN-slug.md`: the task id, zero-padded, plus a slug of the title. Renaming a task renames its file.

```markdown
# Fix login 500

- id: 2
- status: doing
- tags: prod, support
- created: 2026-09-24T10:12:00+02:00

## Description

Customer X reported it at 10:05.
- check nginx logs
- rollback candidate: v1.4.2

## Comments

### 2026-09-24T10:30:00+02:00 (agent, question)

Is a 30s timeout fine for the SSO provider, or should it fail faster?

### 2026-09-24T10:40:00+02:00 (edited 2026-09-24T10:52:00+02:00)

Found it: nginx timeout is 5s. Root cause confirmed.

### 2026-09-24T11:30:00+02:00

Raised to 30s, monitoring.

## Activity

- 2026-09-24T10:35:00+02:00 title (agent)
  - from: Fix login
  - to: Fix login 500
- 2026-09-24T10:36:00+02:00 tags: +prod +support -bug
- 2026-09-24T10:41:00+02:00 related (agent): +5 -shop-api#3
- 2026-09-24T10:50:00+02:00 description

## History

- 2026-09-24T10:15:00+02:00 todo -> doing (agent)
- 2026-09-24T18:00:00+02:00 doing -> parked
- 2026-09-25T09:30:00+02:00 parked -> doing

## Sessions

- 2026-09-24T10:15:00+02:00 -> 2026-09-24T18:00:00+02:00 (7h 45m)
- 2026-09-25T09:30:00+02:00 -> running
```

| part | meaning |
| --- | --- |
| `# title` | first level-1 heading |
| `- id:` | **required**, unique number. New entries get one more than the highest ever given in the folder (see `last-id`), so a deleted entry's number is never reused |
| `- status:` | `todo`, `doing`, `parked` or `done` |
| `- kind:` | `note` or `decision`; tasks have no `kind` line. Notes and decisions have no `status`, `History` or `Sessions` |
| `- decision:` | decisions only: `proposed` (waiting for a person to accept or reject it) or `rejected`. Without it a decision is accepted, in force |
| `- tags:` | comma or space separated, may be empty |
| `- links:` | ids of related entries, e.g. `3, 12`; only written when there are links. Ids are shared by tasks, notes and decisions. `shop-api#3` is entry 3 of the project's `shop-api` repository |
| `- created:` | creation time |
| `## Description` | free text about the task, kept as is. Edit it with `m` in the kanban app |
| `## Comments` | one `### <created>` heading per comment, then the comment's text, oldest first. After the time, in parentheses and separated by commas: `agent` when an agent wrote it, `question` when it's a question to the user, `answer` when it's a verdict on the decision a question asked about (which an agent may give on the user's word), `edited <time>` if it was changed later. A person's plain comment has none. A parked task whose latest question has no person's comment, nor an answer, after it waits for an answer |
| `## Activity` | one item per change to the title, tags, related entries or description, oldest first: `- TIME WHAT`, with ` (agent)` when an agent made it. A title says what it was and became on the two lines under it, as a title may say anything; tags and related entries, what was added (`+`) and removed (`-`), related entries as `- links:` writes them; a description, only that it changed. Notes and decisions have it too. Only written once there's a change; kanbrain 0.6.0 and later write it, and older versions keep it as it is |
| `## History` | one line per status change: `- TIME FROM -> TO`, with ` (agent)` when an agent made it. Oldest first. The latest `-> doing` line says who runs the task: a person runs one at a time, and the board shows it in cyan; an agent, any number, in green |
| `## Sessions` | tracked time, one line per session: `- START -> END (duration)`, or `- START -> running` for the open one. Only written once the task has been in doing; the duration is informational |

Headings inside the description must be `###` or deeper, and inside comments `####` or deeper. A `##` line starts a new
section, and inside Comments a `### <timestamp>` line starts a new comment. The kanban app's editor adjusts headings for
you. When editing by hand, a `### ` line that isn't a timestamp is kept as part of the comment.

Times are stored as RFC 3339 with your UTC offset, so files stay correct across time zones and DST changes.

## Editing by hand

Press `o` on a task (or **Open file**) to open its file in `$EDITOR`. The board reloads when you close the editor,
and also by itself whenever files in the folder change, so edits from anywhere else show up within a second.

When typing times by hand, all of these are accepted:

```
2026-09-24T10:12:00+02:00      (RFC 3339)
2026-09-24 10:12:00
2026-09-24 10:12
2026-09-24T10:12:00
2026-09-24T10:12
```

Times without an offset are read as your local time. On the next save they're rewritten in the full format.

### Fixing mistakes

**Moved a task to done too early, or forgot to.** Move it back on the board; each change adds a history line.
To correct the date of a change, open the task with `o` and edit the time on its `## History` line:

```markdown
- 2026-09-25T09:12:00+02:00 doing -> done     ← before
- 2026-09-24 18:30 doing -> done              ← after
```

History lines are sorted by time when read, so the line order doesn't matter.
The `- status:` line decides where the task stands; the history is only the record of how it got there.

### What kanbrain fixes for you

| situation | what happens on load |
| --- | --- |
| missing `status` | `todo` |
| a `running` session that isn't the last one | closed at the start of the next session |
| a `running` session on a task that isn't `doing` | closed at the task's latest status change |
| `- links:` to an entry that no longer exists (e.g. its file was deleted by hand) | removed |
| `- links:` to another repository's entry that no longer exists (e.g. its file was deleted by hand) | removed. Only once that repository can be read: while its folder is missing (an unmounted disk, say) links to it are kept, and `show` and the app leave them out |
| several tasks a person started in `doing` (e.g. from a sync) | the most recently started stays; the others are moved to `parked`, with a history line. Tasks an agent started stay |
| missing `created` | the time of the first history line, or now |
| other `- key:` lines, `- links:` words or `##` sections (e.g. written by a newer version, or by you) | kept and written back as they are; other sections move to the end of the file |
| an `## Activity` item that doesn't read as one (another kind of change, a time that isn't one) | kept and written back as it is, after the others |
| two entries with one id (a sync tool's conflict copy, or one added on two machines at once) | all but one get the next free ids: the one under its own file name keeps it, else the oldest |
| a title on several lines (as versions before 0.4.0 wrote a title typed with line breaks) | read as one line, the lines right under it joined to it |
| a byte-order mark at the start of a file (some Windows editors write one) | skipped |
| a file that can't be read (e.g. no `id`, or a history line it can't parse) | skipped, with a warning in the status bar. **The file is not touched** |

These fixes are made by whatever works in the folder: the app, which shows every brain, and `kb` run in it or
with `--project`. Reading another repository's entries to show or check a link to them, or completing a command
line, writes nothing there.

Other `.md` files in the folder that aren't tasks (such as a README) are reported as skipped and otherwise ignored.
`last-id` holds the highest id given so far, so ids aren't reused; without it (a folder from an older version),
the highest id among the entries is used.

## Format versions and migrations

Every brain (the central one, each repository's `.brain/`) is in a numbered data format, written in its `format`
file, written when the brain is made; a brain without one is format 1. A kanbrain version that changes the format
brings older brains to it with a **migration**:

- it runs when a brain is opened to be changed (a `kb` command, the kanban app), and for every registered brain on
  `kb update` (through `kb init --all`), under the brain's write lock;
- before it, the whole brain is copied to your cache folder, `<cache>/kanbrain/backups/<key>/<time>-format-<from>-to-<to>/`
  (`~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows; for a brain in the system's
  temporary folder, `<tmp>/kanbrain-cache/`, beside its write lock, both deleted once the brain is gone), so a
  migration can be undone by copying it back. Deleting a brain with `--purge` deletes its backups and lock there too;
- each step writes the new format when it's done, so an interrupted migration resumes where it stopped.

A brain in a format **newer** than your kanbrain knows (a teammate's `.brain/` from git, written by a newer version)
is refused entirely, neither read nor written, since an older version would misread what it doesn't know and
rewriting it would lose it: commands on it fail and the kanban app leaves it out, both saying to update with
`kb update`.

The release notes say when a version changes the format.

## `tags.md`

Tag colors, one per line:

```markdown
# Tag colors

Edit freely. Colors: red, orange, gold, …, or #rrggbb.

- prod: red
- support: #33aaff
```

New tags are added automatically. See [tag colors](tasks-and-tags.md#tag-colors).

## Writes are atomic

Task files are written to a temporary file (`.NNNN-slug.md.PID-N.tmp`), flushed to disk, and then renamed into
place, so a crash or power loss never leaves a half-written task file. A temporary file a crash leaves behind is
deleted once it's an hour old, the next time a `kb` command or the app writes to that brain.
On Windows, a file another program has open for a moment (an antivirus, the search indexer, an editor) can't be
replaced or deleted: the rename, or the delete, is tried again for up to about a third of a second before it fails.
