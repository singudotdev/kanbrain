//! Behaviour tests: drive the board with key presses over a temporary data folder.

mod agents;
mod board;
mod editor;
mod form;
mod on_disk;
mod projects;

use super::nav::Step;
use super::*;
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::model::{Asked, Author, DecisionState, now};
use kanbrain_core::store::Store;
use std::cell::Cell;
use tempfile::TempDir;

/// An entry of the central brain, where these tests' entries are.
fn k(id: u32) -> Key {
    Key::new(0, id)
}

/// Presses `code` on the board, with no modifier: an input of its own, as the window runs each key.
fn press(board: &mut Board, code: KeyCode) {
    board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    board.input_done();
}

/// A board over a fresh folder holding tasks with these titles (ids from 1); the folder lives as long
/// as the fixture. Its clock moves only by [`Fixture::later`].
struct Fixture {
    board: Board,
    dir: TempDir,
    now: Rc<Cell<Instant>>,
}

impl Fixture {
    fn new(titles: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path()).unwrap();
        for (title, id) in titles.iter().zip(1..) {
            store.save(&mut Task::new(id, (*title).to_string(), now())).unwrap();
        }
        let mut board = Board::open(dir.path(), None).unwrap();
        let now = Rc::new(Cell::new(Instant::now()));
        board.clock = Clock::new({
            let now = Rc::clone(&now);
            move || now.get()
        });
        Self { board, dir, now }
    }

    fn later(&self, by: Duration) {
        self.now.set(self.now.get() + by);
    }

    fn press(&mut self, code: KeyCode) {
        press(&mut self.board, code);
    }

    fn shift(&mut self, code: KeyCode) {
        self.board.on_key(KeyEvent::new(code, KeyModifiers::SHIFT));
        self.board.input_done();
    }

    fn type_text(&mut self, text: &str) {
        text.chars().for_each(|c| self.press(KeyCode::Char(c)));
    }

    fn status(&self, id: u32) -> Status {
        self.board.brain(0).task(id).unwrap().status
    }

    /// The task as its file says, independent of the board's memory.
    fn on_disk(&self, id: u32) -> Task {
        let (tasks, _) = Store::at(self.dir.path()).unwrap().load_entries().unwrap();
        tasks.into_iter().find(|t| t.id == id).unwrap()
    }
}

/// A central brain with one task, project shop's `api` and `web` repositories with one task each,
/// and project blog with one; all live as long as the returned folders.
fn projects() -> (TempDir, TempDir) {
    use kanbrain_core::projects;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let task = |dir: &std::path::Path, title: &str| {
        Store::at(dir).unwrap().save(&mut Task::new(1, title.to_string(), now())).unwrap();
    };
    task(central.path(), "renew passport");
    for (project, repo, title) in [("shop", "api", "rate limit"), ("shop", "web", "checkout"), ("blog", "blog", "post")]
    {
        let dir = code.path().join(repo).join(".brain");
        task(&dir, title);
        projects::register(central.path(), project, &dir).unwrap();
        projects::write_repo_card(
            &dir,
            &projects::RepoCard { project: project.into(), repo: None, notes: String::new() },
        )
        .unwrap();
    }
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    (central, code)
}
