//! Text as the window shows it: cut to a width, a date, and a field's text with its selection highlighted.

use super::theme::{self, Line, Span, Style};
use chrono::{DateTime, Local};

/// Cuts `s` to `width` characters, ending with `…` when shortened.
pub(super) fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// "Thu 24 Sep 2026 10:15" in local time.
pub(super) fn when(dt: DateTime<Local>) -> String {
    dt.format("%a %d %b %Y %H:%M").to_string()
}

/// Text being typed into, the characters from `start` to `end` of `selection` highlighted.
pub(super) fn edited(text: &str, selection: Option<(usize, usize)>) -> Line {
    let Some((start, end)) = selection else { return Line::raw(text) };
    let (a, b) = (byte_at(text, start), byte_at(text, end));
    Line::from(vec![
        Span::raw(&text[..a]),
        Span::styled(&text[a..b], Style::new().bg(theme::SELECTION)),
        Span::raw(&text[b..]),
    ])
}

/// Byte offset of character `at` in `text`; past the last character means the end.
pub(super) fn byte_at(text: &str, at: usize) -> usize {
    text.char_indices().nth(at).map_or(text.len(), |(i, _)| i)
}
