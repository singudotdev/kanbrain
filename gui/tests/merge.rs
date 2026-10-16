use super::*;

fn conflicted(merged: Merged) -> Merge {
    match merged {
        Merged::Conflicted(merge) => merge,
        Merged::Clean(text) => panic!("merged cleanly, to {text:?}"),
    }
}

#[test]
fn changes_to_different_lines_merge() {
    let base = "Plan\n\n- one\n- two\n- three";
    let mine = "Plan, mine\n\n- one\n- two\n- three";
    let theirs = "Plan\n\n- one\n- two\n- three, done";
    assert_eq!(merge(base, mine, theirs), Merged::Clean("Plan, mine\n\n- one\n- two\n- three, done".into()));
    // Lines added and removed, on both sides.
    let theirs = "Plan\n\n- one\n- three\n- four";
    assert_eq!(merge(base, mine, theirs), Merged::Clean("Plan, mine\n\n- one\n- three\n- four".into()));
}

#[test]
fn the_last_line_merges_like_any_other() {
    // Without a final line break, "three" and "three\n" would be different lines: a line added after it collided.
    assert_eq!(
        merge("one\nthree", "one\nthree\nfour", "zero\none\nthree"),
        Merged::Clean("zero\none\nthree\nfour".into())
    );
    // A final line break typed is kept, and one taken away is gone.
    assert_eq!(merge("a\nb\nc", "a\nb\nc\n", "a, theirs\nb\nc"), Merged::Clean("a, theirs\nb\nc\n".into()));
    assert_eq!(merge("a\nb\nc\n", "a\nb\nc", "a, theirs\nb\nc\n"), Merged::Clean("a, theirs\nb\nc".into()));
}

#[test]
fn changes_that_only_touch_merge() {
    // Neighbouring lines.
    assert_eq!(merge("a\nb\nc", "a\nB\nc", "a\nb\nC"), Merged::Clean("a\nB\nC".into()));
    // The last line edited while the agent adds one after it, and the other way round.
    assert_eq!(merge("a\nb", "a\nb!", "a\nb\nc"), Merged::Clean("a\nb!\nc".into()));
    assert_eq!(merge("a\n\nb", "a\n\nb\n\nc", "a\n\nb!"), Merged::Clean("a\n\nb!\n\nc".into()));
    // A line added before one the other side changed comes first.
    assert_eq!(merge("a\nb", "a\nnew\nb", "a\nB"), Merged::Clean("a\nnew\nB".into()));
    assert_eq!(merge("a\nb", "a\nB", "a\nnew\nb"), Merged::Clean("a\nnew\nB".into()));
    // Lines added at one place on both sides, or within lines the other changed, still collide.
    assert!(matches!(merge("a\nb", "a\nmine\nb", "a\ntheirs\nb"), Merged::Conflicted(_)));
    assert!(matches!(merge("a\nb\nc", "a\nb\nadded\nc", "a\nB\nC"), Merged::Conflicted(_)));
    assert!(matches!(merge("a\nb\nc", "a\nB\nC", "a\nb\nadded\nc"), Merged::Conflicted(_)));
}

#[test]
fn the_same_change_on_both_sides_is_no_conflict() {
    assert_eq!(merge("a\nb", "a\nB", "a\nB"), Merged::Clean("a\nB".into()));
    assert_eq!(merge("", "", "theirs"), Merged::Clean("theirs".into()));
}

#[test]
fn lines_both_changed_are_a_conflict_with_the_rest_settled() {
    let merge = conflicted(merge("title\na\nb\nc", "title, mine\na\nB, mine\nc", "title\na\nB, theirs\nc"));
    assert_eq!(
        merge.regions,
        vec![
            Region::Settled("title, mine\na\n".into()),
            Region::Conflict(Conflict { mine: "B, mine\n".into(), theirs: "B, theirs\n".into(), pick: None }),
            Region::Settled("c\n".into()),
        ]
    );
    assert_eq!(merge.theirs, "title\na\nB, theirs\nc", "what it merged");
    assert_eq!(merge.conflicts().count(), 1);
    assert_eq!(merge.text(), None, "nothing picked yet");
    // Both typed into an empty text.
    let merge = conflicted(super::merge("", "mine", "the agent's plan"));
    assert_eq!(
        merge.regions,
        vec![Region::Conflict(Conflict { mine: "mine\n".into(), theirs: "the agent's plan\n".into(), pick: None })]
    );
}

#[test]
fn picks_make_the_text() {
    let mut merge = conflicted(merge("a\nb\nc\nd\ne", "A\nb\nc\nd\nE", "a2\nb\nc\nd\ne2"));
    assert_eq!(merge.conflicts().count(), 2);
    assert_eq!(merge.at, 0);
    // A pick moves on to the next conflict without one.
    merge.pick(Pick::Theirs);
    assert_eq!(merge.at, 1);
    assert_eq!(merge.text(), None);
    merge.pick(Pick::Both);
    assert_eq!(merge.text().as_deref(), Some("a2\nb\nc\nd\nE\ne2"));
    // With all picked, it stays, and picks again change the text.
    assert_eq!(merge.at, 1);
    merge.go(-1);
    merge.pick(Pick::Mine);
    assert_eq!(merge.at, 0, "nothing left to pick: it stays");
    assert_eq!(merge.text().as_deref(), Some("A\nb\nc\nd\nE\ne2"));
    // Picked out of order, from a click: the next one without a pick is the one before.
    let mut merge = conflicted(super::merge("a\nb\nc\nd\ne", "A\nb\nc\nd\nE", "a2\nb\nc\nd\ne2"));
    merge.pick_at(1, Pick::Mine);
    assert_eq!(merge.at, 0);
}

#[test]
fn moving_between_conflicts_stops_at_the_ends() {
    let mut merge = conflicted(merge("a\nb\nc\nd\ne", "A\nb\nc\nd\nE", "a2\nb\nc\nd\ne2"));
    merge.go(-1);
    assert_eq!(merge.at, 0);
    merge.go(1);
    merge.go(1);
    assert_eq!(merge.at, 1);
    // Out of range, from a stale click: nothing.
    merge.pick_at(2, Pick::Mine);
    assert_eq!(merge.text(), None);
}

#[test]
fn a_side_that_removed_the_lines_picks_nothing() {
    let mut merge = conflicted(merge("a\nb\nc", "a\nc", "a\nB\nc"));
    assert_eq!(merge.regions[1], Region::Conflict(Conflict { mine: String::new(), theirs: "B\n".into(), pick: None }));
    merge.pick(Pick::Mine);
    assert_eq!(merge.text().as_deref(), Some("a\nc"));
    merge.pick(Pick::Theirs);
    assert_eq!(merge.text().as_deref(), Some("a\nB\nc"));
}

#[test]
fn the_rest_takes_one_side_and_keeps_the_picks_made() {
    let mut merge = conflicted(merge("a\nb\nc\nd\ne", "A\nb\nc\nd\nE", "a2\nb\nc\nd\ne2"));
    merge.pick_at(1, Pick::Theirs);
    merge.pick_rest(Pick::Mine);
    assert_eq!(merge.text().as_deref(), Some("A\nb\nc\nd\ne2"));
}

#[test]
fn a_pick_moves_to_the_next_without_one_after_it() {
    let mut merge = conflicted(merge("a\nb\nc\nd\ne", "A\nb\nC\nd\nE", "a2\nb\nc2\nd\ne2"));
    assert_eq!(merge.conflicts().count(), 3);
    merge.pick_at(1, Pick::Mine);
    assert_eq!(merge.at, 2, "after it, not the first");
    merge.pick_at(2, Pick::Mine);
    assert_eq!(merge.at, 0, "none after it: the first");
    // What each pick takes.
    let takes = |pick| Conflict { mine: String::new(), theirs: String::new(), pick }.taken();
    assert_eq!(takes(None), (false, false));
    assert_eq!(takes(Some(Pick::Mine)), (true, false));
    assert_eq!(takes(Some(Pick::Theirs)), (false, true));
    assert_eq!(takes(Some(Pick::Both)), (true, true));
}
