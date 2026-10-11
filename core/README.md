# kanbrain-core

The core of [kanbrain](https://github.com/singudotdev/kanbrain): entries and their markdown files, tags, time
tracking, projects and the command line, shared by `kb` (the [`kanbrain`](https://crates.io/crates/kanbrain) crate)
and the kanban app.

It's published only because `kanbrain` depends on it. Its API serves those two programs and changes with them: it
isn't meant for other crates, and its versions follow kanbrain's. To use kanbrain, install the command line with
`cargo install kanbrain`.

Licensed [AGPL-3.0-only](https://github.com/singudotdev/kanbrain/blob/main/LICENSE), with the additional terms in
[NOTICE](https://github.com/singudotdev/kanbrain/blob/main/NOTICE).
