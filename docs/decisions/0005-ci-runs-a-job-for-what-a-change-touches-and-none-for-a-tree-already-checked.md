# 5. CI runs a job for what a change touches, and none for a tree already checked

Date: 2026-10-15 (recorded 2026-10-16). Status: accepted.

## Context

A full run is long: tests, the installers and the app started on Linux, macOS and Windows, the upgrade from every
release, and mutation testing in eight shards. Most changes touch one part (a script, the docs, the app), and every
change passes CI twice with the same files: on its pull request, then as the squash of it into `develop` (or of a
release into `main`), which holds the tree the pull request passed with. A re-signed commit is the same tree again.

GitHub's own filter, a workflow's `paths:`, decides only by the files a push changed, can't see that a tree passed
before, and a workflow it skips leaves its checks pending, which a required check then blocks
([GitHub: handling skipped but required checks](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/troubleshooting-required-status-checks#handling-skipped-but-required-checks)).
A job skipped by its `if:` reports success.

## Decision

- Each workflow (`ci.yml`, `mutants.yml`) starts with a `plan` job, `.github/plan.sh`, whose outputs the other jobs'
  `if:` read.
- A tree a passing run of the same workflow already checked runs nothing: each run uploads an artifact named after
  its workflow and tree (`ci-tree-…`, `mutants-tree-…`, kept 90 days), and the plan looks for a passing one. Only
  this repository's runs count, as a fork's pull request runs its own workflow.
- Otherwise a job runs when the change touches what it checks (the table in `docs/development.md`). The change is
  measured from the last commit on the branch, or on a pull request's base, whose push run passed, so what a failing
  push brought is still checked by the next; a rename counts by both its paths.
- A `release/*` branch, a change to the workflows, the plan, the toolchain, the tools' configuration or the
  dependencies, and a change with nothing to compare against run everything.
- `release.yml` refuses a tag whose commit has no passing `ci.yml` and `mutants.yml` run; a run that found its tree
  checked counts.

## Consequences

- The squash into `develop` or `main` costs a plan job, not a second full run, and a docs-only change runs nothing.
- A job's table entry has to name every file it reads: a file built into a crate or read by its tests
  (`tag-changelog/`, `docs/agents.md`, `docs/cli.md`, the README) is listed with that crate, or a change to it would
  skip the job that checks it.
- A tree's artifact outlives its run by 90 days; a tree checked longer ago runs again.
