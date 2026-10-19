//! Whether Linux or FreeBSD has a display to show the board on, checked before GPUI connects to it. Without one GPUI
//! runs headless, its window unseen and its loop never ending; with one that's named but dead it panics, after
//! libxcb's TCP fallback has waited out a connection timeout on X11. Either way the app wouldn't say why.
//!
//! What this can't tell: a host-named X display whose name takes long to resolve, and a server that accepts the
//! connection but then stalls or refuses it (X authorization). GPUI's panic then is caught by [`super::application`].

use anyhow::{Result, bail};
use std::ffi::OsString;
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a host-named X display (`ssh -X`'s `localhost:10`) has to answer.
const TCP_TIMEOUT: Duration = Duration::from_secs(2);

/// What to do when there's none, after what's wrong.
const RUN_FROM_DESKTOP: &str = "run kanbrain from the desktop session, not over ssh or under sudo; `kb` works anywhere";

/// Where GPUI would connect to show its window.
#[derive(Debug, PartialEq)]
enum Socket {
    /// A Unix socket file: Wayland's, or an X server's in `/tmp/.X11-unix`.
    Unix(PathBuf),
    /// A Unix socket in Linux's abstract namespace, its name without the leading NUL: where an X server listens too.
    #[cfg(target_os = "linux")]
    Abstract(String),
    /// An X server on a host, at its port.
    Tcp(String, u16),
}

/// Fails, saying why and to run it from the desktop session, when there's no display to show the board on, or when
/// the one named doesn't answer: over ssh, or under sudo, which drops `DISPLAY` and `WAYLAND_DISPLAY`, or with one
/// left from a session that's gone.
pub fn ensure_display() -> Result<()> {
    ensure_display_on(gpui::guess_compositor(), |name| std::env::var_os(name), answers)
}

/// [`ensure_display`] with `compositor`, what GPUI runs on (from [`gpui::guess_compositor`]), the environment's
/// variables from `var`, and `answers` saying whether something listens at a socket.
fn ensure_display_on(
    compositor: &str,
    var: impl Fn(&str) -> Option<OsString>,
    answers: impl Fn(&Socket) -> bool,
) -> Result<()> {
    let shown = |name| var(name).unwrap_or_default().to_string_lossy().into_owned();
    match compositor {
        "Headless" => bail!(
            "there's no display to open the board's window on (no WAYLAND_DISPLAY or DISPLAY): {RUN_FROM_DESKTOP}"
        ),
        // A socket handed down by a compositor that started the app is already connected.
        "Wayland" if var("WAYLAND_SOCKET").is_none() => {
            let display = shown("WAYLAND_DISPLAY");
            let Some(socket) = wayland_socket(Path::new(&display), var("XDG_RUNTIME_DIR")) else {
                bail!(
                    "WAYLAND_DISPLAY={display} names a socket in XDG_RUNTIME_DIR, which isn't set (to an absolute \
                     path): {RUN_FROM_DESKTOP}, or unset WAYLAND_DISPLAY to use X11"
                );
            };
            if !answers(&Socket::Unix(socket.clone())) {
                bail!(
                    "no Wayland compositor at WAYLAND_DISPLAY={display} (nothing answers at {}): {RUN_FROM_DESKTOP}, \
                     or unset WAYLAND_DISPLAY to use X11",
                    socket.display()
                );
            }
        }
        "X11" => {
            let display = shown("DISPLAY");
            let sockets = x11_sockets(&display);
            if !sockets.is_empty() && !sockets.iter().any(answers) {
                bail!("no X server at DISPLAY={display}: {RUN_FROM_DESKTOP}");
            }
        }
        _ => {}
    }
    Ok(())
}

/// The Wayland compositor's socket `display` names, as libwayland finds it: itself if absolute, else in
/// `runtime_dir`, which must be absolute. None without one.
fn wayland_socket(display: &Path, runtime_dir: Option<OsString>) -> Option<PathBuf> {
    if display.is_absolute() {
        return Some(display.to_path_buf());
    }
    let dir = PathBuf::from(runtime_dir?);
    dir.is_absolute().then(|| dir.join(display))
}

/// Where libxcb looks for the X server `display` names, `[host]:n[.screen]`: on this machine its socket in
/// `/tmp/.X11-unix` (on Linux the abstract one of that name too), on a host its TCP port 6000 + n. None for a name
/// this doesn't read (a protocol given, `DECnet`'s `host::n`), which is left to GPUI.
fn x11_sockets(display: &str) -> Vec<Socket> {
    let Some((host, rest)) = display.rsplit_once(':') else { return Vec::new() };
    let number = rest.split_once('.').map_or(rest, |(number, _)| number);
    let Ok(number) = number.parse::<u16>() else { return Vec::new() };
    if host.contains('/') || host.ends_with(':') {
        return Vec::new();
    }
    if host.is_empty() || host == "unix" {
        let path = format!("/tmp/.X11-unix/X{number}");
        return vec![
            #[cfg(target_os = "linux")]
            Socket::Abstract(path.clone()),
            Socket::Unix(PathBuf::from(path)),
        ];
    }
    number.checked_add(6000).map(|port| vec![Socket::Tcp(host.to_string(), port)]).unwrap_or_default()
}

/// Whether something accepts a connection at `socket`; closed right away, which a compositor or an X server takes
/// for a client gone.
fn answers(socket: &Socket) -> bool {
    match socket {
        Socket::Unix(path) => UnixStream::connect(path).is_ok(),
        #[cfg(target_os = "linux")]
        Socket::Abstract(name) => {
            use std::os::linux::net::SocketAddrExt;
            std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())
                .and_then(|addr| UnixStream::connect_addr(&addr))
                .is_ok()
        }
        Socket::Tcp(host, port) => (host.as_str(), *port)
            .to_socket_addrs()
            .is_ok_and(|mut addrs| addrs.any(|addr| TcpStream::connect_timeout(&addr, TCP_TIMEOUT).is_ok())),
    }
}

#[cfg(test)]
#[path = "../../tests/display.rs"]
mod tests;
