//! The window: a top bar, the tab's content with the details drawer beside it, dialogs over it and a
//! status bar. Buttons run the same keys as the keyboard ([`Board::on_key`]); other clicks select or
//! open something.
//!
//! - [`topbar`]: tabs, search, the running tasks' clocks and the window's buttons.
//! - [`board`]: the kanban columns and their cards.
//! - [`entries`]: the notes and decisions lists.
//! - [`drawer`]: the selected entry's details, beside the board or list.
//! - [`report`]: tracked time by task or tag.
//! - [`calendar`]: each day's interactions, for a day, week or month.
//! - [`dialogs`]: the new / edit form, the text editor, deletes, links and the command palette.
//! - [`widgets`]: buttons, fields, pills and chips.
//! - [`caret`]: the blinking text cursor.
//! - [`mod@pointer`]: the mouse in text fields and the text editor.
//! - [`theme`]: colors, fonts and styled text.

mod board;
mod calendar;
mod caret;
mod dialogs;
mod drawer;
mod dropdown;
mod entries;
mod list;
mod markdown;
mod menu;
mod pointer;
mod report;
mod theme;
mod topbar;
mod widgets;

use crate::state::{Board, Layout, Mode, Request, Scope, Tab};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::update;
use crate::watch::{self, Watcher};
use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Local};
use gpui::{
    App as GpuiApp, Bounds, ClickEvent, ClipboardItem, Context, Div, ElementId, FocusHandle, KeyDownEvent, Keystroke,
    PathPromptOptions, ScrollHandle, SharedString, Stateful, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use theme::{Line, Span, Style};
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

/// Width of the tag filters' dropdown.
const TAG_DROPDOWN_WIDTH: f32 = 260.;

/// Width of the details drawer.
const DRAWER_WIDTH: f32 = 420.;

/// Opens the window and runs until it's closed or the board quits.
pub fn run(board: Board) {
    gpui_platform::application().run(move |cx: &mut GpuiApp| {
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
                Scope::Repo(src) => format!("{} · kanbrain", board.sources[*src].label.clone().unwrap_or_default()),
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
}

/// Scroll positions of the scrollable areas.
#[derive(Default)]
struct Scrolls {
    /// Each board column, in `COLUMNS` order.
    columns: [ScrollHandle; 4],
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
    /// The form's related field.
    form_links: ScrollHandle,
    /// The form's project field.
    form_projects: ScrollHandle,
    /// The link picker.
    links: ScrollHandle,
    /// "Which project?".
    place: ScrollHandle,
    /// The add-project dialog's projects.
    add_project: ScrollHandle,
}

impl BrainView {
    /// The view over `board`, focused so keys work right away, redrawing every `TICK`.
    fn new(board: Board, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
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
                    this.board.notify(format!("{e:#}"));
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
                let blinked = this.update(cx, |_, cx| {
                    let shown = cx.global::<caret::Blink>().0;
                    cx.set_global(caret::Blink(!shown));
                    cx.notify();
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
        let locks = self.board.lock_brains();
        self.catch_up();
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

    /// Re-reads the files if another program changed them since they were last looked at, and says
    /// whether it did. Every input calls it before acting, not only the timer: the board saves whole
    /// entries from memory, so acting on a copy up to a second old would undo a comment or move an
    /// agent made in that second, and the save would then mark that change as seen.
    fn catch_up(&mut self) -> bool {
        let changed = self.watcher.changed();
        if changed.is_empty() {
            return false;
        }
        if let Err(e) = self.board.reload_changed(&changed) {
            self.board.notify(format!("{e:#}"));
        }
        // Loading may have written files itself, when it fixed several tasks left running, and may
        // have found projects added or gone.
        self.watcher.sync(self.board.watched_dirs());
        true
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
                        if let Err(e) = edited.and_then(|()| this.board.reload()) {
                            this.board.notify(format!("{e:#}"));
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Some(Request::Folder(project)) => Self::pick_project_folder(project, cx),
            Some(Request::Update) => self.install_update(cx),
            None => {}
        }
        cx.notify();
    }

    /// Opens the system's folder dialog, and asks which project the folder picked joins or starts, `project` if
    /// given. On Linux the dialog is the desktop portal's: without one, the status bar says so.
    fn pick_project_folder(project: Option<String>, cx: &mut Context<Self>) {
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
                            this.board.name_project(dir, project);
                        }
                    }
                    Ok(Err(e)) => this.board.notify(format!("{e:#}")),
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
    fn share_selection(&mut self, _: &mut Context<Self>) {}
}

impl Render for BrainView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let follow = std::mem::take(&mut self.follow);
        let board = &self.board;
        let content = match board.tab {
            Tab::Board => match board.layout {
                Layout::Kanban => board::draw(board, &self.scroll.columns, follow, cx),
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

/// The tag dropdown of every tab (`#`): "Tags", with the chips of the `#tags` in the search box in
/// the color they have on the board. Picking a tag adds it to the search box (or takes it out), which
/// holds every filter.
fn tag_filter(board: &Board, scroll: &Scrolls, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let chosen = board.search_tags();
    let button = action("filter", "Tags", Kind::Secondary, KeyCode::Char('#'), cx)
        .debug_selector(|| "tag-filter".into())
        .gap_1p5()
        .children(chosen.iter().map(|tag| widgets::labeled_chip(board.tag_brain(tag), tag, format!("#{tag}"))))
        .child("▾");
    let open = match &board.mode {
        Mode::Tags(picker) => Some(picker),
        _ => None,
    };
    div().flex().flex_col().child(button).children(open.map(|picker| {
        let rows = board
            .tag_candidates(picker)
            .into_iter()
            .map(|candidate| dropdown::Row {
                checked: candidate.as_ref().map_or(chosen.is_empty(), |t| chosen.contains(t)),
                nested: false,
                content: match &candidate {
                    Some(t) => div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(widgets::labeled_chip(board.tag_brain(t), t, format!("#{t}")))
                        .child(widgets::dim_small(uses(board.tag_uses(t))))
                        .into_any_element(),
                    None => div().child("All tags").into_any_element(),
                },
            })
            .collect();
        dropdown::Dropdown {
            name: "tag",
            picker,
            search: pointer::TextBox::TagSearch,
            rows,
            choose: Board::choose_tag,
            width: TAG_DROPDOWN_WIDTH,
        }
        .draw(&scroll.dropdown, follow, cx)
    }))
}

/// Where a tag is used: `2 tasks · 1 note`, so one that isn't on any card can be found.
fn uses(counts: [usize; 3]) -> String {
    let names = [("task", "tasks"), ("note", "notes"), ("decision", "decisions")];
    counts
        .iter()
        .zip(names)
        .filter(|(n, _)| **n > 0)
        .map(|(n, (one, many))| format!("{n} {}", if *n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A key as people write it: `s`, `Esc`, `Ctrl+C`.
fn key_name(key: KeyEvent) -> String {
    let name = match key.code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Delete => "Delete".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::Up => "↑".into(),
        KeyCode::Down => "↓".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
    };
    if key.modifiers.contains(KeyModifiers::CONTROL) { format!("Ctrl+{}", name.to_uppercase()) } else { name }
}

/// The board's key events for a GPUI keystroke. Usually one; typed text that composes several
/// characters gives one per character.
fn key_events(keystroke: &Keystroke) -> Vec<KeyEvent> {
    let modifiers = &keystroke.modifiers;
    let named = match keystroke.key.as_str() {
        "enter" => Some(KeyCode::Enter),
        "escape" => Some(KeyCode::Esc),
        "tab" if modifiers.shift => return Vec::new(),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" => Some(KeyCode::Delete),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        _ => None,
    };
    let ctrl = if modifiers.control { KeyModifiers::CONTROL } else { KeyModifiers::NONE };
    if let Some(code) = named {
        // Shift with a movement key selects text, or moves a card on the board.
        let shift = if modifiers.shift { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
        return vec![KeyEvent::new(code, ctrl | shift)];
    }
    // Shortcuts with the super/command key are the desktop's, not the board's.
    if modifiers.platform {
        return Vec::new();
    }
    if modifiers.control {
        let mut chars = keystroke.key.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => vec![KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)],
            _ => Vec::new(),
        };
    }
    // Typed text, after shift, dead keys and compose. Simulated keystrokes (tests) have none: use the key name.
    let text = match &keystroke.key_char {
        Some(text) => text.clone(),
        None if keystroke.key == "space" => " ".to_string(),
        None if keystroke.key.chars().count() == 1 && modifiers.shift => keystroke.key.to_uppercase(),
        None if keystroke.key.chars().count() == 1 => keystroke.key.clone(),
        None => return Vec::new(),
    };
    text.chars().filter(|c| !c.is_control()).map(|c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).collect()
}

/// Opens `path` in `$VISUAL` / `$EDITOR` (which may include arguments, e.g. `code -w`) and waits.
fn open_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| if cfg!(windows) { "notepad" } else { "vi" }.to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("$EDITOR is empty")?;
    let status = Command::new(program).args(parts).arg(path).status().with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

/// Cuts `s` to `width` characters, ending with `…` when shortened.
fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// "Thu 24 Sep 2026 10:15" in local time.
fn when(dt: DateTime<Local>) -> String {
    dt.format("%a %d %b %Y %H:%M").to_string()
}

/// Text being typed into, the characters from `start` to `end` of `selection` highlighted.
fn edited(text: &str, selection: Option<(usize, usize)>) -> Line {
    let Some((start, end)) = selection else { return Line::raw(text) };
    let (a, b) = (byte_at(text, start), byte_at(text, end));
    Line::from(vec![
        Span::raw(&text[..a]),
        Span::styled(&text[a..b], Style::new().bg(theme::SELECTION)),
        Span::raw(&text[b..]),
    ])
}

/// Byte offset of character `at` in `text`; past the last character means the end.
fn byte_at(text: &str, at: usize) -> usize {
    text.char_indices().nth(at).map_or(text.len(), |(i, _)| i)
}

#[cfg(test)]
#[path = "../../tests/ui.rs"]
mod tests;
