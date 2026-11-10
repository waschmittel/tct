use std::collections::HashMap;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use ratatui::style::Color;

use crate::board_editor::BoardEditor;
use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::id::{BoardId, CardId};
use crate::model::label::LabelColor;
use crate::model::list::CardList;
use crate::storage::board_store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    BoardSelector,
    Normal,
    CardDetail,
    /// Insert mode. The active **Insert Handler** lives on `App.insert`.
    Insert,
    Command,
    /// Modal dialog mode. The active dialog itself lives on `App.dialog`.
    Dialog,
    Help,
}

impl AppMode {
    /// Base modes are full-screen views the user can rest in. Everything
    /// else (Insert/Command/Dialog/Help) is a transient *overlay* whose
    /// state lives on a companion `App` field, so an overlay mode must
    /// never be recorded as a return target: restoring it without its
    /// companion state swallows all input (UI freeze).
    pub fn is_base(&self) -> bool {
        matches!(
            self,
            AppMode::BoardSelector | AppMode::Normal | AppMode::CardDetail
        )
    }
}

pub struct LoadedBoard {
    pub meta: BoardMeta,
    pub lists: Vec<CardList>,
    pub cards: HashMap<CardId, Card>,
    pub selected_list: usize,
    /// Per-list selection, as an ordinal into that list's *visible*
    /// (non-archived) cards — NOT a raw `card_ids` index. Archived cards remain
    /// in `card_ids` (ADR-0006) but are never selectable, so navigation,
    /// clamping, rendering, and `current_card_id` all index the visible
    /// sequence. Use `visible_cards(li, None)[ordinal]` to get the raw index.
    pub selected_card: Vec<usize>,
    /// Per-list viewport scroll offset (index of the topmost visible card),
    /// computed and updated by `list_widget::render` each frame based on list
    /// layout and selection. Interior-mutable because rendering takes `&App`.
    pub scroll_offset: Vec<std::cell::Cell<usize>>,
    pub detail_item_idx: usize,
    pub detail_scroll: usize,
    /// Effective max scroll of the description pane, reported by the
    /// renderer each frame (it depends on the popup layout, which only the
    /// renderer knows). Interior-mutable because rendering takes `&App`.
    /// Input clamps `detail_scroll` against this so scrolling up responds
    /// immediately even if the stored value ran past the rendered max.
    pub detail_max_scroll: std::cell::Cell<usize>,
}

impl LoadedBoard {
    pub fn current_card_id(&self) -> Option<&CardId> {
        let list = self.lists.get(self.selected_list)?;
        // `selected_card` is an ordinal into the *visible* (non-archived) cards,
        // not a raw `card_ids` index — archived cards stay in `card_ids` but are
        // never selectable (ADR-0006). Map the ordinal back to a raw index.
        let ord = *self.selected_card.get(self.selected_list)?;
        let raw = *self.visible_cards(self.selected_list, None).get(ord)?;
        list.card_ids.get(raw)
    }

    pub fn current_card(&self) -> Option<&Card> {
        let id = self.current_card_id()?;
        self.cards.get(id)
    }

    /// Indices (into `list.card_ids`) of the visible Cards of a List.
    /// Archived Cards are always hidden; with an active search, non-matching
    /// Cards are hidden too. This is the single source of truth for
    /// visibility — navigation, clamping, and rendering all consume it.
    pub fn visible_cards(&self, list_idx: usize, search: Option<&str>) -> Vec<usize> {
        let Some(list) = self.lists.get(list_idx) else {
            return vec![];
        };
        list.card_ids
            .iter()
            .enumerate()
            .filter(|(_, id)| {
                self.cards
                    .get(*id)
                    .map(|c| {
                        !c.archived
                            && search
                                .map(|q| c.matches_search(q, &self.meta.labels))
                                .unwrap_or(true)
                    })
                    .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn visible_card_count(&self, list_idx: usize) -> usize {
        self.visible_cards(list_idx, None).len()
    }

    pub fn clamp_selection(&mut self) {
        for i in 0..self.lists.len() {
            let count = self.visible_card_count(i);
            if count == 0 {
                self.selected_card[i] = 0;
            } else if self.selected_card[i] >= count {
                self.selected_card[i] = count - 1;
            }
        }
    }
}

pub struct App {
    pub mode: AppMode,
    pub previous_mode: Option<AppMode>,
    /// Folder of the last PDF export this session; the next export dialog starts there.
    pub export_dir: Option<std::path::PathBuf>,
    /// Set in a browser session (`TCT_WEB`, the Docker image): files written on
    /// the server are out of the user's reach, so exports go through this
    /// terminal stream to the page instead of a file dialog.
    pub web_sink: Option<Box<dyn std::io::Write>>,
    pub should_quit: bool,
    pub status_message: Option<(String, Instant)>,
    pub boards: Vec<BoardMeta>,
    pub selected_board_idx: usize,
    /// Board Editor for the currently open board (ADR-0001). Reads go via
    /// `App::board()`; mutations via `App::apply` or editor selection verbs.
    pub editor: Option<BoardEditor>,
    pub search_query: String,
    pub search_active: bool,
    pub label_filter: Option<LabelColor>,
    pub last_reload: Instant,
    pub reload_interval: Duration,
    /// Active modal **Dialog Kind** — present iff `mode == AppMode::Dialog`.
    pub dialog: Option<Box<dyn crate::dialog::Dialog>>,
    /// Active **Insert Handler** — present iff `mode == AppMode::Insert`.
    pub insert: Option<Box<dyn crate::insert::InsertHandler>>,
    /// Build-stamped version string, shown in the help overlay. Defaults to
    /// the compile-time [`crate::VERSION`]; tests pin it for stable goldens.
    pub version: &'static str,
    /// Data directory shown bottom-right in the status bar, home-shortened
    /// (e.g. `~/.tct`). Pinned in test builds for stable goldens (the real
    /// value would be a per-test temp dir).
    pub data_dir_display: String,
    /// Terminal capabilities the renderer degrades to (colors, glyphs).
    /// Defaults to full so tests are environment-independent; `main` sets
    /// the detected value before the first frame.
    pub caps: crate::term_caps::TermCaps,
    /// Grab state in the board view (`m`): while set, plain arrow keys
    /// move the selected card and all other keys are swallowed — the
    /// shift-free move path for terminals that don't report Shift+arrows
    /// (Linux console). Only `input/normal.rs` reads or clears it.
    pub grab_active: bool,
    /// Remembers the focused card for each board across board switches in the current session.
    pub focused_cards: HashMap<BoardId, CardId>,
    /// Remembers the scroll state of each list for each board across board switches in the current session.
    pub scroll_offsets: HashMap<BoardId, Vec<usize>>,
}

/// Resolved once at startup; the data dir cannot change mid-session.
#[cfg(not(test))]
fn data_dir_display() -> String {
    crate::storage::paths::display_base_dir()
}

/// Test builds run under per-test `TCT_DATA_DIR` temp dirs — pin the display
/// so golden screens stay deterministic.
#[cfg(test)]
fn data_dir_display() -> String {
    "~/.tct".into()
}

use std::sync::Mutex;

static INTERNAL_CLIPBOARD: Mutex<Option<String>> = Mutex::new(None);

/// Store text in the internal application clipboard (in-memory fallback).
pub fn set_internal_clipboard(text: String) {
    if let Ok(mut lock) = INTERNAL_CLIPBOARD.lock() {
        *lock = Some(text);
    }
}

/// Retrieve text from the internal application clipboard.
pub fn get_internal_clipboard() -> Option<String> {
    INTERNAL_CLIPBOARD.lock().ok().and_then(|lock| lock.clone())
}

/// Write text to the system clipboard via `arboard`, and always save in the internal application clipboard.
pub fn copy_to_system_clipboard(text: &str) {
    set_internal_clipboard(text.to_string());
    if let Ok(mut cb) = Clipboard::new() {
        let _ = cb.set_text(text);
    }
}

/// Read text from the system clipboard via `arboard`, falling back to the internal application clipboard.
pub fn get_from_system_clipboard() -> Option<String> {
    let system = Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok());
    match system {
        Some(s) if !s.is_empty() => Some(s),
        _ => get_internal_clipboard(),
    }
}

impl App {
    pub fn new(open_board_id: Option<BoardId>) -> anyhow::Result<Self> {
        board_store::open_data_dir()?;
        let boards = board_store::list_boards()?;
        let mut app = Self {
            mode: AppMode::BoardSelector,
            previous_mode: None,
            export_dir: None,
            web_sink: None,
            should_quit: false,
            status_message: None,
            boards,
            selected_board_idx: 0,
            editor: None,
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
            dialog: None,
            insert: None,
            version: crate::VERSION,
            data_dir_display: data_dir_display(),
            caps: crate::term_caps::TermCaps::full(),
            grab_active: false,
            focused_cards: HashMap::new(),
            scroll_offsets: HashMap::new(),
        };
        if let Some(board_id) = open_board_id {
            app.load_board(&board_id)?;
        }
        Ok(app)
    }

    /// Read-only view of the currently loaded board, if any.
    pub fn board(&self) -> Option<&LoadedBoard> {
        self.editor.as_ref().map(|e| e.board())
    }

    /// The active search query, if search is on.
    pub fn search(&self) -> Option<&str> {
        self.search_active.then_some(self.search_query.as_str())
    }

    /// Test-only mutable access to the Loaded Board.
    #[cfg(test)]
    pub fn board_mut(&mut self) -> Option<&mut LoadedBoard> {
        self.editor.as_mut().map(|e| e.board_mut())
    }

    pub fn on_tick(&mut self) {
        if let Some((_, instant)) = &self.status_message
            && instant.elapsed() > Duration::from_secs(6)
        {
            self.status_message = None;
        }

        if self.last_reload.elapsed() >= self.reload_interval && self.should_reload() {
            self.last_reload = Instant::now();
            self.try_reload_board();
        }
    }

    fn should_reload(&self) -> bool {
        self.editor.is_some()
            && matches!(
                self.mode,
                AppMode::Normal | AppMode::Help | AppMode::CardDetail
            )
            && self.insert.is_none()
            && !self.grab_active
    }

    fn try_reload_board(&mut self) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.reload().is_err() {
            // Board file gone — drop the editor and return to the selector.
            self.editor = None;
            let _ = self.reload_boards();
            self.mode = AppMode::BoardSelector;
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// `what` names the copied content in the status message, e.g.
    /// "title 'Fix login flow'" or "description of 'Fix login flow'".
    ///
    /// Tries the native system clipboard (`arboard`) first. When unavailable
    /// (e.g. inside a headless Docker container or remote SSH/ttyd session),
    /// falls back to emitting an OSC 52 terminal escape sequence so browser
    /// and remote terminals (`xterm.js`, iTerm2, kitty, WezTerm, Alacritty)
    /// can copy directly to the client clipboard.
    pub fn copy_to_clipboard(&mut self, text: String, what: &str) {
        set_internal_clipboard(text.clone());
        let native_res = Clipboard::new()
            .and_then(|mut cb| cb.set_text(&text))
            .map_err(|e| e.to_string());
        self.copy_to_clipboard_with(&text, what, native_res, &mut std::io::stdout().lock());
    }

    fn copy_to_clipboard_with(
        &mut self,
        text: &str,
        what: &str,
        native_result: Result<(), String>,
        osc52_out: &mut dyn std::io::Write,
    ) {
        match native_result {
            Ok(()) => {
                self.set_status(format!("Copied {what} to clipboard"));
            }
            Err(e) => {
                if self.caps.rich_glyphs && write_osc52_to(text, osc52_out).is_ok() {
                    self.set_status(format!("Copied {what} to clipboard using terminal"));
                } else {
                    self.set_status(format!("Clipboard not available: {}", e));
                }
            }
        }
    }

    fn remember_current_board_focus(&mut self) {
        let Some((board_id, card_id, offsets)) = self.board().map(|b| {
            (
                b.meta.id.clone(),
                b.current_card_id().cloned(),
                b.scroll_offset
                    .iter()
                    .map(|c| c.get())
                    .collect::<Vec<usize>>(),
            )
        }) else {
            return;
        };
        if let Some(card_id) = card_id {
            self.focused_cards.insert(board_id.clone(), card_id);
        } else {
            self.focused_cards.remove(&board_id);
        }
        self.scroll_offsets.insert(board_id, offsets);
    }

    pub fn load_board(&mut self, board_id: &BoardId) -> anyhow::Result<()> {
        self.remember_current_board_focus();
        let mut editor = BoardEditor::load(board_id)?;
        if let Some(card_id) = self.focused_cards.get(board_id) {
            editor.focus_card(card_id, self.search());
        }
        if let Some(offsets) = self.scroll_offsets.get(board_id) {
            for (i, &offset) in offsets.iter().enumerate() {
                if let Some(cell) = editor.board().scroll_offset.get(i) {
                    cell.set(offset);
                }
            }
        }
        let repaired = editor.diagnostics.len();
        self.editor = Some(editor);
        self.mode = AppMode::Normal;
        if repaired > 0 {
            self.set_status(format!(
                "Repaired {repaired} reference issue(s) on load — see card order/labels"
            ));
        }
        Ok(())
    }

    /// Close the open board and return to the Board Selector.
    pub fn close_board(&mut self) -> anyhow::Result<()> {
        self.remember_current_board_focus();
        self.editor = None;
        self.reload_boards()?;
        self.mode = AppMode::BoardSelector;
        Ok(())
    }

    pub fn reload_boards(&mut self) -> anyhow::Result<()> {
        self.boards = board_store::list_boards()?;
        if self.selected_board_idx >= self.boards.len() && !self.boards.is_empty() {
            self.selected_board_idx = self.boards.len() - 1;
        }
        Ok(())
    }

    /// Record the current mode as the overlay return target. Only *base*
    /// modes are recorded: when one overlay opens another (dialog → insert
    /// → dialog), the originally recorded base mode stays put, so closing
    /// the last overlay always lands on solid ground.
    pub fn remember_return_mode(&mut self) {
        if self.mode.is_base() {
            self.previous_mode = Some(self.mode.clone());
        }
    }

    /// Take the recorded return target. Guaranteed to be a base mode —
    /// anything else falls back to `Normal` (board loaded) or
    /// `BoardSelector`.
    pub fn take_return_mode(&mut self) -> AppMode {
        let fallback = if self.editor.is_some() {
            AppMode::Normal
        } else {
            AppMode::BoardSelector
        };
        match self.previous_mode.take() {
            Some(m) if m.is_base() => m,
            _ => fallback,
        }
    }

    /// Mode ↔ companion-state consistency. Violations froze the UI before
    /// (e.g. `mode == Insert` with no insert handler swallows every key),
    /// so this panics in debug/test builds and self-heals to the return
    /// target in release builds. Called after every input event.
    pub fn enforce_mode_invariants(&mut self) {
        if self.previous_mode.as_ref().is_some_and(|m| !m.is_base()) {
            debug_assert!(
                false,
                "overlay mode {:?} recorded as return target",
                self.previous_mode
            );
            self.previous_mode = None;
        }
        let orphaned = match self.mode {
            AppMode::Insert => self.insert.is_none(),
            AppMode::Dialog => self.dialog.is_none(),
            _ => false,
        };
        if orphaned {
            debug_assert!(
                false,
                "mode {:?} active without its companion handler",
                self.mode
            );
            self.mode = self.take_return_mode();
        }
    }

    /// Ask where to save the PDF of the selected card — or, in a browser
    /// session, send it to the browser straight away.
    pub fn open_export_dialog(&mut self, theme: crate::export::Theme) {
        let Some(card) = self.board().and_then(|b| b.current_card()) else {
            return;
        };
        if self.web_sink.is_some() {
            let card_id = card.id.clone();
            self.export_to_browser(&card_id, theme);
            return;
        }
        let (card_id, title) = (card.id.clone(), card.title.clone());
        let language = crate::export::lang::Lang::describe(card);
        let cwd = std::env::current_dir().unwrap_or_default();
        let start = self.export_dir.clone().unwrap_or_else(|| cwd.clone());
        self.open_dialog(Box::new(crate::dialog::export_pdf::ExportPdf::new(
            card_id, title, language, theme, &start, cwd,
        )));
    }

    /// Render the card and hand the PDF to the page over the terminal stream.
    /// Failures toast; nothing is written on the server.
    fn export_to_browser(
        &mut self,
        card_id: &crate::model::id::CardId,
        theme: crate::export::Theme,
    ) {
        let Some(board) = self.board() else { return };
        let Some(card) = board.cards.get(card_id) else {
            return;
        };
        let list = board
            .lists
            .iter()
            .find(|l| l.id == card.list_id)
            .map_or("", |l| l.name.as_str());
        let file_name = crate::export::path::default_file_name(&card.title);
        let sent = crate::export::card_pdf(&board.meta, list, card, theme).and_then(|pdf| {
            let sink = self.web_sink.as_mut().expect("checked by the caller");
            crate::export::web::send(sink.as_mut(), &file_name, &pdf)
        });
        match sent {
            Ok(()) => self.set_status(format!("Sent {file_name} to your browser")),
            Err(e) => self.set_status(format!("Export failed: {e}")),
        }
    }

    pub fn open_dialog(&mut self, dialog: Box<dyn crate::dialog::Dialog>) {
        self.remember_return_mode();
        self.dialog = Some(dialog);
        self.mode = AppMode::Dialog;
    }

    /// Open a modal **Dialog** while the **Insert** handler stays alive on
    /// `self.insert`. Unlike [`open_dialog`], this does *not* overwrite
    /// `previous_mode` (which still points at the mode the editor was
    /// opened from), so the dialog can either resume editing (back to
    /// Insert) or discard and return to the originating mode.
    pub fn open_dialog_over_insert(&mut self, dialog: Box<dyn crate::dialog::Dialog>) {
        self.dialog = Some(dialog);
        self.mode = AppMode::Dialog;
    }

    /// Close the active dialog, restoring `previous_mode` if set, else
    /// falling back to `Normal` (or `BoardSelector` when no board loaded).
    pub fn close_dialog(&mut self) {
        self.dialog = None;
        self.mode = self.take_return_mode();
    }

    /// Close the active dialog to an explicit target mode (does not
    /// consume `previous_mode`).
    pub fn close_dialog_to(&mut self, target: AppMode) {
        self.dialog = None;
        self.mode = target;
    }

    /// Enter **Insert** mode with the given handler, remembering the
    /// current mode for cancel-return.
    pub fn start_insert(&mut self, handler: Box<dyn crate::insert::InsertHandler>) {
        self.remember_return_mode();
        self.insert = Some(handler);
        self.mode = AppMode::Insert;
    }

    /// Cancel insert and restore the return mode.
    pub fn cancel_insert(&mut self) {
        self.insert = None;
        self.mode = self.take_return_mode();
    }
}

impl App {
    pub fn accent_color(&self) -> Color {
        self.board()
            .map(|b| b.meta.accent_color.to_ratatui_color())
            .unwrap_or(Color::Cyan)
    }

    /// Route a Command through the Board Editor. The result is the id of a
    /// card created by `AddCard`, if any.
    pub fn apply(&mut self, cmd: crate::command::Command) -> anyhow::Result<Option<CardId>> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no board loaded"))?;
        editor.apply(cmd)?;
        Ok(editor.last_added_card_id().cloned())
    }
}

/// Format an OSC 52 clipboard escape sequence (`ESC ] 52 ; c ; <base64> BEL`).
fn osc52_sequence(text: &str) -> String {
    let encoded = base64_encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

fn write_osc52_to(text: &str, out: &mut dyn std::io::Write) -> std::io::Result<()> {
    let seq = osc52_sequence(text);
    out.write_all(seq.as_bytes())?;
    out.flush()
}

/// Standard RFC 4648 Base64 encoding for OSC 52 payloads.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests;
