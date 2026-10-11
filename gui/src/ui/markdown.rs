//! Markdown drawn as formatted text: descriptions, comments, and a project's description and role.
//! The files keep the text as written (the editor shows it raw); only how it's drawn changes.
//!
//! [`blocks`] parses the text into [`Block`]s, plain data that tests can check; [`render`] draws them.
//! Covered: paragraphs, headings, bold, italic, strikethrough, inline code, links (a click opens them),
//! lists and task lists, quotes, code blocks, tables and rules. A single line break is kept as one,
//! as comments and notes are written line by line, not wrapped like prose.

use super::theme::{ACCENT, BORDER, DIM, EDITOR_FONT, INPUT, Line, SURFACE, Span, Style, TEXT};
use gpui::{AnyElement, Div, ElementId, FontWeight, InteractiveText, SharedString, div, prelude::*, px, rgb};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// A block of markdown, ready to draw.
#[derive(Debug)]
pub enum Block {
    /// A paragraph or a heading.
    Text {
        /// Its text, in styled spans.
        line: Line,
        /// Byte ranges of `line`'s text that are links, each with where it points.
        links: Vec<(Range<usize>, String)>,
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
struct Builder {
    /// The containers open, outermost first, each with its blocks so far.
    open: Vec<(Frame, Vec<Block>)>,
    /// The paragraph or heading being read.
    spans: Vec<Span>,
    /// Its text's length so far, where the next span starts.
    len: usize,
    /// The links in it so far.
    links: Vec<(Range<usize>, String)>,
    /// Where the open link starts in the text, and where it points.
    link: Option<(usize, String)>,
    /// Styles of the open emphasis, links and headings; the last applies.
    styles: Vec<Style>,
    /// The open heading's level, 0 outside one.
    heading: u8,
    /// The open code block's text.
    code: Option<String>,
}

/// The blocks of `text`.
pub fn blocks(text: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder {
        open: vec![(Frame::Root, Vec::new())],
        spans: Vec::new(),
        len: 0,
        links: Vec::new(),
        link: None,
        styles: Vec::new(),
        heading: 0,
        code: None,
    };
    for event in Parser::new_ext(text, options) {
        b.event(event);
    }
    b.flush();
    b.open.into_iter().next().map(|(_, blocks)| blocks).unwrap_or_default()
}

impl Builder {
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
                None => self.text(&text, self.style()),
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
                    self.links.push((start..self.len, url));
                }
            }
            _ => {}
        }
    }
}

/// `text` drawn as markdown. `id` names its links' elements, so it must differ between the texts
/// one view shows (`description`, `comment 3`).
pub fn render(id: impl Into<SharedString>, text: &str) -> Div {
    let mut links = Links { id: id.into(), count: 0 };
    draw(blocks(text), &mut links).text_size(px(13.)).text_color(rgb(TEXT))
}

/// Numbers the texts with links, each an element of its own.
struct Links {
    /// The text's id, which theirs start with.
    id: SharedString,
    /// How many have been numbered.
    count: usize,
}

/// The blocks, one under the other.
fn draw(blocks: Vec<Block>, links: &mut Links) -> Div {
    div().flex().flex_col().gap_1p5().children(blocks.into_iter().map(|b| draw_block(b, links)))
}

/// One block, with its links clickable.
fn draw_block(block: Block, links: &mut Links) -> AnyElement {
    match block {
        Block::Text { line, links: urls, heading } => {
            let text = line.render();
            let text = if urls.is_empty() {
                text.into_any_element()
            } else {
                links.count += 1;
                let (ranges, urls): (Vec<_>, Vec<_>) = urls.into_iter().unzip();
                InteractiveText::new(ElementId::named_usize(links.id.clone(), links.count), text)
                    .on_click(ranges, move |i, _, cx| {
                        if let Some(url) = urls.get(i) {
                            cx.open_url(url);
                        }
                    })
                    .into_any_element()
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
            .children(code.lines().map(|l| Line::raw(l).render())),
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

#[cfg(test)]
#[path = "../../tests/markdown.rs"]
mod tests;
