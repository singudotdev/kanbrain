use super::*;
use crate::model::{one_line, sanitize_comment, sanitize_description};
use chrono::Duration;

#[test]
fn round_trip() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(7, "Fix prod login".into(), t0);
    task.description = "customer X reported it\n\n- check logs\n### steps".into();
    task.comments = vec![
        Comment::new(t0, "first\n\nwith a blank line".into(), Author::Human),
        Comment {
            edited: Some(t0 + Duration::minutes(9)),
            ..Comment::new(t0 + Duration::minutes(5), "second".into(), Author::Human)
        },
        Comment::new(t0 + Duration::minutes(7), String::new(), Author::Human),
    ];
    task.tags = vec!["prod".into(), "support".into()];
    task.set_status(Status::Doing, t0 + Duration::minutes(10), Author::Human);
    task.start_session(t0 + Duration::minutes(10));
    task.set_status(Status::Done, t0 + Duration::hours(2), Author::Human);

    let back = Task::from_markdown(&task.to_markdown()).unwrap();
    assert_eq!(back.id, 7);
    assert_eq!(back.title, task.title);
    assert_eq!(back.status, Status::Done);
    assert_eq!(back.created, t0);
    assert_eq!(back.description, task.description);
    assert_eq!(back.comments, task.comments);
    assert_eq!(back.tags, task.tags);
    assert_eq!(back.history, task.history);
    assert_eq!(back.sessions, task.sessions);
    assert_eq!(back.total(t0), Duration::minutes(110));
}

#[test]
fn a_title_written_over_several_lines_reads_back_as_one() {
    // As versions before 0.4.0 wrote `kb add $'Plan\n## Steps'`: the id and every key after it were lost.
    let text = "# Plan\n## Steps\nfirst\n\n- id: 4\n- status: doing\n- tags: api\n";
    let task = Task::from_markdown(text).unwrap();
    assert_eq!((task.id, task.title.as_str(), task.status), (4, "Plan ## Steps first", Status::Doing));
    assert_eq!(task.tags, ["api"]);
    let again = Task::from_markdown(&task.to_markdown()).unwrap();
    assert_eq!(again.title, task.title, "saved on one line");
    // A lone carriage return, which versions before 0.5.0 kept in a title: a line break, as markdown reads it.
    let task = Task::from_markdown("# Plan\rold\r\rMac\n\n- id: 5\n").unwrap();
    assert_eq!(task.title, "Plan old Mac");
}

#[test]
fn hand_edits_keep_what_they_say() {
    let text = "# Notes\n\n- id: 3\n- kind: Note\n- tags: \n- links: 1\nsee the wiki\n- links: 2, api#4\n\n\
                ## Description\n\n    indented code\n    more\n";
    let task = Task::from_markdown(text).unwrap();
    assert_eq!(task.kind, Kind::Note, "in any case, not a task");
    assert_eq!(task.links, [1, 2], "both lines' links");
    assert_eq!(task.repo_links, [RepoLink { repo: "api".into(), id: 4 }]);
    assert_eq!(task.description, "    indented code\n    more", "its first line keeps its indentation");
    let again = task.to_markdown();
    assert!(again.contains("see the wiki"), "a line of its own kept: {again}");
    assert_eq!(Task::from_markdown(&again).unwrap().description, task.description);
}

#[test]
fn keys_before_the_id_stay_keys_and_a_description_never_becomes_the_title() {
    let reordered = Task::from_markdown("# Reordered\n\n- status: done\n- id: 2\n- tags: \n").unwrap();
    assert_eq!((reordered.title.as_str(), reordered.status), ("Reordered", Status::Done));
    let tight = Task::from_markdown("# Tight\n- status: done\n- id: 2\n").unwrap();
    assert_eq!((tight.title.as_str(), tight.status), ("Tight", Status::Done));
    let id_first = Task::from_markdown("- id: 5\n# Late title\n- status: done\n").unwrap();
    assert_eq!((id_first.id, id_first.title.as_str()), (5, "Late title"), "keys before the title too");
    let note = Task::from_markdown("- id: 5\n# Late title\nmy note\n").unwrap();
    assert_eq!(note.title, "Late title", "text under a title below the id isn't more title");
    // No title: a `# ` heading in the description is the description's.
    let untitled = Task::from_markdown("- id: 1\n- status: todo\n\n## Description\n\n# Heading\nbody\n").unwrap();
    assert_eq!(untitled.description, "# Heading\nbody");
    // No id of its own: an id in its description's list doesn't make it an entry.
    assert!(Task::from_markdown("# No id\n\n- status: todo\n\n## Description\n\n- id: 3\n").is_err());
}

#[test]
fn untracked_tasks_have_no_sessions_section() {
    assert!(!Task::new(1, "x".into(), now()).to_markdown().contains("## Sessions"));
}

#[test]
fn open_sessions_that_cannot_be_running_are_closed() {
    let md = "# x\n\n- id: 1\n- status: parked\n\n## History\n\n- 2026-09-24 11:00 doing -> parked\n\n\
              ## Sessions\n\n- 2026-09-24 10:00 -> running\n- 2026-09-24 09:00 -> running\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.sessions[0].end, parse_ts("2026-09-24 10:00"), "closed at the next start");
    assert_eq!(task.sessions[1].end, parse_ts("2026-09-24 11:00"), "closed when the task was parked");
    assert!(!task.is_running());
}

#[test]
fn hand_typed_history_and_missing_status() {
    let md = "# x\n\n- id: 1\n\n## History\n\n- 2026-09-24 10:00 todo -> doing\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.status, Status::Todo, "the status line decides, not the history");
    assert_eq!(task.history[0].at, parse_ts("2026-09-24 10:00").unwrap());
    assert_eq!(task.history[0].to, Status::Doing);
    assert!(Task::from_markdown("# x\n\n- id: 1\n\n## History\n\n- 2026-09-24 10:00 todo -> nope\n").is_err());
}

#[test]
fn missing_id_is_an_error() {
    assert_eq!(Task::from_markdown("# no id\n").unwrap_err(), "missing `- id:` line");
    assert_eq!(Task::parse("# no id\n").unwrap_err(), ParseError::MissingId);
    // A broken line is said as it always was, for code that reads the text.
    let broken = "# Broken\n\n- id: 2\n\n## Sessions\n\n- nonsense\n";
    assert_eq!(Task::from_markdown(broken).unwrap_err(), "bad session line: nonsense");
}

#[test]
fn notes_have_a_kind_and_links_but_no_status_or_history() {
    let mut note = Task::new(3, "How deploys work".into(), now());
    note.kind = Kind::Note;
    note.links = vec![1, 2];
    let md = note.to_markdown();
    assert!(md.contains("- kind: note") && md.contains("- links: 1, 2"));
    assert!(!md.contains("- status:") && !md.contains("## History"));
    let back = Task::from_markdown(&md).unwrap();
    assert_eq!((back.kind, back.links), (Kind::Note, vec![1, 2]));
}

#[test]
fn links_accept_hashes_and_skip_repeats() {
    assert_eq!(parse_links("#5 9, 5, x").0, [5, 9]);
}

#[test]
fn links_to_other_repositories_keep_their_name_and_round_trip() {
    let (links, repo_links) = parse_links("5, shop-api#3, shop-api#3, web#x");
    assert_eq!(links, [5]);
    assert_eq!(repo_links, [RepoLink { repo: "shop-api".into(), id: 3 }], "repeats and bad ids skipped");
    let mut task = Task::new(1, "cart total".into(), now());
    (task.links, task.repo_links) = (links, repo_links);
    let md = task.to_markdown();
    assert!(md.contains("- links: 5, shop-api#3\n"), "{md}");
    assert_eq!(Task::from_markdown(&md).unwrap().repo_links, task.repo_links);
}

#[test]
fn what_a_newer_version_wrote_is_written_back() {
    let newer = "# Ship it\n\n- id: 4\n- status: todo\n- tags: \n- links: 2, web#1, blog/web#3, ~5\n\
                 - due: 2026-11-01\n- created: 2026-10-01T09:00:00+02:00\n\n## Description\n\n## Comments\n\n\
                 ## Checklist\n\n- [x] tests\n- [ ] notes\n\n## History\n\n";
    let mut task = Task::from_markdown(newer).unwrap();
    assert_eq!(task.links, [2]);
    task.comments.push(Comment::new(now(), "saved by this version".into(), Author::Human));
    let md = task.to_markdown();
    assert!(md.contains("- links: 2, web#1, blog/web#3, ~5\n- due: 2026-11-01\n"), "{md}");
    assert!(md.ends_with("\n\n## Checklist\n\n- [x] tests\n- [ ] notes\n\n"), "{md}");
    assert_eq!(Task::from_markdown(&md).unwrap().to_markdown(), md, "and stays as it is");
}

#[test]
fn authors_questions_and_decision_states_round_trip() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(3, "Pick a cache".into(), t0);
    task.comments = vec![
        Comment { question: true, ..Comment::new(t0, "Redis or in-process?".into(), Author::Agent) },
        Comment { unknown: vec!["pinned".into()], ..Comment::new(t0, "Redis".into(), Author::Human) },
    ];
    task.set_status(Status::Parked, t0, Author::Agent);
    let file = task.to_markdown();
    assert!(file.contains("(agent, question)\n") && file.contains("(pinned)\n"), "{file}");
    assert!(file.contains("todo -> parked (agent)\n"), "{file}");
    let back = Task::from_markdown(&file).unwrap();
    assert_eq!((back.comments, back.history), (task.comments, task.history));

    let mut decision = Task::new(4, "Use Redis".into(), t0);
    decision.kind = Kind::Decision;
    decision.decision = DecisionState::Proposed;
    let file = decision.to_markdown();
    assert!(file.contains("- decision: proposed\n"), "{file}");
    assert_eq!(Task::from_markdown(&file).unwrap().decision, DecisionState::Proposed);
    decision.decision = DecisionState::Accepted;
    assert!(!decision.to_markdown().contains("- decision:"), "in force needs no line");
}

#[test]
fn hand_written_headers_and_unknown_authors_still_read() {
    let md = "# x\n\n- id: 1\n- status: parked\n\n## Comments\n\n### 2026-09-24 10:00 (edited 2026-09-24 10:05)\n\n\
              old form\n\n### Steps (draft)\n\nnot a header\n\n## History\n\n- 2026-09-24 11:00 todo -> parked (robot)\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.comments.len(), 1);
    assert_eq!(task.comments[0].edited, parse_ts("2026-09-24 10:05"));
    assert!(task.comments[0].body.contains("### Steps (draft)"));
    assert_eq!(task.history[0].by, Author::Human, "an unknown name doesn't lose the file");
}

/// Words, and pieces the format gives a meaning to: headings, keys, link words, times, markers.
const PIECES: [&str; 24] = [
    "word",
    "Fix",
    "login",
    "## Heading",
    "### 2026-09-24T10:00:00+02:00",
    "- id: 9",
    "- status: done",
    "#tag",
    "a, b",
    "`code`",
    "    indented",
    "日本語",
    "émoji 🚀",
    "->",
    "(agent, question)",
    "- links: 3",
    "api#2",
    "[link](http://x)",
    "**bold**",
    "> quote",
    "1. item",
    "- [ ] box",
    "|a|b|",
    "\t tab",
];

/// A small deterministic random source, so a failing case is the same on every run and every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }

    /// One to `most` lines of [`PIECES`].
    fn text(&mut self, most: usize) -> String {
        let lines = 1 + self.below(most);
        (0..lines)
            .map(|_| (0..=self.below(5)).map(|_| self.pick(&PIECES)).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(if self.below(2) == 0 { "\n" } else { "\n\n" })
    }

    fn time(&mut self, base: DateTime<Local>) -> DateTime<Local> {
        base + Duration::seconds(i64::try_from(self.below(400_000)).unwrap())
    }
}

/// An entry as the API makes one: title on one line, tags parsed, description and comments sanitized.
fn random_task(rng: &mut Rng, id: u32) -> Task {
    let t0 = parse_ts("2026-01-01 09:00").unwrap();
    let mut task = Task::new(id, one_line(&rng.text(2)), rng.time(t0));
    task.kind = [Kind::Task, Kind::Note, Kind::Decision][rng.below(3)];
    task.tags = parse_tags(&rng.text(1));
    task.links = (0..rng.below(3)).map(|_| 1 + u32::try_from(rng.below(50)).unwrap()).collect();
    task.links.dedup();
    task.repo_links = (0..rng.below(2))
        .map(|_| RepoLink {
            repo: rng.pick(&["api", "web", "back-app"]).into(),
            id: 1 + u32::try_from(rng.below(9)).unwrap(),
        })
        .collect();
    task.repo_links.dedup();
    if rng.below(4) > 0 {
        task.description = sanitize_description(&rng.text(5)).0;
    }
    if task.kind == Kind::Decision {
        task.decision = [DecisionState::Accepted, DecisionState::Proposed, DecisionState::Rejected][rng.below(3)];
    }
    for _ in 0..rng.below(4) {
        let body = sanitize_comment(&rng.text(4)).0;
        if body.is_empty() {
            continue;
        }
        let mut comment = Comment::new(rng.time(t0), body, [Author::Human, Author::Agent][rng.below(2)]);
        comment.question = rng.below(4) == 0;
        comment.answer = !comment.question && rng.below(4) == 0;
        comment.edited = (rng.below(3) == 0).then(|| rng.time(t0));
        task.comments.push(comment);
    }
    if task.is_task() {
        let mut at = task.created;
        for _ in 0..rng.below(4) {
            let to = [Status::Doing, Status::Parked, Status::Done, Status::Todo][rng.below(4)];
            if to == task.status {
                continue;
            }
            at += Duration::minutes(1 + i64::try_from(rng.below(300)).unwrap());
            task.set_status(to, at, [Author::Human, Author::Agent][rng.below(2)]);
        }
    }
    task
}

#[test]
fn every_entry_the_api_can_make_reads_back_as_written() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for id in 1..=2000 {
        let task = random_task(&mut rng, id);
        let written = task.to_markdown();
        let read = Task::from_markdown(&written).unwrap_or_else(|e| panic!("{e}\n---\n{written}"));
        assert_eq!(read.to_markdown(), written, "written again differently");
        let fields = |t: &Task| (t.id, t.kind, t.title.clone(), t.status, t.decision, t.tags.clone(), t.links.clone());
        assert_eq!(fields(&read), fields(&task), "\n{written}");
        assert_eq!(read.repo_links, task.repo_links, "\n{written}");
        assert_eq!(read.description, task.description, "\n{written}");
        assert_eq!(read.comments, task.comments, "\n{written}");
        assert_eq!(read.history, task.history, "\n{written}");
        assert_eq!(read.sessions, task.sessions, "\n{written}");
    }
}

#[test]
fn a_hand_edited_file_settles_after_one_save() {
    // Whatever a hand edit leaves, saving once writes the entry as kanbrain reads it; saving that again changes
    // nothing: a read that drops or moves something shows up as a second save that differs.
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..3000 {
        let text = format!(
            "# {}\n\n- id: 1\n- tags: {}\n{}\n\n## Description\n\n{}\n",
            rng.text(1),
            rng.text(1),
            rng.text(3),
            rng.text(6)
        );
        let Ok(first) = Task::from_markdown(&text) else { continue };
        let saved = first.to_markdown();
        let again = Task::from_markdown(&saved).unwrap_or_else(|e| panic!("{e}\n---\n{saved}"));
        assert_eq!(again.to_markdown(), saved, "\nfrom:\n{text}");
        assert_eq!(again.description, first.description, "\nfrom:\n{text}");
    }
}

#[test]
fn any_text_reads_as_an_entry_or_an_error_never_a_crash() {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    for _ in 0..3000 {
        let mut text = rng.text(12);
        // Cut anywhere, as a crash or a sync cut a file short.
        let cut = rng.below(text.len() + 1);
        while !text.is_char_boundary(cut.min(text.len())) {
            text.pop();
        }
        text.truncate(cut.min(text.len()));
        let _ = Task::from_markdown(&format!("# t\n\n- id: 1\n{text}"));
        let _ = Task::from_markdown(&text);
    }
}

/// A task with one change of each kind: a person's, then an agent's, a minute apart.
fn with_activity() -> Task {
    let t0 = parse_ts("2026-10-16 10:00").unwrap();
    let mut task = Task::new(3, "Fix login".into(), t0);
    let tags = |added: &[&str], removed: &[&str]| Changed::Tags {
        added: added.iter().map(ToString::to_string).collect(),
        removed: removed.iter().map(ToString::to_string).collect(),
    };
    task.activity = vec![
        Activity::new(
            t0,
            Author::Human,
            Changed::Title { from: "Fix it -> now (agent)".into(), to: "Fix login".into() },
        ),
        Activity::new(t0 + Duration::minutes(1), Author::Agent, tags(&["ux", "-odd"], &["bug"])),
        Activity::new(
            t0 + Duration::minutes(2),
            Author::Agent,
            Changed::Related { added: vec!["5".into(), "shop-api#3".into()], removed: vec!["9".into()] },
        ),
        Activity::new(t0 + Duration::minutes(3), Author::Human, Changed::Description),
    ];
    task
}

#[test]
fn activity_is_written_after_the_comments_one_line_a_change() {
    let text = with_activity().to_markdown();
    let at = |minutes| fmt_ts(parse_ts("2026-10-16 10:00").unwrap() + Duration::minutes(minutes));
    let expected = format!(
        "## Comments\n\n## Activity\n\n\
         - {} title\n  - from: Fix it -> now (agent)\n  - to: Fix login\n\
         - {} tags (agent): +ux +-odd -bug\n\
         - {} related (agent): +5 +shop-api#3 -9\n\
         - {} description\n\n## History\n",
        at(0),
        at(1),
        at(2),
        at(3)
    );
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn activity_reads_back_as_it_was_written() {
    let task = with_activity();
    let back = Task::from_markdown(&task.to_markdown()).unwrap();
    assert_eq!(back.activity, task.activity);
    assert!(back.unknown.activity.is_empty() && back.unknown.sections.is_empty());
    let note = Task { kind: Kind::Note, ..task };
    assert_eq!(Task::from_markdown(&note.to_markdown()).unwrap().activity, note.activity, "notes have it too");
}

#[test]
fn an_entry_without_activity_has_no_activity_section() {
    let task = Task::new(3, "Fix login".into(), parse_ts("2026-10-16 10:00").unwrap());
    assert!(!task.to_markdown().contains("## Activity"), "files stay as they were");
}

#[test]
fn activity_an_older_version_moved_to_the_end_still_reads() {
    // Versions before 0.6.0 keep the section as one they don't know, and write it after the others.
    let task = with_activity();
    let text = task.to_markdown();
    let start = text.find("## Activity").unwrap();
    let end = text.find("## History").unwrap();
    let moved = format!("{}{}\n{}", &text[..start], &text[end..], &text[start..end]);
    let back = Task::from_markdown(&moved).unwrap();
    assert_eq!(back.activity, task.activity);
    assert!(back.unknown.sections.is_empty(), "{moved}");
}

#[test]
fn activity_lines_this_version_cant_read_are_kept_as_written() {
    let lines = [
        "- 2026-10-16T10:00:00+02:00 status (agent): todo -> doing",
        "- 2026-10-16T10:00:00+02:00 tags (bot): +ux",
        "- 2026-10-16 10:00 tags: +ux",
        "- 2026-10-16T10:00:00+02:00 tags: ux",
        "- 2026-10-16T10:00:00+02:00 tags: +",
        "- 2026-10-16T10:00:00+02:00 tags:",
        "- 2026-10-16T10:00:00+02:00 description: shorter",
        "- 2026-10-16T10:00:00+02:00 title\n  - from: a",
        "- 2026-10-16T10:00:00+02:00 title\n  - from: a\n  - to: b\n  - by: c",
        "- 2026-10-16T10:00:00+02:00 description\n  more",
    ];
    let known = "- 2026-10-16T11:00:00+02:00 description (agent)";
    let text = format!("# x\n\n- id: 1\n\n## Activity\n\n{}\n{known}\n", lines.join("\n"));
    let task = Task::from_markdown(&text).unwrap();
    assert_eq!(task.activity.len(), 1, "{:?}", task.activity);
    assert_eq!((task.activity[0].by, &task.activity[0].change), (Author::Agent, &Changed::Description));
    assert_eq!(task.unknown.activity, lines);
    let again = task.to_markdown();
    for line in lines {
        assert!(again.contains(&format!("{line}\n")), "{line} kept: {again}");
    }
    assert_eq!(Task::from_markdown(&again).unwrap().unknown.activity, lines, "and kept again");
}

#[test]
fn text_a_person_wrote_in_an_activity_section_is_kept() {
    // An `## Activity` of someone's own, before 0.6.0 gave the name a meaning: prose, paragraphs, a list.
    let prose = "Some prose I wrote\n\nA second paragraph\n  indented";
    let text = format!("# x\n\n- id: 1\n\n## Activity\n\n{prose}\n- not a change\n");
    let task = Task::from_markdown(&text).unwrap();
    assert_eq!(task.unknown.activity, [prose, "- not a change"]);
    let again = task.to_markdown();
    assert!(again.contains(&format!("## Activity\n\n{prose}\n- not a change\n")), "{again}");
    assert_eq!(Task::from_markdown(&again).unwrap().unknown.activity, task.unknown.activity, "and kept again");
}
