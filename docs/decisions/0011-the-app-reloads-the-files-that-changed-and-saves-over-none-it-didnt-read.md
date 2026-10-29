# 11. The app reloads the files that changed, and saves over none it didn't read

Date: 2026-10-29. Status: accepted.

## Context

At 10,000 tasks, a file changed on disk took the app 98 ms to take in: the watcher listed every data folder and asked
each file's time on the window's thread, every second and after each key, and a change anywhere read the whole brain
again, with the other repositories of its project. Markdown stays the only source of truth, so there's no index on
disk to ask instead: the folders have to be listed to find what changed.

## Decision

- The window's timer lists the folders on GPUI's background executor
  ([`BackgroundExecutor`](https://github.com/zed-industries/zed/blob/v1.23.2/crates/gpui/src/executor.rs)) and hands
  the listing to the watcher (`watch::Watcher::noticed`), which reports, for each folder, the files added, removed or
  changed (`watch::Changes`).
- A key or a write of the board's own doesn't list a folder whose time hasn't changed since a tick after it was listed
  (`watch::unchanged`). Every save, kb's and the app's, writes a temporary file and renames it, which marks the folder
  as changed ([POSIX `rename`](https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html)). The tick is
  2 s on a file system keeping times to the second or two
  ([FAT's write time](https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times)), seen as a folder time
  with no nanoseconds, and 50 ms otherwise: a change in the same tick as the listing could leave the time as it was.
- The board reads again only the files that changed (`Brain::reload_files`), then repairs what a full load would:
  shared ids, one task doing per person, dangling links, tag colors. A file that isn't an entry (`format`, `last-id`,
  the tag colors), or one outside the folder, reads the whole brain again. The other repositories of the project are
  only repaired, not read.
- A file written over in place, as some editors do, leaves the folder's time as it was: only the timer's listing, a
  second later at most, finds it. Meanwhile, the store refuses to save over a file changed or deleted since it read or
  wrote it (`Store::save`), comparing its time and length
  ([`Metadata::modified`](https://doc.rust-lang.org/1.88.0/std/fs/struct.Metadata.html#method.modified)): written
  from memory, what the other program wrote would be lost. The app says so, and the next reload has it.

## Consequences

- Measured at 10,000 tasks in GPUI's test window, in the release build (`gui/tests/perf.rs`): a file changed and
  reloaded takes 30 ms rather than 98, 8.5 of them listing the folders, which the app does away from the window; a
  key in the search 7.3 ms rather than 27.
- kb's saves get the same refusal: a command that reads, then saves after another program wrote the file, fails
  rather than writing over it, and running it again reads the new file.
- A store that never read a file (one opened to write an entry it made) saves over it as before: the refusal only
  knows the files it read.
