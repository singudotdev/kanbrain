//! `tag`: its words as changes to an entry's tags, or as a tag's new name, and making them.

use super::TagChange;
use crate::api::json::Summary;
use crate::api::text::line;
use crate::brain::Brain;
use crate::model::parse_tags;
use anyhow::{Result, bail};

/// What `tag` says a word after the id has to be.
const TAG_SIGNS: &str = "+TAG adds a tag, -TAG removes one";

/// The words after `tag`'s id as changes, in order. A word may name several tags (`+ui,#bug`), read as `add --tag`
/// reads them.
pub(super) fn tag_changes(words: Vec<String>) -> Result<Vec<TagChange>> {
    let mut changes = Vec::new();
    for word in words {
        let change = match word.as_bytes() {
            // Not an option `--` ends: `tag` takes no text.
            [b'-', b'-'] => bail!("unexpected argument `--`"),
            [b'-', b'-', ..] => bail!("unknown tag option `{word}`: {TAG_SIGNS}"),
            [b'+', ..] => TagChange::Add,
            [b'-', ..] => TagChange::Remove,
            _ => bail!("`{word}`: {TAG_SIGNS}"),
        };
        let tags = parse_tags(&word[1..]);
        if tags.is_empty() {
            bail!("`{word}` names no tag");
        }
        changes.extend(tags.into_iter().map(change));
    }
    if changes.is_empty() {
        bail!("tag needs what to change: {TAG_SIGNS}");
    }
    Ok(changes)
}

/// Makes `changes` to entry `id`'s tags and saves them as the kanban app's form does, colors included; returns
/// whether they changed, as nothing is written when they didn't.
pub(super) fn retag(brain: &mut Brain, id: u32, changes: &[TagChange]) -> Result<bool> {
    let entry = brain.entry(id)?;
    let mut tags = entry.tags.clone();
    for change in changes {
        match change {
            TagChange::Add(tag) if !tags.contains(tag) => tags.push(tag.clone()),
            TagChange::Add(_) => {}
            TagChange::Remove(tag) => tags.retain(|t| t != tag),
        }
    }
    if tags == entry.tags {
        return Ok(false);
    }
    let title = entry.title.clone();
    brain.update(id, &title, tags)?;
    Ok(true)
}

/// `word` as the one tag it names (`#ops` is `ops`), for `tag OLD NEW`.
pub(super) fn one_tag(word: &str) -> Result<String> {
    match parse_tags(word).as_slice() {
        [tag] => Ok(tag.clone()),
        [] => bail!("`{word}` names no tag"),
        _ => bail!("`{word}` names several tags: tag OLD NEW renames one"),
    }
}

/// Renames tag `old` to `new` in every entry of `brain` and prints the entries renamed, one line each (or as `search
/// --json` lists them). `tag 5 ui`, with an entry #5, may be `tag 5 +ui` mistyped: a tag named as an id is renamed only
/// when written `#5` (`hashed`), as a rename that wasn't meant would change every entry that has it.
pub(super) fn rename_tag(brain: &mut Brain, old: &str, new: &str, hashed: bool, json: bool) -> Result<()> {
    if !hashed && old.parse().ok().and_then(|id| brain.task(id)).is_some() {
        bail!("#{old} is an entry: to tag it, `kb tag {old} +{new}`; to rename tag #{old}, `kb tag '#{old}' {new}`");
    }
    let renamed = brain.rename_tag(old, new)?;
    let entries = renamed.iter().filter_map(|&id| brain.task(id));
    if json {
        println!("{}", serde_json::to_string(&entries.map(Summary::new).collect::<Vec<_>>())?);
    } else {
        entries.for_each(|task| println!("{}", line(task)));
    }
    Ok(())
}
