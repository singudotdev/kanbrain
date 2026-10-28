//! How long the window takes over a big board, for the performance work to measure itself by: frames, a scroll, a
//! search key and a file another program changed, at a few board sizes. Ignored by default and timed only in a
//! release build (`cargo test --release -p kanbrain-app perf -- --ignored --nocapture`); they print the times rather
//! than assert them, since machines differ, and assert only that the window settles.

use super::*;
use crate::state::Board;
use kanbrain_core::model::{Author, Comment, Kind, Status, Task, fmt_ts, now};
use std::time::{Duration, Instant};

/// Tags the generated entries take theirs from.
const TAGS: [&str; 12] = ["auth", "api", "ui", "db", "perf", "bug", "docs", "ci", "infra", "web", "mobile", "ops"];

/// Writes a board of `todo` tasks to do, `done` done ones and `notes` notes into `dir`, as markdown files the way the
/// store names them, with tags, links, comments and time worked, and one task running.
pub(super) fn generate(dir: &std::path::Path, todo: u32, done: u32, notes: u32) {
    let at = now() - chrono::Duration::days(30);
    for id in 1..=todo + done + notes {
        let mut task = Task::new(id, format!("task {id} with a title about as long as most"), at);
        let n = id as usize;
        task.tags = vec![TAGS[n % TAGS.len()].to_string(), TAGS[(n / 7) % TAGS.len()].to_string()];
        task.tags.dedup();
        if id > 1 && id % 10 == 0 {
            task.links = vec![id - 1];
        }
        if id % 5 == 0 {
            task.comments.push(Comment::new(at, format!("a comment on #{id}"), Author::Agent));
        }
        // Sessions as the file has them: the model makes none but by starting a task.
        let mut sessions = String::new();
        if id > todo + done {
            task.kind = Kind::Note;
        } else if id > todo {
            task.status = Status::Done;
            let end = at + chrono::Duration::hours(1);
            sessions = format!("\n## Sessions\n\n- {} -> {} (1h 00m)\n", fmt_ts(at), fmt_ts(end));
        } else if id == todo {
            task.status = Status::Doing;
            sessions = format!("\n## Sessions\n\n- {} -> running\n", fmt_ts(now() - chrono::Duration::minutes(5)));
        }
        let file = dir.join(format!("{id:04}-task-{id}-with-a-title-about-as-long-as-most.md"));
        std::fs::write(file, task.to_markdown() + &sessions).unwrap();
    }
}

/// The time `act` takes, on average over `times` runs.
fn average(times: u32, mut act: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..times {
        act();
    }
    start.elapsed() / times
}

/// Opens a window over a board of `todo` tasks to do (and as many done tasks and notes as a fifth of them), the app's
/// own size, and prints what each thing the window does takes.
fn measure(cx: &mut gpui::TestAppContext, todo: u32) {
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path(), todo, todo / 5, todo / 5);
    let opening = Instant::now();
    let board = Board::open(dir.path(), None).unwrap();
    let opened = opening.elapsed();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.simulate_resize(gpui::size(px(1400.), px(860.)));
    next_frames(cx);
    redraw(cx);
    let shown = opening.elapsed();
    let frame = average(20, || redraw(cx));
    let scroll = average(20, || {
        wheel(&view, cx, gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-200.))), false);
    });
    let idle = average(5, || {
        cx.executor().advance_clock(crate::watch::INTERVAL);
        cx.run_until_parked();
    });
    // What another program's write to one entry takes the window: noticing it, reading it again, and drawing it.
    let path = dir.path().join(format!("{todo:04}-task-{todo}-with-a-title-about-as-long-as-most.md"));
    let mut round = 0;
    let reload = average(5, || {
        round += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("## Description", &format!("## Description\n\nround {round}"))).unwrap();
        cx.executor().advance_clock(crate::watch::INTERVAL);
        cx.run_until_parked();
        next_frames(cx);
    });
    cx.simulate_keystrokes("/");
    next_frames(cx);
    let mut key = 0;
    let search = average(6, || {
        key += 1;
        cx.simulate_keystrokes(if key % 2 == 1 { "4" } else { "backspace" });
        next_frames(cx);
    });
    eprintln!(
        "PERF todo={todo}: open {opened:?}, open+shown {shown:?}, frame {frame:?}, scroll {scroll:?}, idle poll \
         {idle:?}, one file reloaded {reload:?}, search key {search:?}"
    );
}

#[gpui::test]
#[ignore = "a benchmark: run in release with --ignored --nocapture"]
fn perf_a_board_of_1000(cx: &mut gpui::TestAppContext) {
    measure(cx, 1000);
}

#[gpui::test]
#[ignore = "a benchmark: run in release with --ignored --nocapture"]
fn perf_a_board_of_10000(cx: &mut gpui::TestAppContext) {
    measure(cx, 10_000);
}
