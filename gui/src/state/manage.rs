//! Renaming, moving and removing projects and their repositories, from Manage project: what `kb project` and
//! `kb deinit` do, through the same functions, after which the window re-reads its brains.

use super::{Board, Manage, Mode, Picker, PickerKey, ProjectOption, Request, Scope, Target};
use crate::text::event::{KeyCode, KeyEvent};
use anyhow::Result;
use kanbrain_core::cli;
use std::path::Path;

impl Board {
    /// Asks for the project's new name, starting from the one it has.
    pub fn ask_rename_project(&mut self, src: usize) {
        let name = self.sources[src].project.clone().unwrap_or_default();
        self.mode = Mode::Manage(Manage::RenameProject { src, name: Picker::named(&name) });
    }

    /// Asks for the repository's new name in its project, starting from the one it has.
    pub fn ask_rename_repo(&mut self, src: usize) {
        let name = self.brain(src).project.as_ref().map(|p| p.repo.clone()).unwrap_or_default();
        self.mode = Mode::Manage(Manage::RenameRepo { src, name: Picker::named(&name) });
    }

    /// Asks which project the repository moves to.
    pub fn ask_move_repo(&mut self, src: usize) {
        self.mode = Mode::Manage(Manage::MoveRepo { src, picker: Picker::default() });
    }

    /// Asks whether to take the repository (or, `whole`, every repository of its project) out, keeping its tasks
    /// or deleting them.
    pub fn ask_remove(&mut self, src: usize, whole: bool) {
        self.mode = Mode::Manage(Manage::Remove { src, whole });
    }

    /// Manage project's keys, as its buttons: `↑` `↓` choose the repository `src` the lowercase ones act on (`r`
    /// rename, `M` move, `x` remove, `N` its role), the others act on the project (`R` rename, `E` describe, `A` add
    /// a repository, `X` remove); `esc`, `enter`, `I` and `q` close it.
    pub(super) fn key_manage_project(&mut self, src: usize, key: KeyEvent) {
        let repos = self.project_repos(src);
        let at = repos.iter().position(|&r| r == src).unwrap_or_default();
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('I' | 'q') => {}
            KeyCode::Up => self.mode = Mode::ManageProject(repos.get(at.wrapping_sub(1)).copied().unwrap_or(src)),
            KeyCode::Down => self.mode = Mode::ManageProject(repos.get(at + 1).copied().unwrap_or(src)),
            KeyCode::Char('r') => self.ask_rename_repo(src),
            KeyCode::Char('M') => self.ask_move_repo(src),
            KeyCode::Char('x') => self.ask_remove(src, false),
            KeyCode::Char('N') => self.open_editor_from(Target::RepoNotes(src), Some(src)),
            KeyCode::Char('R') => self.ask_rename_project(src),
            KeyCode::Char('E') => self.open_editor_from(Target::Project(src), Some(src)),
            KeyCode::Char('X') => self.ask_remove(src, true),
            KeyCode::Char('A') => {
                self.add_repo_to(src);
                self.mode = Mode::ManageProject(src);
            }
            _ => self.mode = Mode::ManageProject(src),
        }
    }

    /// Adds another folder to the project of `src`: asks the window for it, then for its project, this one
    /// already named.
    pub fn add_repo_to(&mut self, src: usize) {
        self.request = Some(Request::Folder(self.sources[src].project.clone()));
    }

    /// The move dialog's rows: as adding a project's, without the repository's own project.
    pub fn move_options(&self, src: usize, picker: &Picker) -> Vec<ProjectOption> {
        let own = self.sources[src].project.as_deref();
        self.add_project_options(picker).into_iter().filter(|option| Some(option.name()) != own).collect()
    }

    /// The dialog's keys: typing names or searches, arrows move, `enter` applies, `esc` goes back to the project's
    /// info. Removing takes `y` (or `enter`) to unlink and keep the tasks, `d` to delete them too.
    pub(super) fn key_manage(&mut self, mut manage: Manage, key: KeyEvent) {
        let src = manage.src();
        if let Manage::Remove { whole, .. } = manage {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.remove(src, whole, false),
                KeyCode::Char('d') => self.remove(src, whole, true),
                KeyCode::Esc | KeyCode::Char('n') => self.mode = Mode::ManageProject(src),
                _ => self.mode = Mode::Manage(manage),
            }
            return;
        }
        let count = match &manage {
            Manage::MoveRepo { picker, .. } => self.move_options(src, picker).len(),
            _ => 0,
        };
        let Some(picker) = manage.picker_mut() else { return };
        let outcome = picker.key(key, count);
        let sel = picker.sel;
        match outcome {
            PickerKey::Close => self.mode = Mode::ManageProject(src),
            PickerKey::Stay => self.mode = Mode::Manage(manage),
            PickerKey::Choose => {
                self.mode = Mode::Manage(manage);
                self.apply_manage(sel);
            }
        }
    }

    /// Renames or moves as the dialog says, `sel` being the move's chosen row. The dialog stays open, with why,
    /// when it can't.
    pub fn apply_manage(&mut self, sel: usize) {
        let Mode::Manage(manage) = &self.mode else { return };
        let src = manage.src();
        let dir = self.brain(src).store.dir().to_path_buf();
        let project = self.sources[src].project.clone().unwrap_or_default();
        let (done, project) = match manage {
            Manage::RenameProject { name, .. } => {
                let new = name.search.text().trim().to_string();
                (cli::rename_project(&self.central, &project, &new), new)
            }
            Manage::RenameRepo { name, .. } => {
                (cli::rename_repo(&self.central, &dir, name.search.text().trim()), project)
            }
            Manage::MoveRepo { picker, .. } => {
                let Some(to) = self.move_options(src, picker).into_iter().nth(sel).map(|o| o.name().to_string()) else {
                    self.notify("type the project's name");
                    return;
                };
                (cli::move_repo(&self.central, &dir, &to), to)
            }
            // Removing has its own keys, which `key_manage` runs.
            Manage::Remove { .. } => return,
        };
        if self.reloaded(done) {
            if matches!(self.scope, Scope::Project(_) | Scope::Repo(_)) {
                self.set_scope(Scope::Project(project));
            }
            self.mode = self.src_at(&dir).map_or(Mode::Normal, Mode::ManageProject);
        }
    }

    /// Takes the repository at `src` (or, `whole`, every repository of its project) out, deleting its tasks with
    /// `purge`, and goes back to the board.
    fn remove(&mut self, src: usize, whole: bool, purge: bool) {
        let dir = self.brain(src).store.dir().to_path_buf();
        let project = self.sources[src].project.clone().unwrap_or_default();
        let done = if whole {
            cli::remove_project(&self.central, &project, purge)
        } else {
            cli::remove_repo(&self.central, &dir, purge)
        };
        if self.reloaded(done) {
            self.mode = Mode::Normal;
            let gone = |board: &Self| board.src_at(Path::new(&dir)).is_none() && board.repos_of(&project).count() == 0;
            if matches!(self.scope, Scope::Project(_) | Scope::Repo(_)) && gone(self) {
                self.set_scope(Scope::All);
            }
        }
    }

    /// After a change to projects: re-reads every brain and says what was done (its first line), or why nothing
    /// was. Returns whether it was done.
    fn reloaded(&mut self, done: Result<Vec<String>>) -> bool {
        match done {
            Ok(done) => {
                let result = self.reload();
                self.report_error(result);
                if let Some(first) = done.first() {
                    self.notify(first.clone());
                }
                true
            }
            Err(e) => {
                self.notify_error(&e);
                false
            }
        }
    }
}
