use super::*;

/// The text of a block's line, and the styles of its spans.
fn line(block: &Block) -> (String, Vec<Style>) {
    let Block::Text { line, .. } = block else { panic!("not text: {block:?}") };
    (line.0.iter().map(|s| s.content.as_str()).collect(), line.0.iter().map(|s| s.style).collect())
}

#[test]
fn inline_styles_and_links() {
    let blocks = blocks(none, "Use **bold**, *it*, ~~old~~ and `code`; see [docs](https://x.dev).");
    let [block] = &blocks[..] else { panic!("{blocks:?}") };
    let (text, styles) = line(block);
    assert_eq!(text, "Use bold, it, old and code; see docs.");
    assert!(styles[1].bold && styles[3].italic && styles[5].strike && styles[7].bg.is_some(), "{styles:?}");
    let Block::Text { links, .. } = block else { unreachable!() };
    assert_eq!(links, &[(32..36, LinkTarget::Url("https://x.dev".to_string()))]);
    assert_eq!(&text[32..36], "docs");
}

#[test]
fn line_breaks_are_kept_and_headings_have_their_level() {
    let blocks = blocks(none, "# Plan\n\nfirst line\nsecond line");
    assert!(matches!(blocks[0], Block::Text { heading: 1, .. }));
    assert_eq!(line(&blocks[1]).0, "first line\nsecond line");
}

#[test]
fn lists_number_and_check_their_items() {
    let blocks = blocks(none, "3. three\n4. four\n\n- [x] done\n- [ ] todo\n- plain\n  - nested");
    let [Block::List(ordered), Block::List(tasks)] = &blocks[..] else { panic!("{blocks:?}") };
    let markers = |items: &[(String, Vec<Block>)]| items.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>();
    assert_eq!(markers(ordered), ["3.", "4."]);
    assert_eq!(markers(tasks), ["☑", "☐", "•"]);
    assert!(matches!(tasks[2].1[..], [Block::Text { .. }, Block::List(_)]), "the nested list is in its item");
}

#[test]
fn code_quotes_tables_and_rules() {
    let blocks = blocks(none, "```\nlet a = 1;\n```\n\n> quoted\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |");
    assert!(matches!(&blocks[0], Block::Code(code) if code == "let a = 1;"));
    assert!(matches!(&blocks[1], Block::Quote(inner) if inner.len() == 1));
    assert!(matches!(blocks[2], Block::Rule));
    let Block::Table { rows, head } = &blocks[3] else { panic!("{blocks:?}") };
    assert_eq!((rows.len(), *head, rows[1].len()), (2, 1, 2));
    assert_eq!(line(&rows[1][1][0]).0, "2");
}

/// Resolves no reference, as for a text no entry's is.
fn none(_: Option<&str>, _: u32) -> Option<Key> {
    None
}

/// Resolves `#1` to `#5` here and `api#1` to `api#5` (`shop/api` too), the repository `api` of project `shop` at
/// brain 1.
fn some(repo: Option<&str>, id: u32) -> Option<Key> {
    let src = match repo {
        None => 0,
        Some("api" | "shop/api") => 1,
        Some(_) => return None,
    };
    (1..=5).contains(&id).then_some(Key::new(src, id))
}

/// The text of every link of the only block, and where it points.
fn links_of(text: &str) -> Vec<(String, LinkTarget)> {
    let blocks = blocks(some, text);
    let [block @ Block::Text { links, .. }] = &blocks[..] else { panic!("{blocks:?}") };
    let (shown, _) = line(block);
    links.iter().map(|(at, to)| (shown[at.clone()].to_string(), to.clone())).collect()
}

#[test]
fn references_to_entries_are_links_to_them() {
    let entry = |src, id| LinkTarget::Entry(Key::new(src, id));
    assert_eq!(
        links_of("Blocked by #3 and api#4, (#5), #2."),
        [
            ("#3".into(), entry(0, 3)),
            ("api#4".into(), entry(1, 4)),
            ("#5".into(), entry(0, 5)),
            ("#2".into(), entry(0, 2))
        ]
    );
    // The text around them is kept, in pieces none of which is empty.
    for (text, pieces) in [("#3 starts, ends #4", 3), ("Blocked by #3 and api#4.", 5)] {
        let blocks = blocks(some, text);
        let Block::Text { line, .. } = &blocks[0] else { panic!("{blocks:?}") };
        assert_eq!(line.0.iter().map(|s| s.content.as_str()).collect::<String>(), text);
        assert_eq!(line.0.iter().filter(|s| !s.content.is_empty()).count(), pieces, "{line:?}");
        assert_eq!(line.0.len(), pieces, "no empty piece: {line:?}");
    }
    // After any space, a non-breaking one too; and named as the app and kb's reports name them, `api #4` and
    // `shop/api #4`, where only a repository of the project is taken for one.
    assert_eq!(
        links_of("a\u{a0}#3, x\u{3000}api#4, api #4, shop/api #5 and the task #2"),
        [
            ("#3".into(), entry(0, 3)),
            ("api#4".into(), entry(1, 4)),
            ("api #4".into(), entry(1, 4)),
            ("shop/api #5".into(), entry(1, 5)),
            ("#2".into(), entry(0, 2))
        ]
    );
    // A path's piece before it isn't a repository's name: `/api #3` and `shop/ #4` name this brain's.
    assert_eq!(links_of("/api #3 and shop/ #4"), [("#3".into(), entry(0, 3)), ("#4".into(), entry(0, 4))]);
    // In headings, list items and table cells as in paragraphs.
    let found = |blocks: &[Block]| format!("{blocks:?}").matches("Entry(").count();
    assert_eq!(found(&blocks(some, "# Plan for #3")), 1);
    assert_eq!(found(&blocks(some, "- check api#4\n- and #2")), 2);
    assert_eq!(found(&blocks(some, "| a | b |\n|---|---|\n| #1 | api#2 |")), 2);
    let blocks = blocks(some, "Blocked by **#3** now");
    let (text, styles) = line(&blocks[0]);
    assert_eq!(text, "Blocked by #3 now");
    assert!(styles[1].bold && styles[1].fg == Some(ACCENT), "a reference is drawn as a link, in its style: {styles:?}");
}

#[test]
fn markdown_and_other_hashes_stay_as_they_are() {
    for text in [
        "`#3` and `api#4` are code",
        "[see #3](https://x.dev/pull/3)",
        "PR #3, pr #4, issue #2, MR #5, merge request #1",
        "#9 isn't an entry, nor is web#3",
        "owner/api#3, x.dev/page#3, a@api#3, #3b and #3_",
        "&#35;",
        "https://x.dev/?#3 and https://x.dev/?q=api#4, x://h#5",
        "PR\n#3, **PR** (#3), **PR**\u{a0}#4 and [PR](#) #5",
        "web/api #3 is another project's",
    ] {
        let blocks = blocks(some, text);
        let entries = |b: &Block| match b {
            Block::Text { links, .. } => links.iter().filter(|(_, t)| matches!(t, LinkTarget::Entry(_))).count(),
            _ => 0,
        };
        assert_eq!(blocks.iter().map(entries).sum::<usize>(), 0, "{text}: {blocks:?}");
    }
    assert!(matches!(&blocks(some, "```\n#3\n```")[..], [Block::Code(_)]));
    assert!(matches!(&blocks(some, "# 3")[..], [Block::Text { heading: 1, links, .. }] if links.is_empty()));
}

#[test]
fn a_preview_line_drops_the_markdown() {
    assert_eq!(
        plain("**Context** Sessions live in `PostgreSQL`, see [docs](https://x.y)."),
        "Context Sessions live in PostgreSQL, see docs."
    );
}

#[test]
fn a_bare_url_is_a_link_without_the_punctuation_around_it() {
    let url = |u: &str| LinkTarget::Url(u.to_string());
    let page = "https://docs.python.org/3/library/http.server.html";
    assert_eq!(links_of(&format!("Uses http.server ({page}).")), [(page.into(), url(page))]);
    assert_eq!(
        links_of("See http://x.dev/a_(b), https://en.wikipedia.org/wiki/Rust_(language)! and https://x.dev/?q=1&hl;"),
        [
            ("http://x.dev/a_(b)".into(), url("http://x.dev/a_(b)")),
            (
                "https://en.wikipedia.org/wiki/Rust_(language)".into(),
                url("https://en.wikipedia.org/wiki/Rust_(language)")
            ),
            ("https://x.dev/?q=1".into(), url("https://x.dev/?q=1")),
        ]
    );
    // Up to a space or `<`, and with its `#`, which isn't a reference there.
    assert_eq!(
        links_of("*https://x.dev/a#3*<b> then #4"),
        [("https://x.dev/a#3".into(), url("https://x.dev/a#3")), ("#4".into(), LinkTarget::Entry(Key::new(0, 4)))]
    );
}

#[test]
fn only_a_url_markdown_did_not_already_draw_is_linked() {
    let url = |u: &str| LinkTarget::Url(u.to_string());
    // A link's text, an autolink and code stay what markdown draws.
    assert_eq!(links_of("[https://a.dev](https://b.dev)"), [("https://a.dev".into(), url("https://b.dev"))]);
    assert_eq!(links_of("<https://a.dev>"), [("https://a.dev".into(), url("https://a.dev"))]);
    assert_eq!(links_of("`https://a.dev` x"), []);
    // A domain needs a period and no `_` in its last two parts, and the URL starts a word, as GFM has it.
    assert_eq!(links_of("http://localhost:8000 https://a_b.dev xhttps://a.dev https://"), []);
    assert_eq!(links_of("https://a_b.c.dev/"), [("https://a_b.c.dev/".into(), url("https://a_b.c.dev/"))]);
    assert_eq!(links_of("https://v.w.a_b.c.dev"), [("https://v.w.a_b.c.dev".into(), url("https://v.w.a_b.c.dev"))]);
    // Only an entity's name, letters and digits, ends it with its `;`.
    assert_eq!(links_of("https://x.dev/?a&b-c;"), [("https://x.dev/?a&b-c;".into(), url("https://x.dev/?a&b-c;"))]);
}

#[test]
fn references_listed_with_slashes_are_each_a_link() {
    let entry = |src, id| LinkTarget::Entry(Key::new(src, id));
    assert_eq!(
        links_of("read #1/#2/api#3 and #4/ #5"),
        [
            ("#1".into(), entry(0, 1)),
            ("#2".into(), entry(0, 2)),
            ("api#3".into(), entry(1, 3)),
            ("#4".into(), entry(0, 4)),
            ("#5".into(), entry(0, 5)),
        ]
    );
    // After a path's `/` it's still that path's: `owner/api#3`, `docs/#2`, `v1/#3`.
    assert_eq!(links_of("owner/api#3 docs/#2 v1/#3 owner/api#3/#4"), []);
}
