# Keybindings

The kanban app has a button for every action, and a key for every button: hover a button and its tooltip names the
key, or press `?` for the command palette: what can be done now with its key, on the selected entry first, then on
the tab, then everywhere, filtered as you type; `enter` runs the highlighted one. Keys act on the selected card, or on the selected entry in the notes and decisions
tabs. While you type in a field, keys are just letters; `esc` or `enter` leave the field.

## Board, notes and decisions

| key | action |
| --- | --- |
| `←` `→` `↑` `↓` | move between cards and columns (up and down on the notes and decisions lists) |
| `shift+←` `shift+→` | move the selected card to the previous or next column |
| `v` | lay the board out as kanban columns or as a list (see [Kanban or list](tasks-and-tags.md#kanban-or-list)) |
| `s` | **start**: move to Doing and run its clock; refused while another of yours runs in that brain |
| `p` | **park**: move to Parked |
| `d` | **done**: move to Done |
| `t` | **back to to do**: move to To do, e.g. to reopen a done task |
| `enter` / `space` | show or hide the details |
| `n` | **new** task (on the notes or decisions tab, a note or decision) |
| `a` | **add** tasks at the top of To do, one after another (see [Quick add](tasks-and-tags.md#quick-add)) |
| `e` | **edit** title and tags |
| `m` | edit the description |
| `c` | add a **comment**; on a task waiting for your answer, **answer** the agent's question (one waiting on a proposed decision is answered with `y`, `Y` or `j`) |
| `C` | edit or delete one of its **comments**: a list of them, the latest highlighted; `enter` edits, `x` deletes (asks first) |
| `D` | open the **decision** an agent proposed, which the selected task waits on |
| `r` | **relate** to other entries: search and pick |
| `o` | **open** the file in `$EDITOR`; the board reloads when it closes |
| `x` | **delete** (asks first) |
| `y` | **accept** the selected decision, or the one the selected task waits on; it answers a question that names it (`#ID`) |
| `Y` | **accept with a comment**: the editor asks for it, and the waiting tasks get it with the verdict |
| `j` | **reject** the selected decision, or the one the selected task waits on: the editor asks why, which a rejection always says |
| `w` | go to the next task **waiting for your answer**, in every project shown |
| `M` | **move** to another project: a list of every other brain, with a search box (see [Moving to another project](tasks-and-tags.md#moving-to-another-project)) |
| `/` | **search** titles and tags, on every tab; `#tag` matches tags only (several `#tags`: entries with all of them) |
| `#` | **filter by tags**, on every tab: a dropdown of the tags of what the tab shows, with a search box; each pick adds or takes out `#tag` in the search box, and it stays open for several (`esc` closes) |
| `esc` | close the details; with them closed, clear the search (words and tags) |
| `f` | on the board, show every done task, or only the latest 20 |
| `1` `2` `3` `4` `5` / `tab` / `shift+tab` | Board, Notes, Decisions, Report, Calendar; `tab` the next, `shift+tab` the previous |
| `I` | **manage the project** shown, or with **All** of the selected card's project: description, repositories with their roles, tasks per status, what's running, this week's time; with buttons to rename, move and remove them and add a repository |
| `E` | **edit the description** of the project shown, or with **All** of the selected card's project, shared by its repositories |
| `N` | edit **the role** of the selected card's repository in its project, or of the repository shown (what it owns, exposes and consumes) |
| `A` | **add a project**: pick its code folder, then a project from the list or a new name; see [Projects](tasks-and-tags.md#projects) |
| `P` | choose the project shown: a dropdown of All, Central, each project and each repository of a project with several; type to filter, `↑` `↓` and `enter` to show one, `esc` to close |
| `u` | **undo** the last change the board made (a delete, a move, an edit…); pressed again, redo it. Refused if another program changed those files since |
| `R` | reload the files (the board also reloads by itself when they change) |
| `U` | **update** to the newer release the status bar offers (it asks first); not while an agent has a task in doing |
| `Q` | **park** your running tasks and **quit**; agents' keep running |
| `?` | the **command palette**: what can be done now, with its key; type to filter, `↑` `↓` move, `enter` runs it, `esc` closes |
| `q` | quit; running tasks keep their clocks running |

## Report tab

`1` `2` `3` `4` `5`, `tab`, `shift+tab`, `n`, `/`, `esc`, `P`, `I`, `E`, `N`, `A`, `u`, `R`, `U`, `Q`, `?` and `q` work as above.

| key | action |
| --- | --- |
| `←` `→` | previous or next period |
| `h` `d` `w` `m` | an hour, day, week or month |
| `t` | back to the current period |
| `g` | group by task or by tag |
| `#` | **filter by tags**: a dropdown of the shown project's task tags; each pick adds or takes out `#tag` in the search box, and it stays open for several (`esc` closes) |
| `e` | export to `reports/` as markdown |

## Calendar tab

`1` `2` `3` `4` `5`, `tab`, `shift+tab`, `n`, `/`, `esc`, `P`, `I`, `E`, `N`, `A`, `u`, `R`, `U`, `Q`, `?` and `q` work as above.

| key | action |
| --- | --- |
| `←` `→` | previous or next period |
| `d` `w` `m` | a day, week or month |
| `t` | back to the current period |
| `#` | **filter by tags**: a dropdown of the shown project's tags; each pick adds or takes out `#tag` in the search box, and it stays open for several (`esc` closes) |
| `i` | which interactions: all, time, moves, comments, new entries |
| `↑` `↓` | select the entries shown, in the order of their first interaction |
| `enter` / `space` | show or hide the selected entry's details |

## Fields and dialogs

| where | keys |
| --- | --- |
| search box | type to filter; `enter` or `esc` stop typing and keep the filter |
| quick add | `enter` adds the task and keeps the field open for the next; `esc` closes it |
| new / edit form | `tab` (`shift+tab` back) moves between project (new entries), title, tags, related and, for a new task, its column, selecting the field's text so typing replaces it; so do `↓` (`↑` back) where no list takes them: in the title, on the column, and in the tags with none to pick, when no tag in use and not added yet contains what's typed (`↑` also with none highlighted); `enter` saves; `esc` cancels. In **Project**: type to search, `↑` `↓` move, `enter` picks it and goes on to the title. In **Tags**: the tags in use drop down, filtered by the one typed; typing a tag's start highlights it, `↓` `↑` move, `enter` adds the highlighted one as a chip in place of what's typed, a click the one clicked, a comma or space the one typed; a chip's × takes it off; with none highlighted, `enter` saves, with the tag typed. In **Related**: type to search, `↑` `↓` move, `enter` checks or unchecks; the Save button saves. On **Column**: `←` `→` pick it. After a save stopped over a title `kb` changed too, `ctrl+t` (**Use it**) takes that title |
| text editor (descriptions, comments, answers, a decision's reason, a project's description and notes) | `enter` new line; `esc` or `ctrl+enter` **save** (the button says what: **Answer**, **Add comment**, **Accept**, **Reject**, **Save**); **Discard changes** (`ctrl+d`) throws the edit away; with nothing typed, `esc` or **Close** closes it. A save over what `kb` changed meanwhile merges it line by line; lines you both changed show as conflicts: `m` yours, `t` theirs, `b` both, `tab` or `↑` `↓` move (`shift+tab` back), `enter` (**Use the result**) puts the merged text in the editor, `esc` (**Yours for the rest**) takes yours where nothing's picked, `ctrl+d` (**Discard changes**) keeps theirs |
| tell the agent? (saving a task an agent is working on) | type an optional note; `enter` saves and tells the agent, as your comment; `ctrl+enter` just saves; `esc` goes back to the form or editor |
| link picker | type to search; `↑` `↓` move; `enter` links or unlinks; `esc` closes |
| comments (`C`) | `↑` `↓` move; `enter` edits the highlighted comment; `x` deletes it, after asking; `esc` closes |
| "which project?" (quick add, or move) | type to filter; `↑` `↓` move; `enter` opens the quick-add field there, or moves the entry; `esc` closes it |
| Manage project (`I`) | `↑` `↓` highlight a repository; `r` renames it, `M` moves it to another project, `x` removes it, `N` edits its role; `R` renames the project, `E` edits its description, `A` adds a repository, `X` removes the project; `esc`, `enter`, `I` or `q` close it |
| add a project (`A`), once its folder is picked | type a new project's name, or to search the projects; `↑` `↓` move; `enter` adds the folder to the highlighted project (a new one: the name typed); `esc` cancels, back to Manage project if it was opened from there |
| rename and move (from Manage project) | type the name, or to search projects; `↑` `↓` move; `enter` applies; `esc` goes back |
| remove (from Manage project) | `y` or `enter` unlinks, keeping the tasks; `d` unlinks and deletes them; `n` or `esc` goes back |
| confirmation (delete, update) | `y` or `enter` goes ahead; `n` or `esc` cancels |
| right-click menu | `↑` `↓` move; `enter` or an item's own key chooses it; `esc` or a click outside closes it |
| command palette | type to filter by what it does or its key; `↑` `↓` move; `enter` runs it; `esc` closes |

Dropdowns and pickers show 8 rows at most (the form's Project, Tags and Related fields, 6; Add project's and Manage
project's project lists, 5; the command palette, 12) and scroll past that; a line under them says
how many more are below (`↓ 4 more · type to filter`) and above (`↑ 2 above`), and `↑` `↓` keep the highlighted row in view.

In every field and the editor, `ctrl+a` selects all, `ctrl+c` / `ctrl+x` / `ctrl+v` copy, cut and paste with the
system clipboard (`cmd` on macOS), and `shift` with the arrows, `home` or `end` selects text. Typing or pasting
replaces the selection. Text shown rather than typed into (the details, the dialogs' text, the conflicts' sides, the
status bar's message) is selected with the mouse, and `ctrl+c` copies it as shown, without the markdown, a line break
between paragraphs, list items and code lines; a field with text selected copies its own first. Cards and list rows
aren't text to select: dragging them moves them, and their title and tags are selected in the details.

## Mouse

| action | effect |
| --- | --- |
| click a card or entry | select it |
| double-click a card or entry | open its details |
| right-click a card or entry | its menu: details, the status moves, edit, description, comment or answer, the proposed decision, link, accept or reject (a decision, or a task waiting on one), move, open file, delete; each item names its key |
| drag a card onto a column | move it there; the column lights up while the card is over it |
| drag a row onto a section's header (list) | move it there; the header lights up while the row is over it |
| **Kanban** / **List** next to the tabs | lay the board out as columns or as a list |
| `+` on a column, or on a list's section | the new task form, with that column picked |
| click a day in the calendar's month | show that day |
| **Project ▾**, then a project in the list | show only it (or All); type in its box to filter, click outside to close |
| **Tags ▾**, then tags in the list | add or take out each as `#tag` in the search box; type in its box to filter, click outside to close |
| click an interaction in the calendar | show its entry's details |
| click the running task in the top bar | open it |
| click an entry under "Related" | go to it |
| click a link in a description or comment | open it in the browser |
| click in text | place the cursor |
| drag, `shift`+click | select text; in text shown, across its paragraphs (all of a description, one comment), not into the next |
| double-click / triple-click in text | select a word / the line (in text shown, the paragraph); dragging on extends by words / lines |
| click outside the text shown selected | drop its selection |
| middle-click in text (Linux) | paste the primary selection (the text last selected, in any app, text shown too) |
