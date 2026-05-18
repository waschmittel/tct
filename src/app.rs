use std::collections::HashMap;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use ratatui::style::{Color, Style};
use ratatui_textarea::TextArea;

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
    Insert(InsertTarget),
    Command,
    Dialog(DialogKind),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertTarget {
    NewCardTitle,
    EditCardTitle,
    EditCardTitleInline,
    EditCardDescription,
    NewListName,
    RenameList,
    NewChecklistItem,
    EditChecklistItem,
    NewBoardName,
    RenameBoard,
    EditDueDate,
    NewLabelName,
    EditLabelName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogKind {
    ConfirmDeleteCard,
    ConfirmDeleteList,
    ConfirmArchiveBoard,
    ConfirmArchiveCard,
    ConfirmCancelEdit,
    ConfirmDeleteLabel,
    ArchivedCards,
    ArchivedBoards,
    LabelPicker,
    LabelManager,
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
    pub input_buffer: String,
    pub input_cursor: usize,
    pub label_picker_idx: usize,
    pub description_editor: Option<TextArea<'static>>,
    pub description_original: Option<String>,
    pub editor_scroll: usize,
    pub archived_cards: Vec<Card>,
    pub archived_boards: Vec<crate::model::board::BoardMeta>,
    pub archived_selected: usize,
    pub last_reload: Instant,
    pub reload_interval: Duration,
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
            input_buffer: String::new(),
            input_cursor: 0,
            label_picker_idx: 0,
            description_editor: None,
            description_original: None,
            editor_scroll: 0,
            archived_cards: Vec::new(),
            archived_boards: Vec::new(),
            archived_selected: 0,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
        };
        if let Some(board_id) = open_board_id {
            app.load_board(&board_id)?;
        }
        Ok(app)
    }

    pub fn on_tick(&mut self) {
        if let Some((_, instant)) = &self.status_message {
            if instant.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }

        if self.last_reload.elapsed() >= self.reload_interval && self.should_reload() {
            self.last_reload = Instant::now();
            let _ = self.try_reload_board();
        }
    }

    fn should_reload(&self) -> bool {
        self.board.is_some()
            && matches!(self.mode, AppMode::Normal | AppMode::Help | AppMode::CardDetail)
            && self.description_editor.is_none()
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

    pub fn start_insert(&mut self, target: InsertTarget) {
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::Insert(target);
    }

    pub fn start_insert_with(&mut self, target: InsertTarget, initial: &str) {
        self.input_buffer = initial.to_string();
        self.input_cursor = initial.len();
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::Insert(target);
    }

    pub fn start_description_edit(&mut self, initial: &str) {
        let lines: Vec<String> = initial.split('\n').map(|s| s.to_string()).collect();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_style(Style::default().fg(Color::White));
        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Edit Description "),
        );
        self.description_original = Some(initial.to_string());
        self.description_editor = Some(textarea);
        self.editor_scroll = 0;
        self.mode = AppMode::Insert(InsertTarget::EditCardDescription);
    }

    pub fn finish_description_edit(&mut self) -> Option<String> {
        self.description_editor.take().map(|ta| ta.into_lines().join("\n"))
    }
}

impl App {
    pub fn accent_color(&self) -> Color {
        self.board
            .as_ref()
            .map(|b| b.meta.accent_color.to_ratatui_color())
            .unwrap_or(Color::Cyan)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::label::LabelColor;
    use crate::model::list::CardList;
    use std::env;

    fn with_temp_dir<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", dir.path()) };
        board_store::ensure_base_dirs().unwrap();
        f();
        unsafe { env::remove_var("TCT_DATA_DIR") };
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
        with_temp_dir(|| {
            let (_, _, cards) = make_board_with_cards();
            let mut board = LoadedBoard {
                meta: BoardMeta::new("X".into()),
                lists: vec![CardList { id: "l1".into(), name: "L".into(), card_ids: cards.iter().map(|c| c.id.clone()).collect() }],
                cards: cards.iter().map(|c| (c.id.clone(), c.clone())).collect(),
                selected_list: 0,
                selected_card: vec![1],
                scroll_offset: vec![0],
                detail_item_idx: 0,
                detail_scroll: 0,
            };
            board.clamp_selection();
            assert_eq!(board.selected_card[0], 1);
        });
    }

    #[test]
    fn clamp_selection_caps_to_last_visible() {
        let mut c1 = Card::new("A".into());
        let mut c2 = Card::new("B".into());
        c1.id = "aaaaaaaa".into();
        c2.id = "bbbbbbbb".into();
        let list = CardList {
            id: "l1".into(),
            name: "L".into(),
            card_ids: vec![c1.id.clone(), c2.id.clone()],
        };
        let mut cards = std::collections::HashMap::new();
        cards.insert(c1.id.clone(), c1);
        cards.insert(c2.id.clone(), c2);

        let mut board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![list],
            cards,
            selected_list: 0,
            selected_card: vec![5], // out of bounds
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 1); // last visible
    }

    #[test]
    fn clamp_selection_zeros_when_no_visible_cards() {
        let mut board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![CardList { id: "l1".into(), name: "L".into(), card_ids: vec![] }],
            cards: Default::default(),
            selected_list: 0,
            selected_card: vec![3],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        board.clamp_selection();
        assert_eq!(board.selected_card[0], 0);
    }

    #[test]
    fn clamp_selection_skips_archived() {
        let mut c1 = Card::new("A".into());
        let mut c2 = Card::new("B".into());
        c1.id = "aaaaaaaa".into();
        c2.id = "bbbbbbbb".into();
        c2.archived = true; // only c1 visible
        let list = CardList {
            id: "l1".into(),
            name: "L".into(),
            card_ids: vec![c1.id.clone(), c2.id.clone()],
        };
        let mut cards = std::collections::HashMap::new();
        cards.insert(c1.id.clone(), c1);
        cards.insert(c2.id.clone(), c2);

        let mut board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![list],
            cards,
            selected_list: 0,
            selected_card: vec![1], // points to archived
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        board.clamp_selection();
        // visible count = 1, so selection capped to 0
        assert_eq!(board.selected_card[0], 0);
    }

    #[test]
    fn visible_card_count_excludes_archived_and_missing() {
        let mut c1 = Card::new("A".into());
        let mut c2 = Card::new("B".into());
        c1.id = "aaaaaaaa".into();
        c2.id = "bbbbbbbb".into();
        c2.archived = true;
        let list = CardList {
            id: "l1".into(),
            name: "L".into(),
            // Includes a card_id with no matching card (orphan)
            card_ids: vec![c1.id.clone(), c2.id.clone(), "orphanid".into()],
        };
        let mut cards = std::collections::HashMap::new();
        cards.insert(c1.id.clone(), c1);
        cards.insert(c2.id.clone(), c2);

        let board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![list],
            cards,
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        assert_eq!(board.visible_card_count(0), 1);
    }

    #[test]
    fn current_card_returns_selected() {
        let c1 = Card { id: "aaaaaaaa".into(), title: "A".into(), ..Card::new("A".into()) };
        let c2 = Card { id: "bbbbbbbb".into(), title: "B".into(), ..Card::new("B".into()) };
        let list = CardList {
            id: "l1".into(),
            name: "L".into(),
            card_ids: vec![c1.id.clone(), c2.id.clone()],
        };
        let mut cards = std::collections::HashMap::new();
        cards.insert(c1.id.clone(), c1);
        cards.insert(c2.id.clone(), c2);

        let board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![list],
            cards,
            selected_list: 0,
            selected_card: vec![1],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        assert_eq!(board.current_card_id().unwrap(), "bbbbbbbb");
        assert_eq!(board.current_card().unwrap().title, "B");
    }

    #[test]
    fn current_card_none_on_empty_list() {
        let board = LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![CardList { id: "l1".into(), name: "L".into(), card_ids: vec![] }],
            cards: Default::default(),
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        assert!(board.current_card_id().is_none());
        assert!(board.current_card().is_none());
    }

    #[test]
    fn accent_color_defaults_to_cyan() {
        let app = App {
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
            input_buffer: String::new(),
            input_cursor: 0,
            label_picker_idx: 0,
            description_editor: None,
            description_original: None,
            editor_scroll: 0,
            archived_cards: vec![],
            archived_boards: vec![],
            archived_selected: 0,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
        };
        assert_eq!(app.accent_color(), Color::Cyan);
    }

    #[test]
    fn accent_color_uses_board_meta() {
        let mut meta = BoardMeta::new("X".into());
        meta.accent_color = LabelColor::Purple;
        let board = LoadedBoard {
            meta,
            lists: vec![],
            cards: Default::default(),
            selected_list: 0,
            selected_card: vec![],
            scroll_offset: vec![],
            detail_item_idx: 0,
            detail_scroll: 0,
        };
        let app = App {
            mode: AppMode::Normal,
            previous_mode: None,
            should_quit: false,
            status_message: None,
            boards: vec![],
            selected_board_idx: 0,
            board: Some(board),
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            input_buffer: String::new(),
            input_cursor: 0,
            label_picker_idx: 0,
            description_editor: None,
            description_original: None,
            editor_scroll: 0,
            archived_cards: vec![],
            archived_boards: vec![],
            archived_selected: 0,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
        };
        let (r, g, b) = LabelColor::Purple.to_rgb();
        assert_eq!(app.accent_color(), Color::Rgb(r, g, b));
    }

    #[test]
    fn on_tick_clears_old_status_message() {
        let mut app = App {
            mode: AppMode::Normal,
            previous_mode: None,
            should_quit: false,
            status_message: Some(("hi".into(), Instant::now() - Duration::from_secs(10))),
            boards: vec![],
            selected_board_idx: 0,
            board: None,
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            input_buffer: String::new(),
            input_cursor: 0,
            label_picker_idx: 0,
            description_editor: None,
            description_original: None,
            editor_scroll: 0,
            archived_cards: vec![],
            archived_boards: vec![],
            archived_selected: 0,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
        };
        app.on_tick();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn on_tick_keeps_fresh_status_message() {
        let mut app = App {
            mode: AppMode::Normal,
            previous_mode: None,
            should_quit: false,
            status_message: Some(("hi".into(), Instant::now())),
            boards: vec![],
            selected_board_idx: 0,
            board: None,
            search_query: String::new(),
            search_active: false,
            label_filter: None,
            input_buffer: String::new(),
            input_cursor: 0,
            label_picker_idx: 0,
            description_editor: None,
            description_original: None,
            editor_scroll: 0,
            archived_cards: vec![],
            archived_boards: vec![],
            archived_selected: 0,
            last_reload: Instant::now(),
            reload_interval: Duration::from_secs(15),
        };
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
            app.mode = AppMode::Insert(InsertTarget::NewCardTitle);

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
            app.start_description_edit("initial");

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
            app.start_insert_with(InsertTarget::EditCardTitleInline, "hello");
            assert_eq!(app.input_buffer, "hello");
            assert_eq!(app.input_cursor, 5);
            assert!(matches!(app.mode, AppMode::Insert(InsertTarget::EditCardTitleInline)));
        });
    }

    #[test]
    fn finish_description_edit_returns_joined_lines() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            app.start_description_edit("line1\nline2");
            let out = app.finish_description_edit().unwrap();
            assert_eq!(out, "line1\nline2");
            assert!(app.description_editor.is_none());
        });
    }
}
