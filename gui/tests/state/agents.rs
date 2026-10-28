//! Agents and you: a task an agent works on, telling it of a save, its questions answered and its proposals decided,
//! and the update waiting for it.

use super::*;

#[test]
fn a_save_that_stops_after_telling_the_agent_asks_again_with_the_note() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.set_description(1, "plan").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("my plan");
    f.press(KeyCode::Esc);
    f.type_text("use mine");
    // The agent writes before the answer: the save stops over it.
    agent.set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some()));
    f.press(KeyCode::Char('m'));
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Tell(tell) if tell.note.text() == "use mine"), "asked again, with the note");
    // The form's title, the same.
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).description, "my plan");
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    f.type_text("renamed");
    agent.update(1, "feature, the agent's", vec![]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.is_some()));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Tell(tell) if tell.note.text() == "renamed"));
}

#[test]
fn opening_what_an_agent_is_working_on_says_so() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Mine"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.add_comment(1, "on it").unwrap();
    f.board.reload().unwrap();
    let opening = |f: &mut Fixture, open: &dyn Fn(&mut Board)| {
        f.board.message = None;
        open(&mut f.board);
        let said = f.board.message().map(str::to_string);
        assert!(!matches!(f.board.mode, Mode::Normal), "opened");
        f.board.mode = Mode::Normal;
        said
    };
    let working = Some("#1 is being worked on by an agent".to_string());

    // The form, its description and its comments: what's saved may cross what the agent writes next.
    f.board.select(k(1));
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), working, "the form");
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('m'))), working, "the description");
    assert_eq!(opening(&mut f, &|b| b.open_editor(Target::Comment(k(1), 0))), working, "a comment");
    // A new comment crosses nothing: it's how to tell the agent.
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('c'))), None, "a new comment");

    // A task a person started, or one the agent parked, isn't being worked on by an agent.
    f.board.select(k(2));
    f.board.set_status(k(2), Status::Doing);
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), None, "the user's");
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Parked).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), None, "parked");
}

#[test]
fn saving_what_an_agent_is_working_on_asks_whether_to_tell_it() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Mine"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.add_comment(1, "on it").unwrap();
    f.board.reload().unwrap();
    let ctrl_enter = |f: &mut Fixture| f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    let last = |f: &Fixture| f.on_disk(1).comments.last().map(|c| (c.body.clone(), c.author)).unwrap();
    let asked = |f: &Fixture| match &f.board.mode {
        Mode::Tell(tell) => Some(tell.pending.what()),
        _ => None,
    };

    // The form: asked before anything is written; `esc` goes back to it as it was.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    assert_eq!(asked(&f), Some("its title, tags or related entries"));
    assert_eq!(f.on_disk(1).title, "Fix login", "nothing written yet");
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix login!"));
    // `enter` tells it, with the note: the user's comment says what changed.
    f.press(KeyCode::Enter);
    f.type_text("keep it short");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).title, "Fix login!");
    assert_eq!(last(&f), ("Changed the title: keep it short".to_string(), Author::Human));
    assert_eq!(f.board.message(), Some("#1 saved; the agent is told"));
    // Saved and told in one go: one undo takes both back.
    f.press(KeyCode::Char('u'));
    assert_eq!(f.on_disk(1).title, "Fix login");
    assert_eq!(last(&f).0, "on it");

    // `ctrl+enter` just saves.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("auth");
    f.press(KeyCode::Enter);
    assert_eq!(asked(&f), Some("its title, tags or related entries"));
    ctrl_enter(&mut f);
    assert_eq!(f.on_disk(1).tags, ["auth"]);
    assert_eq!(last(&f).0, "on it", "not told");
    assert_eq!(f.board.message(), Some("#1 saved"));

    // The description and a comment, without a note.
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    assert_eq!(asked(&f), Some("its description"));
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).description, "the plan");
    assert_eq!(last(&f).0, "Changed the description.");
    assert_eq!(f.board.message(), Some("description saved; the agent is told"));
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", noted");
    f.press(KeyCode::Esc);
    assert_eq!(asked(&f), Some("a comment"));
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).comments[0].body, "on it, noted");
    assert_eq!(last(&f).0, "Changed a comment.");

    // A save that stops over what `kb` changed meanwhile tells nothing: the form is back, to save again.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("?");
    f.press(KeyCode::Enter);
    kb(&f).update(1, "Fix the login", vec!["auth".into()]).unwrap();
    f.board.reload().unwrap();
    let said = f.on_disk(1).comments.len();
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Form(_)), "stopped");
    assert_eq!(f.on_disk(1).comments.len(), said);
    f.press(KeyCode::Esc);

    // Nothing asked: a new comment (it's how to tell the agent), a save that changes nothing, a task a person runs.
    f.press(KeyCode::Char('c'));
    f.type_text("one more thing");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(last(&f).0, "one more thing");
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    f.board.set_status(k(2), Status::Doing);
    f.board.select(k(2));
    f.press(KeyCode::Char('e'));
    f.type_text("!");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
}

#[test]
fn the_question_hears_kb_meanwhile_and_its_button_just_saves() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| {
        let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
        agent.author = Author::Agent;
        agent
    };
    kb(&f).set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    let comments = |f: &Fixture| f.on_disk(1).comments.len();

    // Just save, from its button.
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    f.board.just_save();
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "the plan");
    assert_eq!(comments(&f), 0);

    // The agent finishes it while the question is open: marked and said as in the form, and not told after all.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    kb(&f).set_status(1, Status::Done).unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's status changed on disk"));
    assert!(
        matches!(&f.board.mode, Mode::Tell(tell) if matches!(&tell.pending, Pending::Form(form) if form.changed_on_disk))
    );
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix login!");
    assert_eq!(comments(&f), 0);
    assert_eq!(
        f.board.message(),
        Some("#1 saved; not told: no agent is working on it now · #1's status changed on disk")
    );
}

#[test]
fn the_question_closes_with_its_entry_gone() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Tell(_)));
    f.board.reload().unwrap();
    assert!(matches!(f.board.mode, Mode::Tell(_)), "kept across a reload");
    Store::at(f.dir.path()).unwrap().delete(&f.on_disk(1)).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("closed: #1 is gone, deleted or moved meanwhile"));
}

#[test]
fn the_user_answers_questions_and_decides_on_proposals() {
    let mut f = Fixture::new(&["cache", "css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let new = kanbrain_core::brain::NewEntry { title: "Use Redis".into(), ..Default::default() };
    let decision = agent.propose(1, new).unwrap();
    agent.ask(2, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    assert_eq!(*f.board.waiting(), [k(1), k(2)]);
    for expected in [1, 2, 1] {
        f.press(KeyCode::Char('w'));
        assert_eq!((f.board.selected, f.board.drawer), (Some(k(expected)), true), "w cycles through them");
    }

    f.board.open_entry(k(2));
    f.press(KeyCode::Char('c'));
    f.type_text("#fa0");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert!(matches!(f.on_disk(2).asked(), Some(Asked::Answered(a)) if a.author == Author::Human));
    assert_eq!(f.board.message(), Some("answer sent: the agent picks it up"));
    f.board.notify("");
    f.press(KeyCode::Char('c'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.board.message(), Some(""), "a comment typed and taken back says nothing");
    f.press(KeyCode::Char('c'));
    f.type_text("  ");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.board.message(), Some("nothing added: the comment was empty"), "blank, said");

    f.board.open_entry(k(decision));
    let labels: Vec<&str> = f.board.menu_items(k(decision)).iter().map(|i| i.label).collect();
    assert!(labels.contains(&"Accept") && labels.contains(&"Reject"), "{labels:?}");
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.on_disk(decision).decision, DecisionState::Rejected);
    let task = f.on_disk(1);
    assert_eq!(task.comments.last().unwrap().body, format!("Rejected decision #{decision}, Use Redis: too costly"));
    assert!(matches!(task.asked(), Some(Asked::Answered(_))));
    assert!(f.board.waiting().is_empty());
    f.press(KeyCode::Char('y'));
    assert_eq!(f.on_disk(decision).decision, DecisionState::Accepted, "and reconsidered");
}

#[test]
fn while_the_board_acts_a_kb_run_meanwhile_waits() {
    let mut f = Fixture::new(&["feature"]);
    let locks = f.board.lock_brains(&mut Vec::new());
    assert_eq!(locks.len(), f.board.sources.len(), "every brain's");
    let dir = f.board.brain(0).store.dir().to_path_buf();
    let (taken, waiting) = std::sync::mpsc::channel();
    // As another `kb` process would: a lock of its own on the same brain.
    let other = std::thread::spawn(move || {
        let _lock = kanbrain_core::store::Store::at(&dir).unwrap().lock().unwrap();
        taken.send(()).unwrap();
    });
    assert!(waiting.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "it waits for the board");
    drop(locks);
    assert!(waiting.recv_timeout(std::time::Duration::from_secs(5)).is_ok(), "and goes on once released");
    other.join().unwrap();
}

#[test]
fn an_update_is_offered_once_found_and_waits_for_the_agents() {
    let mut f = Fixture::new(&["rate-limit the login"]);
    f.shift(KeyCode::Char('U'));
    assert!(f.board.message().unwrap().starts_with("no newer release"), "{:?}", f.board.message());
    assert!(matches!(f.board.mode, Mode::Normal));

    f.board.available = Some("9.0.0".into());
    f.board.brain_mut(0).author = Author::Agent;
    f.board.brain_mut(0).set_status(1, Status::Doing).unwrap();
    f.shift(KeyCode::Char('U'));
    let msg = f.board.message().unwrap();
    assert!(msg.contains("can't update to 9.0.0 yet") && msg.contains("#1 rate-limit the login"), "{msg}");
    assert!(matches!(f.board.mode, Mode::Normal), "not even asked");

    f.board.brain_mut(0).set_status(1, Status::Parked).unwrap();
    f.shift(KeyCode::Char('U'));
    assert!(matches!(f.board.mode, Mode::Confirm(Confirm::Update)));
    f.press(KeyCode::Char('y'));
    assert_eq!(f.board.request.take(), Some(Request::Update), "the window installs it");
}

#[test]
fn a_task_waiting_on_you_is_answered_and_its_decision_opened_from_the_palette_and_menu() {
    let mut f = Fixture::new(&["cache", "css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let new = kanbrain_core::brain::NewEntry { title: "Use Redis".into(), ..Default::default() };
    let decision = agent.propose(1, new).unwrap();
    agent.ask(2, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    let labels = |board: &Board, id| board.menu_items(k(id)).iter().map(|a| a.label).collect::<Vec<_>>();
    let rows = |board: &Board| board.commands(&Picker::default()).iter().map(|a| a.does).collect::<Vec<_>>();

    f.board.select(k(2));
    assert!(labels(&f.board, 2).contains(&"Answer") && !labels(&f.board, 2).contains(&"Add comment"));
    assert!(!labels(&f.board, 2).contains(&"Open proposed decision"), "a question, no decision");
    assert!(rows(&f.board).contains(&"Answer the agent's question"));
    f.press(KeyCode::Char('c'));
    assert!(matches!(f.board.mode, Mode::Edit(_)), "c answers it, as it comments");
    f.press(KeyCode::Esc);
    f.press(KeyCode::Esc);

    f.board.select(k(1));
    assert!(labels(&f.board, 1).contains(&"Open proposed decision"), "{:?}", labels(&f.board, 1));
    assert!(rows(&f.board).contains(&"Open the decision it waits on"));
    f.press(KeyCode::Char('D'));
    assert_eq!((f.board.tab, f.board.selected), (Tab::Decisions, Some(k(decision))), "D opens it");
    assert!(!labels(&f.board, decision).contains(&"Answer"), "a decision waits on nothing");
}

#[test]
fn closing_an_answer_never_written_says_nothing_was_answered() {
    let mut f = Fixture::new(&["css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.ask(1, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    f.board.open_entry(k(1));
    let waits = |f: &Fixture| matches!(f.on_disk(1).asked(), Some(Asked::Waiting(_)));

    f.press(KeyCode::Char('c'));
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal) && waits(&f), "closed, and still waiting");
    assert_eq!(f.board.message(), Some("nothing answered: the question still waits"));

    f.press(KeyCode::Char('c'));
    f.type_text("  ");
    f.press(KeyCode::Esc);
    assert!(waits(&f) && f.on_disk(1).comments.len() == 1, "blanks aren't an answer");

    f.press(KeyCode::Char('c'));
    f.type_text("Blue");
    f.press(KeyCode::Esc);
    assert!(!waits(&f), "something said answers it");
}

#[test]
fn a_task_waiting_on_a_decision_is_answered_by_deciding_with_or_without_a_comment() {
    let mut f = Fixture::new(&["cache", "queue", "logs"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let propose = |agent: &mut kanbrain_core::brain::Brain, id, title: &str| {
        agent.propose(id, kanbrain_core::brain::NewEntry { title: title.into(), ..Default::default() }).unwrap()
    };
    let redis = propose(&mut agent, 1, "Use Redis");
    let kafka = propose(&mut agent, 2, "Use Kafka");
    let loki = propose(&mut agent, 3, "Use Loki");
    f.press(KeyCode::Char('R'));
    let labels = |board: &Board, id| board.menu_items(k(id)).iter().map(|a| a.label).collect::<Vec<_>>();
    let last = |f: &Fixture, id| f.on_disk(id).comments.last().unwrap().body.clone();

    f.board.select(k(1));
    let menu = labels(&f.board, 1);
    for offered in ["Accept", "Accept with a comment", "Reject", "Open proposed decision"] {
        assert!(menu.contains(&offered), "{offered}: {menu:?}");
    }
    assert!(!menu.contains(&"Answer"), "it's answered by deciding");
    f.press(KeyCode::Char('Y'));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Proposed, "a comment never written accepts nothing");
    assert_eq!(f.board.message(), Some("not accepted: write the comment, or accept without one (y)"));
    f.press(KeyCode::Char('Y'));
    f.type_text("fine for now");
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted);
    assert_eq!(last(&f, 1), format!("Accepted decision #{redis}, Use Redis: fine for now"), "the task gets it");

    f.board.select(k(2));
    f.press(KeyCode::Char('y'));
    assert_eq!(f.on_disk(kafka).decision, DecisionState::Accepted, "or without one");
    assert_eq!(last(&f, 2), format!("Accepted decision #{kafka}, Use Kafka."));

    f.board.select(k(3));
    f.press(KeyCode::Char('j'));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(loki).decision, DecisionState::Proposed, "rejecting always says why");
    assert_eq!(f.board.message(), Some("not rejected: say why it's rejected"));
    f.press(KeyCode::Char('j'));
    f.type_text("too heavy");
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(loki).decision, DecisionState::Rejected);
    assert_eq!(f.board.message(), Some(format!("#{loki} rejected").as_str()), "named, as `y` names it");
    assert_eq!(last(&f, 3), format!("Rejected decision #{loki}, Use Loki: too heavy"));
}

#[test]
fn the_reject_or_accept_editor_doesnt_overturn_unseen_a_verdict_kb_gave_while_it_was_open() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["cache", "queue"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    let redis = agent.propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    let kafka = agent.propose(2, NewEntry { title: "Use Kafka".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    let saving = |f: &mut Fixture| {
        f.board.message = None;
        f.press(KeyCode::Esc);
    };
    let said = |f: &Fixture, id| f.on_disk(id).comments.len();

    // Accepted by kb while the user writes why it's rejected: the save stops and says so, the editor still open.
    f.board.select(k(redis));
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    kb(&f).resolve(redis, DecisionState::Accepted, "").unwrap();
    f.board.reload().unwrap();
    let before = said(&f, redis);
    saving(&mut f);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "too costly"), "kept, with the reason");
    assert_eq!(
        f.board.message(),
        Some(format!("#{redis} was accepted meanwhile: save again to reject it anyway, ctrl+d to leave it").as_str())
    );
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted, "not overturned unseen");
    assert_eq!(said(&f, redis), before);
    // Saved again, knowing it: the user's verdict.
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(redis).decision, DecisionState::Rejected);
    assert_eq!(f.board.message(), Some(format!("#{redis} rejected").as_str()));
    assert_eq!(said(&f, redis), before + 1, "with the reason");

    // Proposed again meanwhile, the same.
    f.board.select(k(redis));
    f.press(KeyCode::Char('Y'));
    f.type_text("fine after all");
    kb(&f).resolve(redis, DecisionState::Proposed, "reworked").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{redis} was proposed again meanwhile: save again to accept it anyway, ctrl+d to leave it")
                .as_str()
        )
    );
    assert_eq!(f.on_disk(redis).decision, DecisionState::Proposed);
    saving(&mut f);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted);

    // Given the same verdict meanwhile: nothing a save can give, however many times, but the text typed is the user's
    // to copy.
    for (key, verdict, word) in [('j', DecisionState::Rejected, "reason"), ('Y', DecisionState::Accepted, "comment")] {
        kb(&f).resolve(kafka, DecisionState::Proposed, "again").unwrap();
        f.board.reload().unwrap();
        f.board.select(k(kafka));
        f.press(KeyCode::Char(key));
        f.type_text("mine");
        kb(&f).resolve(kafka, verdict, "theirs").unwrap();
        f.board.reload().unwrap();
        let before = said(&f, kafka);
        for _ in 0..2 {
            saving(&mut f);
            assert!(matches!(f.board.mode, Mode::Edit(_)));
            assert_eq!(
                f.board.message(),
                Some(format!("#{kafka} was {} meanwhile: copy your {word}, then ctrl+d", verdict.as_str()).as_str())
            );
        }
        assert_eq!(said(&f, kafka), before, "the verdict was theirs");
        f.board.discard_edit();
    }

    // Given it, then proposed again: the save stops again, saying so, rather than giving the reason over the new
    // proposal unseen.
    kb(&f).resolve(kafka, DecisionState::Proposed, "again").unwrap();
    f.board.reload().unwrap();
    f.board.select(k(kafka));
    f.press(KeyCode::Char('j'));
    f.type_text("mine");
    kb(&f).resolve(kafka, DecisionState::Rejected, "theirs").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    kb(&f).resolve(kafka, DecisionState::Proposed, "reworked").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{kafka} was proposed again meanwhile: save again to reject it anyway, ctrl+d to leave it")
                .as_str()
        )
    );
    assert_eq!(f.on_disk(kafka).decision, DecisionState::Proposed, "not rejected unseen");
}
