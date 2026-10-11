# Time tracking

kanbrain always tracks time: it records how long you actually work on each task, and reports it by hour, day,
week or month. There's nothing to turn on, and nothing extra to do: moving cards is the tracking.

## How it works

- **Doing holds one task at a time, per brain.** Moving a task to Doing starts its clock: a **session** opens. The
  central brain and each project repository have their own, so tasks in different projects can run at once; the top
  bar lists every running task, each with its own **Park**.
- **Starting another task parks the one running in the same brain.** Its session closes and it moves to Parked, where it waits, on
  top, for you to come back to it.
- **Moving the running task out of Doing** (to Parked, Done or To do), or deleting it, stops the clock.
- **Closing the window doesn't stop the clock.** The open session is on disk, so time keeps counting while the board
  is closed, and when an agent works through the command line. Use **Park & quit** to stop every clock for the day.

The same rules apply everywhere: on the board, with `kb start ID` from a terminal or an agent.

The bar under the toolbar shows the running task, its clock, its total time and today's total for every task, with
**Park** and **Park & quit** (`Q`) buttons. Click the task's name to open it. With nothing running, it says **Idle**.
Cards show their tracked total, the running one its clock (▶), and the details show today, this week and the
total, with the number of sessions.

Sessions are saved in the task file under `## Sessions` (see [Data format](data-format.md)), next to the status history.

## "What's next?"

When you stop a brain's running task from the app (Park, moving it to Done or To do, deleting it) and nothing else
runs there, the app asks what's next in that brain:

```
Nothing running in shop/api: what's next?
  [ Search titles, or #tag ]
  ‖ #2 Export CSV                 ← parked tasks first, the latest on top: Enter resumes it
  · #5 Add tests
  + New task, started
  ■ Nothing for now
```

Typing filters the tasks by title or `#tag`; the new task and nothing stay at the end. `↑` `↓` choose, `enter` (or a
click) starts the task, `n` (with nothing typed) or **New task** creates one there already started, `esc` leaves
nothing running. Starting another task, Park & quit, and changes made by agents from the command line don't ask.

## Handling interruptions

```
09:00  drag "Feature: export CSV" to Doing         the clock starts
10:20  + on Doing: "PROD fix login 500"            the feature is parked, the fix runs
11:05  drag the fix to Done                        idle
11:06  drag the feature from Parked to Doing       it resumes
12:30  Park                                        lunch
13:30  s on the feature                            back to it
18:00  Park & quit
```

Forgot to stop overnight? Open the task's file (`o`) and fix the session's end time (see [Data format](data-format.md#task-files)).

## Reports

The **Report** tab (`4`) shows the tracked time of the **current week**, **grouped by task**.

| button | key | action |
| --- | --- | --- |
| Hour / Day / Week / Month | `h` `d` `w` `m` | period: hour, day (with a timeline and untracked gaps), week (a column per day), month (a column per ISO week) |
| ‹ / › | `←` / `→` | previous / next period |
| Now | `t` | back to the period containing now |
| By task / By tag | `g` | group by task (with tags) or by tag |
| Export .md | `e` | write the report as shown to `<data folder>/reports/` |

- The report and the calendar follow the window's [filters](tasks-and-tags.md#filters): project, tag (`#`) and
  text.
- A task with several tags counts under **each** of them when grouping by tag, so tag rows can add up to more than
  the real total. The **total** line is always the real tracked time.
- In the day and hour views, gaps of **5 minutes or more** between sessions are shown as `untracked`.
- Durations are rounded down to whole minutes; a session crossing a period boundary counts in each period only for its own part.
- Export file names follow the view: `report-2026-W39.md`, `report-2026-09-24-by-tag.md`, `report-2026-09-prod.md`.

## Calendar

The **Calendar** tab (`5`) shows what happened on each day, one line per entry, so a day of interruptions stays
readable: the entry's **last status change** that day (`doing → parked`, `todo → done`), or else `new note`,
`comment` or `▶ worked`; then the time worked on it that day (`▶ 1h 05m`) and how many comments it got. It has the
report's controls (Day / Week / Month, ‹ Now ›) and one more, **All interactions** (`i`), to count
only time, moves, comments or new entries.

- **Week:** a column per day, its time worked on top, then its entries in time order.
- **Month:** whole weeks, Monday to Sunday; each day shows its time and first entries (`→ parked 1h 05m`). Click a
  day to see it whole.
- **Day:** every entry with its tags, and a comment's first line.

Click an interaction to open its entry's details beside the calendar. A session that runs past midnight shows on
both days, each with its own part.

From scripts and cron jobs, use [`kb report`](cli.md#kb-report); for status bars,
[`kb status`](cli.md#kb-status).
