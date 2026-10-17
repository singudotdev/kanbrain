//! What each key does in each dialog, handing the rest to [`super::keys`]. Buttons in the window run the same keys
//! ([`Board::on_key`]), so a button and its shortcut can't differ.
//! Also typing into whichever field has the keys, for the mouse and the clipboard.

use super::nav::Step;
use super::{
    Board, COLUMNS, CommentPicker, Edit, Field, Form, Manage, Mode, Picker, PickerKey, Tab, Target, Telling,
    WherePicker,
};
use crate::text::editor::{LineInput, Outcome, Pointer, TextEditor};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::text::merge::{Merge, Pick};
use kanbrain_core::model::{Kind, counted};

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
        // A ctrl+letter is a key only in the editor (`ctrl+d`; the clipboard's are the window's) and in a form showing a
        // title `kb` gave meanwhile (`ctrl+t` takes it). Anywhere else it'd be taken for the letter alone: a second
        // ctrl+d after discarding an edit would move the selected task to Done, and one in the remove dialog would
        // delete its tasks.
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl
            && key.code == KeyCode::Char('t')
            && matches!(&self.mode, Mode::Form(form) if form.theirs_title.is_some())
        {
            return self.take_theirs_title();
        }
        if ctrl && matches!(key.code, KeyCode::Char(_)) && !matches!(self.mode, Mode::Edit(_)) {
            return;
        }
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
                let (ctrl, back) = (key.modifiers.contains(KeyModifiers::CONTROL), edit.back);
                if ctrl && key.code == KeyCode::Char('d') {
                    // Discard changes; the mode is already taken.
                    self.notify("changes discarded");
                } else if edit.merge.is_some() {
                    return self.key_merge(edit, key);
                } else if edit.editor.on_key(key) == Outcome::Save || (ctrl && key.code == KeyCode::Enter) {
                    // A save that stops puts the editor back.
                    self.save_edit(edit, Telling::Ask);
                } else {
                    self.mode = Mode::Edit(edit);
                    return;
                }
                self.back_to_manage(back);
            }
            Mode::Confirm(confirm) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.confirm(confirm),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.mode = Mode::Confirm(confirm),
            },
            // Choosing a row links or unlinks it, and the picker stays open for the next.
            Mode::Links(picker) => {
                let count = self.link_candidates(&picker).len();
                self.pick(picker, key, count, |p| &mut p.rows, Mode::Links, Self::toggle_link);
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
            // `enter` tells the agent, `ctrl+enter` just saves; `esc` goes back to what was saving.
            Mode::Tell(mut tell) => match key.code {
                KeyCode::Esc => self.mode = tell.pending.into_mode(),
                KeyCode::Enter => self.answer_tell(*tell, key.modifiers.contains(KeyModifiers::CONTROL)),
                _ => {
                    tell.note.on_key(key);
                    self.mode = Mode::Tell(tell);
                }
            },
        }
    }

    /// The comment picker: arrows move, `enter` edits the highlighted comment, `x` asks to delete it, `esc` closes.
    fn key_comments(&mut self, mut picker: CommentPicker, key: KeyEvent) {
        let count = self.task(picker.key).map_or(0, |t| t.comments.len());
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter if picker.sel < count => self.open_editor(Target::Comment(picker.key, picker.sel)),
            KeyCode::Char('x') if picker.sel < count => {
                self.ask_delete_comment(picker.key, picker.sel);
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

    /// The conflicts a save stopped over, in place of the editor's text: `tab` and the arrows move between them, `m`,
    /// `t` and `b` pick yours, theirs or both, `enter` puts the merged text in the editor once each has a pick, and
    /// `esc` takes yours where nothing's picked and puts that in the editor.
    fn key_merge(&mut self, edit: Edit, key: KeyEvent) {
        self.mode = Mode::Edit(edit);
        // As the buttons name them, in either case (shift, caps lock); a ctrl+letter is the window's or nothing.
        let pick = match key.code {
            KeyCode::Char(_) if key.modifiers.contains(KeyModifiers::CONTROL) => return,
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                'm' => Pick::Mine,
                't' => Pick::Theirs,
                'b' => Pick::Both,
                _ => return,
            },
            KeyCode::Tab | KeyCode::Down => return self.go_to_conflict(1),
            KeyCode::BackTab | KeyCode::Up => return self.go_to_conflict(-1),
            KeyCode::Enter => return self.use_merge(),
            KeyCode::Esc => return self.keep_mine(),
            _ => return,
        };
        if let Some(merge) = self.merge_mut() {
            merge.pick(pick);
        }
    }

    /// The conflicts shown in the editor's place, if any.
    fn merge_mut(&mut self) -> Option<&mut Merge> {
        match &mut self.mode {
            Mode::Edit(edit) => edit.merge.as_mut(),
            _ => None,
        }
    }

    /// Moves to the next conflict (`step` 1) or the one before (`-1`).
    fn go_to_conflict(&mut self, step: isize) {
        if let Some(merge) = self.merge_mut() {
            merge.go(step);
        }
    }

    /// Picks `pick` for conflict `index`, from a click on one of its sides.
    pub fn pick_conflict(&mut self, index: usize, pick: Pick) {
        if let Some(merge) = self.merge_mut() {
            merge.pick_at(index, pick);
        }
    }

    /// Puts the merged text in the editor, once every conflict has a pick, to look over and save.
    pub fn use_merge(&mut self) {
        let Mode::Edit(edit) = &mut self.mode else { return };
        let Some(merge) = &edit.merge else { return };
        let Some(text) = merge.text() else {
            let left = merge.conflicts().filter(|c| c.pick.is_none()).count();
            return self.notify(format!(
                "{} to pick yet: yours (m), theirs (t) or both (b)",
                counted(left, "conflict", "conflicts")
            ));
        };
        // All of theirs is the text as it is: nothing to save.
        let said =
            if text == merge.theirs { "merged: it's theirs as it is, esc closes" } else { "merged: esc saves it" };
        // What a save merges with from now on is what the conflicts were picked over.
        if let Some(merge) = edit.merge.take() {
            edit.base = merge.theirs;
        }
        edit.editor = TextEditor::new(&text);
        edit.editor.modified = true;
        self.notify(said);
    }

    /// Takes yours in every conflict not picked yet, and puts the merged text in the editor: what `kb` changed
    /// elsewhere stays.
    pub fn keep_mine(&mut self) {
        if let Some(merge) = self.merge_mut() {
            merge.pick_rest(Pick::Mine);
        }
        self.use_merge();
    }

    /// Leaves the text editor without saving.
    pub fn discard_edit(&mut self) {
        if let Mode::Edit(edit) = &self.mode {
            let back = edit.back;
            self.mode = Mode::Normal;
            self.notify("changes discarded");
            self.back_to_manage(back);
        }
    }

    /// Reopens Manage project on `back` once the editor it opened has closed, unless that opened something else.
    pub(super) fn back_to_manage(&mut self, back: Option<usize>) {
        if let (Some(src), Mode::Normal) = (back, &self.mode)
            && src < self.sources.len()
        {
            self.mode = Mode::ManageProject(src);
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
        uses[kind.position()] > 0
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
        // In the tags field, with tags to pick: the arrows move in its list, enter picks the highlighted one.
        if form.field == Field::Tags && self.key_form_tags(&mut form, key) {
            self.mode = Mode::Form(form);
            return;
        }
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
            KeyCode::Enter => self.submit_form(form, Telling::Ask),
            KeyCode::Tab | KeyCode::Down => {
                form.tab_to(form.next_field());
                self.mode = Mode::Form(form);
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.tab_to(form.previous_field());
                self.mode = Mode::Form(form);
            }
            _ => {
                let changed = form.active_mut().is_some_and(|input| input.on_key(key));
                // Not as the cursor merely moves.
                if changed && form.field == Field::Tags {
                    form.split_tags();
                    form.tag_sel = self.tag_highlight(&form);
                }
                self.mode = Mode::Form(form);
            }
        }
    }

    /// What the tags field's list highlights for what's typed, for enter to pick: the first tag starting with the
    /// tag being typed. One only containing it isn't highlighted, nor a whole one typed: enter then saves, as a new
    /// tag typed must.
    fn tag_highlight(&self, form: &Form) -> Option<String> {
        let typed = form.typed_tag();
        let tags = self.form_tags(form).1;
        let partial = !typed.is_empty() && !form.typed_tag_held() && !tags.iter().any(|(tag, _)| *tag == typed);
        partial.then(|| tags.into_iter().map(|(tag, _)| tag).find(|tag| tag.starts_with(typed.as_str()))).flatten()
    }

    /// After the tags field's text changed other than by a key (pasted, cut): the tags before a separator become
    /// chips and its list's highlight follows, as typing it would.
    fn retag(&mut self) {
        let Mode::Form(form) = &mut self.mode else { return };
        if form.field != Field::Tags {
            return;
        }
        form.split_tags();
        let Mode::Form(form) = &self.mode else { return };
        let sel = self.tag_highlight(form);
        if let Mode::Form(form) = &mut self.mode {
            form.tag_sel = sel;
        }
    }

    /// The tags field's list: `↓` and `↑` move its highlight (`↑` off the first leaves it, so enter saves again),
    /// `enter` picks the highlighted tag. Returns whether the list took `key`; it doesn't when there's nothing to
    /// pick, or nothing highlighted for `↑` and `enter`, which then switch fields and save as elsewhere.
    fn key_form_tags(&mut self, form: &mut Form, key: KeyEvent) -> bool {
        let (_, mut tags) = self.form_tags(form);
        // A tag a reload took out of the list is highlighted no more: enter saves.
        let sel = form.tag_sel.as_ref().and_then(|sel| tags.iter().position(|(tag, _)| tag == sel));
        let at = match (key.code, sel) {
            (_, _) if tags.is_empty() => return false,
            (KeyCode::Down, None) => Some(0),
            (KeyCode::Down, Some(sel)) => Some((sel + 1).min(tags.len() - 1)),
            (KeyCode::Up, Some(sel)) => sel.checked_sub(1),
            (KeyCode::Enter, Some(sel)) => {
                form.put_tag(&tags[sel].0);
                return true;
            }
            _ => return false,
        };
        form.tag_sel = at.map(|at| tags.swap_remove(at).0);
        true
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

    /// Whether a field or the editor is being typed into, and so shows a blinking cursor.
    pub fn is_typing(&mut self) -> bool {
        self.typing().is_some()
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
            // The conflicts show in the text's place: nothing types into the text, unseen.
            Mode::Edit(edit) => edit.merge.is_none().then_some(Typing::Text(&mut edit.editor)),
            Mode::Tell(tell) => Some(Typing::Line(&mut tell.note)),
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
            Mode::Tell(tell) => tell.note.selected_text().map(str::to_string),
            Mode::Manage(manage) => match manage {
                Manage::RenameProject { name: picker, .. }
                | Manage::RenameRepo { name: picker, .. }
                | Manage::MoveRepo { picker, .. } => picker.search.selected_text().map(str::to_string),
                Manage::Remove { .. } => None,
            },
            Mode::Edit(edit) => edit.merge.is_none().then(|| edit.editor.selected_text()).flatten(),
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
        self.retag();
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
        self.retag();
        self.keep_selection();
    }
}
