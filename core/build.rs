//! Holds a release to its notes: in the repository, a version whose `tag-changelog/vX.Y.Z.md` isn't written yet
//! doesn't build, so it can't be tagged without them. The crate as published on crates.io has no `tag-changelog/`
//! beside it, and builds.

use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let dir = Path::new(&manifest).join("../tag-changelog");
    println!("cargo:rerun-if-changed={}", dir.display());
    let notes = dir.join(format!("v{version}.md"));
    assert!(!dir.is_dir() || notes.is_file(), "write the release notes first: {} is missing", notes.display());
}
