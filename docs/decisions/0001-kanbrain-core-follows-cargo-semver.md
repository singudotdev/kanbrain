# 1. kanbrain-core follows Cargo's SemVer, and the command line's types are non-exhaustive

Date: 2026-10-15. Status: accepted.

## Context

kanbrain is a command line (`kb`, the crate `kanbrain`) and a kanban app over the same files. Both are built on
`kanbrain-core`, which is published on crates.io only because `kanbrain` depends on it; its README says its API isn't
meant for other crates. The workspace has one version, so a release's number covers all three.

For `kb` and the app, a breaking change is one to what their users meet: commands, options, output, the data format,
the agents' instructions. But `kanbrain-core` is a Rust library on crates.io, and Cargo resolves
`kanbrain-core = "0.4.2"` to any later `0.4.z`: what it promises is its Rust API, which `cargo semver-checks` checks.

The two met with `kb tag`: a new command, additive for `kb`, is a new variant of `cli::EntryCommand`, a public enum
that any `match` outside the crate must cover, so adding it broke the crate's API (`enum_variant_added`).

## Decision

- A release's number follows the stricter of the two: a break of `kb`, the app or the data, or of `kanbrain-core`'s
  API as `cargo semver-checks -p kanbrain-core --baseline-rev vLAST` reports it, takes the breaking slot (`y` in
  `0.y.z`).
- What the command line parses to (`cli::Command`, `cli::EntryCommand`, `cli::ProjectChange` and their parts
  `cli::ProjectPart`, `cli::TagChange` and `cli::Text`, each with its variants; the options' structs) is
  `#[non_exhaustive]` from v0.5.0, so a new command, or a new option of one, is additive for the crate too, as it is
  for `kb`. The variants are marked as well as the enums: a field added to a variant (`project rename`'s `--` was
  one, in v0.4.0) breaks a pattern on it unless the variant is. What `kb hook`, `kb completions` and `kb complete`
  take is left as it is: those types are private to `cli`, and no other crate can name them.

## Consequences

- v0.5.0 is a breaking release for `kanbrain-core`'s API only; `kb`, the app and the data need nothing migrated.
- A new command or option no longer forces a breaking release. A new variant of another public enum still does,
  among them an option's new value (`period::Unit`, `brain::Group`, `model::Kind`, `model::Status`,
  `model::DecisionState`) and `api::ApiError` or `cli::update::Install`, until it's made non-exhaustive in a release
  that breaks anyway.
- Outside the crate, a `match` on these types needs a `_` arm and `..` in each variant's pattern, and they can't be
  built there. Only `kanbrain-core` builds them: `kb` and the app go through `cli::Program`.
