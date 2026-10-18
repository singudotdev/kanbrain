//! The arguments of the entry commands: ids, text words, titles and links.

use super::Text;
use crate::model::{RepoLink, not_a_link, parse_links};
use anyhow::{Result, anyhow, bail};

/// The words `search` looks for, without the first `--`, which only ends the options: `kb search -- --` looks for
/// `--`.
pub(super) fn search_words(mut words: Vec<String>) -> String {
    if let Some(at) = words.iter().position(|w| w == "--") {
        words.remove(at);
    }
    words.join(" ")
}

/// The links typed in `value`; a word that isn't one is refused rather than left out.
pub(super) fn typed_links(value: &str) -> Result<(Vec<u32>, Vec<RepoLink>)> {
    if let Some(word) = not_a_link(value) {
        bail!("`{word}` isn't an entry id (12 or #12) or another repository's entry (REPO#ID)");
    }
    Ok(parse_links(value))
}

/// The words after `rename`'s id as the title, as `add` takes its own: `--` makes every word after it the title's,
/// however it starts.
pub(super) fn new_title(words: Vec<String>) -> Result<String> {
    let mut title = Vec::new();
    let mut words = words.into_iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--" => title.extend(words.by_ref()),
            other if other.starts_with("--") => bail!("unknown rename option `{other}`"),
            _ => title.push(word),
        }
    }
    let title = title.join(" ");
    if title.trim().is_empty() {
        bail!("rename needs the new title");
    }
    Ok(title)
}

/// The first argument as an entry id (`12` or `#12`).
pub(super) fn first_id(args: &mut Vec<String>) -> Result<u32> {
    if args.is_empty() {
        bail!("an entry id is needed");
    }
    let arg = args.remove(0);
    // The parser's own words ("invalid digit found in string") say nothing more to whoever typed it.
    arg.trim_start_matches('#').parse().map_err(|_| anyhow!("`{arg}` isn't an entry id"))
}

/// An id, then the text: the remaining words, or `-` alone for stdin; `needs` says what's missing without it.
pub(super) fn id_and_text(args: &mut Vec<String>, needs: &str) -> Result<(u32, Text)> {
    let id = first_id(args)?;
    Ok((id, rest_as_text(args, needs)?))
}

/// The remaining words as one text, or `-` alone for stdin; `needs` says what's missing without them.
pub(super) fn rest_as_text(args: &mut Vec<String>, needs: &str) -> Result<Text> {
    let mut words = std::mem::take(args);
    // Only says where options end (see `cli::options`); the text is what follows.
    if let Some(at) = words.iter().position(|w| w == "--") {
        words.remove(at);
    }
    match words.as_slice() {
        [] => bail!("{needs}: TEXT, or - to read it from stdin"),
        [dash] if dash == "-" => Ok(Text::Stdin),
        _ => Ok(Text::Given(words.join(" "))),
    }
}
