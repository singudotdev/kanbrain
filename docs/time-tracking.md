# Time tracking

kanbrain always tracks time: it records how long you actually work on each task, and reports it by hour, day,
week or month. There's nothing to turn on, and nothing extra to do: moving cards is the tracking.

## How it works

- **Moving a task to Doing starts its clock**: a **session** opens. Every task in Doing has its own clock running.
- **You run one task at a time, per brain.** Starting another while one of yours is in Doing is refused, saying
  which: park or finish it first. Nothing is parked for you. The central brain and each project repository have
  their own Doing, so tasks in different projects can run at once; the top bar lists every running task, each with
  its own **Park**.
- **Agents aren't limited.** An agent that splits its work between subagents runs one task per subagent, beside
  yours. Running tasks are marked in **cyan** when you started them and in **green** when an agent did (the card's
  border, its clock, the top bar). Each task tracks its own time; totals count by the clock (see [Reports](#reports)).
- **Moving a task out of Doing** (to Parked, Done or To do), or deleting it, stops its clock.
- **Closing the window doesn't stop the clock.** The open session is on disk, so time keeps counting while the board
  is closed, and when an agent works through the command line. Use **Park & quit** to stop your clocks for the day;
  agents' tasks keep running, since an agent may still be at work, and it closes its own.

The same rules apply everywhere: on the board, with `kb start ID` from a terminal or an agent.

The bar under the toolbar shows each running task, its clock, its total time and today's total for every task, with
**Park** and **Park & quit** (`Q`) buttons. Click the task's name to open it. With nothing running, it says **Idle**.
Cards show their tracked total, the running one its clock (▶), and the details show today, this week and the
total, with the number of sessions.

Sessions are saved in the task file under `## Sessions` (see [Data format](data-format.md)), next to the status history.

When you stop your running task (Park, moving it to Done or To do, deleting it), nothing asks what's next: the
board shows what's parked and to do, and you start one, add one, or leave it.

## Handling interruptions

```
09:00  drag "Feature: export CSV" to Doing         the clock starts
10:20  p on the feature, then + on Doing:
       "PROD fix login 500"                         the feature waits in Parked, the fix runs
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
- **Totals count by the clock.** Each task row shows that task's own time, but time several tasks ran at once
  (yours beside an agent's, or an agent's subagents) counts **once** in the total, the column totals, each tag's
  row, the calendar's days and the top bar's **Today**: a task of 1 h and one of 2 h that overlapped for 30
  minutes took 2 h 30 m. So task rows can add up to more than the total.
- A task with several tags counts under **each** of them when grouping by tag, so tag rows can add up to more than
  the total too.
- In the day and hour views, gaps of **5 minutes or more** between sessions are shown as `untracked`.
- Durations are rounded down to whole minutes, and less than one shows as `<1m` (`0m` is none at all); a session crossing a period boundary counts in each period only for its own part.
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
