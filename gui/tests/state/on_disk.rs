//! What `kb` changed while a dialog is open: marked and said once, beside what the input's action says, and the
//! status bar's messages.

use super::*;

#[test]
fn a_save_that_stops_over_what_kb_wrote_says_it_once() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    // Saved by the key whose read saw it: the stop says what that read said, and more.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    kb(&f).set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed on disk"));
    f.press(KeyCode::Esc);
    let stop = "#1's description changed meanwhile: pick yours or theirs where you both changed it";
    assert_eq!(f.board.message(), Some(stop));
    f.board.discard_edit();
    // Read by the window's timer just before: the stop goes beside it, so it's taken out the same.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    kb(&f).set_description(1, "the agent's other plan").unwrap();
    f.board.reload().unwrap();
    f.board.input_done();
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some(stop));
    f.board.discard_edit();

    // What else the read saw is still said, and only that.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    let mut brain = kb(&f);
    brain.set_description(1, "the agent's next plan").unwrap();
    brain.set_status(1, Status::Parked).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some(format!("{stop} · #1's status changed on disk").as_str()));
    f.board.discard_edit();

    // The form's title, the same.
    f.press(KeyCode::Char('e'));
    f.type_text(" now");
    kb(&f).update(1, "Fix SSO", vec![]).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's title changed on disk"));
    f.press(KeyCode::Enter);
    let renamed = "#1's title changed meanwhile, to “Fix SSO”: save again to keep yours, ctrl+t to take it";
    assert_eq!(f.board.message(), Some(renamed));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('e'));
    f.type_text(" again");
    kb(&f).update(1, "Fix SSO", vec!["auth".into()]).unwrap();
    kb(&f).update(1, "Fix SSO 500", vec!["auth".into()]).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's title and tags changed on disk"));
    f.press(KeyCode::Enter);
    assert_eq!(
        f.board.message(),
        Some(
            "#1's title changed meanwhile, to “Fix SSO 500”: save again to keep yours, ctrl+t to take it · #1's tags \
             changed on disk"
        )
    );
    f.press(KeyCode::Esc);

    // Another comment put at the one being edited's place.
    kb(&f).add_comment(1, "first").unwrap();
    kb(&f).add_comment(1, "second").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 1));
    f.type_text(", edited");
    kb(&f).delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's comment changed on disk"));
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"));
    f.board.discard_edit();

    // A verdict given meanwhile.
    let decision = kb(&f).propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(decision));
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    kb(&f).resolve(decision, DecisionState::Accepted, "").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some(format!("#{decision}'s verdict changed on disk").as_str()));
    f.press(KeyCode::Esc);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{decision} was accepted meanwhile: save again to reject it anyway, ctrl+d to leave it").as_str()
        )
    );
}

#[test]
fn a_title_kb_changes_again_after_a_stopped_save_is_the_one_saving_keeps_yours_over() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.type_text(" mine");
    kb(&f).update(1, "T1", vec![]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    kb(&f).update(1, "T2", vec![]).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("T2")));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal), "saved, as the form says");
    assert_eq!(f.on_disk(1).title, "feature mine");
}

#[test]
fn an_open_dialog_says_until_closed_that_kb_changed_what_it_edits() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    let mut f = Fixture::new(&["Fix login", "Other"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let reload = |f: &mut Fixture| {
        f.board.message = None;
        f.board.reload().unwrap();
    };
    let marked = |f: &Fixture| match &f.board.mode {
        Mode::Edit(edit) => edit.changed_on_disk,
        Mode::Form(form) => form.changed_on_disk,
        _ => panic!("the dialog closed"),
    };

    // The description editor: a comment, or another entry changed, isn't what it edits.
    f.board.select(k(1));
    f.press(KeyCode::Char('m'));
    kb(&f).add_comment(1, "on it").unwrap();
    kb(&f).set_description(2, "not this one").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    assert_eq!(f.board.message(), None);
    kb(&f).set_description(1, "the agent's plan").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's description changed on disk"));
    // Said once, though it changes again; marked until closed, even once it's back as it was.
    kb(&f).set_description(1, "the agent's newer plan").unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), None);
    kb(&f).set_description(1, "").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), None);
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    assert!(!marked(&f), "a dialog opened again starts unmarked");

    // The task's status counts in any of its editors: the agent may have finished it.
    f.press(KeyCode::Esc);
    f.board.open_editor(Target::NewComment(k(1)));
    kb(&f).set_status(1, Status::Doing).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's status changed on disk"));
    f.press(KeyCode::Esc);

    // A comment's editor: that comment's text, not another's.
    kb(&f).add_comment(1, "second").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    kb(&f).update_comment(1, 1, "second, edited").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    kb(&f).update_comment(1, 0, "on it, edited").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's comment changed on disk"));
    f.press(KeyCode::Esc);
    // Another put at its index, its text the same: the one before it deleted.
    let mut task = f.on_disk(2);
    let at = now() - chrono::Duration::minutes(5);
    task.comments = ["first", "same", "same"]
        .into_iter()
        .zip(0..)
        .map(|(body, s)| Comment::new(at + chrono::Duration::seconds(s), body.into(), Author::Agent))
        .collect();
    Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(2), 1));
    kb(&f).delete_comment(2, 0).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    f.press(KeyCode::Esc);

    // The form: its title, tags, related entries and the task's status, together when they came together.
    f.press(KeyCode::Char('e'));
    kb(&f).add_comment(1, "third").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    let mut brain = kb(&f);
    brain.update(1, "Fix the SSO login", vec!["auth".into()]).unwrap();
    brain.set_status(1, Status::Done).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's title, tags and status changed on disk"));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('e'));
    kb(&f).toggle_link(1, 2).unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
    // Said once here too; kept through a save that stops.
    f.press(KeyCode::Char('!'));
    kb(&f).update(1, "Fix it", vec!["auth".into()]).unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), None);
    f.press(KeyCode::Enter);
    assert!(marked(&f));
    f.press(KeyCode::Esc);
    // A related entry taken out, none added.
    f.press(KeyCode::Char('e'));
    kb(&f).toggle_link(1, 2).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
}

#[test]
fn what_kb_changed_in_an_open_dialog_isnt_replaced_by_what_else_is_said() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let reload = |f: &mut Fixture| {
        f.board.message = None;
        f.board.reload().unwrap();
    };
    // Said beside what the next key does: `catch_up` reads the brains right before it, which is when it's found.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Char('!'));
    kb(&f).set_status(1, Status::Doing).unwrap();
    reload(&mut f);
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved · #1's status changed on disk"));
    // Nor is it lost to a file found skipped by the same read.
    f.press(KeyCode::Char('m'));
    kb(&f).set_description(1, "theirs").unwrap();
    std::fs::write(f.dir.path().join("0009-broken.md"), "# Broken\n").unwrap();
    reload(&mut f);
    let said = f.board.message().unwrap();
    assert!(said.contains("#1's description changed on disk") && said.contains("0009-broken.md"), "{said}");
}

#[test]
fn what_kb_changed_goes_beside_its_inputs_message_however_long_and_a_later_ones_while_it_has_just_shown() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let changed = |f: &mut Fixture, status| {
        kb(f).set_status(1, status).unwrap();
        f.board.reload().unwrap();
    };
    f.board.select(k(1));
    // Read for the key that saves, which takes seconds on a slow disk or a busy machine: still said beside it.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Char('!'));
    changed(&mut f, Status::Doing);
    f.later(Duration::from_secs(2));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved · #1's status changed on disk"));
    // Read by the window's timer, or for the key before, right before the save: the form marked with it closes, so
    // it's said beside the save.
    f.press(KeyCode::Char('e'));
    changed(&mut f, Status::Todo);
    f.board.input_done();
    f.press(KeyCode::Char('?'));
    f.later(watch::INTERVAL.checked_sub(Duration::from_millis(1)).unwrap());
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved · #1's status changed on disk"));
    // Shown a while already: replaced, as any message.
    f.press(KeyCode::Char('e'));
    changed(&mut f, Status::Doing);
    f.board.input_done();
    f.press(KeyCode::Char('?'));
    f.later(watch::INTERVAL);
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved"));
}

#[test]
fn a_board_prints_its_clock_by_name() {
    assert_eq!(format!("{:?}", Clock::default()), "Clock");
}

#[test]
fn a_message_shows_for_its_time_then_goes() {
    let mut f = Fixture::new(&["Fix login"]);
    f.board.notify("undone");
    let at = f.now.get();
    assert_eq!(f.board.message_at(at), Some("undone"));
    assert_eq!(f.board.message_at((at + MESSAGE_TTL).checked_sub(Duration::from_millis(1)).unwrap()), Some("undone"));
    assert_eq!(f.board.message_at(at + MESSAGE_TTL), None);
    f.later(MESSAGE_TTL);
    assert_eq!(f.board.message(), None);
}

#[test]
fn an_open_verdict_says_kb_decided_meanwhile() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["cache"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let redis = kb(&f).propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(redis));
    f.press(KeyCode::Char('j'));
    kb(&f).resolve(redis, DecisionState::Accepted, "").unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.changed_on_disk));
    assert_eq!(f.board.message(), Some(format!("#{redis}'s verdict changed on disk").as_str()));
}

#[test]
fn an_open_form_says_kb_swapped_a_related_entry() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Other", "Third"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).toggle_link(1, 2).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    // As many related entries as before, but another one.
    let mut brain = kb(&f);
    brain.toggle_link(1, 2).unwrap();
    brain.toggle_link(1, 3).unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.changed_on_disk));
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
}

#[test]
fn a_new_entrys_form_has_nothing_on_disk_to_mark() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    f.press(KeyCode::Char('n'));
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().update(1, "Renamed", Vec::new()).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if !form.changed_on_disk));
}
