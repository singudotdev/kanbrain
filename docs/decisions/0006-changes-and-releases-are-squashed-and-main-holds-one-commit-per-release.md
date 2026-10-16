# 6. Changes and releases are squashed, and `main` holds one commit per release

Date: 2026-10-15 (recorded 2026-10-16). Status: accepted.

## Context

`main` is what's released: the README and the docs people read are `main`'s, and `kb update` in releases before
v0.4.0 runs `main`'s `install.sh`. It has to hold the last version tagged, working, and nothing else. Work goes
through pull requests that often take several commits (a fix after review, a test a mutant asked for), and the
version is derived from the commits in a release's range, so their messages have to say what changed and whether
it breaks.

A merge commit keeps each pull request's own commits in the branch's history; a rebase rewrites them, and with them
their signatures.

## Decision

- A change goes from its branch (`feat/…`, `fix/…`, `docs/…`, `test/…`, `ci/…`) into `develop` through a pull request,
  once CI and mutation testing pass on it, squashed into one signed commit whose message is the pull request's title:
  one line, imperative, conventional, with `!` when it breaks. A feature's fixes go into its open pull request, not
  into another after it.
- A release is cut from `develop` on `release/vX.Y.Z`, which adds only its notes and the version, is checked there in
  full, and is squashed into `main` through a pull request as `chore: release vX.Y.Z`: one commit per release, which
  gets the signed tag.
- `main` is then merged back into `develop` with a signed merge commit, so the next release's pull request holds only
  what's new.
- A fix for a released version starts from `main` on `hotfix/…` and goes back to both.

## Consequences

- `develop`'s history is one line per change, from which the next version and its notes are read; `main`'s is one
  line per release.
- The squash holds the tree its pull request passed with, so CI doesn't run again on it
  ([ADR 5](0005-ci-runs-a-job-for-what-a-change-touches-and-none-for-a-tree-already-checked.md)).
- A pull request's own commits survive only on GitHub, in the pull request.
