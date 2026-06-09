use std::collections::HashMap;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use ratatui::style::Color;

use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::ids::ShortId;
use crate::model::label::LabelColor;
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

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

    pub fn visible_card_count(&self, list_idx: usize) -> usize {
        self.lists
            .get(list_idx)
            .map(|l| {
                l.card_ids
                    .iter()
                    .filter(|id| {
                        self.cards
                            .get(*id)
                            .map(|c| !c.archived)
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0)
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
    pub board: Option<LoadedBoard>,
    pub search_query: String,
    pub search_active: bool,
    pub label_filter: Option<LabelColor>,
    pub last_reload: Instant,
    pub reload_interval: Duration,
    /// Active modal **Dialog Kind** — present iff `mode == AppMode::Dialog`.
    pub dialog: Option<Box<dyn crate::dialog::Dialog>>,
    /// Active **Insert Handler** — present iff `mode == AppMode::Insert`.
    pub insert: Option<Box<dyn crate::insert::InsertHandler>>,
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
            board: None,
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
            dialog: None,
            insert: None,
        };
        if let Some(board_id) = open_board_id {
            app.load_board(&board_id)?;
        }
        Ok(app)
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
        self.board.is_some()
            && matches!(self.mode, AppMode::Normal | AppMode::Help | AppMode::CardDetail)
            && self.insert.is_none()
    }

    fn try_reload_board(&mut self) {
        let board_id = match &self.board {
            Some(b) => b.meta.id.clone(),
            None => return,
        };

        let old = self.board.as_ref().unwrap();
        let old_selected_list = old.selected_list;
        let old_selected_card = old.selected_card.clone();
        let old_scroll_offset = old.scroll_offset.clone();
        let old_detail_item_idx = old.detail_item_idx;
        let old_detail_scroll = old.detail_scroll;

        let meta = match board_store::load_board(&board_id) {
            Ok(m) => m,
            Err(_) => {
                self.board = None;
                let _ = self.reload_boards();
                self.mode = AppMode::BoardSelector;
                return;
            }
        };
        let lists = match list_store::load_all_lists(&board_id, &meta.list_order) {
            Ok(l) => l,
            Err(_) => return,
        };
        let mut cards = HashMap::new();
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(card) = card_store::load_card(&board_id, card_id) {
                    cards.insert(card_id.clone(), card);
                }
            }
        }

        let num_lists = lists.len();
        let board = self.board.as_mut().unwrap();
        board.meta = meta;
        board.lists = lists;
        board.cards = cards;

        board.selected_card.resize(num_lists, 0);
        board.scroll_offset.resize(num_lists, 0);
        board.selected_list = old_selected_list.min(num_lists.saturating_sub(1));
        for i in 0..num_lists {
            if i < old_selected_card.len() {
                board.selected_card[i] = old_selected_card[i];
            }
            if i < old_scroll_offset.len() {
                board.scroll_offset[i] = old_scroll_offset[i];
            }
        }
        board.detail_item_idx = old_detail_item_idx;
        board.detail_scroll = old_detail_scroll;
        board.clamp_selection();
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
        let meta = board_store::load_board(board_id)?;
        let lists = list_store::load_all_lists(board_id, &meta.list_order)?;
        let mut cards = HashMap::new();
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(card) = card_store::load_card(board_id, card_id) {
                    cards.insert(card_id.clone(), card);
                }
            }
        }

        let num_lists = lists.len();
        self.board = Some(LoadedBoard {
            meta,
            lists,
            cards,
            selected_list: 0,
            selected_card: vec![0; num_lists],
            scroll_offset: vec![0; num_lists],
            detail_item_idx: 0,
            detail_scroll: 0,
        });
        self.mode = AppMode::Normal;
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

    /// Close the active dialog, restoring `previous_mode` if set, else
    /// falling back to `Normal` (or `BoardSelector` when no board loaded).
    pub fn close_dialog(&mut self) {
        self.dialog = None;
        let fallback = if self.board.is_some() {
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
            if self.board.is_some() { AppMode::Normal } else { AppMode::BoardSelector }
        });
    }
}

impl App {
    pub fn accent_color(&self) -> Color {
        self.board
            .as_ref()
            .map(|b| b.meta.accent_color.to_ratatui_color())
            .unwrap_or(Color::Cyan)
    }

    /// Route a Command through the Board Editor while keeping the
    /// take-replace shim until `App.board` is renamed to `editor` (later
    /// phase). The result is the id of a card created by `AddCard`, if any.
    pub fn apply(&mut self, cmd: crate::command::Command) -> anyhow::Result<Option<crate::model::ids::ShortId>> {
        let board = self
            .board
            .take()
            .ok_or_else(|| anyhow::anyhow!("no board loaded"))?;
        let mut editor = crate::board_editor::BoardEditor::from_loaded(board);
        let result = editor.apply(cmd);
        let new_id = editor.last_added_card_id().cloned();
        self.board = Some(editor.into_loaded());
        result.map_err(anyhow::Error::from)?;
        Ok(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::label::LabelColor;
    use crate::model::list::CardList;
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
            board: None,
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
            dialog: None,
            insert: None,
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
        app.board = Some(board);
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
            assert_eq!(app.board.as_ref().unwrap().lists[0].card_ids.len(), 3);

            // Add a new card on disk
            let new_card = Card::new("Disk-added".into());
            card_store::save_card(&meta.id, &new_card).unwrap();
            list.card_ids.push(new_card.id.clone());
            list_store::save_list(&meta.id, &list).unwrap();

            // Force reload
            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board.as_ref().unwrap();
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
            app.board.as_mut().unwrap().selected_card[0] = 2;
            app.board.as_mut().unwrap().detail_item_idx = 5;
            app.board.as_mut().unwrap().detail_scroll = 7;

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board.as_ref().unwrap();
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
            app.board.as_mut().unwrap().selected_card[0] = 2; // points at last

            // Remove last card from disk
            list.card_ids.retain(|id| id != &cards[2].id);
            list_store::save_list(&meta.id, &list).unwrap();
            card_store::delete_card(&meta.id, &cards[2].id).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            let board = app.board.as_ref().unwrap();
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
            assert!(app.board.is_some());

            // Delete board from disk
            board_store::delete_board(&meta.id).unwrap();

            app.reload_interval = Duration::from_millis(0);
            app.last_reload = Instant::now() - Duration::from_secs(1);
            app.on_tick();

            // App should drop the board and switch to selector
            assert!(app.board.is_none());
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
            assert_eq!(app.board.as_ref().unwrap().lists[0].card_ids.len(), 3);
        });
    }

    #[test]
    fn reload_skipped_when_description_editor_active() {
        with_temp_dir(|| {
            let (meta, mut list, _) = make_board_with_cards();
            let mut app = App::new(Some(meta.id.clone())).unwrap();
            app.mode = AppMode::CardDetail;
            // Simulate active description edit
            let card_id = app.board.as_ref().unwrap().current_card_id().cloned().unwrap();
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
            assert_eq!(app.board.as_ref().unwrap().lists[0].card_ids.len(), 3);
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
