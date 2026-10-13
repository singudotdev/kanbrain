# Project memory for coding agents

Coding agents forget everything between sessions. kanbrain gives them, and you, one place for what a project is
doing and why: **tasks** (what's being worked on), **decisions** (what was chosen and why) and **notes** (what was
learned), linked to each other, as markdown files next to the code.

## Set it up in a project

```sh
cd my-project
kb init        # creates .brain/ and tells the project's agents about it
```

`init` does three things:

1. **Creates `.brain/`**, the project's data folder. Every command run in the project (or any folder below it) now
   uses it. Commit it to share the project's memory, or add it to `.gitignore` to keep it local.
2. **Makes it a project:** registers the repository in the central brain, so the kanban app shows it among the others
   and filters by it, and creates the project's card. Its description, the application's shared context, is what
   `context` starts with, so an agent knows the project before exploring it. The agent writes it and keeps it up to
   date with `kb project describe`; until there is one, `context` asks for it (see
   [What goes where](#what-goes-where)).
3. **Tells the agents about it.** It writes a short "Project memory" section into the project's `CLAUDE.md` and
   `AGENTS.md`, whichever exist, or creates `CLAUDE.md` if there's neither. Agents read that file at the start of
   every session, so from the first one they know to run `kb context` and how to record their work.
   With nothing else in the repository, it's enough for an agent to work this way from its first session.
   The section sits between `<!-- kanbrain:start -->` and `<!-- kanbrain:end -->`; the rest of the file
   isn't touched. Running `init` again updates the section in place, and upgrading does it in every project: the
   installer and `kb update` run `kb init --all`.

   For Claude Code (when the section goes in `CLAUDE.md`), it also adds three hooks to
   `.claude/settings.local.json`, keeping whatever else is there:

   | hook | runs | does |
   | --- | --- | --- |
   | `SessionStart` | `kb context` | when a session starts or resumes, after `/clear` and after compaction, the agent gets the brain's state without spending a tool call, and keeps it deep into a long session |
   | `UserPromptSubmit` | `kb hook prompt` | while the project isn't described (or, with several repositories, this one's role isn't), every prompt arrives with the request to write it before anything else |
   | `Stop` | `kb hook stop` | an agent about to end its turn with it still missing is sent back to write it, once a turn, so it can't loop. It also reminds an agent of each task it started and hasn't commented on since, to say where it stands |

   Agents skipped describing the project when only the section and `context` asked them to; with these, Claude
   Code holds them to it on their first turn. Once it's described and the tasks the agent started commented on,
   both print nothing and cost nothing. An explicit
   instruction from you still wins: told to use no tools, the agent says it couldn't describe the project and stops.

`kb deinit` undoes all three, except that `.brain/` is kept: see [Command line](cli.md#kb-deinit).

The section reads:

```markdown
## Project memory: kanbrain

This project's tasks, to-dos, decisions and notes live in `.brain/`, managed only with `kb`, kanbrain's CLI.
The user follows them live on a kanban board: keep it showing what you're doing as you go, not at the end.

- Start of session: `kb context`, unless its output is already above (a session hook adds it).
- Work that changes the repository, or that the user should see, gets a task; questions and lookups don't. Find it
  (`search WORDS`) or create it (`add TITLE`), then `start ID` (`add --start TITLE` does both). "Todos", a plan or
  what's pending, in any language, are tasks here, one `add` each; your built-in to-do list isn't on the board, so
  keep it for steps within the task you've started. A task with real work gets a body, `add --body - TITLE`:
  **Goal** (what and why, a line or two) and **Done when** (what the user will check).
- While working: `comment ID TEXT` when a step is done, when you find a cause, and before you stop (one or two
  lines: what changed, what's next); `done ID` as soon as one is finished. Stopping before it is: `comment`, then
  `park ID`. Doing isn't limited for you: several of your tasks can run at once. Never park, finish or move a task
  you didn't start (the user's, or another session's).
- Splitting work between subagents: give each part that changes the repository its own task, and run the board
  yourself: `add --start` it before launching the subagent, then `comment` its result and `done` it. Subagents
  only do the work; lookups and research get no task. `kb` commands can run in parallel.
- Blocked on the user (a requirement, access, a choice that's theirs): `ask ID TEXT` parks the task with the
  question; don't guess. A choice you shouldn't make alone and weren't asked to (architecture, a dependency,
  storage, a public interface): `propose ID --body - TITLE`, Context / Options / Recommendation on stdin; it parks
  the task until the user accepts or rejects it. Then, unless the user said they're away, if your tool can run a
  command in the background and wake you when it exits, run `kb wait ID` (the task) that way and end your turn: it
  prints the answer, or that there's none yet (then stop, don't wait again). Otherwise go on with other work or
  stop; answers come first in `context`, under Answered: `show ID`, then pick the task up. If the user answers in
  the chat instead, `start` the task (that ends the wait). `accept`/`reject` a decision only when the user tells
  you to.
- Choosing between approaches within your task: `search` for earlier decisions (only accepted ones are in force),
  then `add --decision --link ID --body - TITLE` with Context / Decision / Consequences on stdin. Non-obvious
  findings: `add --note --link ID --body - TITLE`; a problem someone has to fix is a task instead, in the
  repository that has to change.
- `## Project` in `context` is the application's shared context. If it isn't described, or you're asked to set up
  or load the brain, describe it first, from the README and manifests: `project describe -` (what the app is, its
  services and how they connect, conventions they share: the big picture, in at most five lines, without tables,
  columns, function signatures, files or dependencies, which the code already says). Rewrite it when the big
  picture changes (a service added, another storage), not after every change. With several repositories, this
  one's role too: `project notes -`. How to build, test and run this repository, and its code style, go in this
  file, outside this section: add them as you learn them.
- Also: `show ID` (one entry in full), `describe ID TEXT`, `link ID OTHER…` (an id here, or `REPO#ID` in another
  repository of the project); `-` reads text from stdin. Never edit the files in `.brain/` by hand: `kb` keeps
  them consistent.
```

It's read every session, so it's short, yet it's the whole CLI an agent needs; `context` gives the state. It's
also blunt on purpose: agents have habits of their own. Claude Code keeps a built-in to-do list, and works through
a plan without a word to the board, unless it's told that the board is watched live and that "make the todos" means
kanbrain tasks.

### What goes where

Each thing an agent should know lives in one place, the one it already reads, so nothing is paid for twice in a
session or drifts out of date in a second copy:

| where | holds | read by the agent |
| --- | --- | --- |
| the repository's `CLAUDE.md` / `AGENTS.md`, outside the section | how to work on this code: build, test and run commands, code style, its pitfalls; the agent adds them as it learns them, so a repository with nothing but kanbrain gets them too | every session, by the agent itself, with or without kanbrain |
| the project's card (`projects/NAME.md` in the central brain), `project describe` | the application's horizontal context: what it is, its services and how they connect, conventions they share (money in cents, auth, event formats) | every session, at the top of `context`, in every repository of the project |
| the repository's `.brain/project.md`, `project notes` | with several repositories, this one's role: what it owns, what it exposes and consumes from the others | every session, in `context`, only in this repository |
| tasks, decisions and notes | what's being done, what was decided and why, what was found out | titles in `context`, the rest with `show` |

A project of one repository needs only the card: there are no others to fit in with, so `.brain/project.md` holds
just the project's name.

### When the agent needs you

An agent that guesses at a requirement, or picks a database on its own, costs more than one that asks. Two commands
hand the choice to you and leave it visible on the board:

- `kb ask ID TEXT` adds the question to the task as a comment marked as one, and parks the task. Until a person
  comments on it, the board shows it as **Waiting for you** with the question, `kb status` counts it
  (`· 1 waiting`), and `context` marks it `· waiting for an answer`, so another session doesn't pick it up.
- `kb propose ID --body - TITLE` creates a **proposed** decision linked to the task, with Context / Options /
  Recommendation, and asks about it on the task. You **accept** it, or **reject** it saying why, from its details
  in the app (`y` / `j`) or with `kb accept ID` / `kb reject ID TEXT`; the tasks waiting on it get the verdict as
  your answer. A rejected decision stays, so the same approach isn't proposed again; only accepted ones are in force.

An agent that can run a command in the background and be woken when it exits, as Claude Code does, then runs
`kb wait ID` that way and ends its turn. `wait` reads the task's file every second and exits with your answer as
soon as you give it (from the app, or `kb comment ID TEXT`): the agent picks the task up by itself, with no need to
tell it to go on. Waiting calls no model, so it costs nothing until you answer. Answering in the chat works too: the
agent starts the task, which ends the wait. After 8 minutes without an answer `wait` gives up and the agent stops
there: Claude Code run headless (`claude -p`) doesn't exit while a background command runs, and an answer that
late is picked up just as well by the next session.

Otherwise, or in a later session, `context` lists the task first, under **Answered: pick these up first**, with your
answer. Starting it clears the mark.

Comments and status changes record who made them: `kb` writes `agent` when a coding agent runs it, which it finds
from the environment (`AGENT`, `AI_AGENT`, `CLAUDECODE`, `CODEX_SANDBOX`, `CODEX_CI`, `CODEX_THREAD_ID`,
`GEMINI_CLI`); `KANBRAIN_AUTHOR=human` or `agent` overrides it. Only a person's comment answers a question, and the
app always writes as a person.

### Working in parallel

An agent that splits its work between subagents (Claude Code's, for one) gives each part that changes the repository
its own task and runs the board itself: it starts the task before launching the subagent, then comments the result
and finishes it. Subagents only do the work, so they need to know nothing about kanbrain. Doing isn't limited for
agents, so the board shows every part running at once, in green; your own task, if one runs, stays in cyan, and
the agent never parks, finishes or moves it. Its `kb` commands can run at the same time: each takes the brain's
write lock, as the kanban app does while it acts, so none overwrites another's change.

Each task tracks its own time, but the report's totals count by the clock: four subagents working for an hour add
an hour to the day, not four (see [Reports](time-tracking.md#reports)).

### Projects with several repositories

Services of one project often live in separate repositories. Run `kb init shop` in each: every repository
keeps its own tasks, and they all share project shop's card, so the same horizontal context reaches every agent.
In `shop-web`, `context` starts like this:

```markdown
## Project shop

Online shop: catalog, cart and Stripe payments. shop-api (Rust, axum + Postgres) serves the REST API; shop-web
(React) is the store and only talks to shop-api, over `/api/v1` with a JWT. Prices are in cents everywhere.

This is its `shop-web` repository.

The store: UI only; every piece of data comes from shop-api, nothing is kept but the local cart.

Its other repositories. Work one of them needs, or a problem found in it, is a task there, not a note here: `kb --project shop/NAME add TITLE`.

- shop-api: /home/you/code/shop-api
```

So an agent in the front end can look at the API's tasks (`kb --project shop/shop-api list`) or leave one
there when its work needs the API to change (`kb --project shop/shop-api add "Expose the cart total"`).
While a repository's role isn't written down, `context` asks for it (`project notes -`) in its place.

### Keeping it cheap

Everything an agent reads costs tokens and context, so the outputs are made for that:

- **The section** in `CLAUDE.md` / `AGENTS.md` is about 900 tokens, read every session. It's the one place the
  commands are explained: `context` doesn't repeat them inside a project. What only matters with several
  repositories (their roles, tasks in a sibling) is in `context`, and only in those projects.
- **`context`** lists titles and ids, not contents. Each section stops at 20 entries (5 for recently done) and says
  how many more there are, so it stays the same size however big the brain grows. Tasks in doing or parked also show
  their latest comment, which is usually where the work stopped.
- **`show`** is compact text, not the file: no empty sections, dates to the minute, the time tracked as one line
  instead of every session, and only the latest 5 status changes. The file's path is there if the rest is needed.
- **Changes** (`add`, `start`, `comment`, …) answer with one line, `#4 Fix login (doing) #auth`. The agent knows
  what it wrote, so the entry isn't sent back.
- **`search`** returns one line per entry, at most 20.
- **`--json`** is for scripts: one line, without empty fields. For an agent, the text is shorter.
- **A CLI, not tools:** nothing is loaded into the agent's context until it runs a command, and the section already
  explains them.

The files hold everything (every session, the whole history), so agents should read them only to fix something by
hand.

## Commands for agents and scripts

| command | does |
| --- | --- |
| `context` | markdown summary: the project's description, answered tasks (with the answer), tasks in doing (with their latest comment), parked (with the question they wait on) and todo; the latest done; decisions (proposed and rejected ones marked); notes; outside a project, also how to use the commands |
| `add [--note\|--decision] [--tag T]… [--link ID]… [--body TEXT\|-] [--start] TITLE…` | creates an entry, prints `#id title (state)` |
| `show ID` | the entry as compact text: state, time tracked, description, comments, related entries, latest status changes, file path |
| `list notes decisions` | every note and decision (add statuses such as `doing` to include tasks) |
| `search WORDS…` | entries whose title, tags, description or comments contain every word, one line each, at most 20; `#tag` matches tags only |
| `start` / `park` / `done` / `todo ID` | changes a task's status; `start` runs its clock. An agent can have several tasks running; a person, one per brain (another `start` is refused, naming it) |
| `comment ID TEXT…\|-` | adds a timestamped comment |
| `ask ID TEXT…\|-` | asks the user a question on a task and parks it until a person comments |
| `wait ID [--timeout MINUTES]` | blocks until task `ID` no longer waits for an answer (or decision `ID` is accepted or rejected), reading the files every second; prints the answer, or that the task waits on nothing (never asked, or picked up since). Fails after 8 minutes without one (`--timeout`, or `KANBRAIN_WAIT_TIMEOUT`, in minutes; 0: never). No `--json` |
| `propose ID [--tag T]… [--link ID]… [--body TEXT\|-] TITLE…` | creates a proposed decision linked to task `ID`, which asks about it and is parked; prints both |
| `accept ID [TEXT…\|-]` / `reject ID TEXT…\|-` | accepts or rejects a decision, with why (needed to reject), and answers the tasks waiting on it |
| `describe ID TEXT…\|-` | replaces the description |
| `link ID OTHER…` | links an entry to others: an id here, or `REPO#ID` in another repository of the project |
| `project [describe\|notes TEXT…\|-]` | shows the project's description (shared by its repositories) and this repository's role, or replaces one |
| `project rename NAME…` / `repo NAME` / `move PROJECT…` / `remove [--purge]` | renames the project or this repository, moves this repository to another project, or takes the project out (see [Managing a project](cli.md#managing-a-project)); only when the user asks |

`-` reads the text from stdin, for multi-line markdown. Ids may be written `12` or `#12`.

Errors go to stderr with a non-zero exit code and say what's wrong: `no entry #12`,
`#3 is a note: only tasks have a status`, `the comment is empty`, `the reason is empty`.

## JSON

For scripts, `show`, `add`, the status moves, `comment`, `describe`, `link`, `ask`, `propose`, `accept` and
`reject` print the entry as one line of JSON with `--json` (shown here spread out):

```json
{
  "id": 1,
  "kind": "task",
  "title": "Fix SSO login 500",
  "status": "doing",
  "tags": ["auth"],
  "linked_from": [2],
  "created": "2026-10-07T18:14:04+02:00",
  "comments": [{ "created": "2026-10-07T18:14:04+02:00", "body": "root cause: token refresh race" }],
  "tracked_minutes": 42,
  "running": true,
  "path": "/home/you/my-project/.brain/0001-fix-sso-login-500.md"
}
```

Fields without a value are left out: `status` for notes and decisions, `links`, `linked_from`, `description` and
`comments` when empty, `tracked_minutes` for tasks never in doing, `running` unless the clock runs, and `edited` on
comments never edited. Decisions have `decision` (`accepted`, `proposed` or `rejected`); a parked task with a
question has `asked` (`waiting for an answer` or `answered`); a comment has `"author": "agent"` when an agent wrote
it and `"question": true` when it's one. `search --json` and `list --json` print every match as
`{ id, kind, title, status, decision, asked, tags }`, all on one line.
