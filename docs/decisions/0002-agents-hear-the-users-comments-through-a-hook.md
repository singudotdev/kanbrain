# 2. Agents hear the user's comments mid-work through a hook, with no new comment kind

Date: 2026-10-16. Status: accepted.

## Context

While an agent works on a task, the user may change it from the board (its title, tags, description) or comment on
it. The agent learns of that only if it reads the task again, which it has no reason to do mid-work: it went on with
the old plan, and the user's change could also cross what it wrote next. v0.5.1 made the app stop a save over what
`kb` changed meanwhile, and warn on opening an agent's task; what was missing was the other direction, the agent
hearing the user.

Claude Code runs hooks around the agent's tool calls. A `PreToolUse` hook may print `additionalContext` (JSON on
stdout, exit 0), which Claude Code adds to the conversation next to the tool's result, without blocking the call or
asking for permission. The hooks `init` writes already include `PreToolUse` on the tools that write files
(`kb hook edit`).

The first plan marked the user's word to the agent as a comment of its own kind, `notice` in its heading, and had the
hook look for those. `model::Comment` is a public struct with public fields and no `#[non_exhaustive]`, so a `notice`
field breaks `kanbrain-core`'s API: v0.6.0 rather than v0.5.2 under [ADR 1](0001-kanbrain-core-follows-cargo-semver.md),
for a distinction nothing needed.

## Decision

- `kb hook tool`, on `PreToolUse` with no matcher (every tool), tells the agent of any person's comment on a task an
  agent has in doing, written after the agent's last comment there and since the task was last started: `kb show`
  it, then `kb comment` what it'll do about it. It says so before every tool call until the agent comments. It's
  context, never a decision, and only reads the brain.
- After, by the comments' order, not their times: those are to the second, and an agent's and the user's often share
  one.
- No new comment kind. The app's **Save and tell** writes a plain comment by the user that says what changed
  (`Changed the title and tags: <note>`). Any other comment of the user's reaches the agent the same way.
- `kb hook stop`'s reminder of a task left uncommented counts only the agent's comments: the user's says nothing of
  where the agent stands.
- `kb context`, which the `SessionStart` hook runs, adds the hooks a release added where kanbrain's are already
  there (`claude::update_hooks`), never anew: a repository whose hooks `kb deinit` took out, or that has only
  `AGENTS.md`, stays without.

## Consequences

- The agent hears the user with its next step (the context comes next to the tool's result), in whichever session
  works on the task. Another session's agent is told too; the wording ("if it's yours") leaves the task to the one
  that started it, as `stop`'s reminder does.
- A comment the user meant only as a note to self on an agent's task reaches the agent all the same. On a task the
  agent is working on, that's taken as wanted.
- It costs a process per tool call: about a millisecond on this repository's brain, 6 ms (median) with 500 entries.
- `kanbrain-core`'s API and the files are unchanged. If the board ever needs to tell the user's notices apart from
  comments (drawn differently in the thread), that's a new comment kind then, made `#[non_exhaustive]` with it.
- Projects get the new hook with their next session after the update, without `kb init` run again in each.
