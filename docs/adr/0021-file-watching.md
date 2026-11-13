# File watching: reload on change, polling as fallback

## Decision

The TUI learns about external changes (CLI, `git pull`, another tct) from a native file watcher instead of waiting for the 15 s poll (`src/watch/`).

- **`notify` + `notify-debouncer-mini`** watch the whole data directory recursively (FSEvents, inotify, …) — no external binary, no subprocess. Events are debounced for 200 ms, which collapses the `.tmp` write + rename of one atomic save into one event.
- **`App.reload_pending`** is set by the watcher and consumed by `on_tick` when the mode allows a reload (board view, detail, help; not editing/dialog/grab). An event arriving in a deferred mode is therefore applied afterwards, never lost.
- **Polling stays.** Without a watcher the poll is the usual 15 s. With one it is a 5 min safety net (some filesystems — network, FUSE, some container mounts — start a watcher but deliver no events).
- **Fallback.** If the watcher cannot start (e.g. inotify limits) the app polls as before. If it reports an error or dies at runtime, the app drops it, reloads once and toasts "File watching stopped; polling instead".
- **Opt-out: `tct --no-watch`** forces the poll. It is a flag, not an environment variable or a file in `.tct` (the data directory is untrusted, see AGENTS.md).
- Our own saves trigger events too; the resulting reload is idempotent and preserves selection, so they are not filtered.

## Rejected

- **Shelling out to `fswatch`** — an optional external binary (rare on Linux, absent in the Docker image), a supervised subprocess, and two code paths per platform.
- **Dropping polling entirely** — a silent watcher (no events delivered) would mean a stale board with no recovery.
- **Watching only the open board's directory** — the watcher would need re-targeting on every board switch for little gain.

## Limits

Network/FUSE filesystems may deliver no events (the 5 min safety net applies; `--no-watch` gives 15 s). A data directory deleted and recreated while running is not re-watched.
