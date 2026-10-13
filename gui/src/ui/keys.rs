//! Keys between GPUI and the board: a GPUI keystroke as the board's key events, and a key as people write it.

use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use gpui::Keystroke;

/// A key as people write it: `s`, `Esc`, `Ctrl+C`.
pub(super) fn key_name(key: KeyEvent) -> String {
    let name = match key.code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => "Shift+Tab".into(),
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
pub(super) fn key_events(keystroke: &Keystroke) -> Vec<KeyEvent> {
    let modifiers = &keystroke.modifiers;
    let named = match keystroke.key.as_str() {
        "enter" => Some(KeyCode::Enter),
        "escape" => Some(KeyCode::Esc),
        // Its own key, which shift is part of, so `shift+tab` matches it alone.
        "tab" if modifiers.shift => return vec![KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)],
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
