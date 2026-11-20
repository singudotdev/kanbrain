# 17. `kb context` says what a decision decided, and what bears on the task in progress

Date: 2026-11-20. Status: accepted.

## Context

`kb context` is what an agent reads first, and it lists only titles. Two things it left out:

- An accepted decision's title often names the options ("v0.6.0 or v0.5.3"); which one was chosen is in the comment
  `resolve` writes (`Accepted: REASON`), so the agent ran `kb show` on each decision, or guessed.
- Decisions and notes are the newest twenty of each, whatever the task. As a brain grows, the oldest, often the
  foundational ones, fall off the list, and nothing says which of the rest bear on the task in hand.

## Decision

- Under an accepted decision, `  decided: …`: the first line of its latest `Accepted: REASON` comment, cut like a
  last comment (120 characters, `…` when more follows). An acceptance with no reason says nothing more than the
  state, so it adds no line. A decision in another state shows its state, as before, and no line.
- Under a task in **doing**, `  related: …`: the entries linked to it, whichever side stores the link
  (`Brain::related`), that are notes or decisions in force (accepted), notes first, three at most and then
  `· +N more`. Rejected and proposed decisions and other tasks aren't in force, so they are left out; parked and todo
  tasks aren't being worked on, so they cost no context.
- The output stays markdown only (`context --json` is refused, as before), and no section's cap changes, so the size
  of the context grows by at most four lines per task in doing plus one per accepted decision listed.

## Consequences

- An agent starts a task knowing the decisions that bind it, as long as they were linked to it (`add --decision
  --link ID`, which the section `init` writes already asks for).
- A decision accepted by editing its file, or before `resolve` wrote the comment, shows no `decided:` line.
- A comment by a person that starts with `Accepted: ` counts as the acceptance. That is the same text `resolve`
  writes, and a decision accepted later wins, since the latest one is read.
