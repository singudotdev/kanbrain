//! What frames read of the brains, worked out once per change rather than each frame: the columns in order, each
//! entry's place and related entries, the tags in use, the counts the top bar shows. A brain's [`Index`] is built the
//! first time it's asked for after the brain changed ([`super::sources::Source`] drops it on every change); what the
//! board shows of all of them, [`Showing`], once per change of the brains, the scope or the search.

use super::{COLUMNS, Key};
use kanbrain_core::brain::Brain;
use kanbrain_core::model::{Kind, Status, Task};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// One brain's entries, as frames read them.
#[derive(Debug, Default)]
pub struct Index {
    /// Where each entry is in `Brain::tasks`, by id: the first, as `Brain::task` finds it, should two share one.
    at: HashMap<u32, usize>,
    /// The ids of each column's tasks, in [`COLUMNS`] order, most recently moved there first, as `Brain::column` has
    /// them.
    columns: [Vec<u32>; COLUMNS.len()],
    /// The ids of the notes, then of the decisions, newest first.
    entries: [Vec<u32>; 2],
    /// The entries related to each one, whichever side stores the link, in `Brain::related`'s order; none for an
    /// entry related to nothing.
    related: HashMap<u32, Vec<u32>>,
    /// Every tag in use, with how many tasks, notes and decisions carry it (in `Kind::ALL` order).
    tags: BTreeMap<String, [usize; Kind::ALL.len()]>,
    /// How many tasks, notes and decisions there are, in `Kind::ALL` order.
    kinds: [usize; Kind::ALL.len()],
    /// How many tasks aren't done.
    active: usize,
    /// The ids of the tasks waiting for an answer, lowest first.
    waiting: Vec<u32>,
}

impl Index {
    /// `brain`'s entries, indexed.
    pub fn new(brain: &Brain) -> Self {
        let tasks = &brain.tasks;
        let mut index = Self::default();
        for (at, task) in tasks.iter().enumerate() {
            index.at.entry(task.id).or_insert(at);
            index.kinds[task.kind.position()] += 1;
            for tag in &task.tags {
                index.tags.entry(tag.clone()).or_default()[task.kind.position()] += 1;
            }
        }
        for (column, &status) in index.columns.iter_mut().zip(&COLUMNS) {
            *column = brain.column(status).iter().map(|t| t.id).collect();
        }
        for (entries, kind) in index.entries.iter_mut().zip(ENTRY_KINDS) {
            let mut of_kind: Vec<&Task> = tasks.iter().filter(|t| t.kind == kind).collect();
            of_kind.sort_by_key(|t| (Reverse(t.created), Reverse(t.id)));
            *entries = of_kind.iter().map(|t| t.id).collect();
        }
        // Each link both ways, once: `Brain::related` sorted the same.
        let mut related: HashMap<u32, Vec<&Task>> = HashMap::new();
        let task = |id: u32| index.at.get(&id).map(|&at| &tasks[at]);
        for from in tasks {
            for &to in from.links.iter().filter(|&&to| to != from.id) {
                if let Some(to) = task(to) {
                    related.entry(from.id).or_default().push(to);
                    related.entry(to.id).or_default().push(from);
                }
            }
        }
        index.related = related
            .into_iter()
            .map(|(id, mut entries)| {
                entries.sort_by_key(|t| (related_rank(t.kind), Reverse(t.id)));
                entries.dedup_by_key(|t| t.id);
                (id, entries.iter().map(|t| t.id).collect())
            })
            .collect();
        index.active = tasks.iter().filter(|t| t.is_task() && t.status != Status::Done).count();
        index.waiting = tasks.iter().filter(|t| t.waits_for_answer()).map(|t| t.id).collect();
        index.waiting.sort_unstable();
        index
    }

    /// Where the entry with this id is in `Brain::tasks`.
    pub fn at(&self, id: u32) -> Option<usize> {
        self.at.get(&id).copied()
    }

    /// Every tag in use, by name, with how many tasks, notes and decisions carry it (in `Kind::ALL` order).
    pub fn tags(&self) -> &BTreeMap<String, [usize; Kind::ALL.len()]> {
        &self.tags
    }

    /// The ids of the entries related to the one with this id, as `Brain::related` lists them.
    pub fn related(&self, id: u32) -> &[u32] {
        self.related.get(&id).map_or(&[], Vec::as_slice)
    }
}

/// The kinds listed on their own tab, in the order [`Index`] and [`Showing`] keep them.
const ENTRY_KINDS: [Kind; 2] = [Kind::Note, Kind::Decision];

/// Where `kind`'s entries are among [`Showing::entries`]: none for tasks, which the board shows by column.
pub fn entries_place(kind: Kind) -> Option<usize> {
    ENTRY_KINDS.iter().position(|&k| k == kind)
}

/// Where `Brain::related` lists an entry of this kind: notes and decisions before tasks.
fn related_rank(kind: Kind) -> u8 {
    match kind {
        Kind::Note => 0,
        Kind::Decision => 1,
        Kind::Task => 2,
    }
}

/// What the board shows of the brains in scope, through the search: what frames draw, and the counts beside it.
#[derive(Debug, Default)]
pub struct Showing {
    /// Each column's tasks matching the search, in [`COLUMNS`] order, most recently moved there first: every done
    /// task, which the Done column cuts unless "Show all".
    pub columns: [Rc<[Key]>; COLUMNS.len()],
    /// How many tasks each column has in scope, matching the search or not.
    pub totals: [usize; COLUMNS.len()],
    /// The notes, then the decisions, matching the search, newest first.
    pub entries: [Rc<[Key]>; 2],
    /// Every tag in use in scope, with how many tasks, notes and decisions carry it.
    pub tags: BTreeMap<String, [usize; Kind::ALL.len()]>,
    /// How many tasks, notes and decisions are in scope.
    pub kinds: [usize; Kind::ALL.len()],
    /// How many tasks in scope aren't done.
    pub active: usize,
    /// The tasks in scope waiting for an answer, by brain then id.
    pub waiting: Rc<[Key]>,
    /// The entries of a project's other repositories related to each entry, whichever side stores the link, by
    /// repository then where each is in its brain; none for one related to nothing there. Every brain's, whatever the
    /// scope, as an entry shown links to ones that may not be.
    pub repo_related: HashMap<Key, Vec<Key>>,
}

impl Showing {
    /// What the brains in `scoped` (each with its place in the board's list and its index) show of `search`: those
    /// `shows` keeps.
    pub fn new<'a>(
        scoped: impl Iterator<Item = (usize, &'a Brain, &'a Index)> + Clone,
        shows: impl Fn(&Task) -> bool,
    ) -> Self {
        let mut showing = Self::default();
        for (column, place) in showing.columns.iter_mut().zip(0..) {
            let mut keys: Vec<(Key, &Task)> = scoped
                .clone()
                .flat_map(|(src, brain, index)| {
                    index.columns[place].iter().map(move |&id| (Key::new(src, id), &brain.tasks[index.at[&id]]))
                })
                .filter(|(_, t)| shows(t))
                .collect();
            // As within one brain: among moves at the same time, the newest entry first.
            keys.sort_by_key(|(key, t)| (Reverse(t.status_since()), key.src, Reverse(key.id)));
            *column = keys.into_iter().map(|(key, _)| key).collect();
        }
        for (entries, place) in showing.entries.iter_mut().zip(0..) {
            let mut keys: Vec<(Key, &Task)> = scoped
                .clone()
                .flat_map(|(src, brain, index)| {
                    index.entries[place].iter().map(move |&id| (Key::new(src, id), &brain.tasks[index.at[&id]]))
                })
                .filter(|(_, t)| shows(t))
                .collect();
            keys.sort_by_key(|(key, t)| (Reverse(t.created), key.src, Reverse(key.id)));
            *entries = keys.into_iter().map(|(key, _)| key).collect();
        }
        let mut waiting = Vec::new();
        for (src, _, index) in scoped {
            for (total, column) in showing.totals.iter_mut().zip(&index.columns) {
                *total += column.len();
            }
            for (tag, uses) in &index.tags {
                let all = showing.tags.entry(tag.clone()).or_default();
                all.iter_mut().zip(uses).for_each(|(all, n)| *all += n);
            }
            showing.kinds.iter_mut().zip(&index.kinds).for_each(|(all, n)| *all += n);
            showing.active += index.active;
            waiting.extend(index.waiting.iter().map(|&id| Key::new(src, id)));
        }
        showing.waiting = waiting.into();
        showing
    }
}

/// An entry related to another, after what orders it among the others: its repository, its kind's rank, its id.
type Ranked = (usize, u8, Reverse<u32>, Key);

/// Each entry's related entries in the project's other repositories (see [`Showing::repo_related`]), from every brain
/// on the board: each with its place there, the project and repository it is, and its index.
pub fn repo_related<'a>(
    sources: &(impl Iterator<Item = (usize, &'a Brain, &'a Index)> + Clone),
) -> HashMap<Key, Vec<Key>> {
    let at = |project: &str, repo: &str| {
        sources
            .clone()
            .find(|(_, brain, _)| brain.project.as_ref().is_some_and(|p| p.name == project && p.repo == repo))
    };
    // Each pair once from each side, with what orders them: by repository, then as `Brain::repo_related` does (notes,
    // decisions, tasks, the newest first), not by where they are in their brain, which a reload of one changes.
    let mut related: HashMap<Key, Vec<Ranked>> = HashMap::new();
    for (src, brain, _) in sources.clone() {
        let Some(here) = &brain.project else { continue };
        for task in &brain.tasks {
            for link in &task.repo_links {
                let Some((there, other, index)) = at(&here.name, &link.repo).filter(|&(there, ..)| there != src) else {
                    continue;
                };
                let Some(other_place) = index.at(link.id) else { continue };
                let target = &other.tasks[other_place];
                let (from, to) = (Key::new(src, task.id), Key::new(there, target.id));
                related.entry(from).or_default().push((there, related_rank(target.kind), Reverse(target.id), to));
                related.entry(to).or_default().push((src, related_rank(task.kind), Reverse(task.id), from));
            }
        }
    }
    related
        .into_iter()
        .map(|(key, mut keys)| {
            // A key is its repository and id: those order and tell apart.
            keys.sort_unstable_by_key(|&(src, rank, id, _)| (src, rank, id));
            keys.dedup_by_key(|&mut (src, rank, id, _)| (src, rank, id));
            (key, keys.into_iter().map(|(.., key)| key).collect())
        })
        .collect()
}
