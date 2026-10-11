//! The workspace's own tasks, run with `cargo xtask` (an alias in `.cargo/config.toml`), in Rust like the rest.
//!
//! - `notices kb|kanbrain`: prints the program's THIRD-PARTY-LICENSES.txt, which its release archives carry. The
//!   permissive licenses of the crates compiled into it (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, MPL-2.0, ...)
//!   require their notices with every copy. cargo-about finds the crates and their license texts (see
//!   `about.toml`); what's compiled in but isn't a crate comes from `licenses/`: Rust's standard library, musl (the
//!   static C library of `kb` on Linux) and the fonts GPUI embeds in the kanban app.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fmt::Write;
use std::path::Path;
use std::process::Command;

/// The line between sections.
const RULE: &str =
    "====================================================================================================";

/// The top of the file.
const HEADER: &str = "Third-party software\n\n\
    This program is kanbrain's, under AGPL-3.0-only (LICENSE) with the additional terms in NOTICE. It\n\
    includes the software below, each under its own license, whose text follows the list of what uses it. The\n\
    source of every crate is available from https://crates.io/crates/NAME/VERSION, or from the repository given.\n";

/// Rust's standard library, compiled into both programs.
const RUST: Extra = Extra { title: "The Rust standard library", text: include_str!("../licenses/rust.txt") };

/// A program the release ships, as the notices see it.
struct Program {
    /// Its name, which the command takes and the archives carry.
    name: &'static str,
    /// The folder of the crate that builds it.
    crate_dir: &'static str,
    /// What's compiled into it that isn't a crate.
    extras: &'static [Extra],
}

/// A license of something compiled in that isn't a crate, kept in `licenses/`.
struct Extra {
    /// What it is.
    title: &'static str,
    /// Its license, as its authors give it.
    text: &'static str,
}

/// The programs the release ships.
const PROGRAMS: [Program; 2] = [
    Program {
        name: "kb",
        crate_dir: "cli",
        extras: &[
            RUST,
            Extra {
                title: "musl, the C library linked statically into kb on Linux",
                text: include_str!("../licenses/musl.txt"),
            },
        ],
    },
    Program {
        name: "kanbrain",
        crate_dir: "gui",
        extras: &[
            RUST,
            Extra {
                title: "IBM Plex Sans, a font embedded by GPUI",
                text: include_str!("../licenses/ibm-plex-sans.txt"),
            },
            Extra { title: "Lilex, a font embedded by GPUI", text: include_str!("../licenses/lilex.txt") },
        ],
    },
];

/// What `cargo about generate --format json` reports: every license text, with the crates under it.
#[derive(Deserialize)]
struct About {
    /// One entry per distinct text: crates whose license files differ get their own.
    licenses: Vec<License>,
}

/// One license text and the crates it covers.
#[derive(Deserialize)]
struct License {
    /// Its full name.
    name: String,
    /// Its SPDX id.
    id: String,
    /// The text, with the copyright lines of the crates it came from.
    text: String,
    /// The crates under it.
    used_by: Vec<Use>,
}

/// A crate under a license.
#[derive(Deserialize)]
struct Use {
    /// The crate.
    #[serde(rename = "crate")]
    krate: Crate,
}

/// A crate, as the notices name it.
#[derive(Deserialize)]
struct Crate {
    /// Its name on crates.io.
    name: String,
    /// The version compiled in.
    version: String,
    /// Where its source is, when it says.
    repository: Option<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ["notices", name] = args.iter().map(String::as_str).collect::<Vec<_>>()[..] else {
        bail!("usage: cargo xtask notices kb|kanbrain");
    };
    let Some(program) = PROGRAMS.iter().find(|p| p.name == name) else {
        bail!("no program named {name}: kb or kanbrain");
    };
    print!("{}", notices(program, &about(program)?));
    Ok(())
}

/// Runs cargo-about on the crate that builds `program`. It fails on a license `about.toml` doesn't accept.
fn about(program: &Program) -> Result<About> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().context("finding the workspace")?;
    let output = Command::new("cargo")
        .args(["about", "generate", "--fail", "--locked", "--format", "json", "-c"])
        .arg(root.join("xtask/about.toml"))
        .arg("-m")
        .arg(root.join(program.crate_dir).join("Cargo.toml"))
        .output()
        .context("running cargo about")?;
    if !output.status.success() {
        bail!(
            "cargo about failed (install it with `cargo install --locked --features cli cargo-about`):\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("reading cargo about's output")
}

/// The text of `program`'s THIRD-PARTY-LICENSES.txt: its extras, then each license with the crates under it,
/// leaving out this workspace's own crates, whose license is the program's.
fn notices(program: &Program, about: &About) -> String {
    let mut out = String::from(HEADER);
    for extra in program.extras {
        let _ = write!(out, "\n{RULE}\n{}\n\n{}\n", extra.title, extra.text.trim());
    }
    for license in &about.licenses {
        let mut crates =
            license.used_by.iter().map(|u| &u.krate).filter(|c| !c.name.starts_with("kanbrain")).peekable();
        if crates.peek().is_none() {
            continue;
        }
        let _ = write!(out, "\n{RULE}\n{} ({}), used by:\n", license.name, license.id);
        for krate in crates {
            let _ = write!(out, "  - {} {}", krate.name, krate.version);
            if let Some(repository) = &krate.repository {
                let _ = write!(out, " ({repository})");
            }
            out.push('\n');
        }
        let _ = write!(out, "\n{}\n", license.text.trim());
    }
    out
}

#[cfg(test)]
#[path = "../tests/main.rs"]
mod tests;
