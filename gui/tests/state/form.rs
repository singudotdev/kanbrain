//! The new / edit form: its column, tags held as chips and picked from its list, related entries, and an edit saving
//! only what was changed in it.

use super::*;

#[test]
fn the_form_picks_a_new_tasks_column() {
    let mut f = Fixture::new(&["a"]);
    f.board.new_entry(Status::Parked);
    let Mode::Form(form) = &f.board.mode else { panic!("a column's + opens the form") };
    assert_eq!(form.status(), Some(Status::Parked), "with that column picked");
    f.type_text("picked");
    for _ in 0..3 {
        f.press(KeyCode::Tab);
    }
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.field, Field::Status, "title, tags, related, then the column");
    f.press(KeyCode::Right);
    f.press(KeyCode::Right);
    f.press(KeyCode::Right);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.status(), Some(Status::Done), "right goes on to Done, and stops there");
    f.press(KeyCode::Left);
    f.press(KeyCode::Enter);
    assert_eq!(f.status(2), Status::Parked);
    // Doing, while one of yours runs: it stays in To do, and the status bar says why.
    f.board.set_status(k(1), Status::Doing);
    f.board.new_entry(Status::Doing);
    f.type_text("second");
    f.press(KeyCode::Enter);
    assert_eq!(f.status(3), Status::Todo);
    let said = f.board.message().unwrap();
    assert!(said.starts_with("created #3, in To do: #1 is in doing"), "{said}");
    f.board.switch_tab(Tab::Notes);
    f.press(KeyCode::Char('n'));
    let Mode::Form(form) = &f.board.mode else { panic!("n opens a note's form") };
    assert_eq!(form.status(), None, "notes have no column");
    assert_eq!(form.previous_field(), Field::Links, "so tab skips it");
}

#[test]
fn the_form_creates_a_tagged_task_and_edits_it() {
    let mut f = Fixture::new(&[]);
    f.press(KeyCode::Char('n'));
    f.type_text("Fix login");
    f.press(KeyCode::Tab);
    f.type_text("prod, #Auth");
    f.press(KeyCode::Enter);
    let task = f.board.selected_task().unwrap();
    assert_eq!((task.title.as_str(), task.tags.clone()), ("Fix login", vec!["prod".to_string(), "auth".to_string()]));
    f.press(KeyCode::Char('e'));
    (0..5).for_each(|_| f.press(KeyCode::Backspace));
    f.type_text("SSO");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix SSO");
    assert_eq!(f.board.message(), Some("#1 saved"));
    let written = std::fs::read_dir(f.dir.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap());
    let written: Vec<_> = written.collect();
    f.board.message = None;
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), None, "saved as it was: nothing said");
    let again = std::fs::read_dir(f.dir.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap());
    assert_eq!(again.collect::<Vec<_>>(), written, "nor written");
    // Tags in another order are another set of tags to show: saved.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.board.untag_in_form("prod");
    f.board.untag_in_form("auth");
    f.type_text("auth, prod");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "prod"]);

    f.press(KeyCode::Char('n'));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title_missed), "said under the field, kept open");
    f.press(KeyCode::Esc);

    // Tab into the title selects its text, so typing replaces it; shift+tab goes back.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.board.untag_in_form("auth");
    f.board.untag_in_form("prod");
    f.type_text("ops");
    f.press(KeyCode::BackTab);
    f.type_text("Fix SAML");
    f.press(KeyCode::Enter);
    let task = f.on_disk(1);
    assert_eq!((task.title.as_str(), task.tags.as_slice()), ("Fix SAML", &["ops".to_string()][..]));
}

#[test]
fn the_form_suggests_the_tags_of_its_own_kind() {
    use kanbrain_core::brain::NewEntry;
    use kanbrain_core::model::Kind as EntryKind;
    let mut f = Fixture::new(&["login bug"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into()]).unwrap();
    brain
        .add(NewEntry {
            kind: EntryKind::Note,
            title: "card".into(),
            tags: vec!["stripe".into()],
            ..NewEntry::default()
        })
        .unwrap();
    f.press(KeyCode::Char('n'));
    let Mode::Form(form) = &f.board.mode else { panic!("n opens the form") };
    assert_eq!(f.board.form_tags(form), (EntryKind::Task, vec![("auth".to_string(), 1)]), "a task's: no #stripe");
    f.press(KeyCode::Esc);
    f.board.switch_tab(Tab::Notes);
    f.press(KeyCode::Char('e'));
    let Mode::Form(form) = &f.board.mode else { panic!("e edits the note") };
    assert_eq!(
        f.board.form_tags(form),
        (EntryKind::Note, Vec::new()),
        "a note's, but not the #stripe it has: no #auth"
    );
}

#[test]
fn the_form_suggests_only_the_tags_containing_the_one_typed() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "bug".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let tags = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        f.board.form_tags(form).1.into_iter().map(|(tag, _)| tag).collect::<Vec<_>>()
    };
    f.press(KeyCode::Char('n'));
    f.type_text("p");
    assert_eq!(tags(&f), ["auth", "bug", "perf", "prod"], "what the title says filters nothing");
    f.press(KeyCode::Tab);
    f.type_text("#R");
    assert_eq!(tags(&f), ["perf", "prod"], "as saved (no #, lowercase), and contained, as the dropdowns filter");
    f.type_text("o");
    assert_eq!(tags(&f), ["prod"]);
    f.type_text("d, ");
    assert_eq!(tags(&f), ["auth", "bug", "perf", "prod"], "after a separator, #rod is held and a new tag: every one");
    f.type_text("x");
    assert!(tags(&f).is_empty(), "none in use has it");
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!((form.tags.as_slice(), form.tag_input.text()), (["rod".to_string()].as_slice(), "x"));
}

#[test]
fn pasting_or_cutting_in_the_tags_field_highlights_as_typing_does() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "prod".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let sel = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        form.tag_sel.clone()
    };
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.board.paste("pe");
    assert_eq!(sel(&f).as_deref(), Some("perf"), "pasted, the start of a tag is highlighted");
    f.board.select_all();
    assert_eq!(f.board.cut().as_deref(), Some("pe"));
    assert_eq!(sel(&f), None, "cut, nothing is typed: enter saves");
    f.board.paste("prod");
    assert_eq!(sel(&f), None, "a whole tag pasted: enter saves it as typed");
}

#[test]
fn the_tags_highlight_stays_on_its_tag_when_a_reload_adds_one_ahead() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    f.board.brain_mut(0).update(2, "slow page", vec!["perf".into(), "speed".into()]).unwrap();
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.type_text("pe");
    // Another writer (an agent's `kb`) tags a task `ape`, which sorts ahead of `perf` and contains `pe`.
    let mut task = f.on_disk(1);
    task.tags = vec!["ape".into()];
    Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.tags, ["perf"], "enter picks the tag highlighted, not the one now in its row");
}

#[test]
fn the_form_picks_tags_from_its_list() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "prod".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let form = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (f.board.form_tags(form).1, form.tag_sel.clone(), form.tags.join(" "))
    };
    let owned = |tags: &[(&str, usize)]| tags.iter().map(|&(t, n)| (t.to_string(), n)).collect::<Vec<_>>();
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    assert_eq!(form(&f).0, owned(&[("auth", 1), ("perf", 1), ("prod", 2)]), "each with how many use it");
    assert_eq!(form(&f).1, None, "nothing highlighted until a tag is typed");
    f.type_text("prod");
    assert_eq!(form(&f).1, None, "a whole tag typed is taken as it is");
    (0..3).for_each(|_| f.press(KeyCode::Backspace));
    assert_eq!(form(&f).0, owned(&[("perf", 1), ("prod", 2)]));
    assert_eq!(form(&f).1.as_deref(), Some("perf"), "part of one highlights the first");
    f.press(KeyCode::Down);
    f.press(KeyCode::Down);
    assert_eq!(form(&f).1.as_deref(), Some("prod"), "the last stays highlighted");
    f.press(KeyCode::Up);
    f.press(KeyCode::Up);
    assert_eq!(form(&f).1, None, "up off the first leaves the list");
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.field == Field::Tags), "and stays in the field");
    f.press(KeyCode::Down);
    f.press(KeyCode::Enter);
    assert_eq!((form(&f).1, form(&f).2), (None, "perf".to_string()), "enter picks it, ready for the next");
    assert_eq!(form(&f).0, owned(&[("auth", 1), ("prod", 2)]), "what the field holds isn't offered again");
    f.board.choose_form_tag("prod");
    assert_eq!(form(&f).2, "perf prod", "a click picks one too");
    f.board.choose_form_tag("ape");
    assert_eq!(form(&f).2, "perf prod", "but not one the list no longer holds");
    f.type_text("zz");
    assert!(form(&f).0.is_empty(), "nothing to pick");
    f.press(KeyCode::Down);
    assert!(
        matches!(&f.board.mode, Mode::Form(form) if form.field == Field::Links),
        "so down goes on to the next field"
    );
    f.press(KeyCode::BackTab);
    f.press(KeyCode::End);
    f.press(KeyCode::Enter);
    let task = f.board.brain(0).tasks.iter().find(|t| t.title == "x").expect("enter with nothing to pick saves");
    assert_eq!(task.tags, ["perf", "prod", "zz"]);
}

#[test]
fn enter_saves_the_form_unless_the_start_of_a_tag_was_typed() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["build".into(), "project".into()]).unwrap();
    brain.update(2, "slow page", vec!["prod".into()]).unwrap();
    let sel = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        form.tag_sel.clone()
    };
    f.board.selected = Some(Key { src: 0, id: 2 });
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text(" pro");
    assert_eq!(sel(&f).as_deref(), Some("project"), "typing the start of #project does");
    let Mode::Form(form) = &mut f.board.mode else { panic!("the form is open") };
    form.tag_sel = Some("gone".into());
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(2).tags, ["prod", "pro"], "a highlighted tag no longer listed is none: enter saves");

    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.type_text("ui");
    assert_eq!(sel(&f), None, "#build only contains ui");
    f.press(KeyCode::Enter);
    let task = f.board.brain(0).tasks.iter().find(|t| t.title == "x").expect("enter saves a new tag");
    assert_eq!(task.tags, ["ui"]);
}

#[test]
fn a_picked_tag_takes_the_place_of_the_one_typed() {
    let mut form = Form::new(0, Kind::Task, Status::Todo, false);
    form.go_to(Field::Tags);
    form.tag_input = LineInput::new("#Pr");
    form.put_tag("prod");
    assert_eq!((form.tags.as_slice(), form.tag_input.text()), (["prod".to_string()].as_slice(), ""));
    form.tag_input = LineInput::new("au");
    form.put_tag("auth");
    form.put_tag("prod");
    assert_eq!(form.tags, ["prod", "auth"], "after the others, and once");
    form.tag_input = LineInput::new("bug");
    form.tag_input.select_all();
    assert_eq!(form.typed_tag(), "", "selected text isn't a tag being typed");
    form.tag_input = LineInput::new("#Bu");
    assert_eq!(form.typed_tag(), "bu", "as it's saved");
}

#[test]
fn editing_shows_the_tags_held_as_chips_and_takes_them_off_only_with_their_x() {
    let mut f = Fixture::new(&["login bug"]);
    f.board.brain_mut(0).update(1, "login bug", vec!["auth".into(), "prod".into(), "ui".into()]).unwrap();
    let field = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (form.tags.join(" "), form.tag_input.text().to_string())
    };
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    assert_eq!(field(&f), ("auth prod ui".into(), String::new()), "each tag a chip, the box empty to type one");
    f.board.untag_in_form("prod");
    assert_eq!(field(&f).0, "auth ui", "its × takes it off");
    f.type_text("x");
    f.press(KeyCode::Backspace);
    assert_eq!(field(&f), ("auth ui".into(), String::new()), "backspace deletes what's typed first");
    f.press(KeyCode::Backspace);
    assert_eq!(field(&f).0, "auth ui", "and in the empty box, nothing: a chip comes off only with its ×");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "ui"]);
}

#[test]
fn a_separator_typed_or_pasted_turns_the_tags_before_it_into_chips() {
    let mut f = Fixture::new(&["login bug"]);
    f.board.brain_mut(0).update(1, "login bug", vec!["auth".into()]).unwrap();
    let field = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (form.tags.join(" "), form.tag_input.text().to_string())
    };
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("#Bug,");
    assert_eq!(field(&f), ("auth bug".into(), String::new()), "as it's saved: lowercase, without #");
    f.type_text(" auth ");
    assert_eq!(field(&f).0, "auth bug", "a tag held isn't held twice");
    f.board.paste("ui, prod  db");
    assert_eq!(field(&f), ("auth bug ui prod".into(), "db".into()), "pasted, the last is still being typed");
    f.board.paste(", ");
    f.board.paste(",");
    assert_eq!(field(&f), ("auth bug ui prod db".into(), String::new()), "a separator alone adds nothing more");
    f.board.paste("ops");
    f.press(KeyCode::Home);
    f.board.paste("x, ");
    assert_eq!(field(&f), ("auth bug ui prod db x".into(), "ops".into()), "nor empties what follows");
    f.press(KeyCode::Right);
    f.type_text(" ");
    assert_eq!(field(&f), ("auth bug ui prod db x o".into(), "ps".into()), "typed mid-word, it splits it there");
    f.type_text("1");
    assert_eq!(field(&f).1, "1ps", "the cursor where it was in what's left");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "bug", "ui", "prod", "db", "x", "o", "1ps"], "and saved with the chips");
}

#[test]
fn a_tag_held_typed_again_is_neither_highlighted_nor_new_and_enter_saves() {
    let mut f = Fixture::new(&["login bug", "signup"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into()]).unwrap();
    brain.update(2, "signup", vec!["author".into()]).unwrap();
    f.board.selected = Some(Key { src: 0, id: 1 });
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("auth");
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!((form.tag_sel.as_deref(), form.typed_tag_held()), (None, true), "#author isn't picked");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal), "enter saves");
    assert_eq!(f.on_disk(1).tags, ["auth"]);
}

#[test]
fn the_form_relates_entries_from_a_searchable_list() {
    let mut f = Fixture::new(&["login bug", "export", "sso"]);
    f.press(KeyCode::Char('n'));
    f.type_text("hotfix");
    f.press(KeyCode::Tab);
    f.press(KeyCode::Tab);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.field, Field::Links, "title, tags, then related");
    f.type_text("login");
    f.press(KeyCode::Enter);
    let Mode::Form(form) = &f.board.mode else { panic!("enter checks an entry, the form stays") };
    assert_eq!(form.links, [k(1)]);
    f.press(KeyCode::Tab);
    f.press(KeyCode::Enter);
    let hotfix = f.board.brain(0).tasks.iter().find(|t| t.title == "hotfix").map(|t| t.id).unwrap();
    assert!(f.board.brain(0).are_related(hotfix, 1), "created already linked");

    // Editing starts from what it's related to; unchecking unlinks, checking links.
    f.board.select(k(hotfix));
    f.press(KeyCode::Char('e'));
    let Mode::Form(form) = &f.board.mode else { panic!("e edits") };
    assert_eq!(form.links, [k(1)]);
    f.board.unlink_in_form(k(1));
    f.press(KeyCode::Tab);
    f.press(KeyCode::Tab);
    f.type_text("sso");
    f.press(KeyCode::Enter);
    f.board.save_form();
    let brain = f.board.brain(0);
    assert!(!brain.are_related(hotfix, 1) && brain.are_related(hotfix, 3));

    // It can stay empty.
    f.press(KeyCode::Char('n'));
    f.type_text("alone");
    f.press(KeyCode::Enter);
    let alone = f.board.brain(0).tasks.iter().find(|t| t.title == "alone").unwrap();
    assert_eq!(alone.links, Vec::<u32>::new());
}

#[test]
fn an_edit_saves_only_what_was_changed_in_the_form_and_keeps_what_kb_changed_meanwhile() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Other", "Third"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut brain = kb(&f);
    brain.update(1, "Fix login", vec!["auth".into(), "wip".into()]).unwrap();
    brain.link(1, &[3], &[]).unwrap();
    f.board.reload().unwrap();
    let related = |f: &Fixture| {
        let mut keys = f.board.related_keys(k(1));
        keys.sort_by_key(|k| k.id);
        keys
    };
    let tags = |f: &Fixture| f.on_disk(1).tags;

    // Saved untouched while an agent renamed it, retagged it and changed what it's related to: nothing of theirs
    // is undone.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    let mut brain = kb(&f);
    brain.update(1, "Fix the SSO login", vec!["bug".into(), "auth".into()]).unwrap();
    brain.link(1, &[2], &[]).unwrap();
    brain.toggle_link(1, 3).unwrap();
    f.board.reload().unwrap();
    f.board.message = None;
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), None, "nothing of the form's to save: nothing said");
    assert_eq!(f.on_disk(1).title, "Fix the SSO login");
    assert_eq!(tags(&f), ["bug", "auth"], "theirs, in their order");
    assert_eq!(related(&f), [k(2)]);

    // What the form changed is saved, over what the agent did to the same field; the rest of theirs stays.
    f.press(KeyCode::Char('e'));
    let mut brain = kb(&f);
    brain.update(1, "Fix SSO", vec!["bug".into(), "auth".into(), "db".into()]).unwrap();
    brain.link(1, &[3], &[]).unwrap();
    f.board.reload().unwrap();
    f.type_text(" now");
    f.press(KeyCode::Tab);
    f.board.untag_in_form("auth");
    f.type_text("ui");
    // #2 unchecked in the Related field.
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    let at = f.board.form_link_options(form).iter().position(|&(key, _)| key == k(2)).unwrap();
    f.board.toggle_form_link(at);
    // Both changed the title: the first save stops and says what it is now, saving nothing; the second replaces it.
    f.board.message = None;
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Form(_)), "kept open with what was typed");
    assert_eq!(
        f.board.message(),
        Some("#1's title changed meanwhile, to “Fix SSO”: save again to keep yours, ctrl+t to take it")
    );
    assert_eq!((f.on_disk(1).title.as_str(), tags(&f).len()), ("Fix SSO", 3), "nothing saved");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix the SSO login now", "the title typed");
    assert_eq!(tags(&f), ["bug", "ui", "db"], "auth taken out in the form, db added by the agent");
    assert_eq!(related(&f), [k(3)], "#2 unrelated in the form, #3 related by the agent");

    // ctrl+t takes theirs instead, which then saves as the agent left it.
    let ctrl_t = |f: &mut Fixture| f.board.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    f.press(KeyCode::Char('e'));
    ctrl_t(&mut f);
    assert!(
        matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix the SSO login now"),
        "nothing to take"
    );
    f.type_text("!");
    f.press(KeyCode::Tab);
    f.press(KeyCode::End);
    f.type_text(", api");
    kb(&f).update(1, "Fix SSO", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("Fix SSO")));
    // Renamed again before it's taken: the form shows, and takes, the title as it is now.
    kb(&f).update(1, "Fix SSO 2", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("Fix SSO 2")));
    kb(&f).update(1, "Fix SSO", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    ctrl_t(&mut f);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix SSO" && form.theirs_title.is_none()
            && form.field == Field::Title));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).title, "Fix SSO");
    assert_eq!(tags(&f), ["bug", "ui", "db", "api"], "the form's other changes saved");
}

#[test]
fn an_edit_forms_entry_as_opened_follows_its_brain_when_the_list_of_brains_changes() {
    use kanbrain_core::brain::Brain;
    let (central, code) = projects();
    let blog = code.path().join("blog/.brain");
    let mut brain = Brain::open(Store::at(&blog).unwrap()).unwrap();
    let new = |title: &str| kanbrain_core::brain::NewEntry { title: title.into(), ..Default::default() };
    let (second, third) = (brain.add(new("draft")).unwrap(), brain.add(new("photos")).unwrap());
    brain.link(1, &[second], &[]).unwrap();
    let mut board = Board::open(central.path(), None).unwrap();
    let src = |board: &Board| board.sources.iter().position(|s| s.label.as_deref() == Some("blog")).unwrap();
    board.select(Key::new(src(&board), 1));
    press(&mut board, KeyCode::Char('e'));
    // An agent relates it to another entry instead, and a repository listed before blog leaves: blog's index moves.
    let mut brain = Brain::open(Store::at(&blog).unwrap()).unwrap();
    brain.toggle_link(1, second).unwrap();
    brain.link(1, &[third], &[]).unwrap();
    kanbrain_core::projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    press(&mut board, KeyCode::Enter);
    let related: Vec<_> = board.related_keys(Key::new(src(&board), 1)).iter().map(|k| k.id).collect();
    assert_eq!(related, [third], "saved untouched: the agent's links kept");
}

#[test]
fn an_edit_whose_links_fail_isnt_said_saved() {
    let mut f = Fixture::new(&["Fix login"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.type_text(" now");
    // An entry deleted meanwhile, among the related.
    let Mode::Form(form) = &mut f.board.mode else { panic!("the form is open") };
    form.links.push(k(99));
    f.press(KeyCode::Enter);
    assert_ne!(f.board.message(), Some("#1 saved"), "the link failed: that's what's said");
}

#[test]
fn a_new_note_isnt_started_whatever_column_the_form_was_opened_from() {
    let mut f = Fixture::new(&[]);
    f.board.switch_tab(Tab::Notes);
    f.board.new_entry(Status::Doing);
    f.type_text("Webhooks repeat");
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("created #1"), "no status to refuse");
}
