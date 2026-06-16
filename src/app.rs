use std::collections::HashMap;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use ratatui::style::Color;

use crate::board_editor::BoardEditor;
use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::ids::ShortId;
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

pub struct LoadedBoard {
    pub meta: BoardMeta,
    pub lists: Vec<CardList>,
    pub cards: HashMap<ShortId, Card>,
    pub selected_list: usize,
    pub selected_card: Vec<usize>,
    pub scroll_offset: Vec<usize>,
    pub detail_item_idx: usize,
    pub detail_scroll: usize,
}

impl LoadedBoard {
    pub fn current_card_id(&self) -> Option<&ShortId> {
        let list = self.lists.get(self.selected_list)?;
        let card_idx = *self.selected_card.get(self.selected_list)?;
        list.card_ids.get(card_idx)
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
}

impl App {
    pub fn new(open_board_id: Option<String>) -> anyhow::Result<Self> {
        board_store::ensure_base_dirs()?;
        let boards = board_store::list_boards()?;
        let mut app = Self {
            mode: AppMode::BoardSelector,
            previous_mode: None,
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
            && instant.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }

        if self.last_reload.elapsed() >= self.reload_interval && self.should_reload() {
            self.last_reload = Instant::now();
            self.try_reload_board();
        }
    }

    fn should_reload(&self) -> bool {
        self.editor.is_some()
            && matches!(self.mode, AppMode::Normal | AppMode::Help | AppMode::CardDetail)
            && self.insert.is_none()
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

    pub fn copy_to_clipboard(&mut self, text: String) {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(text) {
                    self.set_status(format!("Clipboard error: {}", e));
                } else {
                    self.set_status("Copied to clipboard".into());
                }
            }
            Err(e) => {
                self.set_status(format!("Clipboard not available: {}", e));
            }
        }
    }

    pub fn load_board(&mut self, board_id: &str) -> anyhow::Result<()> {
        let editor = BoardEditor::load(board_id)?;
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

    /// Open a modal **Dialog**, remembering the current mode so the
    /// dispatcher can restore it on `Close`.
    pub fn open_dialog(&mut self, dialog: Box<dyn crate::dialog::Dialog>) {
        if !matches!(self.mode, AppMode::Dialog) {
            self.previous_mode = Some(self.mode.clone());
        }
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
        let fallback = if self.editor.is_some() {
            AppMode::Normal
        } else {
            AppMode::BoardSelector
        };
        self.mode = self.previous_mode.take().unwrap_or(fallback);
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
        if !matches!(self.mode, AppMode::Insert) {
            self.previous_mode = Some(self.mode.clone());
        }
        self.insert = Some(handler);
        self.mode = AppMode::Insert;
    }

    /// Cancel insert and restore `previous_mode`.
    pub fn cancel_insert(&mut self) {
        self.insert = None;
        self.mode = self.previous_mode.take().unwrap_or_else(|| {
            if self.editor.is_some() { AppMode::Normal } else { AppMode::BoardSelector }
        });
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
    pub fn apply(&mut self, cmd: crate::command::Command) -> anyhow::Result<Option<ShortId>> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no board loaded"))?;
        editor.apply(cmd)?;
        Ok(editor.last_added_card_id().cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::label::LabelColor;
    use crate::model::list::CardList;
    use crate::storage::{card_store, list_store};
    use crate::test_support::with_temp_dir;

    /// Build a `Card` with a fixed `id` for stable test assertions.
    fn fixed_card(id: &str, title: &str) -> Card {
        let mut c = Card::new(title.into());
        c.id = id.into();
        c
    }

    /// Build a `LoadedBoard` with a single list containing the given cards.
    fn single_list_board(cards: Vec<Card>) -> LoadedBoard {
        let card_ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
        let cards_map: HashMap<_, _> = cards.into_iter().map(|c| (c.id.clone(), c)).collect();
        LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![CardList {
                id: "l1".into(),
                name: "L".into(),
                card_ids,
                archived: false,
            }],
            cards: cards_map,
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        }
    }

    /// Build a bare `App` (no disk, no board). Caller can override fields.
    fn bare_app() -> App {
        App {
            mode: AppMode::BoardSelector,
            previous_mode: None,
            should_quit: false,
            status_message: None,
            boards: vec![],
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
        }
    }

    fn make_board_with_cards() -> (BoardMeta, CardList, Vec<Card>) {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let mut list = CardList::new("To Do".into());
        let cards: Vec<Card> = (0..3).map(|i| Card::new(format!("Card {i}"))).collect();
        for c in &cards {
            card_store::save_card(&meta.id, c).unwrap();
            list.card_ids.push(c.id.clone());
        }
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();
        (meta, list, cards)
    }

    #[test]
    fn clamp_selection_within_bounds_unchanged() {
        let cards = vec![
            fixed_card("aaaaaaaa", "A"),
            fixed_card("bbbbbbbb", "B"),
            fixed_card("cccccccc", "C"),
        ];
        let mut board = single_list_board(cards);
        board.selected_card[0] = 1;
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 1);
    }

    #[test]
    fn clamp_selection_caps_to_last_visible() {
        let cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
        let mut board = single_list_board(cards);
        board.selected_card[0] = 5; // out of bounds
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 1);
    }

    #[test]
    fn clamp_selection_zeros_when_no_visible_cards() {
        let mut board = single_list_board(vec![]);
        board.selected_card[0] = 3;
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 0);
    }

    #[test]
    fn clamp_selection_skips_archived() {
        let mut cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
        cards[1].archived = true; // only first visible
        let mut board = single_list_board(cards);
        board.selected_card[0] = 1; // points to archived
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 0);
    }

    #[test]
    fn visible_card_count_excludes_archived_and_missing() {
        let mut cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
        cards[1].archived = true;
        let mut board = single_list_board(cards);
        // Inject orphan card_id with no matching card
        board.lists[0].card_ids.push("orphanid".into());
        assert_eq!(board.visible_card_count(0), 1);
    }

    #[test]
    fn current_card_returns_selected() {
        let cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
        let mut board = single_list_board(cards);
        board.selected_card[0] = 1;
        assert_eq!(board.current_card_id().unwrap(), "bbbbbbbb");
        assert_eq!(board.current_card().unwrap().title, "B");
    }

    #[test]
    fn current_card_none_on_empty_list() {
        let board = single_list_board(vec![]);
        assert!(board.current_card_id().is_none());
        assert!(board.current_card().is_none());
    }

    #[test]
    fn accent_color_defaults_to_cyan() {
        assert_eq!(bare_app().accent_color(), Color::Cyan);
    }

    #[test]
    fn accent_color_uses_board_meta() {
        let mut board = single_list_board(vec![]);
        board.meta.accent_color = LabelColor::Purple;
        let mut app = bare_app();
        app.mode = AppMode::Normal;
        app.editor = Some(BoardEditor::from_loaded(board));
        let (r, g, b) = LabelColor::Purple.to_rgb();
        assert_eq!(app.accent_color(), Color::Rgb(r, g, b));
    }

    #[test]
    fn on_tick_clears_old_status_message() {
        let mut app = bare_app();
        app.mode = AppMode::Normal;
        app.status_message = Some(("hi".into(), Instant::now() - Duration::from_secs(10)));
        app.on_tick();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn on_tick_keeps_fresh_status_message() {
        let mut app = bare_app();
        app.mode = AppMode::Normal;
        app.status_message = Some(("hi".into(), Instant::now()));
        app.on_tick();
        assert!(app.status_message.is_some());
    }

    #[test]
    fn reload_picks_up_new_card() {
        with_temp_dir(|| {
            let (meta, mut list, _cards) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);

            // Add a new card on disk
            let new_card = Card::new("Disk-added".into());
            card_store::save_card(&meta.id, &new_card).unwrap();
            list.card_ids.push(new_card.id.clone());
            list_store::save_list(&meta.id, &list).unwrap();

            // Force reload
            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids.len(), 4);
            assert!(board.cards.contains_key(&new_card.id));
        });
    }

    #[test]
    fn reload_preserves_selection() {
        with_temp_dir(|| {
            let (meta, _, _) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            // Move selection to card index 2
            app.board_mut().unwrap().selected_card[0] = 2;
            app.board_mut().unwrap().detail_item_idx = 5;
            app.board_mut().unwrap().detail_scroll = 7;

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board().unwrap();
            assert_eq!(board.selected_card[0], 2);
            assert_eq!(board.detail_item_idx, 5);
            assert_eq!(board.detail_scroll, 7);
        });
    }

    #[test]
    fn reload_clamps_selection_when_card_removed() {
        with_temp_dir(|| {
            let (meta, mut list, cards) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            app.board_mut().unwrap().selected_card[0] = 2; // points at last

            // Remove last card from disk
            list.card_ids.retain(|id| id != &cards[2].id);
            list_store::save_list(&meta.id, &list).unwrap();
            card_store::delete_card(&meta.id, &cards[2].id).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids.len(), 2);
            // Selection clamped to new last index
            assert_eq!(board.selected_card[0], 1);
        });
    }

    #[test]
    fn reload_handles_board_deleted_externally() {
        with_temp_dir(|| {
            let (meta, _, _) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            assert!(app.editor.is_some());

            // Delete board from disk
            board_store::delete_board(&meta.id).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            // App should drop the board and switch to selector
            assert!(app.editor.is_none());
            assert_eq!(app.mode, AppMode::BoardSelector);
        });
    }

    #[test]
    fn reload_skipped_in_insert_mode() {
        with_temp_dir(|| {
            let (meta, mut list, _) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            app.start_insert(Box::new(crate::insert::line_editor::NewCardTitle::new()));

            // Add a card on disk
            let new_card = Card::new("Should not appear".into());
            card_store::save_card(&meta.id, &new_card).unwrap();
            list.card_ids.push(new_card.id.clone());
            list_store::save_list(&meta.id, &list).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            // Should NOT have reloaded
            assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);
        });
    }

    #[test]
    fn reload_skipped_when_description_editor_active() {
        with_temp_dir(|| {
            let (meta, mut list, _) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            app.mode = AppMode::CardDetail;
            // Simulate active description edit
            let card_id = app.board().unwrap().current_card_id().cloned().unwrap();
            app.start_insert(Box::new(
                crate::insert::markdown_editor::MarkdownEditor::new(card_id, "initial"),
            ));

            let new_card = Card::new("Disk".into());
            card_store::save_card(&meta.id, &new_card).unwrap();
            list.card_ids.push(new_card.id.clone());
            list_store::save_list(&meta.id, &list).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            // Editor active → skip reload
            assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);
        });
    }

    #[test]
    fn start_insert_with_prefill() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            app.start_insert(Box::new(crate::insert::line_editor::EditCardTitle::new(
                "abc".into(),
                "hello",
                true,
            )));
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some("hello"));
            assert_eq!(h.line_cursor(), Some(5));
            assert!(matches!(app.mode, AppMode::Insert));
        });
    }
}
