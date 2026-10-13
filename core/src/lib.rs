//! kanbrain's core: entries and their markdown files, tags, and the operations on them ([`brain`]).
//! Shared by the command line (`kb`) and the kanban desktop app (`kanbrain`, which also takes its commands).
//! See `docs/development.md` for an overview of the modules.
//!
//! It's published on crates.io only because `kanbrain`, the command line, depends on it. Its API serves those two
//! programs and changes with them: it isn't meant for other crates, and its versions follow kanbrain's.

// An internal crate, documented with `--document-private-items`: module overviews link to private modules.
#![allow(rustdoc::private_intra_doc_links)]

pub mod api;
pub mod brain;
pub mod cli;
mod enums;
pub mod export;
pub mod migrate;
pub mod model;
pub mod period;
pub mod platform;
pub mod projects;
pub mod store;
pub mod tags;
