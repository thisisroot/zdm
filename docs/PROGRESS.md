# ZDM — Progress Notes

A fast, segmented download manager: Rust/Tauri backend, React/TypeScript frontend.
This file tracks what's been built and what's currently broken, so work can pick
up cold without re-deriving context.

## Architecture

```
crates/zdm-core/   Pure Rust download engine — no GUI dependency
src-tauri/         Tauri backend — app state, queue scheduler, SQLite persistence
src/               React + TypeScript UI
```

- **zdm-core** probes a URL for range support, splits the file into chunks pulled
  from a shared work queue by N concurrent workers (not fixed per-connection
  ranges, so a fast connection naturally picks up more work), and persists
  progress in a `.zdm.json` sidecar next to the destination file for resume.
- **src-tauri** wraps the engine: `AppState` holds the engine, a SQLite `Db`
  (lifecycle metadata only — the sidecar is the real resume mechanism), the
  in-memory `records: HashMap<Uuid, DownloadRecord>`, queues, and settings.
  `queue.rs` owns the scheduler (`try_promote_queue`) and the event forwarder
  that mirrors engine events into `DownloadRecord` state. `commands.rs` is the
  Tauri IPC surface.
- **src** is a fairly flat component tree (`App.tsx` owns most state) talking to
  the backend through `src/lib/api.ts`, a thin typed wrapper over `invoke()`.

## Features implemented

- Multi-connection segmented downloads with genuine per-connection progress
  (not simulated), adaptive retry/backoff on 429/5xx, and a concurrency gate
  that permanently shrinks a download's connection count on repeated 429s.
- Resume across app restart, pause, or connection loss, validated against the
  remote ETag/Last-Modified before trusting old progress.
- Queues: group downloads, bulk pause/resume/delete, drag-and-drop reorder.
- Batch/pattern downloading (`part[01-99].zip` → N files).
- Auto-categorized folders (11 categories, extension-based detection).
- Bulk selection and bulk pause/resume/remove on the download list.
- Clear-list actions (remove finished, with or without deleting files).
- Filename conflict handling (Replace / Rename dialog) before a download starts.
- Link validation before adding: probes URL(s) concurrently (bounded), shows
  total size, and lets broken links in a batch be dropped via a
  "Download Anyway" prompt instead of failing silently later.
- Custom titlebar, glassmorphism UI, light/dark theme, 5-color accent picker.
- System tray (close-to-tray, quit from tray menu).
- About panel: version, GitHub link, manual "check for updates" against the
  GitHub releases API.
- CI (GitHub Actions) builds Windows/macOS/Linux installers on tag push,
  publishes a draft GitHub release. Unsigned — see README for the Gatekeeper/
  SmartScreen workaround. Code-signing env vars were tried and reverted (see
  below); real signing needs paid certificates as repo secrets.

## Scheduler rework (in progress / under suspicion)

The original scheduler only ever filled *empty* slots — it never reconsidered
a download that was already `Downloading`. This meant:

- Drag-and-drop reordering a download above the concurrency limit had no
  effect on what was actually running.
- "Stop" on a queue only paused the one item that happened to be actively
  downloading; anything still `Queued` in that same queue was untouched, so
  the scheduler immediately promoted the next one — stop looked like it did
  nothing.
- The global pause/resume-all topbar button had the identical bug.

This was reworked so `try_promote_queue` recomputes the top-N (by `seq`, N =
`max_simultaneous_downloads`) among `Queued`/`Downloading` records on every
relevant event (add, remove, finish, reorder, settings change) and reconciles
both directions — demoting anything running that fell outside the window,
promoting anything queued that's now inside it. `toggle_queue`/`toggle_all`
were added as atomic backend commands so "stop" holds every member (not just
the running one) and "start" requeues held downloads and lets the scheduler
pick up to the concurrency cap rather than force-starting everything.
`resume_download`/`retry_download` were unified behind a shared
`resume_or_restart` helper that resumes a live paused task if one exists, or
falls back to disk-sidecar-resume-or-fresh-start — needed once a download
could be `Paused` without ever having had an engine task (e.g. held by
`toggle_queue` before it ever started).

**Fixed.** Root cause: `try_promote_queue`'s should-run loop trusted a
record's `Downloading` status at face value and never verified it actually
had a live task behind it (`if record.status != DownloadStatus::Queued {
continue; }`). A record can end up `Downloading` in `state.records` with no
corresponding entry in the engine's `tasks` map — e.g. the frontend's initial
`listen`/`invoke` calls and the backend's startup `resume_interrupted_downloads`
run concurrently, so an `add_download` landing while startup recovery is
still resolving an old `Downloading`/`Paused` record sees that record as
occupying a slot before recovery has confirmed (or corrected) it; the same
gap would also strand a slot if a task's terminal event was ever lost. Once
one record like that exists, it permanently occupies its slot in the top-N
window — `max_simultaneous_downloads` defaults to 1, so with just one such
record *every* subsequently added download (single or batch) sits `Queued`
forever, matching the report exactly.

Fix (`crates/zdm-core/src/engine.rs`, `src-tauri/src/queue.rs`): added
`DownloadEngine::is_active(id)` (checks the `tasks` map), and
`try_promote_queue` now falls through to `resume_or_restart` for a
`should_run` record marked `Downloading` when `is_active` says otherwise,
instead of trusting the status and moving on — self-healing the drift
instead of freezing on it. While in there, also fixed a related bug in the
same loop: the "resume a live paused task" path inside `resume_or_restart`
(`resume_silent`) deliberately emits no event of its own, but the success
branch here never called `publish()` either — so a record demoted then
re-promoted (drag-and-drop reorder, or a queue stop/start) could be
genuinely downloading again while the frontend still showed it stuck
`Queued`. Now published explicitly.

Reproduced and verified via `src-tauri/examples/repro_scheduler.rs` (a
manual `cargo run` harness, not a `cargo test` target — Wry's event loop
requires the process's actual main thread, which `cargo test` worker
threads aren't; see the file's header comment for why `tauri::test::mock_app`
couldn't be used instead). `SCENARIO=3` builds an `AppState` seeded with a
`Downloading` record that has no engine task and adds a new download on top
of it: before the fix it stayed `Queued` forever, after the fix it reaches
`Downloading`/`Completed`.

## Known gaps

- The scheduler fix above is covered by a manual repro script, not real
  `cargo test` coverage (see the file header for the main-thread blocker).
  If this needs proper CI coverage later, `queue.rs`'s `AppHandle`-taking
  functions (and the `add_download`/`add_batch_urls` commands) would need to
  become generic over `R: tauri::Runtime` so `tauri::test::mock_app()`
  (`MockRuntime`) can stand in for it — a real but separate refactor.
- macOS builds are unsigned and un-notarized; Gatekeeper will call the app
  "damaged" until the user clears the quarantine flag (`xattr -cr`, documented
  in the README) or real Apple/Windows signing certificates are added as repo
  secrets — not something obtainable without the user's own paid developer
  accounts.
- `resume_interrupted_downloads` (startup recovery) resumes both `Downloading`
  and `Paused` records unconditionally via `resume_from_disk`, which means a
  `Paused` download starts transferring again on app restart instead of
  staying paused. Pre-existing behavior, not yet fixed.
