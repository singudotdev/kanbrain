//! The mouse in text fields and the text editor, as in any desktop app: a press places the cursor,
//! shift+click and dragging select, a double-click selects a word and a triple-click the line (dragging on
//! from there selects whole words or lines). On Linux a middle-click pastes the primary selection.
//! Each field remembers where its text was drawn ([`Hit`]) to turn the mouse position into a character.

use super::BrainView;
use crate::state::Field;
use crate::text::editor::Pointer;
use gpui::{
    Context, DispatchPhase, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Stateful,
    StyledText, TextLayout, Window, canvas, point, prelude::*, px,
};
use std::rc::Rc;

/// A place text is typed into, to know which one a drag started in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextBox {
    /// The search box.
    Search,
    /// The form's title field.
    Title,
    /// The form's tags field.
    Tags,
    /// The form's related field: the search box of its dropdown.
    Links,
    /// The form's project field: the search box of its dropdown.
    Project,
    /// A column's quick-add field.
    QuickAdd,
    /// The link picker's search box.
    LinkSearch,
    /// The project dropdown's search box.
    ScopeSearch,
    /// A tag dropdown's search box.
    TagSearch,
    /// The command palette's search box.
    PaletteSearch,
    /// The search box of "which project?".
    WhereSearch,
    /// The new name of a project or repository, or the project a repository moves to.
    ManageName,
    /// The name of a project being added.
    ProjectName,
    /// The description or comment editor.
    Editor,
}

impl TextBox {
    /// The element's id.
    pub fn id(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Title => "title",
            Self::Tags => "tags",
            Self::Links => "links",
            Self::Project => "project",
            Self::QuickAdd => "quick-add",
            Self::LinkSearch => "link-search",
            Self::ScopeSearch => "scope-search",
            Self::TagSearch => "tag-search",
            Self::WhereSearch => "where-search",
            Self::PaletteSearch => "palette-search",
            Self::ProjectName => "project-name",
            Self::ManageName => "manage-name",
            Self::Editor => "editor",
        }
    }
}

/// One line of text as drawn, to find the character under the mouse.
pub struct Hit {
    /// Where the line was laid out; filled in when the frame is drawn, before any mouse event can use it.
    layout: TextLayout,
    /// Characters in the line being edited: the drawn text may be a placeholder, or a space for an empty line.
    len: usize,
}

impl Hit {
    /// The line `text` draws, of a `len`-character line.
    pub fn new(text: &StyledText, len: usize) -> Self {
        Self { layout: text.layout().clone(), len }
    }

    /// The character boundary closest to `position`. Above or below the line counts as its first or last row.
    fn column(&self, position: Point<Pixels>) -> usize {
        let bounds = self.layout.bounds();
        let line_height = self.layout.line_height();
        // Each drawn line is one paragraph (no newlines), possibly wrapped into several rows.
        let Some(line) = self.layout.line_layouts().into_iter().next() else { return 0 };
        // GPUI answers "the start" for a point below the last row, so keep the point inside.
        let bottom = (line.size(line_height).height - px(1.)).max(px(0.));
        let y = (position.y - bounds.top()).clamp(px(0.), bottom);
        let byte = line.closest_index_for_position(point(position.x - bounds.left(), y), line_height);
        let byte = byte.unwrap_or_else(|end| end);
        let drawn = self.layout.text();
        drawn.char_indices().take_while(|&(i, _)| i < byte).count().min(self.len)
    }
}

/// The (line, character) under `position` in `lines`, drawn top to bottom; below the last line is the last.
fn at(lines: &[Hit], position: Point<Pixels>) -> (usize, usize) {
    let row = lines.iter().position(|line| position.y < line.layout.bounds().bottom()).unwrap_or(lines.len() - 1);
    (row, lines[row].column(position))
}

/// Makes `element`, which shows `lines` of `which`, respond to the mouse like a text field.
pub fn pointable(
    element: Stateful<Div>,
    which: TextBox,
    lines: Vec<Hit>,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    if lines.is_empty() {
        return element;
    }
    let lines: Rc<[Hit]> = lines.into();
    let pressed = Rc::clone(&lines);
    let middle_clicked = Rc::clone(&lines);
    let press = cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
        focus(this, which);
        let how = match event.click_count {
            2 => Pointer::Word,
            n if n >= 3 => Pointer::Line,
            _ if event.modifiers.shift => Pointer::Extend,
            _ => Pointer::Place,
        };
        let (row, col) = at(&pressed, event.position);
        this.board.point(row, col, how);
        this.drag = Some(which);
        cx.stop_propagation();
        this.after_input(cx);
    });
    // Window-wide listeners, so the selection follows the mouse outside the field too.
    // They're registered again every frame, with that frame's layout.
    let view = cx.entity().downgrade();
    let drag = canvas(
        |_, _, _| {},
        move |_, (), window, _| {
            let moved = Rc::clone(&lines);
            let view_ = view.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                _ = view_.update(cx, |this, cx| {
                    if this.drag != Some(which) {
                        return;
                    }
                    // The button came up where the window couldn't see it.
                    if !event.dragging() {
                        this.drag = None;
                        return;
                    }
                    let (row, col) = at(&moved, event.position);
                    this.board.point(row, col, Pointer::Extend);
                    // Keeps the editor's cursor line in view while dragging past its edge.
                    this.follow = true;
                    this.share_selection(cx);
                    this.restart_blink(cx);
                    cx.notify();
                });
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    _ = view.update(cx, |this, _| {
                        if this.drag == Some(which) {
                            this.drag = None;
                        }
                    });
                }
            });
        },
    )
    .absolute()
    .size_0();
    let element = element.cursor_text().on_mouse_down(MouseButton::Left, press).child(drag);
    paste_on_middle_click(element, which, middle_clicked, cx)
}

/// Makes it the field being typed into, if it isn't always.
fn focus(this: &mut BrainView, which: TextBox) {
    match which {
        TextBox::Search => this.board.focus_search(),
        TextBox::Title => this.board.focus_field(Field::Title),
        TextBox::Tags => this.board.focus_field(Field::Tags),
        TextBox::Links => this.board.focus_field(Field::Links),
        TextBox::Project => this.board.focus_field(Field::Project),
        TextBox::QuickAdd
        | TextBox::LinkSearch
        | TextBox::ScopeSearch
        | TextBox::TagSearch
        | TextBox::WhereSearch
        | TextBox::PaletteSearch
        | TextBox::ProjectName
        | TextBox::ManageName
        | TextBox::Editor => {}
    }
}

/// A middle-click pastes the primary selection (the text last selected anywhere) where it was clicked.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn paste_on_middle_click(
    element: Stateful<Div>,
    which: TextBox,
    lines: Rc<[Hit]>,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    element.on_mouse_down(
        MouseButton::Middle,
        cx.listener(move |this, event: &MouseDownEvent, _: &mut Window, cx| {
            focus(this, which);
            let (row, col) = at(&lines, event.position);
            this.board.point(row, col, Pointer::Place);
            if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
                this.board.paste(&text);
            }
            cx.stop_propagation();
            this.after_input(cx);
        }),
    )
}

/// Only Linux desktops have a primary selection.
#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
fn paste_on_middle_click(
    element: Stateful<Div>,
    _: TextBox,
    _: Rc<[Hit]>,
    _: &mut Context<BrainView>,
) -> Stateful<Div> {
    element
}
