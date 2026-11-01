# Viewport scroll reconciliation and session navigation persistence

List viewport scroll offsets are computed and reconciled during UI rendering via interior mutability (`LoadedBoard.scroll_offset: Vec<Cell<usize>>`), and persisted across board switches in memory for the duration of the application session (`App.scroll_offsets: HashMap<BoardId, Vec<usize>>`).

## Why

Card heights in list columns are dynamic: wrapping titles, label chips, and optional date stamps mean height cannot be predicted by ordinal index alone. Crucially, the available vertical space (`inner.height`) is only known during the ratatui render pass. Neither the domain model nor the input handlers have layout context.

Previously, `LoadedBoard.scroll_offset` was a plain `Vec<usize>`. Because `ui::render` receives `&App` immutably, the renderer calculated a local `scroll` offset to keep the selected card in view, but never wrote that value back to `board.scroll_offset`. Consequently:
1. `scroll` reset to 0 at the start of every frame.
2. While navigating down, whenever focus passed the initial viewport, the renderer pushed `scroll` down so the selected card sat at the bottom of the viewport.
3. When navigating back up, restarting `scroll` at 0 immediately triggered `selected_card >= scroll + max_visible`, immediately scrolling the viewport up by one item and pinning the cursor to the bottom row instead of letting focus move upward through the visible cards.
4. Switching boards (via `j`/`k` or the board selector) remembered the focused card ID (`App.focused_cards`), but discarded the scroll state of every list upon unloading the board, causing reopened boards to start with uninitialized scroll positions.

Using `Cell<usize>` per list slot provides interior mutability so the renderer can record the authoritative viewport scroll offset without requiring `&mut App`. The renderer preserves viewport bounds across frames: moving up moves focus across visible cards without scrolling until the cursor hits the top of the visible window. Furthermore, `App` tracks `scroll_offsets` alongside `focused_cards`, preserving full navigation and scroll context across board switches in the session.

## Considered and rejected

- **Calculate scroll offset in input handlers / Board Editor.** Rejected — `BoardEditor` has no knowledge of terminal dimensions, frame splits, or text-wrapping geometry. Simulating layout inside the editor would duplicate ratatui layout logic and violate separation of concerns between domain and presentation.
- **Pass `&mut App` or `&mut LoadedBoard` to `ui::render`.** Rejected — Ratatui's `Terminal::draw` API takes `FnOnce(&mut Frame)`, and rendering functions across ratatui idioms treat application state as read-only. `detail_max_scroll: Cell<usize>` already established this exact pattern in `LoadedBoard` for layout-derived scroll bounds.
- **Persist scroll state to disk in `board.json`.** Rejected — viewport scroll position is transient session state dependent on terminal size. Persisting terminal-specific scroll indices across process launches would lead to invalid offsets on different terminal sizes. Session-scoped in-memory tracking matches `focused_cards`.

## Consequences

- Scrolling up and down in lists behaves symmetrically: focus moves through the visible cards first; scrolling only begins once the cursor touches the viewport boundary.
- If list contents shrink or a card is archived from the bottom, the renderer shifts `scroll` up to ensure available vertical space is filled without blank gaps.
- Switching between boards and returning restores both the selected card and the list viewport offsets.
- Unit and snapshot tests can verify multi-step scroll navigation deterministically by exercising the render pass between key events.
