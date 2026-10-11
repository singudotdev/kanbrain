# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-11

### Added

- `kb`, the command line for agents and scripts: `init` connects a repository to its agents (`.brain/`, the project
  registry, the agent instructions and Claude Code hooks), and commands add, move, comment on, link, search, list and
  report on tasks, notes and decisions, with `--json` output.
- `kanbrain`, the kanban app: board or list, notes, decisions, report and calendar tabs, project and tag filters,
  undo, a command palette, and adding a project from a folder. It shows other programs' changes within a second.
- Agents ask instead of guessing: `kb ask` parks a task with a question and `kb propose` proposes a decision; the
  board shows them as waiting for you until you answer or accept or reject. `kb wait`, run in the background,
  wakes the agent with your answer; otherwise its next `context` lists the answered ones first. Comments and moves
  record whether a person or an agent made them.
- Projects of several repositories sharing one card, with entries linked across them.
- Time tracking with one running task per brain, and reports by task or tag.
- Installers for Linux, macOS and Windows, with checksums, the kanban app's launcher and icon, and `kb update`.
- Release archives with SHA-256 checksums and signed build provenance, and the licenses of everything compiled into
  each program (`THIRD-PARTY-LICENSES.txt`).
- The command line on crates.io: `cargo install kanbrain` installs `kb`.
- Licensed AGPL-3.0-only with additional terms in `NOTICE`: attribution, no presenting kanbrain as one's own work,
  modified versions marked as such, and no rights to the kanbrain name or logo.
