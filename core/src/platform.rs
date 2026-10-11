//! Operating-system specifics.

use anyhow::{Result, bail};

/// kanbrain is a per-user tool: as root it would write root-owned files into the user's data folder,
/// or use root's instead. Containers that only have root can opt in with `KANBRAIN_ALLOW_ROOT=1`.
pub fn ensure_not_root() -> Result<()> {
    if is_root() && std::env::var_os("KANBRAIN_ALLOW_ROOT").is_none_or(|v| v != "1") {
        bail!(
            "kanbrain is a per-user app and doesn't run as root (sudo).\n\
             Run it as your normal user. Only if root is your only user (e.g. a container), set KANBRAIN_ALLOW_ROOT=1."
        );
    }
    Ok(())
}

/// Whether the effective user is root.
#[cfg(unix)]
fn is_root() -> bool {
    rustix::process::geteuid().is_root()
}

/// Windows has no root; an elevated shell still runs as the user's own account.
#[cfg(not(unix))]
fn is_root() -> bool {
    false
}
