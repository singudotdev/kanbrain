//! The text editor: descriptions and comments written, merged with what `kb` wrote meanwhile, conflicts picked, and
//! comments edited or deleted.

use super::*;

#[test]
fn comments_and_descriptions_are_written_from_the_editor() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('c'));
    f.type_text("found it");
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    f.type_text("notes");
    f.board.discard_edit();
    f.press(KeyCode::Char('m'));
    f.type_text("more notes");
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal), "ctrl+d discards too");
    f.press(KeyCode::Char('m'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if !f.board.changed(edit)), "no • in its title, Close not Save");
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("changes discarded"), "typed and put back: nothing said to be saved");
    let task = f.on_disk(1);
    assert_eq!(task.comments[0].body, "found it");
    assert_eq!(task.description, "", "discarded");
}

#[test]
fn the_editor_doesnt_replace_unseen_what_kb_wrote_while_it_was_open() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let saving = |f: &mut Fixture| {
        f.board.message = None;
        f.press(KeyCode::Esc);
    };

    // An agent describes the task while the user does: the user's save stops and says so, the editor still open.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    // enter is a new line, not a save (ctrl+enter is).
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Edit(_)));
    f.press(KeyCode::Backspace);
    kb(&f).set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    let merging = |f: &Fixture| matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some());
    assert!(merging(&f), "kept open, the lines both changed to pick from");
    assert_eq!(
        f.board.message(),
        Some("#1's description changed meanwhile: pick yours or theirs where you both changed it")
    );
    assert_eq!(f.on_disk(1).description, "the agent's plan", "not replaced unseen");
    // esc takes yours where nothing's picked, back in the editor.
    saving(&mut f);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "mine"));
    assert_eq!(f.board.message(), Some("merged: esc saves it"));

    // Changed again before the second save: stopped again, still not replaced.
    kb(&f).set_description(1, "the agent's new plan").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert!(merging(&f));
    assert_eq!(f.on_disk(1).description, "the agent's new plan");
    saving(&mut f);

    // Saved again with nothing new on disk: the user's replaces it.
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("description saved"));
    assert_eq!(f.on_disk(1).description, "mine");

    // ctrl+d after being told keeps the agent's.
    f.press(KeyCode::Char('m'));
    f.type_text(" too");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs");

    // Typed and put back is nothing of the user's: what the agent wrote meanwhile stays, and the editor says Close.
    f.press(KeyCode::Char('m'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    kb(&f).set_description(1, "theirs, again").unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if !f.board.changed(edit)), "Close, not Save");
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs, again");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();

    // Unchanged on disk, a save is a save, as before.
    f.press(KeyCode::Char('m'));
    f.type_text(" and more");
    saving(&mut f);
    assert_eq!(f.on_disk(1).description, "theirs and more");

    // A comment the agent edited meanwhile, the same.
    kb(&f).add_comment(1, "first").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", mine");
    kb(&f).update_comment(1, 0, "first, the agent's").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some("#1's comment changed meanwhile: pick yours or theirs where you both changed it")
    );
    assert_eq!(f.on_disk(1).comments[0].body, "first, the agent's");
    saving(&mut f);
    saving(&mut f);
    assert_eq!(f.on_disk(1).comments[0].body, "first, mine");

    // A new comment adds to what's there: nothing to replace, so nothing to stop for.
    f.press(KeyCode::Char('c'));
    f.type_text("second");
    kb(&f).add_comment(1, "the agent's").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    let bodies: Vec<_> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(bodies, ["first, mine", "the agent's", "second"]);
}

#[test]
fn the_editor_merges_lines_only_one_side_changed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "Plan\n\n- one\n- two\n- three").unwrap();
    f.board.reload().unwrap();
    let typing = |f: &mut Fixture, text: &str| {
        f.board.select_all();
        f.board.paste(text);
    };

    // The user's line and the agent's, both kept, and said so.
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan, mine\n\n- one\n- two\n- three");
    kb(&f).set_description(1, "Plan\n\n- one\n- three\n- four").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("description saved, merged with the change made meanwhile"));
    assert_eq!(f.on_disk(1).description, "Plan, mine\n\n- one\n- three\n- four");

    // The agent made the same change, and another: nothing to write.
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan, mine\n\n- one\n- three\n- five");
    kb(&f).set_description(1, "Plan, done\n\n- one\n- three\n- five").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("nothing to save: #1's description has your changes already"));

    // Told to the agent working on it, the save says it merged.
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan\n\n- one\n- three\n- five");
    agent.set_description(1, "Plan, done\n\n- one\n- three\n- five, done").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Tell(_)));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("description saved, merged with the change made meanwhile; the agent is told"));
    assert_eq!(f.on_disk(1).description, "Plan\n\n- one\n- three\n- five, done");
}

#[test]
fn lines_both_changed_are_picked_in_place_of_the_text() {
    use crate::text::merge::Pick;
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    let at = |f: &Fixture| match &f.board.mode {
        Mode::Edit(Edit { merge: Some(merge), .. }) => merge.at,
        _ => panic!("picking"),
    };
    assert_eq!(at(&f), 0);

    // The arrows and tab move between conflicts; other keys type nothing.
    f.press(KeyCode::Down);
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Up);
    assert_eq!(at(&f), 0);
    f.press(KeyCode::Tab);
    assert_eq!(at(&f), 1);
    f.shift(KeyCode::BackTab);
    assert_eq!(at(&f), 0);
    f.press(KeyCode::Char('x'));
    assert_eq!(at(&f), 0);

    // Each pick moves on; enter waits for the last.
    f.press(KeyCode::Char('t'));
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("1 conflict to pick yet: yours (m), theirs (t) or both (b)"));
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Char('b'));
    // A click on a side picks it.
    f.board.pick_conflict(0, Pick::Mine);
    f.press(KeyCode::Enter);
    assert!(
        matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "A\nb\nc\nd\nE\ne2")
    );
    assert_eq!(f.board.message(), Some("merged: esc saves it"));
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if f.board.changed(edit)), "Save, not Close");
    assert_eq!(f.on_disk(1).description, "a2\nb\nc\nd\ne2", "not saved yet");
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("description saved"));
    assert_eq!(f.on_disk(1).description, "A\nb\nc\nd\nE\ne2");

    // Picking mine everywhere, then theirs: the same.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A!\nb");
    kb(&f).set_description(1, "a3\nb\nc\nd\nE\ne3").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    f.press(KeyCode::Char('m'));
    f.board.use_merge();
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "A!\nb");
    // Nothing types into the text the conflicts hide: a paste, a cut, a select-all.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("mine\nb");
    kb(&f).set_description(1, "theirs\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.board.select_all();
    f.board.paste("pasted");
    assert_eq!(f.board.selected_text(), None);
    assert_eq!(f.board.cut(), None);
    assert!(!f.board.is_typing(), "no cursor");
    // All theirs is the text as it is: nothing to save, and said so.
    f.press(KeyCode::Char('t'));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "theirs\nb" && !f.board.changed(edit)));
    assert_eq!(f.board.message(), Some("merged: it's theirs as it is, esc closes"));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "theirs\nb");
    // ctrl+d while picking discards it all.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("mine");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs");
}

#[test]
fn yours_for_the_rest_keeps_the_lines_only_kb_changed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc, theirs\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    // Picked in either case; ctrl+a letter picks nothing.
    f.board.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(
        matches!(&f.board.mode, Mode::Edit(Edit { merge: Some(m), .. }) if m.conflicts().all(|c| c.pick.is_none()))
    );
    f.shift(KeyCode::Char('T'));
    assert!(matches!(&f.board.mode, Mode::Edit(Edit { merge: Some(m), .. }) if m.at == 1));
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "a2\nb\nc, theirs\nd\nE"));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "a2\nb\nc, theirs\nd\nE", "theirs, and the line only kb changed");
}

#[test]
fn conflicts_kb_changes_again_while_picked_are_merged_again() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('t'));
    // Changed again: the conflicts are of the new text, picked anew.
    kb(&f).set_description(1, "a3\nb\nc\nd\ne2").unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: pick again where you both changed it"));
    let Mode::Edit(Edit { merge: Some(merge), .. }) = &f.board.mode else { panic!("still picking") };
    assert_eq!(merge.theirs, "a3\nb\nc\nd\ne2");
    assert!(merge.conflicts().all(|c| c.pick.is_none()));
    // Read again with nothing new: as it was.
    f.press(KeyCode::Char('t'));
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), None);
    // Changed again to lines the user didn't: it all merges, back in the editor.
    kb(&f).set_description(1, "a\nb\nc, theirs\nd\ne").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: it all merges now, esc saves it"));
    assert!(
        matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "A\nb\nc, theirs\nd\nE")
    );
    // What a save merges with is that text now: taken back, it's taken back in the editor's too.
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "A\nb\nc\nd\nE");
}

#[test]
fn conflicts_kb_changes_again_to_the_text_typed_leave_nothing_to_save() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb");
    kb(&f).set_description(1, "a2\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    kb(&f).set_description(1, "A\nb").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: it's theirs as it is now, esc closes"));
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal), "closed: nothing to save");
}

#[test]
fn conflicts_of_a_comment_kb_moved_while_picked_give_back_the_text_typed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    for body in ["zero", "x\nq", "other"] {
        kb(&f).add_comment(1, body).unwrap();
    }
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 1));
    f.board.select_all();
    f.board.paste("x, mine\nq\nmy addition");
    kb(&f).update_comment(1, 1, "x, theirs\nq").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some()));
    // The comment before it deleted: its index holds another, which isn't merged with.
    kb(&f).delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    let said = f.board.message().unwrap_or_default();
    assert!(said.ends_with("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"), "{said}");
    let typed = "x, mine\nq\nmy addition";
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == typed));
    f.press(KeyCode::Char('t'));
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(_)), "the save stops for good");
    let comments: Vec<_> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(comments, ["x, theirs\nq", "other"]);
}

#[test]
fn the_editor_doesnt_save_a_comment_over_another_moved_to_its_place_meanwhile() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    type Meanwhile = fn(&mut Brain);
    let mut f = Fixture::new(&["feature"]);
    // Written in one second, as an agent's quick comments are, with the same text: neither tells them apart.
    let at = now() - chrono::Duration::minutes(5);
    let mut task = f.on_disk(1);
    for _ in 0..3 {
        task.comments.push(Comment::new(at, "ok".into(), Author::Agent));
    }
    let bodies = |f: &Fixture| f.on_disk(1).comments.into_iter().map(|c| c.body).collect::<Vec<_>>();
    let cases: [(usize, Meanwhile, &[&str]); 5] = [
        // Another window of the app, or a sync, takes it out: the next is at its index now.
        (0, |kb| kb.delete_comment(1, 0).unwrap(), &["ok", "ok"]),
        // One before it taken out.
        (1, |kb| kb.delete_comment(1, 0).unwrap(), &["ok", "ok"]),
        // The next one edited, then this one taken out: the edited one is at its index.
        (
            0,
            |kb| {
                kb.update_comment(1, 1, "edited").unwrap();
                kb.delete_comment(1, 0).unwrap();
            },
            &["edited", "ok"],
        ),
        // The last taken out and another written after: the new one is at its index.
        (
            2,
            |kb| {
                kb.delete_comment(1, 2).unwrap();
                kb.add_comment(1, "new").unwrap();
            },
            &["ok", "ok", "new"],
        ),
        // Another one edited: the comments aren't as the editor found them, so it can't tell which is which.
        (
            1,
            |kb| {
                kb.update_comment(1, 0, "edited").unwrap();
            },
            &["edited", "ok", "ok"],
        ),
    ];
    for (index, meanwhile, left) in cases {
        Store::at(f.dir.path()).unwrap().save(&mut task.clone()).unwrap();
        f.board.reload().unwrap();
        f.board.open_editor(Target::Comment(k(1), index));
        f.type_text(", edited");
        meanwhile(&mut Brain::open(Store::at(f.dir.path()).unwrap()).unwrap());
        f.board.reload().unwrap();
        for _ in 0..2 {
            f.board.message = None;
            f.press(KeyCode::Esc);
            assert!(matches!(f.board.mode, Mode::Edit(_)), "kept open, to copy the text from");
            assert_eq!(
                f.board.message(),
                Some("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"),
                "editing {index}, {left:?}"
            );
        }
        assert_eq!(bodies(&f), left, "no comment saved over");
        f.board.discard_edit();
    }

    // The last one replaced in its file by one written when it was, by someone else; or by the same author, later.
    let replacements = [
        Comment::new(at, "new".into(), Author::Human),
        Comment::new(at + chrono::Duration::seconds(1), "new".into(), Author::Agent),
    ];
    for replacement in replacements {
        let store = Store::at(f.dir.path()).unwrap();
        store.save(&mut task.clone()).unwrap();
        f.board.reload().unwrap();
        f.board.open_editor(Target::Comment(k(1), 2));
        f.type_text(", edited");
        let mut replaced = f.on_disk(1);
        replaced.comments[2] = replacement;
        store.save(&mut replaced).unwrap();
        f.board.reload().unwrap();
        for _ in 0..2 {
            f.board.message = None;
            f.press(KeyCode::Esc);
            assert!(
                f.board.message().is_some_and(|m| m.contains("changed around this one")),
                "{:?}",
                f.board.message()
            );
        }
        assert_eq!(bodies(&f), ["ok", "ok", "new"], "no comment saved over");
        f.board.discard_edit();
    }

    // A comment written after it, as an agent goes on, changes nothing around it: it's saved.
    Store::at(f.dir.path()).unwrap().save(&mut task.clone()).unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", mine");
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().add_comment(1, "the agent's").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(bodies(&f), ["ok, mine", "ok", "ok", "the agent's"]);
}

#[test]
fn capital_c_picks_a_comment_to_edit_or_delete() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('C'));
    assert!(matches!(f.board.mode, Mode::Normal), "no comments: nothing to pick");
    assert_eq!(f.board.message(), Some("no comments yet: c adds one"), "and said");
    for text in ["first", "second"] {
        f.press(KeyCode::Char('c'));
        f.type_text(text);
        f.press(KeyCode::Esc);
    }
    f.press(KeyCode::Char('C'));
    assert!(matches!(f.board.mode, Mode::Comments(CommentPicker { sel: 1, .. })), "the latest highlighted");
    f.press(KeyCode::Up);
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.target == Target::Comment(k(1), 0)));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('C'));
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('y'));
    let comments: Vec<String> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(comments, ["first"]);
}

#[test]
fn a_comment_is_edited_and_deleted_from_the_details() {
    let mut f = Fixture::new(&["feature"]);
    f.board.brain_mut(0).add_comment(1, "first try").unwrap();
    f.board.brain_mut(0).add_comment(1, "second try").unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.press(KeyCode::End);
    f.type_text(", failed");
    f.press(KeyCode::Esc);
    let bodies = |f: &Fixture| f.on_disk(1).comments.iter().map(|c| c.body.clone()).collect::<Vec<_>>();
    assert_eq!(bodies(&f), ["first try, failed", "second try"]);
    assert!(f.on_disk(1).comments[0].edited.is_some(), "it says it was edited");
    f.board.ask_delete_comment(k(1), 0);
    f.press(KeyCode::Char('n'));
    assert_eq!(bodies(&f).len(), 2, "n keeps it");
    f.board.ask_delete_comment(k(1), 0);
    f.press(KeyCode::Char('y'));
    assert_eq!(bodies(&f), ["second try"]);
}

#[test]
fn deleting_a_comment_deletes_the_one_asked_about_wherever_it_is_now() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    // What `kb` (or a hand, or a pull) does between the question and its answer.
    type Meanwhile = fn(&mut Brain);
    let mut f = Fixture::new(&["feature"]);
    let bodies = |f: &Fixture| f.on_disk(1).comments.iter().map(|c| c.body.clone()).collect::<Vec<_>>();
    let cases: [(Meanwhile, &[&str], Option<&str>); 3] = [
        // One before it gone: another is at its index now.
        (|kb| kb.delete_comment(1, 0).unwrap(), &["c"], Some("comment deleted")),
        (
            |kb| _ = kb.update_comment(1, 1, "b, edited").unwrap(),
            &["a", "b, edited", "c"],
            Some("not deleted: #1's comment was edited or deleted meanwhile"),
        ),
        (
            |kb| kb.delete_comment(1, 1).unwrap(),
            &["a", "c"],
            Some("not deleted: #1's comment was edited or deleted meanwhile"),
        ),
    ];
    // Emptied meanwhile: nothing there to ask about, and the list stays.
    f.board.brain_mut(0).add_comment(1, "only").unwrap();
    f.press(KeyCode::Char('C'));
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('x'));
    assert!(matches!(f.board.mode, Mode::Comments(_)), "{:?}", f.board.mode);
    f.press(KeyCode::Esc);
    for (meanwhile, left, said) in cases {
        let mut task = f.on_disk(1);
        task.comments = ["a", "b", "c"].map(|body| Comment::new(now(), body.into(), Author::Human)).to_vec();
        Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
        f.board.reload().unwrap();
        f.press(KeyCode::Char('C'));
        f.press(KeyCode::Up);
        f.press(KeyCode::Char('x'));
        meanwhile(&mut Brain::open(Store::at(f.dir.path()).unwrap()).unwrap());
        f.board.reload().unwrap();
        f.board.message = None;
        f.press(KeyCode::Char('y'));
        assert_eq!((bodies(&f), f.board.message()), (left.iter().map(ToString::to_string).collect(), said));
    }
}
