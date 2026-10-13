//! What each key does in each dialog, handing the rest to [`super::keys`]. Buttons in the window run the same keys
//! ([`Board::on_key`]), so a button and its shortcut can't differ.
//! Also typing into whichever field has the keys, for the mouse and the clipboard.

use super::nav::Step;
use super::{
    Board, COLUMNS, CommentPicker, Confirm, Field, Form, Manage, Mode, Picker, PickerKey, Tab, Target, WherePicker,
};
use crate::text::editor::{LineInput, Outcome, Pointer, TextEditor};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::model::Kind;

/// Where typed text goes: a one-line field or the text editor.
enum Typing<'a> {
    /// A one-line field.
    Line(&'a mut LineInput),
    /// The text editor.
    Text(&'a mut TextEditor),
}

impl Board {
    /// Handles one key press for whatever has the keys.
    pub fn on_key(&mut self, key: KeyEvent) {
        self.recorded(|board| board.key(key));
    }

    /// A key, without recording what it changes.
    fn key(&mut self, key: KeyEvent) {
        // Each handler puts back the mode it continues in; the default is the board.
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => self.key_normal(key),
            Mode::Search => self.key_search(key),
            Mode::Form(form) => self.key_form(form, key),
            Mode::QuickAdd(mut add) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => self.submit_quick_add(&add),
                _ => {
                    add.title.on_key(key);
                    self.mode = Mode::QuickAdd(add);
                }
            },
            Mode::Edit(mut edit) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl && key.code == KeyCode::Char('d') {
                    // Discard changes; the mode is already taken.
                    self.notify("changes discarded");
                } else if edit.editor.on_key(key) == Outcome::Save || (ctrl && key.code == KeyCode::Enter) {
                    self.save_edit(&edit);
                } else {
                    self.mode = Mode::Edit(edit);
                }
            }
            Mode::Confirm(confirm) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.confirm(confirm),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.mode = Mode::Confirm(confirm),
            },
            // Choosing a row links or unlinks it, and the picker stays open for the next.
            Mode::Links(mut picker) => {
                let count = self.link_candidates(&picker).len();
                let outcome = picker.rows.key(key, count);
                let sel = picker.rows.sel;
                if outcome != PickerKey::Close {
                    self.mode = Mode::Links(picker);
                }
                if outcome == PickerKey::Choose {
                    self.toggle_link(sel);
                }
            }
            Mode::Comments(picker) => self.key_comments(picker, key),
            Mode::Scopes(picker) => {
                let count = self.scope_candidates(&picker).len();
                self.pick(picker, key, count, |p| p, Mode::Scopes, Self::choose_scope);
            }
            Mode::Where(asked) => self.key_where(asked, key),
            Mode::Menu(menu) => self.key_menu(menu, key),
            Mode::Tags(picker) => {
                let count = self.tag_candidates(&picker).len();
                self.pick(picker, key, count, |p| p, Mode::Tags, Self::choose_tag);
            }
            Mode::ManageProject(src) => self.key_manage_project(src, key),
            Mode::Palette(picker) => self.key_palette(picker, key),
            Mode::AddProject(add) => self.key_add_project(add, key),
            Mode::Manage(manage) => self.key_manage(manage, key),
        }
    }

    /// The comment picker: arrows move, `enter` edits the highlighted comment, `x` asks to delete it, `esc` closes.
    fn key_comments(&mut self, mut picker: CommentPicker, key: KeyEvent) {
        let count = self.task(picker.key).map_or(0, |t| t.comments.len());
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter if picker.sel < count => self.open_editor(Target::Comment(picker.key, picker.sel)),
            KeyCode::Char('x') if picker.sel < count => {
                self.mode = Mode::Confirm(Confirm::DeleteComment(picker.key, picker.sel));
            }
            code => {
                match code {
                    KeyCode::Up => picker.sel = picker.sel.saturating_sub(1),
                    KeyCode::Down => picker.sel = (picker.sel + 1).min(count.saturating_sub(1)),
                    _ => {}
                }
                self.mode = Mode::Comments(picker);
            }
        }
    }

    /// "Which project?": arrows move, typing filters, Enter opens the quick-add field there or moves the
    /// entry there.
    fn key_where(&mut self, asked: WherePicker, key: KeyEvent) {
        let count = self.where_candidates(&asked.picker, asked.then).len();
        self.pick(asked, key, count, |a| &mut a.picker, Mode::Where, Self::choose_where);
    }

    /// Hands `key` to the picker in `dialog`, which shows `count` rows: the dialog stays open, as `mode` makes it,
    /// unless the key closed it, and `enter` chooses the highlighted row.
    pub(super) fn pick<T>(
        &mut self,
        mut dialog: T,
        key: KeyEvent,
        count: usize,
        picker: fn(&mut T) -> &mut Picker,
        mode: fn(T) -> Mode,
        choose: fn(&mut Self, usize),
    ) {
        let picker = picker(&mut dialog);
        let outcome = picker.key(key, count);
        let sel = picker.sel;
        if outcome != PickerKey::Close {
            self.mode = mode(dialog);
        }
        if outcome == PickerKey::Choose {
            choose(self, sel);
        }
    }

    /// Leaves the text editor without saving.
    pub fn discard_edit(&mut self) {
        if matches!(self.mode, Mode::Edit(_)) {
            self.mode = Mode::Normal;
            self.notify("changes discarded");
        }
    }

    /// Moves the selected card to the next column in the direction of `step`.
    pub fn move_card(&mut self, step: Step) {
        if self.tab != Tab::Board {
            return;
        }
        if let (Some(id), Some(status)) = (self.selected, self.neighbour_status(step)) {
            self.set_status(id, status);
        }
    }

    /// The tags the dropdown offers: "all tags" first (`None`), then the tags of what the tab shows
    /// in the scope (tasks on the board and the report, notes, decisions, or anything on the
    /// calendar) whose name contains every word of its search.
    pub fn tag_candidates(&self, picker: &Picker) -> Vec<Option<String>> {
        let tags = self.all_tags().into_iter().filter(|t| self.tab_uses(t)).filter(|t| picker.matches(t)).map(Some);
        std::iter::once(None).filter(|_| picker.is_empty()).chain(tags).collect()
    }

    /// Whether something the tab shows carries `tag`.
    fn tab_uses(&self, tag: &str) -> bool {
        let uses = self.tag_uses(tag);
        let kind = match self.tab {
            Tab::Calendar => return uses.iter().any(|&n| n > 0),
            Tab::Report => Kind::Task,
            tab => tab.kind().unwrap_or(Kind::Task),
        };
        uses[Kind::ALL.iter().position(|&k| k == kind).unwrap_or(0)] > 0
    }

    /// How many tasks, notes and decisions in the scope carry `tag`, in that order: where a tag
    /// that isn't on any card is.
    pub fn tag_uses(&self, tag: &str) -> [usize; Kind::ALL.len()] {
        let mut uses = [0; Kind::ALL.len()];
        for task in self.scoped().flat_map(|(_, brain)| &brain.tasks).filter(|t| t.tags.iter().any(|t| t == tag)) {
            uses[task.kind.position()] += 1;
        }
        uses
    }

    /// Opens the tag dropdown, on the first tag the search filters by.
    pub fn open_tags(&mut self) {
        let picker = Picker::default();
        let chosen = self.search_tags();
        let sel = self.tag_candidates(&picker).iter().position(|t| t.as_ref().is_some_and(|t| chosen.contains(t)));
        self.mode = Mode::Tags(Picker { sel: sel.unwrap_or(0), ..picker });
    }

    /// Runs the dropdown's candidate at `index` on the search box, which holds every filter: a tag
    /// is added as `#tag`, or taken out if it's there; "all tags" takes every `#tag` out, keeping the
    /// words. The dropdown stays open, so several tags can be picked; `esc` or a click outside
    /// closes it.
    pub fn choose_tag(&mut self, index: usize) {
        let Mode::Tags(picker) = &mut self.mode else { return };
        picker.sel = index;
        let Mode::Tags(picker) = &self.mode else { return };
        let Some(candidate) = self.tag_candidates(picker).into_iter().nth(index) else { return };
        let words = self.search.text().split_whitespace().map(str::to_string);
        let text: Vec<String> = match candidate {
            None => words.filter(|w| !w.starts_with('#')).collect(),
            Some(tag) => {
                let term = format!("#{tag}");
                let mut words: Vec<String> = words.collect();
                if words.iter().any(|w| w.eq_ignore_ascii_case(&term)) {
                    words.retain(|w| !w.eq_ignore_ascii_case(&term));
                } else {
                    words.push(term);
                }
                words
            }
        };
        self.search = LineInput::new(&text.join(" "));
        self.keep_selection();
    }

    /// Typing into the search box; `enter` or `esc` stop typing, keeping the filter.
    fn key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {}
            _ => {
                self.search.on_key(key);
                self.mode = Mode::Search;
            }
        }
        self.keep_selection();
    }

    /// The new / edit dialog: `tab` switches fields, `enter` saves, `esc` cancels.
    fn key_form(&mut self, mut form: Box<Form>, key: KeyEvent) {
        let links = form.field == Field::Links;
        let picking = form.field == Field::Status;
        match key.code {
            KeyCode::Esc => {}
            // In the project field, its dropdown takes every key but tab: enter, the arrows, what's typed.
            code if form.field == Field::Project && !matches!(code, KeyCode::Tab | KeyCode::BackTab) => {
                self.key_form_project(form, key);
            }
            // In the related field, its dropdown takes every key but tab: enter checks the highlighted entry.
            code if links && !matches!(code, KeyCode::Tab | KeyCode::BackTab) => {
                let count = self.form_link_options(&form).len();
                let chosen = form.related.key(key, count) == PickerKey::Choose;
                let sel = form.related.sel;
                self.mode = Mode::Form(form);
                if chosen {
                    self.toggle_form_link(sel);
                }
            }
            KeyCode::Left | KeyCode::Right if picking => {
                if let Some(status) = form.status() {
                    let at = COLUMNS.iter().position(|&s| s == status).unwrap_or(0);
                    let at = match key.code {
                        KeyCode::Left => at.saturating_sub(1),
                        _ => (at + 1).min(COLUMNS.len() - 1),
                    };
                    form.set_status(COLUMNS[at]);
                }
                self.mode = Mode::Form(form);
            }
            KeyCode::Enter => self.submit_form(form),
            KeyCode::Tab | KeyCode::Down => {
                form.tab_to(form.next_field());
                self.mode = Mode::Form(form);
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.tab_to(form.previous_field());
                self.mode = Mode::Form(form);
            }
            _ => {
                if let Some(input) = form.active_mut() {
                    input.on_key(key);
                }
                self.mode = Mode::Form(form);
            }
        }
    }

    /// The form's project field: arrows move in its dropdown, typing filters it, enter picks the
    /// highlighted brain.
    fn key_form_project(&mut self, mut form: Box<Form>, key: KeyEvent) {
        let count = self.form_projects(&form).len();
        let Some(picker) = &mut form.project else { return self.mode = Mode::Form(form) };
        let outcome = picker.key(key, count);
        let sel = picker.sel;
        self.mode = Mode::Form(form);
        if outcome == PickerKey::Choose {
            self.choose_form_project(sel);
        }
    }

    /// Runs `key` for a button: a button acts on the board, so it first leaves the search box (whose
    /// text stays as the filter) or a column's quick-add field, instead of typing its key into them:
    /// clicking the tag filter while searching would type `#`. Dialogs keep their keys, since their
    /// buttons are theirs.
    pub fn press(&mut self, key: KeyEvent) {
        if matches!(self.mode, Mode::Search | Mode::QuickAdd(_)) {
            self.mode = Mode::Normal;
        }
        self.on_key(key);
    }

    /// Types into the search box, from a click on it.
    pub fn focus_search(&mut self) {
        if matches!(self.mode, Mode::Normal) {
            self.mode = Mode::Search;
        }
    }

    /// Types into this field of the form, from a click on it.
    pub fn focus_field(&mut self, field: Field) {
        if let Mode::Form(form) = &mut self.mode {
            form.go_to(field);
        }
    }

    /// Where typed text goes now, if anywhere.
    fn typing(&mut self) -> Option<Typing<'_>> {
        match &mut self.mode {
            Mode::Search => Some(Typing::Line(&mut self.search)),
            Mode::Form(form) => form.active_mut().map(Typing::Line),
            Mode::QuickAdd(add) => Some(Typing::Line(&mut add.title)),
            Mode::Links(picker) => Some(Typing::Line(&mut picker.rows.search)),
            Mode::Scopes(picker) | Mode::Tags(picker) | Mode::Where(WherePicker { picker, .. }) => {
                Some(Typing::Line(&mut picker.search))
            }
            Mode::Palette(picker) => Some(Typing::Line(&mut picker.search)),
            Mode::AddProject(add) => Some(Typing::Line(&mut add.picker.search)),
            Mode::Manage(manage) => manage.picker_mut().map(|picker| Typing::Line(&mut picker.search)),
            Mode::Edit(edit) => Some(Typing::Text(&mut edit.editor)),
            Mode::Normal | Mode::Confirm(_) | Mode::ManageProject(_) | Mode::Menu(_) | Mode::Comments(_) => None,
        }
    }

    /// A mouse press or drag in the field being typed into, at character `col` of line `row`
    /// (one-line fields have only row 0).
    pub fn point(&mut self, row: usize, col: usize, how: Pointer) {
        match self.typing() {
            Some(Typing::Line(input)) => input.point(col, how),
            Some(Typing::Text(editor)) => editor.point((row, col), how),
            None => {}
        }
    }

    /// Selects all the text of the field being typed into.
    pub fn select_all(&mut self) {
        match self.typing() {
            Some(Typing::Line(input)) => input.select_all(),
            Some(Typing::Text(editor)) => editor.select_all(),
            None => {}
        }
    }

    /// The text selected in the field being typed into, to copy.
    pub fn selected_text(&self) -> Option<String> {
        match &self.mode {
            Mode::Search => self.search.selected_text().map(str::to_string),
            Mode::Form(form) => form.active().and_then(LineInput::selected_text).map(str::to_string),
            Mode::QuickAdd(add) => add.title.selected_text().map(str::to_string),
            Mode::Links(picker) => picker.rows.search.selected_text().map(str::to_string),
            Mode::Scopes(picker) | Mode::Tags(picker) | Mode::Where(WherePicker { picker, .. }) => {
                picker.search.selected_text().map(str::to_string)
            }
            Mode::Palette(picker) => picker.search.selected_text().map(str::to_string),
            Mode::AddProject(add) => add.picker.search.selected_text().map(str::to_string),
            Mode::Manage(manage) => match manage {
                Manage::RenameProject { name: picker, .. }
                | Manage::RenameRepo { name: picker, .. }
                | Manage::MoveRepo { picker, .. } => picker.search.selected_text().map(str::to_string),
                Manage::Remove { .. } => None,
            },
            Mode::Edit(edit) => edit.editor.selected_text(),
            Mode::Normal | Mode::Confirm(_) | Mode::ManageProject(_) | Mode::Menu(_) | Mode::Comments(_) => None,
        }
    }

    /// Removes the selected text from the field being typed into and returns it, to cut.
    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        match self.typing() {
            Some(Typing::Line(input)) => _ = input.delete_selection(),
            Some(Typing::Text(editor)) => _ = editor.delete_selection(),
            None => {}
        }
        self.keep_selection();
        Some(text)
    }

    /// Types `text` into the field being typed into, in place of the selection, to paste.
    pub fn paste(&mut self, text: &str) {
        match self.typing() {
            Some(Typing::Line(input)) => _ = input.insert(text),
            Some(Typing::Text(editor)) => editor.insert(text),
            None => {}
        }
        self.keep_selection();
    }
}
