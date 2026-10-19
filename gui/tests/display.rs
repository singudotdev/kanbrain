use super::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// The environment `vars` gives, as `std::env::var_os` would.
fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    let vars: HashMap<_, _> = vars.iter().copied().collect();
    move |name| vars.get(name).map(OsString::from)
}

/// [`ensure_display_on`] with `vars`, where only `listening` answers; its error, and the sockets it tried.
fn check(compositor: &str, vars: &[(&str, &str)], listening: &[Socket]) -> (Option<String>, Vec<Socket>) {
    let tried = RefCell::new(Vec::new());
    let result = ensure_display_on(compositor, env(vars), |socket| {
        let answers = listening.contains(socket);
        tried.borrow_mut().push(match socket {
            Socket::Unix(path) => Socket::Unix(path.clone()),
            #[cfg(target_os = "linux")]
            Socket::Abstract(name) => Socket::Abstract(name.clone()),
            Socket::Tcp(host, port) => Socket::Tcp(host.clone(), *port),
        });
        answers
    });
    (result.err().map(|e| e.to_string()), tried.into_inner())
}

/// Where an X server on this machine listens for display `n`.
fn local_x(n: u16) -> Vec<Socket> {
    let path = format!("/tmp/.X11-unix/X{n}");
    vec![
        #[cfg(target_os = "linux")]
        Socket::Abstract(path.clone()),
        Socket::Unix(PathBuf::from(path)),
    ]
}

#[test]
fn without_a_display_the_app_says_to_run_it_from_the_desktop() {
    let (error, tried) = check("Headless", &[], &[]);
    let error = error.expect("GPUI would run headless, its loop never ending");
    assert!(error.contains("no WAYLAND_DISPLAY or DISPLAY") && error.contains("run kanbrain from the desktop session"));
    assert_eq!(tried, Vec::new());
}

#[test]
fn a_wayland_compositor_that_answers_opens_the_board() {
    let socket = Socket::Unix("/run/user/1000/wayland-1".into());
    let vars = [("WAYLAND_DISPLAY", "wayland-1"), ("XDG_RUNTIME_DIR", "/run/user/1000")];
    assert_eq!(check("Wayland", &vars, &[Socket::Unix("/run/user/1000/wayland-1".into())]), (None, vec![socket]));
    let absolute = [("WAYLAND_DISPLAY", "/run/compositor")];
    assert_eq!(check("Wayland", &absolute, &[Socket::Unix("/run/compositor".into())]).0, None, "used as it is");
}

#[test]
fn a_wayland_display_nothing_answers_at_says_to_unset_it() {
    let vars = [("WAYLAND_DISPLAY", "wayland-1"), ("XDG_RUNTIME_DIR", "/run/user/1000")];
    let error = check("Wayland", &vars, &[]).0.expect("GPUI would panic");
    assert!(
        error.starts_with(
            "no Wayland compositor at WAYLAND_DISPLAY=wayland-1 (nothing answers at /run/user/1000/wayland-1)"
        ),
        "{error}"
    );
    assert!(
        error.contains("run kanbrain from the desktop session") && error.ends_with("unset WAYLAND_DISPLAY to use X11")
    );
}

#[test]
fn a_wayland_display_without_its_runtime_dir_says_so() {
    for vars in
        [&[("WAYLAND_DISPLAY", "wayland-1")][..], &[("WAYLAND_DISPLAY", "wayland-1"), ("XDG_RUNTIME_DIR", "run")]]
    {
        let (error, tried) = check("Wayland", vars, &[]);
        let error = error.expect("libwayland finds no socket");
        assert!(
            error.starts_with("WAYLAND_DISPLAY=wayland-1 names a socket in XDG_RUNTIME_DIR, which isn't set"),
            "{error}"
        );
        assert_eq!(tried, Vec::new());
    }
}

#[test]
fn a_wayland_socket_handed_down_is_already_connected() {
    let vars = [("WAYLAND_SOCKET", "3"), ("WAYLAND_DISPLAY", "wayland-1")];
    assert_eq!(check("Wayland", &vars, &[]), (None, Vec::new()));
}

#[test]
fn an_x_server_on_this_machine_is_tried_at_its_sockets() {
    for display in [":0", ":0.1", "unix:0"] {
        let (error, tried) = check("X11", &[("DISPLAY", display)], &[Socket::Unix("/tmp/.X11-unix/X0".into())]);
        assert_eq!(error, None, "{display}");
        assert_eq!(tried, local_x(0), "{display}");
    }
    #[cfg(target_os = "linux")]
    assert_eq!(
        check("X11", &[("DISPLAY", ":3")], &[Socket::Abstract("/tmp/.X11-unix/X3".into())]).0,
        None,
        "abstract only"
    );
}

#[test]
fn an_x_display_nothing_answers_at_says_so() {
    let (error, tried) = check("X11", &[("DISPLAY", ":99")], &[]);
    let error = error.expect("GPUI would panic, after libxcb's TCP fallback");
    assert!(error.starts_with("no X server at DISPLAY=:99: run kanbrain from the desktop session"), "{error}");
    assert_eq!(tried, local_x(99));
}

#[test]
fn an_x_display_on_a_host_is_tried_at_its_port() {
    let ssh = Socket::Tcp("localhost".into(), 6010);
    assert_eq!(
        check("X11", &[("DISPLAY", "localhost:10.0")], &[Socket::Tcp("localhost".into(), 6010)]),
        (None, vec![ssh])
    );
    let error = check("X11", &[("DISPLAY", "localhost:10.0")], &[]).0.expect("nothing at 6010");
    assert!(error.starts_with("no X server at DISPLAY=localhost:10.0"), "{error}");
}

#[test]
fn an_x_display_it_cant_read_is_left_to_gpui() {
    for display in ["tcp/localhost:0", "host::0", "nonsense", ":x", "host:65000"] {
        assert_eq!(check("X11", &[("DISPLAY", display)], &[]), (None, Vec::new()), "{display}");
    }
}

#[test]
fn something_listening_answers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wayland-1");
    assert!(!answers(&Socket::Unix(path.clone())), "nothing there yet");
    let _unix = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(answers(&Socket::Unix(path)));

    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    assert!(answers(&Socket::Tcp("127.0.0.1".into(), port)));
    drop(tcp);
    assert!(!answers(&Socket::Tcp("127.0.0.1".into(), port)), "closed");
}

#[cfg(target_os = "linux")]
#[test]
fn something_listening_in_the_abstract_namespace_answers() {
    use std::os::linux::net::SocketAddrExt;
    let name = format!("kanbrain-test-{}", std::process::id());
    assert!(!answers(&Socket::Abstract(name.clone())), "nothing there yet");
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind_addr(&addr).unwrap();
    assert!(answers(&Socket::Abstract(name)));
}
