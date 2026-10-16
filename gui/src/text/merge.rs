//! The three-way merge of a text both the editor and `kb` changed from the one the editor opened: what only one side
//! changed is taken as is, and where both changed the same lines, the user picks.

use similar::{Algorithm, DiffOp, DiffTag, MergeResolution, TextMerge, capture_diff_slices};
use std::ops::Range;

/// A text merged from the one both sides started from.
#[derive(Debug, PartialEq, Eq)]
pub enum Merged {
    /// No lines both changed: the text with every change.
    Clean(String),
    /// Some lines both changed, which the user picks.
    Conflicted(Merge),
}

/// A merge with conflicts, and the one being picked.
#[derive(Debug, PartialEq, Eq)]
pub struct Merge {
    /// The text, in order: lines either side's change settles, and lines both changed.
    pub regions: Vec<Region>,
    /// Which conflict is being picked, counting only conflicts.
    pub at: usize,
    /// `kb`'s text, as merged: once it's another, the merge is of a text no longer there.
    pub theirs: String,
}

/// A run of a merged text's lines. Each holds whole lines, every one ending in a line break, so they join as they are.
#[derive(Debug, PartialEq, Eq)]
pub enum Region {
    /// As neither side changed them, or as the one that did.
    Settled(String),
    /// Changed on both sides, differently.
    Conflict(Conflict),
}

/// Lines both sides changed, and which the user took.
#[derive(Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The editor's lines; empty if it removed them.
    pub mine: String,
    /// What `kb` made of them; empty if it removed them.
    pub theirs: String,
    /// `None` until picked.
    pub pick: Option<Pick>,
}

impl Conflict {
    /// Whether its pick takes the editor's lines, and whether `kb`'s.
    pub fn taken(&self) -> (bool, bool) {
        match self.pick {
            Some(Pick::Mine) => (true, false),
            Some(Pick::Theirs) => (false, true),
            Some(Pick::Both) => (true, true),
            None => (false, false),
        }
    }
}

/// Which side's lines a conflict takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// The editor's.
    Mine,
    /// `kb`'s.
    Theirs,
    /// The editor's, then `kb`'s.
    Both,
}

/// Merges `mine` and `theirs`, both changed from `base`, line by line.
pub fn merge(base: &str, mine: &str, theirs: &str) -> Merged {
    // Every line ends in a break, so a text's last line is the same line as it is with one after it: otherwise a line
    // added after it changes it too, and collides with a change to it.
    let [base_ended, mine_ended, theirs_ended] = [base, mine, theirs].map(|text| format!("{text}\n"));
    let merge = TextMerge::from_lines(base_ended.as_str(), mine_ended.as_str(), theirs_ended.as_str());
    let base_lines = |range: Range<usize>| range.filter_map(|at| merge.base_line(at)).collect::<Vec<_>>();
    let ours_lines = |range: Range<usize>| range.filter_map(|at| merge.ours_line(at)).collect::<Vec<_>>();
    let theirs_lines = |range: Range<usize>| range.filter_map(|at| merge.theirs_line(at)).collect::<Vec<_>>();
    let mut regions = Vec::new();
    for region in merge.regions() {
        let settled = match region.resolution() {
            MergeResolution::Conflict => {
                let (ours, theirs) = (ours_lines(region.ours_range()), theirs_lines(region.theirs_range()));
                let Some(both) = untangled(&base_lines(region.base_range()), &ours, &theirs) else {
                    let conflict = Conflict { mine: ours.concat(), theirs: theirs.concat(), pick: None };
                    regions.push(Region::Conflict(conflict));
                    continue;
                };
                both
            }
            MergeResolution::Theirs => theirs_lines(region.theirs_range()).concat(),
            // Unchanged, changed only in the editor, or the same on both sides; `similar` may add resolutions.
            _ => ours_lines(region.ours_range()).concat(),
        };
        match regions.last_mut() {
            Some(Region::Settled(text)) => text.push_str(&settled),
            _ => regions.push(Region::Settled(settled)),
        }
    }
    let merge = Merge { regions, at: 0, theirs: theirs.to_string() };
    match merge.text() {
        Some(text) => Merged::Clean(text),
        None => Merged::Conflicted(merge),
    }
}

/// A region `similar` calls a conflict, merged when no line of `base` is changed on both sides: it takes changes that
/// only touch, as an edit to a line and a line added after it, for one. Each side's changes are applied to `base`; a
/// line added where the other side added one, or within lines it changed, is a conflict, `None`.
fn untangled(base: &[&str], ours: &[&str], theirs: &[&str]) -> Option<String> {
    let changes = |side: &[&str]| -> Vec<(Range<usize>, Range<usize>)> {
        capture_diff_slices(Algorithm::Myers, base, side)
            .iter()
            .map(DiffOp::as_tag_tuple)
            .filter(|(tag, ..)| *tag != DiffTag::Equal)
            .map(|(_, old, new)| (old, new))
            .collect()
    };
    let (ours_changes, theirs_changes) = (changes(ours), changes(theirs));
    let collide = |a: &Range<usize>, b: &Range<usize>| {
        a.start.max(b.start) < a.end.min(b.end)
            || (a.is_empty() && b.is_empty() && a.start == b.start)
            || (a.is_empty() && b.start < a.start && a.start < b.end)
            || (b.is_empty() && a.start < b.start && b.start < a.end)
    };
    if ours_changes.iter().any(|(a, _)| theirs_changes.iter().any(|(b, _)| collide(a, b))) {
        return None;
    }
    // Lines added before a change at the same line go first, as they come before it.
    let mut all: Vec<(&Range<usize>, &[&str])> = ours_changes
        .iter()
        .map(|(old, new)| (old, &ours[new.clone()]))
        .chain(theirs_changes.iter().map(|(old, new)| (old, &theirs[new.clone()])))
        .collect();
    all.sort_by_key(|(old, _)| (old.start, !old.is_empty()));
    let mut text = String::new();
    let mut at = 0;
    for (old, new) in all {
        text.extend(base[at..old.start].iter().chain(new).copied());
        at = old.end;
    }
    text.extend(base[at..].iter().copied());
    Some(text)
}

impl Merge {
    /// The conflicts, in order.
    pub fn conflicts(&self) -> impl Iterator<Item = &Conflict> {
        self.regions.iter().filter_map(|r| match r {
            Region::Conflict(c) => Some(c),
            Region::Settled(_) => None,
        })
    }

    /// The merged text, once every conflict is picked, without the line break [`merge`] added after its last line.
    pub fn text(&self) -> Option<String> {
        let mut text = String::with_capacity(self.theirs.len() * 2);
        for region in &self.regions {
            match region {
                Region::Settled(lines) => text.push_str(lines),
                Region::Conflict(c) => {
                    c.pick?;
                    let (mine, theirs) = c.taken();
                    if mine {
                        text.push_str(&c.mine);
                    }
                    if theirs {
                        text.push_str(&c.theirs);
                    }
                }
            }
        }
        text.pop();
        Some(text)
    }

    /// Picks `pick` for the conflict being picked, then moves to the next without a pick.
    pub fn pick(&mut self, pick: Pick) {
        self.pick_at(self.at, pick);
    }

    /// Picks `pick` for conflict `index` (one clicked), then moves to the first after it without a pick, else the
    /// first before it; with all picked, to `index`. An index past the last (from a stale click) picks nothing.
    pub fn pick_at(&mut self, index: usize, pick: Pick) {
        let mut conflicts = self.regions.iter_mut().filter_map(|r| match r {
            Region::Conflict(c) => Some(c),
            Region::Settled(_) => None,
        });
        let Some(conflict) = conflicts.nth(index) else { return };
        conflict.pick = Some(pick);
        let unpicked = || self.conflicts().enumerate().filter(|(_, c)| c.pick.is_none()).map(|(at, _)| at);
        let at = unpicked().find(|&at| at > index).or_else(|| unpicked().next()).unwrap_or(index);
        self.at = at;
    }

    /// Picks `pick` for every conflict without a pick.
    pub fn pick_rest(&mut self, pick: Pick) {
        for region in &mut self.regions {
            if let Region::Conflict(conflict) = region {
                conflict.pick.get_or_insert(pick);
            }
        }
    }

    /// Moves to the next conflict (`step` 1) or the one before (`-1`), stopping at the ends.
    pub fn go(&mut self, step: isize) {
        self.at = self.at.saturating_add_signed(step).min(self.conflicts().count().saturating_sub(1));
    }
}

#[cfg(test)]
#[path = "../../tests/merge.rs"]
mod tests;
