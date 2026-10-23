//! Markdown drawn as formatted text: descriptions, comments, and a project's description and role.
//! The files keep the text as written (the editor shows it raw); only how it's drawn changes.
//!
//! [`blocks`] parses the text into [`Block`]s, plain data that tests can check; [`render`] draws them, as one text
//! the mouse selects ([`Selectable`]): each paragraph, heading, code line and cell a piece of it, a list's markers
//! and the rules left out. Covered: paragraphs, headings, bold, italic, strikethrough, inline code, links (a click
//! opens them, and a bare URL is one, as GFM has it), lists and task lists, quotes, code blocks, tables and rules. A single line break is kept as one,
//! as comments and notes are written line by line, not wrapped like prose.
//!
//! References to entries, `#12` and `api#3` as kb and agents write them (and `api #3` or `shop/api #3`, as the app
//! and kb's reports name them), are links too, which open the entry. Only in prose: markdown comes first, so one in
//! code or inside a link stays what that draws, and only to an entry that's there. After `PR`, `issue`, `MR` or a
//! pull or merge `request` it's left alone, since there it's a pull request's or an issue's number.

use super::BrainView;
use super::shown::Selectable;
use super::theme::{ACCENT, BORDER, DIM, EDITOR_FONT, INPUT, Line, SURFACE, Span, Style, TEXT};
use crate::state::{Board, Key};
use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, InteractiveText, SharedString, WeakEntity, div, prelude::*, px,
    rgb,
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, TextMergeStream};
use std::ops::Range;

/// Where a link points.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub enum LinkTarget {
    /// A page, which a click opens in the browser.
    Url(String),
    /// An entry a reference names, which a click opens in the drawer.
    Entry(Key),
}

/// The entry a reference names (`None` for `#3`, the repository for `api#3`, then the id), if it's there.
type Resolve<'r> = &'r dyn Fn(Option<&str>, u32) -> Option<Key>;

/// A block of markdown, ready to draw.
#[derive(Debug)]
pub enum Block {
    /// A paragraph or a heading.
    Text {
        /// Its text, in styled spans.
        line: Line,
        /// Byte ranges of `line`'s text that are links, each with where it points.
        links: Vec<(Range<usize>, LinkTarget)>,
        /// 1 to 6 for a heading, 0 for a paragraph.
        heading: u8,
    },
    /// A code block, as written.
    Code(String),
    /// A quote.
    Quote(Vec<Block>),
    /// A list's items: the marker (`•`, `3.`, `☐`, `☑`) and what the item holds.
    List(Vec<(String, Vec<Block>)>),
    /// A horizontal rule.
    Rule,
    /// A table.
    Table {
        /// Rows of cells, each cell its blocks.
        rows: Vec<Vec<Vec<Block>>>,
        /// How many of the first rows are the header.
        head: usize,
    },
}

/// A container being filled while parsing, with the blocks it holds so far.
enum Frame {
    /// The text itself, never closed.
    Root,
    /// A quote.
    Quote,
    /// The number of the next item, for an ordered list.
    List(Option<u64>, Vec<(String, Vec<Block>)>),
    /// Whether it's a task list's item, checked or not.
    Item(Option<bool>),
    /// A table: how many header rows it has so far, and its rows.
    Table(usize, Vec<Vec<Vec<Block>>>),
    /// A row: its cells so far.
    Row(Vec<Vec<Block>>),
    /// A cell.
    Cell,
}

/// What [`blocks`] keeps while it reads the events.
struct Builder<'r> {
    /// Finds the entries references name.
    resolve: Resolve<'r>,
    /// The containers open, outermost first, each with its blocks so far.
    open: Vec<(Frame, Vec<Block>)>,
    /// The paragraph or heading being read.
    spans: Vec<Span>,
    /// Its text's length so far, where the next span starts.
    len: usize,
    /// The links in it so far.
    links: Vec<(Range<usize>, LinkTarget)>,
    /// Where the open link starts in the text, and where it points.
    link: Option<(usize, String)>,
    /// Styles of the open emphasis, links and headings; the last applies.
    styles: Vec<Style>,
    /// The open heading's level, 0 outside one.
    heading: u8,
    /// The open code block's text.
    code: Option<String>,
}

/// The blocks of `text`, its references to entries those `resolve` finds.
pub fn blocks(resolve: impl Fn(Option<&str>, u32) -> Option<Key>, text: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder {
        resolve: &resolve,
        open: vec![(Frame::Root, Vec::new())],
        spans: Vec::new(),
        len: 0,
        links: Vec::new(),
        link: None,
        styles: Vec::new(),
        heading: 0,
        code: None,
    };
    // Merged, so a reference reads as one text: the parser splits `#3_` at the `_` an emphasis might start with.
    for event in TextMergeStream::new(Parser::new_ext(text, options)) {
        b.event(event);
    }
    b.flush();
    b.open.into_iter().next().map(|(_, blocks)| blocks).unwrap_or_default()
}

impl Builder<'_> {
    /// The style text gets now: the innermost open one's.
    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    /// Opens a style: the current one, changed.
    fn push_style(&mut self, change: fn(Style) -> Style) {
        self.styles.push(change(self.style()));
    }

    /// Adds text to the paragraph or heading being read.
    fn text(&mut self, text: &str, style: Style) {
        self.len += text.len();
        self.spans.push(Span::styled(text, style));
    }

    /// Adds prose to the paragraph or heading being read, its URLs and references to entries as links. Inside a
    /// link, it's that link's text.
    fn prose(&mut self, text: &str) {
        let style = self.style();
        let mut from = 0;
        if self.link.is_none() {
            let mut found: Vec<_> = urls(text).map(|at| (at.clone(), LinkTarget::Url(text[at].to_string()))).collect();
            for (mut at, repo, id) in references(text) {
                let before = &text[..at.start];
                // `api #3` or `shop/api #3`: the entry of the repository before it, if that's one of the project.
                let named = repo.is_none().then(|| repo_before(before)).flatten();
                let key = match named.map(|(start, name)| (start, name, (self.resolve)(Some(name), id))) {
                    Some((start, _, Some(key))) => {
                        at.start = start;
                        Some(key)
                    }
                    // Another project's repository's.
                    Some((_, name, None)) if name.contains('/') => None,
                    _ => (!after_a_number_word(&self.spans, before)).then(|| (self.resolve)(repo, id)).flatten(),
                };
                found.extend(key.map(|key| (at, LinkTarget::Entry(key))));
            }
            found.sort_by_key(|(at, _)| at.start);
            for (at, target) in found {
                if at.start < from {
                    continue;
                }
                if at.start > from {
                    self.text(&text[from..at.start], style);
                }
                let start = self.len;
                self.text(&text[at.clone()], style.fg(ACCENT));
                self.links.push((start..self.len, target));
                from = at.end;
            }
        }
        if from < text.len() {
            self.text(&text[from..], style);
        }
    }

    /// Adds a finished block to the innermost open container.
    fn push(&mut self, block: Block) {
        if let Some((_, blocks)) = self.open.last_mut() {
            blocks.push(block);
        }
    }

    /// Ends the paragraph or heading being read, if any, as a block of the open container.
    fn flush(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        let line = Line::from(std::mem::take(&mut self.spans));
        let links = std::mem::take(&mut self.links);
        self.len = 0;
        self.push(Block::Text { line, links, heading: self.heading });
    }

    /// Closes the innermost container; the root stays open.
    fn close(&mut self) -> Option<(Frame, Vec<Block>)> {
        self.flush();
        if self.open.len() > 1 { self.open.pop() } else { None }
    }

    /// Reads one parser event.
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match &mut self.code {
                Some(code) => code.push_str(&text),
                None => self.prose(&text),
            },
            Event::Code(code) => self.text(&code, self.style().bg(INPUT)),
            Event::Html(html) | Event::InlineHtml(html) => self.text(&html, self.style()),
            Event::SoftBreak | Event::HardBreak => self.text("\n", Style::new()),
            Event::Rule => {
                self.flush();
                self.push(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                if let Some((Frame::Item(task), _)) = self.open.last_mut() {
                    *task = Some(checked);
                }
            }
            // Footnotes and math aren't enabled.
            Event::FootnoteReference(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
        }
    }

    /// Opens what `tag` starts: a container, a style, a link, a heading or a code block.
    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = level as u8;
                self.push_style(Style::bold);
            }
            Tag::BlockQuote(_) => self.open_frame(Frame::Quote),
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::List(start) => self.open_frame(Frame::List(start, Vec::new())),
            Tag::Item => self.open_frame(Frame::Item(None)),
            Tag::Table(_) => self.open_frame(Frame::Table(0, Vec::new())),
            Tag::TableHead | Tag::TableRow => self.open_frame(Frame::Row(Vec::new())),
            Tag::TableCell => self.open_frame(Frame::Cell),
            Tag::Emphasis => self.push_style(Style::italic),
            Tag::Strong => self.push_style(Style::bold),
            Tag::Strikethrough => self.push_style(Style::strike),
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.link = Some((self.len, dest_url.to_string()));
                self.push_style(|s| s.fg(ACCENT));
            }
            _ => {}
        }
    }

    /// Opens a container, ending the paragraph being read first.
    fn open_frame(&mut self, frame: Frame) {
        self.flush();
        self.open.push((frame, Vec::new()));
    }

    /// Closes what `tag` ends, adding the finished block to its container.
    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.flush();
                self.heading = 0;
                self.styles.pop();
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.push(Block::Code(code.trim_end_matches('\n').to_string()));
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some((_, blocks)) = self.close() {
                    self.push(Block::Quote(blocks));
                }
            }
            TagEnd::List(_) => {
                if let Some((Frame::List(_, items), _)) = self.close() {
                    self.push(Block::List(items));
                }
            }
            TagEnd::Item => {
                let Some((Frame::Item(task), blocks)) = self.close() else { return };
                if let Some((Frame::List(next, items), _)) = self.open.last_mut() {
                    let marker = match (task, next) {
                        (Some(true), _) => "☑".to_string(),
                        (Some(false), _) => "☐".to_string(),
                        (None, Some(n)) => {
                            *n += 1;
                            format!("{}.", *n - 1)
                        }
                        (None, None) => "•".to_string(),
                    };
                    items.push((marker, blocks));
                }
            }
            TagEnd::TableCell => {
                let Some((Frame::Cell, blocks)) = self.close() else { return };
                if let Some((Frame::Row(cells), _)) = self.open.last_mut() {
                    cells.push(blocks);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                let Some((Frame::Row(cells), _)) = self.close() else { return };
                if let Some((Frame::Table(head, rows), _)) = self.open.last_mut() {
                    if tag == TagEnd::TableHead {
                        *head += 1;
                    }
                    rows.push(cells);
                }
            }
            TagEnd::Table => {
                if let Some((Frame::Table(head, rows), _)) = self.close() {
                    self.push(Block::Table { rows, head });
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link | TagEnd::Image => {
                self.styles.pop();
                if let Some((start, url)) = self.link.take() {
                    self.links.push((start..self.len, LinkTarget::Url(url)));
                }
            }
            _ => {}
        }
    }
}

/// Where `text` says `#12` or `api#3`, references to entries: the byte range, the repository if it names one, and
/// the id. A repository's name is letters, digits, `-`, `_` and `.`, all a link word's can hold that prose doesn't
/// put around it, as `(#3)` and `#3,` are, and starts with neither `-` nor `.`; one after `/` or `@` is a path's or an
/// address's (`owner/api#3`), unless the `/` follows a reference, listing them (`#1/#2/api#3`), after `#` or `&` an
/// anchor's or an entity's, and none is in a URL.
fn references(text: &str) -> impl Iterator<Item = (Range<usize>, Option<&str>, u32)> {
    let name = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | '.');
    // Where the last reference ends.
    let mut last = None;
    text.match_indices('#').filter_map(move |(hash, _)| {
        let digits = text[hash + 1..].find(|c: char| !c.is_ascii_digit()).map_or(text.len(), |n| hash + 1 + n);
        let ends = text[digits..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let start =
            text[..hash].rfind(|c| !name(c)).map_or(0, |at| at + text[at..].chars().next().map_or(1, char::len_utf8));
        let before = text[..start].chars().next_back();
        let repo = &text[start..hash];
        // A URL markdown didn't make a link (`https://x.dev/?q=api#4`) is one word with `://`.
        // `://` holds no `#`, so it's wholly on one side of it.
        let in_url = [text[..hash].rsplit(char::is_whitespace).next(), text[hash..].split(char::is_whitespace).next()]
            .into_iter()
            .flatten()
            .any(|w| w.contains("://"));
        let listed = before == Some('/') && last == Some(start - 1);
        let fits =
            !in_url && !repo.starts_with(['-', '.']) && (listed || !matches!(before, Some('/' | '@' | '#' | '&')));
        let id = text[hash + 1..digits].parse().ok().filter(|_| ends && fits)?;
        last = Some(digits);
        Some((start..digits, Some(repo).filter(|r| !r.is_empty()), id))
    })
}

/// Where `text` has a URL markdown left as text, which GFM's extended autolinks make links
/// ([GFM: autolinks (extension)](https://github.github.com/gfm/#autolinks-extension-)): `http://` or `https://` at a
/// word's start (or after `*`, `_`, `~` or `(`), a domain of at least two parts, `_` in neither of its last two, up
/// to a space or `<`, less the punctuation, unmatched `)` and entity (`&hl;`) that end it.
fn urls(text: &str) -> impl Iterator<Item = Range<usize>> {
    text.match_indices("http").filter_map(move |(start, _)| {
        let rest = &text[start..];
        let host = rest.strip_prefix("https://").or_else(|| rest.strip_prefix("http://"))?;
        let starts = text[..start].chars().next_back().is_none_or(|c| c.is_whitespace() || "*_~(".contains(c));
        let domain = &host[..host.find(|c: char| !c.is_alphanumeric() && !"-_.".contains(c)).unwrap_or(host.len())];
        let parts: Vec<_> = domain.trim_end_matches('.').split('.').collect();
        let valid =
            parts.len() > 1 && parts.iter().all(|p| !p.is_empty()) && !parts[parts.len() - 2..].concat().contains('_');
        let mut url = &rest[..rest.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(rest.len())];
        loop {
            let entity = url
                .strip_suffix(';')
                .and_then(|u| u.rsplit_once('&'))
                .filter(|(_, name)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()));
            let unmatched = url.matches(')').count() > url.matches('(').count();
            let shorter = url.strip_suffix(['?', '!', '.', ',', ':', '*', '_', '~']);
            match shorter.or_else(|| url.strip_suffix(')').filter(|_| unmatched)).or(entity.map(|(u, _)| u)) {
                Some(shorter) => url = shorter,
                None => break,
            }
        }
        (starts && valid).then(|| start..start + url.len())
    })
}

/// The repository a `#3` right after `before` belongs to when `before` ends with its name and a space, as the app and
/// kb's reports name entries (`api #3`, `shop/api #3`): where that name starts, and the name.
fn repo_before(before: &str) -> Option<(usize, &str)> {
    let name = before.strip_suffix(' ')?;
    let start = name
        .rfind(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')))
        .map_or(0, |at| at + name[at..].chars().next().map_or(1, char::len_utf8));
    let name = &name[start..];
    (!name.is_empty() && !name.starts_with(['/', '-', '.']) && !name.ends_with('/')).then_some((start, name))
}

/// Whether what's said before a reference, the paragraph's `spans` then `before`, ends with a word a pull request's
/// or an issue's number follows: `PR`, `issue`, `MR` or `request` (a pull or merge request), plurals too, after which
/// a space, a line break or an opening bracket may come (`**PR** (#3)`).
fn after_a_number_word(spans: &[Span], before: &str) -> bool {
    let said = before.chars().rev().chain(spans.iter().rev().flat_map(|s| s.content.chars().rev()));
    // Read backwards, and only as far as one letter past the longest of them.
    let word: String = said
        .skip_while(|&c| c.is_whitespace() || matches!(c, '(' | '['))
        .take_while(|c| c.is_alphanumeric())
        .take(9)
        .flat_map(char::to_lowercase)
        .collect();
    matches!(word.as_str(), "rp" | "srp" | "eussi" | "seussi" | "rm" | "srm" | "tseuqer" | "stseuqer")
}

/// `text` drawn as markdown, its pieces those of `shown`, which the caller finishes. `id` names its links' elements,
/// so it must differ between the texts one view shows (`description`, `comment 3`). `resolve` finds the entries its
/// references name (see [`references`]).
pub fn render(
    id: impl Into<SharedString>,
    text: &str,
    resolve: impl Fn(Option<&str>, u32) -> Option<Key>,
    shown: &mut Selectable,
    cx: &mut Context<BrainView>,
) -> Div {
    let mut links = Links { id: id.into(), count: 0, shown, view: cx.entity().downgrade() };
    draw(blocks(resolve, text), &mut links).text_size(px(13.)).text_color(rgb(TEXT))
}

/// The entries the references in a text written in the brain at `src` name: `#3` there, `api#3` in its project's
/// repository `api`.
pub fn written_in(board: &Board, src: usize) -> impl Fn(Option<&str>, u32) -> Option<Key> {
    move |repo, id| board.resolve(src, repo, id)
}

/// The entries the references in a text of the project of the brain at `src` name, its description: only `api#3`,
/// since a bare `#3` is no more one repository's than another's.
pub fn written_for_project(board: &Board, src: usize) -> impl Fn(Option<&str>, u32) -> Option<Key> {
    move |repo, id| board.resolve(src, Some(repo?), id)
}

/// Numbers the texts with links, each an element of its own, and takes every piece drawn into the text selected.
struct Links<'s, 'a> {
    /// The text's id, which theirs start with.
    id: SharedString,
    /// How many have been numbered.
    count: usize,
    /// The text the mouse selects, of every piece drawn.
    shown: &'s mut Selectable<'a>,
    /// The view, which opens the entries references name.
    view: WeakEntity<BrainView>,
}

/// The blocks, one under the other.
fn draw(blocks: Vec<Block>, links: &mut Links) -> Div {
    div().flex().flex_col().gap_1p5().children(blocks.into_iter().map(|b| draw_block(b, links)))
}

/// One block, with its links clickable.
fn draw_block(block: Block, links: &mut Links) -> AnyElement {
    match block {
        Block::Text { line, links: urls, heading } => {
            let text = links.shown.line(line);
            let text =
                if urls.is_empty() {
                    text.into_any_element()
                } else {
                    links.count += 1;
                    let (ranges, targets): (Vec<_>, Vec<_>) = urls.into_iter().unzip();
                    let view = links.view.clone();
                    // `links ID N` for the window tests, which click a text's links; nothing outside them.
                    let (named, count) = (links.id.clone(), links.count);
                    let text = InteractiveText::new(ElementId::named_usize(links.id.clone(), links.count), text)
                        .on_click(ranges, move |i, _, cx| match targets.get(i) {
                            Some(LinkTarget::Url(url)) => cx.open_url(url),
                            Some(&LinkTarget::Entry(key)) => {
                                // Gone only with the window, which the click came from.
                                view.update(cx, |view, cx| view.act(cx, |b| b.follow(key))).ok();
                            }
                            None => {}
                        });
                    div().debug_selector(move || format!("links {named} {count}")).child(text).into_any_element()
                };
            let size = match heading {
                1 => 18.,
                2 => 16.,
                3 => 14.,
                _ => 13.,
            };
            div().text_size(px(size)).when(heading > 0, |d| d.font_weight(FontWeight::SEMIBOLD)).child(text)
        }
        Block::Code(code) => div()
            .p_2()
            .rounded_md()
            .bg(rgb(INPUT))
            .font_family(EDITOR_FONT)
            .text_size(px(12.))
            .children(code.lines().map(|l| links.shown.line(Line::raw(l)))),
        Block::Quote(blocks) => draw(blocks, links).pl_3().border_l_2().border_color(rgb(BORDER)).text_color(rgb(DIM)),
        Block::List(items) => div().flex().flex_col().gap_0p5().children(items.into_iter().map(|(marker, blocks)| {
            div()
                .flex()
                .gap_2()
                .child(div().flex_none().text_color(rgb(DIM)).child(marker))
                .child(draw(blocks, links).flex_1().min_w_0())
        })),
        Block::Rule => div().h(px(1.)).my_1().bg(rgb(BORDER)),
        Block::Table { rows, head } => {
            div().flex().flex_col().rounded_md().border_1().border_color(rgb(BORDER)).children(
                rows.into_iter().enumerate().map(|(i, cells)| {
                    div()
                        .flex()
                        .when(i > 0, |d| d.border_t_1().border_color(rgb(BORDER)))
                        .when(i < head, |d| d.bg(rgb(SURFACE)).font_weight(FontWeight::SEMIBOLD))
                        .children(cells.into_iter().map(|cell| draw(cell, links).flex_1().min_w_0().px_2().py_1()))
                }),
            )
        }
    }
    .into_any_element()
}

/// `line`'s text without its markdown, for a one-line preview: `**Context** Sessions…` as `Context Sessions…`.
pub fn plain(line: &str) -> String {
    Parser::new(line).fold(String::new(), |mut plain, event| {
        if let Event::Text(text) | Event::Code(text) = event {
            plain.push_str(&text);
        }
        plain
    })
}

#[cfg(test)]
#[path = "../../tests/markdown.rs"]
mod tests;
