# 14. Times with jiff, not chrono

Date: 2026-11-18. Status: accepted.

## Context

chrono, which the core and the app kept every time in, was declared unmaintained, with no release to move to
([RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543)), and the CI's `cargo deny` fails on it. The
advisory names two alternatives, jiff and time. Its types were in the core's public API: `Task`'s times, `Period`,
`Report`, `now`, `parse_ts`.

kanbrain needs more than instants: a time typed by hand (`2026-09-24 10:12`) and a report period's bounds (a day's,
a week's midnight) are local civil times to turn into instants, which takes the time zone's rules, DST's among them.
time 0.3.55 has only offsets from UTC, the local one at a given instant
([`UtcOffset::local_offset_at`](https://docs.rs/time/0.3.55/time/struct.UtcOffset.html#method.local_offset_at)), not
the rules to find which offset a civil time has. jiff 1.0.1 has the system's time zone
([`TimeZone::system`](https://docs.rs/jiff/1.0.1/jiff/tz/struct.TimeZone.html#method.system), from `TZ`, else the
system's setting) with a time zone database (the system's on Unix, one bundled on Windows: the default features'
`tzdb-bundle-platform`, [features](https://docs.rs/jiff/1.0.1/jiff/index.html#crate-features)).

## Decision

- The core and the app use jiff 1.0: an instant is a
  [`Timestamp`](https://docs.rs/jiff/1.0.1/jiff/struct.Timestamp.html) (in `DateTime<Local>`'s place, also `Copy` and
  ordered), a civil date or time `jiff::civil`'s, a length of time a
  [`SignedDuration`](https://docs.rs/jiff/1.0.1/jiff/struct.SignedDuration.html) (subtracting two `Timestamp`s gives
  one, as two chrono times gave a `chrono::Duration`), and steps of the calendar (a day, a month) a `Span`.
- A `Timestamp` shows in UTC: whatever's shown or written in local time goes through `model::local` first (as
  `DateTime<Local>` did by itself).
- Stored timestamps are written as they were, RFC 3339 with the local offset, `%Y-%m-%dT%H:%M:%S%:z`
  ([`strftime`](https://docs.rs/jiff/1.0.1/jiff/fmt/strtime/index.html)), and read as before: no data format change.
- A civil time turns into an instant by RFC 5545's rule (`model::local_moment`,
  [`AmbiguousTimestamp::compatible`](https://docs.rs/jiff/1.0.1/jiff/tz/struct.AmbiguousTimestamp.html#method.compatible)):
  in the hour a DST change repeats, the first of the two; in the hour it skips, as many minutes past the change, so
  the change's own minute (a period's edge, `02:00`) is the moment it happened, as chrono 0.4.45 had it. chrono's
  [`earliest`](https://docs.rs/chrono/0.4.45/chrono/offset/type.MappedLocalTime.html#method.earliest) gave none
  within the hour skipped, so a file with such a time typed by hand didn't read; and in the hour repeated, the
  second of the two on Linux and macOS (it orders them by offset, `offset/local/tz_info/timezone.rs`), the first on
  Windows.
- GPUI still depends on chrono, so its advisory stays ignored in `deny.toml`, for it alone.

## Consequences

- kanbrain-core's public API changes its time types: 0.7.0 (Cargo's convention for 0.y). Timestamps are written
  and read as before (`stored_timestamps_are_written_as_they_always_were`, and `e2e/upgrade.sh`, which compares each
  release's `kb --json` output with this one's on the same brain), and so are times typed by hand away from a DST
  change.
- Around a DST change, on Linux and macOS: a time typed in the hour repeated reads as the first of the two, and a
  report of hours puts both repeated hours in the row of the first; a time typed in the hour skipped reads instead
  of making its file unreadable (`a_time_typed_in_an_hour_a_dst_change_skips_or_repeats_reads`).
- A `TZ` that names no time zone reads as UTC, where chrono fell back to the system's setting; on Windows, where
  chrono ignored `TZ`, it's taken. On Windows the zone's rules come from the database bundled in kanbrain's build,
  not the system's: a country changing its rules needs a newer kanbrain.
- jiff's parser ([`Timestamp`'s `FromStr`](https://docs.rs/jiff/1.0.1/jiff/struct.Timestamp.html#impl-FromStr-for-Timestamp))
  also reads a timestamp with its seconds left out; it reads at most nine digits of a fraction of a second (chrono
  dropped the rest) and a leap second `:60` as `:59`. kanbrain writes neither. An offset with seconds, which local
  time had before about 1972 in some zones, is written with them (`-00:14:44`), which kanbrain before 0.7.0 can't
  read; chrono rounded it to the minute.
- One more dependency in the tree until GPUI moves off chrono.
