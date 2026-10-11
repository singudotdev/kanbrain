//! Embeds the icon into kanbrain.exe when building for Windows, as resource 1, which GPUI loads for the window
//! and Windows shows for the taskbar, Explorer and the Start Menu shortcut. Other systems get their icon elsewhere:
//! the window's app id and the installed SVG on Linux, the app bundle the installer makes on macOS.

fn main() {
    println!("cargo:rerun-if-changed=icons/kanbrain.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("icons/kanbrain.ico")
            .compile()
            .expect("embedding the icon into kanbrain.exe");
    }
}
