# Multi-tier clipboard fallback and description editor shortcut discoverability

Clipboard operations use a tiered fallback model: text copied or cut inside the application is staged in an in-memory fallback buffer (`app::INTERNAL_CLIPBOARD: Mutex<Option<String>>`) before attempting synchronization with the operating system clipboard (`arboard`) and terminal OSC 52. Pasting prioritizes the system clipboard, transparently falling back to the internal buffer if the system clipboard is unavailable, empty, or returns an error. Bracketed paste sequences from the terminal emulator (`CrosstermEvent::Paste`) are surfaced as `AppEvent::Paste` and dispatched to the active `InsertHandler` or command query. Description editor shortcuts are made discoverable through a non-destructive contextual help workflow (`InsertOutcome::Help`, accessible via `F1` or `Ctrl+H`), dynamic bottom modal border hints (`F1 / Ctrl+H:help`), and status bar guidance without forcing `tui-textarea` into static keymap tables.

## Why

Terminal environments are heterogeneous and often hostile to desktop clipboard APIs:
1. **Clipboard unavailability:** In headless Linux servers, Docker containers, remote SSH sessions without X11/Wayland forwarding, or locked-down sandboxes, `arboard::Clipboard::new()` fails immediately. Without an internal fallback buffer, copy, cut, and paste fail silently, preventing users from copying or duplicating text even within the same running application session.
2. **Keybinding hijacking by underlying widgets:** `tui-textarea` defaults `Ctrl+V` to `PageDown` rather than paste. Users attempting to paste with `Ctrl+V` (or `Cmd+V` on macOS terminals reporting super/meta modifiers) experienced unexpected cursor jumps instead of pasting.
3. **Dropped bracketed paste events:** When users paste multi-line text into a modern terminal emulator, the terminal sends bracketed paste escape sequences (`CrosstermEvent::Paste(text)`). Because the event loop previously only listened for `CrosstermEvent::Key`, external paste events were silently discarded.
4. **Discoverability gap in edit mode:** ADR-0005 intentionally excluded the description editor from static `KEYMAP` tables because `MarkdownEditor` relies heavily on modifier chords, text selection, and dynamic text continuation logic. However, because standard modal help was triggered by `?` (which enters literal text in an editor), users had no way to inspect editor shortcuts (such as bold `Ctrl+B`, italic `Ctrl+I`, code `Ctrl+K`, list continuation, indentation `Tab`/`Shift+Tab`, undo/redo `Ctrl+Z`/`Ctrl+Shift+Z`, selection navigation `Shift+Arrows`, and cut/copy/paste) without exiting the editor and losing their cursor context.

## Considered and rejected

- **Force description editor into static `KEYMAP` tables (re-evaluating ADR-0005).** Rejected — `tui-textarea` manages text entry, cursor positioning, and selection state internally. Mapping text editing keystrokes through static dispatch tables would add excessive indirection without capturing text mutation subtleties or custom chords. Hand-authored help sections paired with explicit key matching keep the editor self-contained.
- **Rely solely on system clipboard (`arboard`).** Rejected — terminal users frequently run inside SSH, tmux, or containers where OS clipboard daemons do not exist. An internal fallback ensures intra-app copy/cut/paste always works reliably.
- **Rely solely on bracketed paste without key intercepts (`Ctrl+V` / `Cmd+V`).** Rejected — many terminal emulators or key configurations send physical `Ctrl+V` keystrokes rather than bracketed paste; relying exclusively on bracketed paste leaves users on those terminals stranded with `PageDown`.
- **Exit edit mode when opening help.** Rejected — discarding or stashing editor state to show help creates poor UX. Returning `InsertOutcome::Help` keeps the `Box<dyn InsertHandler>` alive on `App` while switching `AppMode` to `Help`; closing help immediately restores `AppMode::Insert` with the editor buffer, cursor, and selection intact.
- **Status-bar-only hint for help.** Rejected — during card description editing, the user's attention is focused on the centered modal dialog. Displaying ` F1 / Ctrl+H:help ` directly on the bottom border of the card detail modal (`title_bottom`) places the shortcut hint directly in the user's line of sight.

## Consequences

- Copy and cut operations within `MarkdownEditor` populate both the internal application clipboard and the system clipboard.
- Paste works seamlessly whether triggered by `Ctrl+V`, `Cmd+V`, `Shift+Insert`, terminal bracketed paste, or external OS copy.
- If the OS clipboard is unavailable or fails, intra-app copy/cut/paste remains 100% functional via `INTERNAL_CLIPBOARD`.
- `MarkdownEditor` handles standard selection expansion (`Shift+Arrows`), `Cmd+A` (select all), and clipboard operations (`Ctrl+C`, `Ctrl+X`, `Ctrl+V`).
- Users can press `F1`, `Ctrl+H`, or `Ctrl+?` at any point during description editing to view the full categorized shortcut cheat sheet, and return to editing with `Esc`, `q`, `?`, `F1`, or `Ctrl+H` without losing changes.
- Snapshot tests verify both the editor modal bottom hint and the dedicated description editor help overlay.
