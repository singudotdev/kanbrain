# 12. kb reads a brain side by side, and one entry's file alone

Date: 2026-10-30. Status: accepted.

## Context

Every `kb` command read every entry of the brain, one file after another, then repaired what it found: shared ids, one
task doing per person, dangling links, tag colors. Over 14,000 entries that took 54 ms a command (the median of 21, on
a warm page cache), and 40 ms of it was reading the files. Reading the folder alone, sixteen threads at once, takes
7 ms; listing it, 1.4 ms. Markdown stays the only source of truth, with no index on disk to ask instead.

A command can't always read its entry alone: `show` lists the entries linking to it, which any file may hold, and
`start` looks among the others for a person's task in doing. The repairs need every entry too.

## Decision

- `Store::load_entries` reads the files on up to four threads
  ([`std::thread::scope`](https://doc.rust-lang.org/1.88.0/std/thread/fn.scope.html)), in shares of at least 512
  files (the last one aside), so a small brain is read by the calling thread alone; a share no thread can be had for
  ([`Builder::spawn_scoped`](https://doc.rust-lang.org/1.88.0/std/thread/struct.Builder.html#method.spawn_scoped)),
  there too. With four, `kb show` over 14,000 entries took as long as with eight (36 ms), and with one, 53. Each file
  is opened once, its time and length taken from the open file
  ([`File::metadata`](https://doc.rust-lang.org/1.88.0/std/fs/struct.File.html#method.metadata)) before it's read,
  rather than looked up by its path twice.
- A command that reads and changes one entry and no other (`comment`, `ask`, `describe`, a move to a status other than
  doing, `wait`), said as a line rather than JSON, reads only the files named for its id (`0007-…`):
  `Brain::open_for`. Only when one file is named for it, reads, and holds that id; otherwise, renamed by hand, a sync
  conflict's copy beside it, or broken, the whole brain is read and repaired as before, so the copy gets an id of its
  own and the broken file is said. The brain is migrated first, as a whole read does.
- Those commands repair nothing else: the next command that reads the whole brain does (`list`, `context`, `show`,
  `start`, `add`, an agent's session start). The app repairs as it reads too.
- `wait` reads its entry's file each second, and every entry only when no file named for it holds it.

## Consequences

- At 14,000 entries, `show`, `list` and `context` take 36 to 40 ms instead of 54 (the median of runs measured apart,
  the thread counts compared in one); `done` without a change, 16 ms instead of 54. A write adds the file's flush to disk, which the command waits for: about 27 ms on a spinning disk.
- An entry whose id another file shares under another name (not a sync tool's copy, which keeps the name) is commented
  on before the next whole read renumbers one of the two.
- A broken file elsewhere in the brain isn't said by those commands: they never read it.
