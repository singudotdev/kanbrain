# Tasks, tags & search

## The board

The **Board** tab (`1`) has one column per status:

| column | means |
| --- | --- |
| **To do** | not started |
| **Doing** | being worked on, with its clock running. **One of yours at a time per project repository** (and in the central brain): starting another there is refused until you park or finish it. Agents can run several (see [Time tracking](time-tracking.md)) |
| **Parked** | started, then set aside: blocked, waiting on someone, or interrupted |
| **Done** | finished. The 20 most recently finished show; **Show N older** (or `f`) shows the rest |

Within a column, the task that moved there most recently is on top. Nothing is ranked by hand: the order is the
order things happened.

Each card shows its id, how long it's been in its column (`5m`, `3h`, `2d`, `6w`), its title and tags, and what it
carries: how many comments and related entries, its tracked time, or the running clock ▶ on the task in Doing.

### Kanban or list

**Kanban | List** next to the tabs (or `v`) switches how the board lays its tasks out. **List** shows the same
tasks, in the same order, as one list with a section per status: each task is a single row (id, title, tags, what
it carries, its age), so long titles fit and more tasks show at once. Everything else works the same: drag a row
onto a section's header to move it there, `+` on a header opens the new task form with that section picked, `↑` `↓` go row by
row through every section, and `shift+←` `shift+→` move the selected task to the previous or next status.

## Projects

The app shows the central brain and every project registered with `kb init` together.

To add a project from the app, click **+ Add project** under the toolbar (or press `A`):

1. Pick the project's code folder.
2. Choose the project. Pick one you already have from the list, to add the folder as another of its repositories,
   or type a new name. The folder's name is filled in for you.
3. Press **Add**. The app does what `kb init` does there: it creates `.brain/` and tells the project's agents to use
   kanbrain.

While there are projects, the **Project ▾** button under the toolbar filters what every tab shows. It opens a dropdown with a search
box, so it stays quick with many projects: type part of a name, then click it or press `enter`.

| filter | shows |
| --- | --- |
| **All** | everything; each card says where it's from (`shop/api #3`) |
| **Central** | what's outside any project |
| **shop** | every repository of project shop |
| **shop/api** | one repository, for a project with several |

`P` opens the dropdown from the keyboard. Started from inside a project, the app opens on it. With a project shown, the bar also shows
its description, shared by its repositories, and with one repository shown, that repository's role; **Edit** changes
them (`E` the description). **Manage project**, before **+ Add project** at the end of the bar (`I`), shows the whole project: its description, each repository with
its folder, notes, tasks per status and what's running, and the time tracked this week.

It's also where a project is managed:

| button | does |
| --- | --- |
| **Rename…** by About | renames the project |
| **Rename…** on a repository | renames it in the project (one word); the other repositories' links to it follow |
| **Move…** on a repository | moves it to another project, found with a search or named new; its links to this project's repositories are dropped |
| **Remove…** on a repository, **Remove project…** | takes it (or every repository of the project) out: **Unlink** keeps its `.brain/` and tasks, to add it again later; **Unlink and delete tasks** deletes them too |
| **+ Add repository** | picks another folder for this project, as **+ Add project** does with the project already chosen |

Each repository of a project has its own name, its folder's unless another repository has it: then the folder above
goes first (`backend-app`), so two folders named alike can join one project.

New entries (**+ New task**, `n` or a column's `+`) go where the filter points: with **Central**, a project of one
repository, or one repository shown, that brain; with **All**, or a project of several repositories, the selected
card's brain, or else the first shown. The form's **Project** field says which and changes it: click it (or `tab`
to it) for every brain, with a search box, whatever the filter shows. Picking another drops the related entries
already checked, since they're within one repository.

`a` has no form, so with **All** or a project of several repositories it first asks **which project**: a list of the
brains shown, with a search box, the selected card's highlighted. Pick one with a click or `enter`; `esc` creates
nothing.

Each repository numbers its entries from 1, so `#3` can be in several; the label tells them apart. Entries of
different repositories of one project can be related too (see [Related entries](#related-entries)). Each repository
has its own Doing, so different projects run side by side.

## Tasks

| action | mouse | key |
| --- | --- | --- |
| create | **+ New task**, or `+` on a column for a task there | `n`, or `a` to add several to To do |
| select | click a card | arrows |
| move to another column | drag the card there, or a status button in the details | `shift+←/→` (next column), `s` doing, `p` parked, `d` done, `t` to do |
| open the details | double-click | `enter` or `space` |
| edit title and tags | **Edit title and tags** in the details | `e` |
| edit the description | **Edit** next to Description | `m` |
| add a comment | **Add** next to Comments | `c` |
| relate to other entries | **Link…** | `r` |
| open the file in your editor | **Open file** | `o` |
| move to another project | **Move to project…**, then pick one | `M` |
| delete | **Delete…**, then confirm | `x`, then `y` (`u` undoes it) |
| answer an agent's question | **Answer…** in the details | `c` |
| go to the next task waiting for you | **N waiting for you** in the top bar | `w` |

Any status can move to any other, e.g. straight from To do to Done. A right click on a card (or on an entry in the
notes and decisions lists) opens a menu with all of these actions, each naming its key.

### Quick add

`a` opens a field at the top of To do; with several brains shown, the app asks which project first (see
[Projects](#projects)). Type a title and press `enter`: the task is created in To do, and the field stays open for
the next one. `esc` closes it. A `#word` typed in the title stays part of it, not a tag: tags go in the new task
form (`n`), or with `e` afterwards.

### Moving to another project

**Move to project…** in the details (`M`) moves an entry to another brain: a list of every other one (the central brain, each
project and each repository of a project with several), whatever the filter shows, with a search box. The entry gets
a new number there and keeps its status, history, tracked time, description, comments and tags. Its links
are dropped, on both sides (links from other repositories too), since they name it by its old number. A task running its clock keeps it, unless
one of yours already runs there: then the move is refused. The button only shows while there's more than one brain.

### The new / edit form

**+ New task** (`n`) and **Edit title and tags** (`e`) open a form with three fields: the **title**, the **tags** (separated by
commas or spaces), and **Related**, the entries to link it to, which can stay empty. A new task also has a
**Column**, To do unless you pick another: a column's `+` opens the form with that column picked, and **+ New task**
and `n` with To do. A task created in Doing starts its clock. `tab` moves between the fields, `←` `→` pick the
column, `enter` saves (or **Create** / **Save**), `esc` cancels.

**Related** is a search box with a dropdown: typing filters the other entries of the same brain (tasks, notes and
decisions, by title or `#tag`), `↑` `↓` move, `enter` or a click checks or unchecks one. The checked ones show above
the box as chips; × takes one out. Editing starts from the entry's related entries, and saving links and unlinks to
match. Below the fields are the tags already
used by entries of the same kind (task tags for a task, note tags for a note), so you can reuse the same names. The title can't be empty.

### Status history

Every move is recorded with its date and time, e.g. `Mon 28 Sep 2026 10:12  To do → Doing`. The details show the
whole history, newest first, and it's stored in the task file's `## History` section; see [Data format](data-format.md).

## The details

Double-click a card (or select it and press `enter`) to open its details beside the board. They follow the selection,
so with the details open, the arrows go through cards and the details change with them. `esc` (or `✕`) closes them.

They show, top to bottom:

- the title, id, when it was created and its tags;
- for a task an agent asked about, the question with **Answer…**; for one waiting on a proposed decision, **Accept**,
  **Accept with a comment…**, **Reject…** and **Open decision #N** instead;
- the status buttons (To do · Doing · Parked · Done), and the tracked time: today, this week and in total;
- Edit title and tags, Link…, Accept, Accept with a comment… and Reject… (on a decision), Open file, Move to
  project… (with more than one brain) and Delete…;
- **Description**, what the task is about, like an issue's body;
- **Related**: linked notes, decisions and tasks. Click one to go there;
- **Comments**, timestamped entries you add as work goes on ("found the cause", "waiting for review"), oldest first.
  Each records when it was written and, if changed later, when it was **edited**, and has its own Edit and Delete
  buttons. An agent's say **Agent comment** or **Agent question**;
- **History**: every status change, `· agent` on those an agent made.

### Questions from agents

When an agent needs your answer (`kb ask`) or proposes a decision (`kb propose`), its task goes to Parked with a
**Waiting for you** badge and the question on the card, and the top bar counts them: **2 waiting for you** (`w`)
opens each in turn. Answer with a comment (`c`, or **Answer…** in the details); the card then says **Answered**
until the agent starts it again. An agent that waits for it (`kb wait`) picks the answer up as soon as you save it.

A proposal is answered by deciding, from the waiting task or from the decision itself (**Open proposed decision**,
`D`): **Accept** it (`y`), **Accept with a comment…** (`Y`), or **Reject…** it (`j`), which always says why. The
waiting tasks get the verdict, with your comment, as your answer. Proposed and rejected decisions carry a badge in
the decisions list too.

Descriptions and comments are written in the text editor: `esc` or **Save** saves (`ctrl+enter` too; the button
says **Add comment**, **Answer**, **Accept** or **Reject** when that's what saving does),
**Discard changes** (`ctrl+d`) throws the edit away; until something's typed, the button is **Close**. An empty comment or
answer adds nothing, and the status bar says so (an answer never written leaves the question waiting); to remove a
comment, delete it.
Markdown headings inside descriptions and comments are moved down a level when saved (`##` → `###` in a description,
`##`/`###` → `####` in a comment), because those levels mark sections and comments in the task file.

The details show descriptions and comments (and a project's description and notes) as formatted markdown: headings,
**bold**, *italic*, ~~strikethrough~~, `code`, links (click to open), lists and `- [ ]` task lists, quotes, code
blocks, tables and rules. A single line break stays a line break. The editor and the files keep the text as written.

## Notes and decisions

Next to tasks, kanbrain keeps two other kinds of entry, each in its own tab:

| tab | for | starts from |
| --- | --- | --- |
| **Notes** (`2`) | what you learned: how something works, where a log is, a gotcha | a blank description |
| **Decisions** (`3`) | what was chosen, and why: an [ADR](https://adr.github.io) | the sections Context, Decision, Consequences |

They have no status and no clock. Each tab is a list, newest first; the details work as for tasks. **+ New note**
or **+ New decision** (`n`) creates one.

### Related entries

**Link…** (`r`) opens a picker of every other entry, filtered as you type (`#tag` matches tags only). The entries
already related are checked and listed first. Click one (or `enter`) to link or unlink it. Links go both ways: each
entry shows the other under **Related**, whichever one you linked from. Agents link entries with
`kb link` or `add --link`.

In a project of several repositories, an entry can also be related to one in another of its repositories:
`kb link 1 shop-api#3` links this repository's #1 to #3 of `shop-api`, e.g. a front-end task to the API task it waits
for. Both show it under **Related**, named by repository (`shop/shop-api #3 …`); a click opens it. **Link…** and the
form's **Related** field list the project's other repositories' entries too, after the entry's own repository's.
Deleting an entry, or moving it to another project, removes the links to it everywhere, other repositories
included; so does deleting its file by hand, once the app or `kb` next loads the repositories that link to it.

## Tags

Tags group related work, e.g. `prod`, `support`, `meeting`, `feature`, or a client or project name.

- Separate tags with commas or spaces: `prod, support`, `prod support` and `#prod #support` all mean the same.
- Tags are saved **in lowercase, without `#`**, and duplicates are removed.
- An entry can have any number of tags, or none.
- Tags are shown wherever an entry appears: cards, lists, the details, dialogs and `kb list`.

To change an entry's tags, select it and press `e` (or **Edit title and tags**).

### Tag colors

Each tag is shown as a colored chip.

- A new tag gets a **random color from the colors used least so far**. The 16 palette colors are all used once before any repeats.
- Once assigned, **a tag keeps its color**. Adding tags or deleting tasks doesn't reshuffle them.
- The text on the chip is black or white, whichever is easier to read on that background.
- Each brain colors its own tags. A tag that several have shows in one color all over the app: the first one's that
  has it, the central brain's, then the repositories' in the order they were registered.

The assignments are stored in `tags.md` in your data folder, and you can edit it:

```markdown
# Tag colors

- prod: red
- support: #33aaff
- meeting: steel
```

Available palette names: `red`, `orange`, `gold`, `green`, `emerald`, `teal`, `blue`, `indigo`, `purple`, `magenta`, `pink`, `brown`,
`steel`, `olive`, `salmon`, `sky`. You can also use any hex color `#rrggbb`. The board picks up the change by itself.

## Filters

Every tab (the board, notes, decisions, the report and the calendar) is filtered the same three ways, from the top
of the window:

| filter | how |
| --- | --- |
| **project** | **Project ▾** (`P`): all, the central brain, a project or one of its repositories. Shared by every tab |
| **text** | the search box (`/`): words in titles and tags |
| **tags** | **Tags ▾** (`#`) next to it, or `#tag` typed in the search box |

The search box holds the tab's whole filter: words and `#tags`. **Tags ▾** is a dropdown with a search box of the
tags of what the tab shows, in the shown project (task tags on the board and the report, note tags on Notes,
decision tags on Decisions, all of them on the calendar), each saying where it's used (`2 tasks · 1 note`). Picking a
tag adds `#tag` to the search box, picking it again takes it out, and **All tags** takes every `#tag` out but keeps
the words. It stays open, so several tags can be picked: an entry must have all of them. The tags picked show on the
button too.

So `login #backend #auth` in shop finds shop's entries with "login" in the title or tags, tagged both `backend` and
`auth`. Each tab keeps its own search: filtering notes by `#stripe` leaves the board as it was, and coming back to
Notes finds the filter still there. **Clear** (or `esc`) empties the tab's search.

### Search

Click the search box (or press `/`) and type: the tab filters as you type.

| query | matches |
| --- | --- |
| `login` | titles or tags containing "login" |
| `#pro` | entries with a tag **starting with** "pro" (tags only) |
| `login #prod` | **all** terms must match: "login" in the title or tags, **and** a `prod…` tag |

Matching ignores upper/lower case. Column headers show `3 of 12` while a search hides cards.
`enter` or `esc` stop typing and keep the filter, so the keys work on the filtered tab.

From a terminal, `kb search WORDS…` also looks inside descriptions and comments.
