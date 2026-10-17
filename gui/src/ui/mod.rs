//! The window: a top bar, the tab's content with the details drawer beside it, dialogs over it and a
//! status bar. Buttons run the same keys as the keyboard ([`Board::on_key`]); other clicks select or
//! open something.
//!
//! - [`topbar`]: tabs, search, the running tasks' clocks and the window's buttons.
//! - [`board`]: the kanban columns and their cards.
//! - [`list`]: the board as one list, a section per status.
//! - [`entries`]: the notes and decisions lists.
//! - [`drawer`]: the selected entry's details, beside the board or list.
//! - [`report`]: tracked time by task or tag.
//! - [`calendar`]: each day's interactions, for a day, week or month.
//! - [`dialogs`]: the new / edit form, the text editor, confirmations, links, the command palette and the project
//!   dialogs.
//! - [`mod@menu`]: the right-click menu.
//! - [`dropdown`]: the project and tag filters' dropdowns.
//! - [`markdown`]: descriptions and comments drawn as formatted text.
//! - [`widgets`]: buttons, fields, pills and chips.
//! - [`caret`]: the blinking text cursor.
//! - [`mod@pointer`]: the mouse in text fields and the text editor.
//! - [`theme`]: colors, fonts and styled text.
//! - [`keys`]: GPUI's keystrokes as the board's keys, and keys as people write them.
//! - [`text`]: text cut to a width, dates, and a field's text with its selection.

mod board;
mod calendar;
mod caret;
mod dialogs;
mod drawer;
mod dropdown;
mod entries;
mod keys;
mod list;
mod markdown;
mod menu;
mod pointer;
mod report;
mod text;
mod theme;
mod topbar;
mod widgets;

use crate::state::{Board, COLUMNS, Layout, Mode, Request, Scope, Tab};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::update;
use crate::watch::{self, Watcher};
use anyhow::{Context as _, Result, bail};
use gpui::{
    App as GpuiApp, Bounds, ClickEvent, ClipboardItem, Context, Div, ElementId, FocusHandle, KeyDownEvent, Keystroke,
    PathPromptOptions, ScrollHandle, SharedString, Stateful, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
use keys::{key_events, key_name};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use text::{byte_at, edited, truncate, truncate_words, when};
use widgets::Kind;

/// The window's application id on Linux: the name of the installer's `kanbrain.desktop` and of its icon, so
/// the desktop shows the window with them.
const APP_ID: &str = "kanbrain";

/// The window's icon on X11, which takes it from the window; Wayland desktops take the installed one by app id.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    const PNG: &[u8] = include_bytes!("../../icons/kanbrain-128.png");
    let decoded = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?;
    Some(std::sync::Arc::new(decoded.into_rgba8()))
}

/// How often the window redraws without input, so clocks tick and messages expire.
const TICK: Duration = Duration::from_millis(500);

/// Ticks between redraws with nothing running and no message: a minute, as cards give their ages.
const IDLE_TICKS: u32 = 120;

/// Width of the details drawer.
const DRAWER_WIDTH: f32 = 420.;

/// The fonts the window draws with, IBM Plex Sans and Lilex (licenses beside them in `gui/fonts/`): GPUI takes "IBM
/// Plex Sans" for the system font on Linux, where few have it, and found none to make bold or italic in its stead.
/// Lilex is the editor's and code's monospace on every platform.
const FONTS: [&[u8]; 8] = [
    include_bytes!("../../fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf"),
    include_bytes!("../../fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"),
    include_bytes!("../../fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../../fonts/lilex/Lilex-Italic.ttf"),
    include_bytes!("../../fonts/lilex/Lilex-Bold.ttf"),
    include_bytes!("../../fonts/lilex/Lilex-BoldItalic.ttf"),
];

/// Opens the window and runs until it's closed or the board quits.
pub fn run(board: Board) {
    gpui_platform::application().run(move |cx: &mut GpuiApp| {
        // Without them the text still shows, in whatever the system has.
        if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|&font| Cow::Borrowed(font)).collect()) {
            eprintln!("kanbrain: couldn't load the window's fonts: {e:#}");
        }
        cx.on_window_closed(|cx, _window| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1400.), px(860.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            app_id: Some(APP_ID.to_string()),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            icon: window_icon(),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            let title = match &board.scope {
                Scope::Project(name) => format!("{name} · kanbrain"),
                Scope::Repo(src) => format!("{} · kanbrain", board.sources[*src].label.as_deref().unwrap_or_default()),
                Scope::All | Scope::Central => "kanbrain".to_string(),
            };
            window.set_window_title(&title);
            cx.new(|cx| BrainView::new(board, window, cx))
        });
        if let Err(e) = opened {
            eprintln!("kanbrain: could not open the window: {e:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}

/// The root view: the board, and what the window needs to draw it.
pub struct BrainView {
    /// Entries, the tab shown, and everything that changes them.
    pub board: Board,
    /// Keyboard focus of the window, so key presses reach the board.
    focus: FocusHandle,
    /// Scroll positions of the scrollable areas.
    scroll: Scrolls,
    /// Whether the next frame scrolls the selection into view: after the keyboard moved it, but not on
    /// every frame, so the wheel can scroll freely.
    follow: bool,
    /// The field or editor a mouse drag that selects text started in, while the button is down.
    drag: Option<pointer::TextBox>,
    /// Where the last right click was, in the window: where its menu is drawn while it's open.
    menu_at: Option<gpui::Point<gpui::Pixels>>,
    /// The text last put in the primary selection, so it's only replaced when the selection changes.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    shared: Option<String>,
    /// Redraws every `TICK`; dropped with the view.
    _tick: gpui::Task<()>,
    /// The data folder, to reload when other programs change it.
    watcher: Watcher,
    /// Looks at the data folder every `watch::INTERVAL`; dropped with the view.
    _watch: gpui::Task<()>,
    /// Asks for a newer release at start and every `update::CHECK_EVERY`; dropped with the view.
    _check: Option<gpui::Task<()>>,
    /// Whether the newer release is being installed, after which the app closes.
    updating: bool,
    /// Blinks the text cursor; replaced on every input, which starts the blink over.
    blink: gpui::Task<()>,
    /// Whether the details were shown in the last frame.
    drawer_shown: bool,
}

/// Scroll positions of the scrollable areas.
#[derive(Default)]
struct Scrolls {
    /// Each board column, in `COLUMNS` order.
    columns: [ScrollHandle; COLUMNS.len()],
    /// The board's columns, sideways when the window (with the details open) is too narrow for them.
    board: ScrollHandle,
    /// The notes or decisions list.
    list: ScrollHandle,
    /// The board's tasks in its list layout.
    tasks: ScrollHandle,
    /// The details drawer.
    drawer: ScrollHandle,
    /// The report tab.
    report: ScrollHandle,
    /// The calendar tab.
    calendar: ScrollHandle,
    /// The command palette.
    palette: ScrollHandle,
    /// The text editor.
    editor: ScrollHandle,
    /// The open dropdown of the top bar: projects or tags (one at a time).
    dropdown: ScrollHandle,
    /// The form's tags field.
    form_tags: ScrollHandle,
    /// The form's related field.
    form_links: ScrollHandle,
    /// The form's project field.
    form_projects: ScrollHandle,
    /// The link picker.
    links: ScrollHandle,
    /// "Which project?".
    place: ScrollHandle,
    /// Manage project's body.
    manage_project: ScrollHandle,
    /// The projects listed by the add-project and manage dialogs.
    add_project: ScrollHandle,
}

impl BrainView {
    /// The view over `board`, focused so keys work right away, redrawing every `TICK` while something shown changes
    /// with time.
    fn new(board: Board, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let tick = cx.spawn(async move |this, cx| {
            let (mut ticks, mut said) = (0_u32, false);
            loop {
                cx.background_executor().timer(TICK).await;
                ticks = ticks.wrapping_add(1);
                let ticked = this.update(cx, |view, cx| {
                    // Redrawn when something shown changes with time: a clock's seconds, a message going, else the
                    // ages on cards, by the minute. Idle, drawing twice a second took several percent of a core.
                    let says = view.board.message().is_some();
                    if says || said || !view.board.running().is_empty() || ticks.is_multiple_of(IDLE_TICKS) {
                        cx.notify();
                    }
                    said = says;
                });
                if ticked.is_err() {
                    break;
                }
            }
        });
        let watch = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(watch::INTERVAL).await;
                if this.update(cx, BrainView::follow_disk).is_err() {
                    break;
                }
            }
        });
        let mut board = board;
        if let Some(failed) = update::take_failure() {
            board.notify(failed);
        }
        Self {
            watcher: Watcher::new(board.watched_dirs()),
            _watch: watch,
            _check: update::checks().then(|| Self::checking(cx)),
            updating: false,
            board,
            focus,
            scroll: Scrolls::default(),
            menu_at: None,
            follow: true,
            drag: None,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            shared: None,
            _tick: tick,
            blink: Self::blinking(cx),
            drawer_shown: false,
        }
    }

    /// Asks whether a newer release is out now and every `update::CHECK_EVERY`, off the window's thread. Not being
    /// able to ask, offline say, leaves what it last found.
    fn checking(cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let newer = cx.background_executor().spawn(async { kanbrain_core::cli::update::newer_release() });
                if let Ok(newer) = newer.await {
                    let found = this.update(cx, |this, cx| {
                        this.board.available = newer;
                        cx.notify();
                    });
                    if found.is_err() {
                        break;
                    }
                }
                cx.background_executor().timer(update::CHECK_EVERY).await;
            }
        })
    }

    /// Installs the newer release in the background and closes, for the new one the update starts; or says why
    /// it couldn't, and stays.
    fn install_update(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.updating, true) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let Ok(central) = this.update(cx, |this, _| this.board.central.clone()) else { return };
            let installed = cx.background_executor().spawn(async move { update::install(&central) }).await;
            this.update(cx, |this, cx| match installed {
                Ok(()) => cx.quit(),
                Err(e) => {
                    this.updating = false;
                    this.board.notify_error(&e);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Shows the text cursor and starts its blink over. Dropping the previous blink task stops it.
    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        drop(std::mem::replace(&mut self.blink, Self::blinking(cx)));
    }

    /// Shows the text cursor and blinks it from now on: every input restarts it, so it stays solid while typing.
    fn blinking(cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.set_global(caret::Blink(true));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(caret::BLINK).await;
                // Redrawn only with a cursor to blink: idle, the window would otherwise draw twice a second for
                // nothing.
                let blinked = this.update(cx, |view, cx| {
                    if view.board.is_typing() {
                        let shown = cx.global::<caret::Blink>().0;
                        cx.set_global(caret::Blink(!shown));
                        cx.notify();
                    }
                });
                if blinked.is_err() {
                    break;
                }
            }
        })
    }

    /// A key press: a clipboard shortcut, or handed to the board.
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keys = key_events(&event.keystroke);
        let handled = self.locked(|view| {
            if view.clipboard(&event.keystroke, cx) {
                return true;
            }
            for &key in &keys {
                view.board.on_key(key);
            }
            !keys.is_empty()
        });
        if !handled {
            return;
        }
        cx.stop_propagation();
        self.follow = true;
        self.after_input(cx);
    }

    /// Runs `input` holding every brain's write lock, from catching up with the files to marking the
    /// board's own saves as seen: a `kb` an agent runs meanwhile waits, so it can't write between the
    /// board's read and its save, nor the board over it. Released before `after_input`, which may run
    /// `$EDITOR` for as long as the user edits.
    fn locked<T>(&mut self, input: impl FnOnce(&mut Self) -> T) -> T {
        let mut tried = Vec::new();
        let mut locks = self.board.lock_brains(&mut tried);
        self.catch_up();
        self.board.lock_found_brains(&mut locks, &mut tried);
        let result = input(self);
        self.watcher.sync(self.board.watched_dirs());
        drop(locks);
        result
    }

    /// Ctrl+A / C / X / V (Cmd on macOS): select all, copy, cut and paste in the field or editor being
    /// typed into, with the system clipboard. Returns whether `keystroke` was one of them.
    fn clipboard(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let modifiers = &keystroke.modifiers;
        let shortcut = if cfg!(target_os = "macos") { modifiers.platform } else { modifiers.control };
        if !shortcut || modifiers.alt {
            return false;
        }
        match keystroke.key.as_str() {
            "a" => self.board.select_all(),
            "c" => {
                if let Some(text) = self.board.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "x" => {
                if let Some(text) = self.board.cut() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "v" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.board.paste(&text);
                }
            }
            _ => return false,
        }
        true
    }

    /// A button: runs `key` as if it had been pressed, on the board rather than into a field.
    fn press(&mut self, key: KeyEvent, cx: &mut Context<Self>) {
        self.locked(|view| view.board.press(key));
        self.follow = true;
        self.after_input(cx);
    }

    /// A click that changes the board some other way than a key.
    fn act(&mut self, cx: &mut Context<Self>, act: impl FnOnce(&mut Board)) {
        self.locked(|view| view.board.recorded(act));
        self.after_input(cx);
    }

    /// Re-reads the files when another program, such as an agent using the command line, changed them.
    /// The selection stays on the same entry and whatever is being typed is kept. Under the locks, as input
    /// is, once there's something to read: a write landing between the reload and taking the files as seen
    /// would otherwise never be read.
    fn follow_disk(&mut self, cx: &mut Context<Self>) {
        if self.watcher.dirty() {
            self.locked(|_| ());
            cx.notify();
        }
    }

    /// Re-reads the files if another program changed them since they were last looked at. Every input calls it
    /// before acting, not only the timer: the board saves whole entries from memory, so acting on a copy up to a
    /// second old would undo a comment or move an agent made in that second, and the save would then mark that
    /// change as seen.
    fn catch_up(&mut self) {
        let changed = self.watcher.changed();
        if changed.is_empty() {
            return;
        }
        if let Err(e) = self.board.reload_changed(&changed) {
            self.board.notify_error(&e);
        }
        // Loading may have written files itself, when it fixed several tasks left running, and may
        // have found projects added or gone.
        self.watcher.sync(self.board.watched_dirs());
    }

    /// Quits when the board asked to, runs a requested `$EDITOR`, shares the selected text, and redraws.
    fn after_input(&mut self, cx: &mut Context<Self>) {
        if self.board.quit {
            cx.quit();
            return;
        }
        self.share_selection(cx);
        self.restart_blink(cx);
        match self.board.request.take() {
            // "Open file" asked for an external editor: run it, and re-read the files when it exits since
            // anything may have changed.
            Some(Request::Edit(path)) => {
                let edit = cx.background_executor().spawn(async move { open_editor(&path) });
                cx.spawn(async move |this, cx| {
                    let edited = edit.await;
                    this.update(cx, |this, cx| {
                        // Under the locks, as every read: the watcher then takes the edit as seen.
                        if let Err(e) = edited.and_then(|()| this.locked(|this| this.board.reload())) {
                            this.board.notify_error(&e);
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Some(Request::Folder(from)) => Self::pick_project_folder(from, cx),
            Some(Request::Update) => self.install_update(cx),
            None => {}
        }
        cx.notify();
    }

    /// Opens the system's folder dialog, and asks which project the folder picked joins or starts: that of the
    /// repository whose `.brain/` is `from`, if given. On Linux the dialog is the desktop portal's: without one,
    /// the status bar says so.
    fn pick_project_folder(from: Option<PathBuf>, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add".into()),
        });
        cx.spawn(async move |this, cx| {
            let picked = picked.await;
            this.update(cx, |this, cx| {
                match picked {
                    Ok(Ok(Some(dirs))) => {
                        if let Some(dir) = dirs.into_iter().next() {
                            this.board.name_project(dir, from.as_deref());
                        }
                    }
                    Ok(Err(e)) => this.board.notify_error(&e),
                    // Cancelled, or the window closed meanwhile.
                    Ok(Ok(None)) | Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Puts selected text in the primary selection, as Linux desktops do, for a middle-click to paste elsewhere.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn share_selection(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.board.selected_text() else { return };
        if self.shared.as_ref() != Some(&text) {
            cx.write_to_primary(ClipboardItem::new_string(text.clone()));
            self.shared = Some(text);
        }
    }

    /// Only Linux desktops have a primary selection.
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    #[expect(clippy::unused_self, reason = "the same method as on Linux, where it keeps what it shared")]
    fn share_selection(&mut self, _: &mut Context<Self>) {}
}

impl Render for BrainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::next_frame(cx);
        let follow = std::mem::take(&mut self.follow);
        // The details opening or closing changes the board's width after this frame is laid out, with the
        // scroll to the selection worked out on the old one: it runs again once the new width is known.
        if self.board.drawer != self.drawer_shown {
            self.drawer_shown = self.board.drawer;
            cx.on_next_frame(window, |view, _, cx| {
                view.follow = true;
                cx.notify();
            });
        }
        let board = &self.board;
        let content = match board.tab {
            Tab::Board => match board.layout {
                Layout::Kanban => board::draw(board, &self.scroll.columns, &self.scroll.board, follow, cx),
                Layout::List => list::draw(board, &self.scroll.tasks, follow, cx),
            },
            Tab::Notes | Tab::Decisions => entries::draw(board, &self.scroll.list, follow, cx),
            Tab::Report => report::draw(board, &self.scroll.report, cx),
            Tab::Calendar => calendar::draw(board, &self.scroll.calendar, cx),
        };
        let drawer = (board.tab != Tab::Report)
            .then(|| {
                let key = board.selected?;
                board.drawer_task().map(|task| drawer::draw(board, key, task, &self.scroll.drawer, cx))
            })
            .flatten();
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap_3()
            .p_3()
            .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().child(content))
            .children(drawer.map(|d| d.w(px(DRAWER_WIDTH)).flex_none()));
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG))
            .text_color(rgb(theme::TEXT))
            .font_family(theme::UI_FONT)
            .text_size(px(14.))
            .child(topbar::draw(board, &self.scroll, follow, cx))
            .child(body)
            .child(status_bar(board, self.updating, cx))
            .children(dialogs::draw(board, &self.scroll, follow, cx))
            .children(menu::draw(board, self.menu_at, cx))
    }
}

/// The bottom bar: the latest message, or a tip; and where the files are, after the button to a newer release.
fn status_bar(board: &Board, updating: bool, cx: &mut Context<BrainView>) -> Div {
    let left = match board.message() {
        Some(msg) => div().text_color(rgb(theme::YELLOW)).child(msg.to_string()),
        None if matches!(board.mode, Mode::Normal) && board.tab == Tab::Board => widgets::dim(match board.layout {
            Layout::Kanban => {
                "Drag cards between columns · double-click opens one · v for a list · ? for commands and keys"
            }
            Layout::List => {
                "Drag rows onto a section · double-click opens one · v for columns · ? for commands and keys"
            }
        }),
        None => widgets::dim("Press ? for every command and its key"),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap_4()
        .h(px(26.))
        .px_3()
        .border_t_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::SURFACE))
        .text_size(px(12.))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(left)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .children(board.available.as_ref().map(|version| {
                    if updating {
                        return div().text_color(rgb(theme::YELLOW)).child(format!("Updating to {version}…"));
                    }
                    let update =
                        action("update", format!("Update to {version}"), Kind::Primary, KeyCode::Char('U'), cx);
                    div().child(widgets::small(update).h(px(20.)))
                }))
                .child(widgets::dim(board.brain(board.home()).store.dir().display().to_string())),
        )
}

/// A button of an entry's details that runs what the key `code` does to it, whichever tab shows the details (see
/// [`Board::run_on_entry`]); its tooltip names the shortcut.
fn entry_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: Kind,
    code: KeyCode,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let key = KeyEvent::new(code, KeyModifiers::NONE);
    widgets::button(id, label, kind).tooltip(widgets::tooltip(format!("Shortcut: {}", key_name(key)))).on_click(
        cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.follow = true;
            this.act(cx, |b| b.run_on_entry(key));
        }),
    )
}

/// A button that runs the key `code`; its tooltip names the shortcut.
fn action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: Kind,
    code: KeyCode,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let key = KeyEvent::new(code, KeyModifiers::NONE);
    widgets::button(id, label, kind)
        .tooltip(widgets::tooltip(format!("Shortcut: {}", key_name(key))))
        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| this.press(key, cx)))
}

/// Opens `path` in `$VISUAL` / `$EDITOR` (which may include arguments, e.g. `code -w`) and waits. One set but empty
/// counts as unset, as shells treat it.
fn open_editor(path: &Path) -> Result<()> {
    let editor = editor_of(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok());
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("no editor to run")?;
    let status = Command::new(program).args(parts).arg(path).status().with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

/// The editor to run, from `$VISUAL` and `$EDITOR`: the first set to something, as one set but empty (`VISUAL=`)
/// names none; else the system's own.
fn editor_of(visual: Option<String>, editor: Option<String>) -> String {
    let set = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
    set(visual).or_else(|| set(editor)).unwrap_or_else(|| if cfg!(windows) { "notepad" } else { "vi" }.to_string())
}

#[cfg(test)]
#[path = "../../tests/ui.rs"]
mod tests;
