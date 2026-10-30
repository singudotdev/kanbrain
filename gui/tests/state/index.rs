//! The board's indexes against working the same out from every entry, as frames did before them: after any change,
//! through any scope and search, they must say the same.

use super::*;
use kanbrain_core::brain::Brain;

/// A sequence of numbers that looks random, the same on every run: the changes the tests make, by seed.
struct Dice(u64);

impl Dice {
    /// A number below `below`.
    fn roll(&mut self, below: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        u32::try_from((self.0 >> 33) % u64::from(below)).unwrap()
    }
}

/// A board of the central brain and the project `blog`'s repositories `api` and `web`, `n` entries each of every
/// kind, some tagged, linked (within a brain, and each repository to the other) and parked waiting for an answer.
fn board(n: u32) -> (Board, TempDir, TempDir) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let repos = [code.path().join("api/.brain"), code.path().join("web/.brain")];
    for (dir, other) in [(central.path(), None), (repos[0].as_path(), Some("web")), (repos[1].as_path(), Some("api"))] {
        let store = Store::at(dir).unwrap();
        for id in 1..=n {
            let mut task = Task::new(id, format!("entry {id}"), now() - chrono::Duration::minutes(i64::from(id)));
            task.kind = [Kind::Task, Kind::Task, Kind::Note, Kind::Decision][id as usize % 4];
            task.tags = ["auth", "api", "ui"].iter().take(id as usize % 4).map(ToString::to_string).collect();
            if id > 2 && id % 3 == 0 {
                task.links = vec![id - 2, id - 1];
            }
            if let Some(repo) = other.filter(|_| id % 5 == 0) {
                task.repo_links = vec![kanbrain_core::model::RepoLink { repo: repo.into(), id: id - 1 }];
            }
            store.save(&mut task).unwrap();
        }
    }
    for repo in &repos {
        kanbrain_core::projects::register(central.path(), "blog", repo).unwrap();
        let card = kanbrain_core::projects::RepoCard { project: "blog".into(), repo: None, notes: String::new() };
        kanbrain_core::projects::write_repo_card(repo, &card).unwrap();
    }
    (Board::open(central.path(), None).unwrap(), central, code)
}

/// The entries of the project's other repositories related to `key`'s, worked out as frames did before the index, in
/// the order of the core's `Brain::repo_related`: by repository, then notes, decisions and tasks, the newest first.
fn repo_related(board: &Board, key: Key) -> Vec<Key> {
    let Some(here) = &board.brain(key.src).project else { return Vec::new() };
    let links = board.brain(key.src).task(key.id).map_or(&[][..], |t| t.repo_links.as_slice());
    let back = kanbrain_core::model::RepoLink { repo: here.repo.clone(), id: key.id };
    let mut related = Vec::new();
    for src in (0..board.sources.len()).filter(|&src| src != key.src) {
        let brain = board.brain(src);
        let Some(there) = brain.project.as_ref().filter(|p| p.name == here.name) else { continue };
        let linked =
            |t: &&Task| links.iter().any(|l| l.repo == there.repo && l.id == t.id) || t.repo_links.contains(&back);
        let mut found: Vec<&Task> = brain.tasks.iter().filter(linked).collect();
        let rank = |kind| [Kind::Note, Kind::Decision, Kind::Task].iter().position(|k| *k == kind);
        found.sort_by_key(|t| (rank(t.kind), std::cmp::Reverse(t.id)));
        related.extend(found.into_iter().map(|t| Key::new(src, t.id)));
    }
    related
}

/// What the board's indexes must say: see [`from_every_entry`].
struct Expected {
    /// Each column's keys shown, in [`COLUMNS`] order.
    columns: Vec<Vec<Key>>,
    /// Each column's tasks in scope.
    totals: Vec<usize>,
    /// The notes, then the decisions, shown.
    entries: Vec<Vec<Key>>,
    /// Every tag in scope with its uses per kind.
    tags: Vec<(String, [usize; 3])>,
    /// The tasks waiting for an answer.
    waiting: Vec<Key>,
}

/// What the board's indexes must say, worked out from every entry of the brains in scope, as frames did before them.
fn from_every_entry(board: &Board) -> Expected {
    let search = board.search.text();
    let shows = |t: &Task| t.matches(search);
    let scoped: Vec<(usize, &Brain)> = board.scoped().collect();
    let columns = COLUMNS
        .iter()
        .map(|&status| {
            let mut cards: Vec<(Key, &Task)> = scoped
                .iter()
                .flat_map(|&(src, brain)| brain.column(status).into_iter().map(move |t| (Key::new(src, t.id), t)))
                .filter(|(_, t)| shows(t))
                .collect();
            cards.sort_by_key(|(key, t)| (std::cmp::Reverse(t.status_since()), key.src, std::cmp::Reverse(key.id)));
            cards.into_iter().map(|(key, _)| key).collect()
        })
        .collect();
    let totals = COLUMNS.iter().map(|&s| scoped.iter().map(|(_, brain)| brain.column(s).len()).sum()).collect();
    let entries = [Kind::Note, Kind::Decision]
        .iter()
        .map(|&kind| {
            let mut entries: Vec<(Key, &Task)> = scoped
                .iter()
                .flat_map(|&(src, brain)| brain.tasks.iter().map(move |t| (Key::new(src, t.id), t)))
                .filter(|(_, t)| t.kind == kind && shows(t))
                .collect();
            entries.sort_by_key(|(key, t)| (std::cmp::Reverse(t.created), key.src, std::cmp::Reverse(key.id)));
            entries.into_iter().map(|(key, _)| key).collect()
        })
        .collect();
    let mut tags: Vec<String> = scoped.iter().flat_map(|(_, b)| b.all_tags()).collect();
    tags.sort();
    tags.dedup();
    let tags = tags
        .into_iter()
        .map(|tag| {
            let mut uses = [0; 3];
            for task in scoped.iter().flat_map(|(_, b)| &b.tasks).filter(|t| t.tags.contains(&tag)) {
                uses[task.kind.position()] += 1;
            }
            (tag, uses)
        })
        .collect();
    let mut waiting: Vec<Key> = scoped
        .iter()
        .flat_map(|&(src, brain)| brain.tasks.iter().filter(|t| t.waits_for_answer()).map(move |t| Key::new(src, t.id)))
        .collect();
    waiting.sort_by_key(|k| (k.src, k.id));
    Expected { columns, totals, entries, tags, waiting }
}

/// Checks every index of `board` against [`from_every_entry`], and each entry's place and related entries.
fn agrees(board: &Board, after: &str) {
    let Expected { columns, totals, entries, tags, waiting } = from_every_entry(board);
    let showing = board.showing();
    for (place, status) in COLUMNS.iter().enumerate() {
        assert_eq!(&*showing.columns[place], columns[place], "{status:?} column, after {after}");
        assert_eq!(board.column_total(*status), totals[place], "{status:?} total, after {after}");
        let limit = if *status == Status::Done && !board.all_done { DONE_LIMIT } else { usize::MAX };
        let shown: Vec<Key> = board.column(*status).into_iter().map(|(key, _)| key).collect();
        assert_eq!(shown, columns[place].iter().copied().take(limit).collect::<Vec<_>>(), "{status:?}, after {after}");
    }
    let done = columns[COLUMNS.len() - 1].len();
    assert_eq!(board.hidden_done(), if board.all_done { 0 } else { done.saturating_sub(DONE_LIMIT) }, "{after}");
    assert_eq!(&*showing.entries[0], entries[0], "notes, after {after}");
    assert_eq!(&*showing.entries[1], entries[1], "decisions, after {after}");
    let indexed: Vec<(String, [usize; 3])> = showing.tags.iter().map(|(t, u)| (t.clone(), *u)).collect();
    assert_eq!(indexed, tags, "tags, after {after}");
    assert_eq!(&*board.waiting(), waiting, "waiting, after {after}");
    let kinds = [Kind::Task, Kind::Note, Kind::Decision]
        .map(|kind| board.scoped().flat_map(|(_, b)| &b.tasks).filter(|t| t.kind == kind).count());
    assert_eq!(showing.kinds, kinds, "counts, after {after}");
    let active = board.scoped().flat_map(|(_, b)| &b.tasks).filter(|t| t.is_task() && t.status != Status::Done).count();
    assert_eq!(showing.active, active, "active, after {after}");
    let tab = board.tab.kind().and_then(|kind| [Kind::Note, Kind::Decision].iter().position(|&k| k == kind));
    let listed: Vec<Key> = board.entries().into_iter().map(|(key, _)| key).collect();
    assert_eq!(listed, tab.map_or_else(Vec::new, |place| entries[place].clone()), "{:?} tab, after {after}", board.tab);
    for (src, brain) in board.scoped() {
        for task in &brain.tasks {
            let key = Key::new(src, task.id);
            assert_eq!(board.task(key).map(|t| t.id), Some(task.id), "{key:?}, after {after}");
            let mut related: Vec<Key> = brain.related(task.id).iter().map(|t| Key::new(src, t.id)).collect();
            related.extend(repo_related(board, key));
            assert_eq!(board.related_keys(key), related, "related to {key:?}, after {after}");
        }
        assert!(board.task(Key::new(src, 9999)).is_none(), "an id no entry has");
    }
}

#[test]
fn the_indexes_say_what_every_entry_does_after_any_change() {
    let limit = u32::try_from(DONE_LIMIT).unwrap();
    let (mut board, _central, _code) = board(limit + 6);
    let mut dice = Dice(7);
    agrees(&board, "opening");
    let api_to_web = board.related_keys(Key::new(1, 5));
    assert_eq!(api_to_web, [Key::new(1, 6), Key::new(2, 4)], "linked from api's #6, and to web's #4");
    let statuses = [Status::Todo, Status::Doing, Status::Parked, Status::Done];
    let ids = limit + 8;
    for round in 0..60 {
        let src = dice.roll(3) as usize;
        let id = dice.roll(ids) + 1;
        // Some rounds change no brain: the scope, the search or "Show all" alone must show what they do.
        let change = match dice.roll(9) {
            0 => format!("status {:?}", board.brain_mut(src).set_status(id, statuses[dice.roll(4) as usize])),
            1 => {
                let tags: Vec<String> =
                    ["auth", "db", "ui", "ops"].iter().filter(|_| dice.roll(2) == 0).map(ToString::to_string).collect();
                format!("tags {:?}", board.brain_mut(src).update(id, &format!("entry {id} {round}"), tags))
            }
            2 => {
                format!("link {:?}", board.toggle_link_between(Key::new(src, id), Key::new(dice.roll(3) as usize, id)))
            }
            3 => format!("ask {:?}", board.brain_mut(src).ask(id, "which?")),
            4 => format!("delete {:?}", board.brain_mut(src).delete(id)),
            5 => format!("comment {:?}", board.brain_mut(src).add_comment(id, "seen")),
            _ => "nothing".to_string(),
        };
        let shown = match dice.roll(5) {
            0 => {
                board.scope = vec![];
                "all"
            }
            1 => {
                board.scope = vec![Scope::Central];
                "central"
            }
            2 => {
                board.scope = vec![Scope::Project("blog".into())];
                "blog"
            }
            3 => {
                board.scope = vec![Scope::Repo(2), Scope::Central];
                "web and central"
            }
            _ => "as it was",
        };
        let search = ["", "entry 1", "#a", "#auth 2", "zz"][dice.roll(5) as usize];
        board.search = crate::text::editor::LineInput::new(search);
        board.all_done = dice.roll(3) == 0;
        board.tab = [Tab::Board, Tab::Notes, Tab::Decisions, Tab::Report][dice.roll(4) as usize];
        let after = format!("round {round}: {change} in brain {src}, #{id}, scope {shown}, search {search:?}");
        agrees(&board, &after);
        // Read twice, it says the same: the second time from what it kept.
        agrees(&board, &format!("{after}, again"));
    }
}

#[test]
fn a_brain_read_again_is_indexed_again() {
    let (mut board, central, _code) = board(6);
    agrees(&board, "opening");
    let mut cli = Brain::open(Store::at(central.path()).unwrap()).unwrap();
    cli.set_status(1, Status::Done).unwrap();
    cli.update(2, "entry 2", vec!["fresh".into()]).unwrap();
    board.reload_changed(&all_of(&board, &[central.path().to_path_buf()])).unwrap();
    agrees(&board, "kb changed it and the board read it again");
    assert!(board.showing().tags.contains_key("fresh"), "the tag kb added");
    board.reload().unwrap();
    agrees(&board, "reading every brain again");
}

#[test]
fn the_scope_or_the_search_alone_changes_what_shows() {
    let (mut board, _central, _code) = board(8);
    agrees(&board, "opening");
    for (scope, said) in [
        (vec![Scope::Central], "central"),
        (vec![Scope::Repo(1)], "api"),
        (vec![Scope::Project("blog".into())], "blog"),
        (vec![], "all"),
    ] {
        board.scope = scope;
        agrees(&board, &format!("the scope alone, now {said}"));
    }
    for search in ["entry 1", "#auth", "", "zz"] {
        board.search = crate::text::editor::LineInput::new(search);
        agrees(&board, &format!("the search alone, now {search:?}"));
    }
}

#[test]
fn another_repositorys_related_entries_keep_their_order_once_one_is_read_again() {
    let (mut board, _central, code) = board(6);
    let (api, web) = (code.path().join("api/.brain"), code.path().join("web/.brain"));
    // api's #1 linked to web's #1 to #4: a task, a note, a decision and a task.
    let mut cli = Brain::open(Store::at(&api).unwrap()).unwrap();
    let index = cli.tasks.iter().position(|t| t.id == 1).unwrap();
    let links = (1..=4).map(|id| kanbrain_core::model::RepoLink { repo: "web".into(), id });
    cli.tasks[index].repo_links = links.collect();
    cli.store.save(&mut cli.tasks[index]).unwrap();
    board.reload().unwrap();
    let in_web = |board: &Board| -> Vec<u32> {
        board.related_keys(Key::new(1, 1)).into_iter().filter(|key| key.src == 2).map(|key| key.id).collect()
    };
    assert_eq!(in_web(&board), [2, 3, 4, 1], "the note, the decision, then the tasks, the newest first");
    // An agent comments on web's #4: read again alone, it's the last of its brain's entries.
    let mut cli = Brain::open(Store::at(&web).unwrap()).unwrap();
    cli.add_comment(4, "seen").unwrap();
    board.reload_changed(&all_of(&board, &[web])).unwrap();
    assert_eq!(in_web(&board), [2, 3, 4, 1], "as before");
}
