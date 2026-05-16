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
        let mut meta = board_store::load_board(board_id)?;
        let lists = list_store::load_all_lists(board_id, &meta.list_order)?;
        let mut cards = HashMap::new();
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(card) = card_store::load_card(board_id, card_id) {
                    cards.insert(card_id.clone(), card);
                }
            }
        }

        // Migrate old per-card labels to board-level labels
        migrate_labels(&mut meta, &mut cards);
        board_store::save_board(&meta)?;

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
        self.mode = AppMode::Insert(target);
    }

    pub fn start_insert_with(&mut self, target: InsertTarget, initial: &str) {
        self.input_buffer = initial.to_string();
        self.input_cursor = initial.len();
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

fn migrate_labels(meta: &mut BoardMeta, cards: &mut HashMap<ShortId, Card>) {
    if !meta.labels.is_empty() {
        return;
    }

    // Collect all unique label (name, color) combos across cards by reading raw JSON
    // Since we already migrated cards in load_card and dropped old labels,
    // we only need to migrate if there were labels that were preserved.
    // The card_store migration drops old labels, so board-level migration
    // happens only if labels already exist on the board (handled by serde(default)).
    // This function is a no-op safety net.
    let _ = (meta, cards);
}
