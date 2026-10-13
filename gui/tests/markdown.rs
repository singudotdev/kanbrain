use super::*;

/// The text of a block's line, and the styles of its spans.
fn line(block: &Block) -> (String, Vec<Style>) {
    let Block::Text { line, .. } = block else { panic!("not text: {block:?}") };
    (line.0.iter().map(|s| s.content.as_str()).collect(), line.0.iter().map(|s| s.style).collect())
}

#[test]
fn inline_styles_and_links() {
    let blocks = blocks("Use **bold**, *it*, ~~old~~ and `code`; see [docs](https://x.dev).");
    let [block] = &blocks[..] else { panic!("{blocks:?}") };
    let (text, styles) = line(block);
    assert_eq!(text, "Use bold, it, old and code; see docs.");
    assert!(styles[1].bold && styles[3].italic && styles[5].strike && styles[7].bg.is_some(), "{styles:?}");
    let Block::Text { links, .. } = block else { unreachable!() };
    assert_eq!(links, &[(32..36, "https://x.dev".to_string())]);
    assert_eq!(&text[32..36], "docs");
}

#[test]
fn line_breaks_are_kept_and_headings_have_their_level() {
    let blocks = blocks("# Plan\n\nfirst line\nsecond line");
    assert!(matches!(blocks[0], Block::Text { heading: 1, .. }));
    assert_eq!(line(&blocks[1]).0, "first line\nsecond line");
}

#[test]
fn lists_number_and_check_their_items() {
    let blocks = blocks("3. three\n4. four\n\n- [x] done\n- [ ] todo\n- plain\n  - nested");
    let [Block::List(ordered), Block::List(tasks)] = &blocks[..] else { panic!("{blocks:?}") };
    let markers = |items: &[(String, Vec<Block>)]| items.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>();
    assert_eq!(markers(ordered), ["3.", "4."]);
    assert_eq!(markers(tasks), ["☑", "☐", "•"]);
    assert!(matches!(tasks[2].1[..], [Block::Text { .. }, Block::List(_)]), "the nested list is in its item");
}

#[test]
fn code_quotes_tables_and_rules() {
    let blocks = blocks("```\nlet a = 1;\n```\n\n> quoted\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |");
    assert!(matches!(&blocks[0], Block::Code(code) if code == "let a = 1;"));
    assert!(matches!(&blocks[1], Block::Quote(inner) if inner.len() == 1));
    assert!(matches!(blocks[2], Block::Rule));
    let Block::Table { rows, head } = &blocks[3] else { panic!("{blocks:?}") };
    assert_eq!((rows.len(), *head, rows[1].len()), (2, 1, 2));
    assert_eq!(line(&rows[1][1][0]).0, "2");
}

#[test]
fn a_preview_line_drops_the_markdown() {
    assert_eq!(
        plain("**Context** Sessions live in `PostgreSQL`, see [docs](https://x.y)."),
        "Context Sessions live in PostgreSQL, see docs."
    );
}
