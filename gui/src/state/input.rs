//! What each key does: on the board, the notes and decisions lists and the report, and in each dialog.
//! Buttons in the window run the same keys ([`Board::on_key`]), so a button and its shortcut can't differ.
//! Also typing into whichever field has the keys, for the mouse and the clipboard.

use super::nav::Step;
use super::{
    Board, COLUMNS, Confirm, Field, Form, Mode, NextPicker, Picker, PickerKey, Request, Tab, Target, WherePicker,
};
use crate::text::editor::{LineInput, Outcome, Pointer, TextEditor};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::brain::Group;
use kanbrain_core::model::{Kind, Status};
use kanbrain_core::period::{Period, Unit};

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
                if edit.editor.on_key(key) == Outcome::Save || (ctrl && key.code == KeyCode::Enter) {
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
            Mode::Links(mut picker) => {
                let count = self.link_candidates(&picker).len();
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Up => picker.sel = picker.sel.saturating_sub(1),
                    KeyCode::Down => picker.sel = (picker.sel + 1).min(count.saturating_sub(1)),
                    KeyCode::Enter => {
                        let sel = picker.sel;
                        self.mode = Mode::Links(picker);
                        self.toggle_link(sel);
                        return;
                    }
                    _ => {
                        if picker.search.on_key(key) {
                            picker.sel = 0;
                        }
                    }
                }
                if key.code != KeyCode::Esc {
                    self.mode = Mode::Links(picker);
                }
            }
            Mode::Scopes(mut picker) => {
                let count = self.scope_candidates(&picker).len();
                let outcome = picker.key(key, count);
                let sel = picker.sel;
                if outcome != PickerKey::Close {
                    self.mode = Mode::Scopes(picker);
                }
                if outcome == PickerKey::Choose {
                    self.choose_scope(sel);
                }
            }
            Mode::Where(asked) => self.key_where(asked, key),
            Mode::Menu(menu) => self.key_menu(menu, key),
            Mode::Tags(mut picker) => {
                let count = self.tag_candidates(&picker).len();
                let outcome = picker.key(key, count);
                let sel = picker.sel;
                if outcome != PickerKey::Close {
                    self.mode = Mode::Tags(picker);
                }
                if outcome == PickerKey::Choose {
                    self.choose_tag(sel);
                }
            }
            Mode::Next(picker) => self.key_next(picker, key),
            Mode::ProjectInfo(src) => {
                if !matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('I' | 'q')) {
                    self.mode = Mode::ProjectInfo(src);
                }
            }
            Mode::Palette(picker) => self.key_palette(picker, key),
            Mode::AddProject(add) => self.key_add_project(add, key),
        }
    }

    /// "Which project?": arrows move, typing filters, Enter opens the quick-add field there or moves the
    /// entry there.
    fn key_where(&mut self, mut asked: WherePicker, key: KeyEvent) {
        let count = self.where_candidates(&asked.picker, asked.then).len();
        let outcome = asked.picker.key(key, count);
        let sel = asked.picker.sel;
        if outcome != PickerKey::Close {
            self.mode = Mode::Where(asked);
        }
        if outcome == PickerKey::Choose {
            self.choose_where(sel);
        }
    }

    /// "What's next?": arrows move, typing filters the tasks, enter runs the highlighted row, `esc` is
    /// nothing for now. `n` is a new task while nothing is searched; once something is, it's a letter.
    fn key_next(&mut self, mut picker: NextPicker, key: KeyEvent) {
        let count = self.next_choices(&picker).len();
        match key.code {
            KeyCode::Up => picker.sel = picker.sel.saturating_sub(1),
            KeyCode::Down => picker.sel = (picker.sel + 1).min(count - 1),
            KeyCode::Enter => picker.sel = picker.sel.min(count - 1),
            KeyCode::Char('n') if picker.search.is_empty() => picker.sel = count - 2,
            KeyCode::Esc => picker.sel = count - 1,
            _ => {
                if picker.search.on_key(key) {
                    picker.sel = 0;
                }
                self.mode = Mode::Next(picker);
                return;
            }
        }
        let sel = picker.sel;
        self.mode = Mode::Next(picker);
        if matches!(key.code, KeyCode::Enter | KeyCode::Char('n') | KeyCode::Esc) {
            self.choose_next(sel);
        }
    }

    /// Leaves the text editor without saving.
    pub fn discard_edit(&mut self) {
        if matches!(self.mode, Mode::Edit(_)) {
            self.mode = Mode::Normal;
            self.notify("changes discarded");
        }
    }

    /// The board, lists and report: keys that work on every tab, then the tab's own.
    fn key_normal(&mut self, key: KeyEvent) {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char(c @ '1'..='5') => self.switch_tab(Tab::ALL[c as usize - '1' as usize]),
            KeyCode::Tab => {
                let next = Tab::ALL.iter().position(|&t| t == self.tab).map_or(0, |i| (i + 1) % Tab::ALL.len());
                self.switch_tab(Tab::ALL[next]);
            }
            KeyCode::Char('/') => self.mode = Mode::Search,
            KeyCode::Char('#') => self.open_tags(),
            KeyCode::Char('?') => self.open_palette(),
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('Q') => self.park_and_quit(),
            KeyCode::Char('R') => self.reload_from_disk(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('P') => self.open_scopes(),
            KeyCode::Char('I') => self.open_project_info(),
            KeyCode::Char('A') => self.request = Some(Request::Folder),
            KeyCode::Esc if self.drawer => self.drawer = false,
            KeyCode::Esc => {
                self.search.clear();
                self.keep_selection();
            }
            // Before the report's and calendar's own keys, so that their "+ New task" button works too.
            KeyCode::Char('n') => self.new_entry(Status::Todo),
            _ if self.tab == Tab::Report => self.key_report(key),
            _ if self.tab == Tab::Calendar => self.key_calendar(key),
            KeyCode::Left if !shift => self.step(Step::Left),
            KeyCode::Right if !shift => self.step(Step::Right),
            KeyCode::Up => self.step(Step::Up),
            KeyCode::Down => self.step(Step::Down),
            KeyCode::Left => self.move_card(Step::Left),
            KeyCode::Right => self.move_card(Step::Right),
            KeyCode::Char('v') if self.tab == Tab::Board => self.toggle_layout(),
            // Always To do from the keyboard: the `+` on a column or section is how to add somewhere else.
            KeyCode::Char('a') if self.tab == Tab::Board => self.quick_add(Status::Todo),
            KeyCode::Char('f') => {
                self.all_done = !self.all_done;
                self.keep_selection();
            }
            KeyCode::Char('w') => self.next_waiting(),
            _ => self.key_on_selected(key),
        }
    }

    /// Keys that act on the selected entry.
    fn key_on_selected(&mut self, key: KeyEvent) {
        let Some(id) = self.selected else { return };
        let status = match key.code {
            KeyCode::Char('s') => Some(Status::Doing),
            KeyCode::Char('p') => Some(Status::Parked),
            KeyCode::Char('d') => Some(Status::Done),
            KeyCode::Char('t') => Some(Status::Todo),
            _ => None,
        };
        if let Some(status) = status {
            self.set_status(id, status);
            return;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.drawer = !self.drawer,
            KeyCode::Char('e') => self.edit_selected(),
            KeyCode::Char('m') => self.open_editor(Target::Description(id)),
            KeyCode::Char('c') => self.open_editor(Target::NewComment(id)),
            KeyCode::Char('r') => self.open_links(),
            KeyCode::Char('o') => self.open_file(),
            KeyCode::Char('x') => self.mode = Mode::Confirm(Confirm::Delete(id)),
            KeyCode::Char('M') => self.ask_move(),
            KeyCode::Char('y') => self.accept_selected(),
            KeyCode::Char('j') => self.reject_selected(),
            _ => {}
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

    /// The report tab: which period, and how it's grouped and filtered.
    fn key_report(&mut self, key: KeyEvent) {
        let period = self.report.period;
        match key.code {
            KeyCode::Left => self.report.period = period.prev(),
            KeyCode::Right => self.report.period = period.next(),
            KeyCode::Char('h') => self.report.period = period.with_unit(Unit::Hour),
            KeyCode::Char('d') => self.report.period = period.with_unit(Unit::Day),
            KeyCode::Char('w') => self.report.period = period.with_unit(Unit::Week),
            KeyCode::Char('m') => self.report.period = period.with_unit(Unit::Month),
            KeyCode::Char('t') => self.report.period = Period::current(period.unit),
            KeyCode::Char('g') => {
                self.report.group = if self.report.group == Group::Task { Group::Tag } else { Group::Task };
            }
            KeyCode::Char('e') => self.export_report(),
            _ => {}
        }
    }

    /// The calendar tab: which period and which interactions, and selecting the entries shown.
    fn key_calendar(&mut self, key: KeyEvent) {
        let period = self.calendar.period;
        match key.code {
            KeyCode::Left => self.calendar.period = period.prev(),
            KeyCode::Right => self.calendar.period = period.next(),
            KeyCode::Char('d') => self.calendar.period = period.with_unit(Unit::Day),
            KeyCode::Char('w') => self.calendar.period = period.with_unit(Unit::Week),
            KeyCode::Char('m') => self.calendar.period = period.with_unit(Unit::Month),
            KeyCode::Char('t') => self.calendar.period = Period::current(period.unit),
            KeyCode::Char('i') => self.calendar.show = self.calendar.show.next(),
            KeyCode::Up => self.step(Step::Up),
            KeyCode::Down => self.step(Step::Down),
            KeyCode::Enter | KeyCode::Char(' ') if self.selected.is_some() => self.drawer = !self.drawer,
            _ => {}
        }
        self.keep_selection();
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
    pub fn tag_uses(&self, tag: &str) -> [usize; 3] {
        let mut uses = [0; 3];
        for task in self.scoped().flat_map(|(_, brain)| &brain.tasks).filter(|t| t.tags.iter().any(|t| t == tag)) {
            uses[Kind::ALL.iter().position(|&k| k == task.kind).unwrap_or(0)] += 1;
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
            code if form.field == Field::Project && code != KeyCode::Tab => self.key_form_project(form, key),
            // In the related field, enter checks the highlighted entry and arrows move in its dropdown.
            KeyCode::Enter if links => {
                let sel = form.links_sel;
                self.mode = Mode::Form(form);
                self.toggle_form_link(sel);
            }
            KeyCode::Up if links => {
                form.links_sel = form.links_sel.saturating_sub(1);
                self.mode = Mode::Form(form);
            }
            KeyCode::Down if links => {
                let count = self.form_link_options(&form).len();
                form.links_sel = (form.links_sel + 1).min(count.saturating_sub(1));
                self.mode = Mode::Form(form);
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
                form.go_to(form.next_field());
                self.mode = Mode::Form(form);
            }
            KeyCode::Up => {
                form.go_to(form.previous_field());
                self.mode = Mode::Form(form);
            }
            _ if links => {
                if form.links_search.on_key(key) {
                    form.links_sel = 0;
                }
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
            Mode::Links(picker) => Some(Typing::Line(&mut picker.search)),
            Mode::Scopes(picker) | Mode::Tags(picker) | Mode::Where(WherePicker { picker, .. }) => {
                Some(Typing::Line(&mut picker.search))
            }
            Mode::Next(picker) => Some(Typing::Line(&mut picker.search)),
            Mode::Palette(picker) => Some(Typing::Line(&mut picker.search)),
            Mode::AddProject(add) => Some(Typing::Line(&mut add.picker.search)),
            Mode::Edit(edit) => Some(Typing::Text(&mut edit.editor)),
            Mode::Normal | Mode::Confirm(_) | Mode::ProjectInfo(_) | Mode::Menu(_) => None,
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
            Mode::Links(picker) => picker.search.selected_text().map(str::to_string),
            Mode::Scopes(picker) | Mode::Tags(picker) | Mode::Where(WherePicker { picker, .. }) => {
                picker.search.selected_text().map(str::to_string)
            }
            Mode::Next(picker) => picker.search.selected_text().map(str::to_string),
            Mode::Palette(picker) => picker.search.selected_text().map(str::to_string),
            Mode::AddProject(add) => add.picker.search.selected_text().map(str::to_string),
            Mode::Edit(edit) => edit.editor.selected_text(),
            Mode::Normal | Mode::Confirm(_) | Mode::ProjectInfo(_) | Mode::Menu(_) => None,
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
