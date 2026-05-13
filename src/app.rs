use std::collections::HashMap;
use std::time::Instant;

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
    EditCardDescription,
    NewListName,
    RenameList,
    NewChecklistTitle,
    NewChecklistItem,
    EditChecklistItem,
    NewBoardName,
    EditDueDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogKind {
    ConfirmDeleteCard,
    ConfirmDeleteList,
    ConfirmDeleteBoard,
    LabelPicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardDetailTab {
    Description,
    Checklists,
    Labels,
    DueDate,
}

impl CardDetailTab {
    pub fn next(self) -> Self {
        match self {
            Self::Description => Self::Checklists,
            Self::Checklists => Self::Labels,
            Self::Labels => Self::DueDate,
            Self::DueDate => Self::Description,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Checklists => "Checklists",
            Self::Labels => "Labels",
            Self::DueDate => "Due Date",
        }
    }
}

pub struct LoadedBoard {
    pub meta: BoardMeta,
    pub lists: Vec<CardList>,
    pub cards: HashMap<ShortId, Card>,
    pub selected_list: usize,
    pub selected_card: Vec<usize>,
    pub scroll_offset: Vec<usize>,
    pub detail_tab: CardDetailTab,
    pub detail_checklist_idx: usize,
    pub detail_item_idx: usize,
    pub grabbed_card: Option<ShortId>,
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

    pub fn is_grabbed(&self) -> bool {
        self.grabbed_card.is_some()
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
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        board_store::ensure_base_dirs()?;
        let boards = board_store::list_boards()?;
        Ok(Self {
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
        })
    }

    pub fn on_tick(&mut self) {
        if let Some((_, instant)) = &self.status_message {
            if instant.elapsed() > std::time::Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
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
            detail_tab: CardDetailTab::Description,
            detail_checklist_idx: 0,
            detail_item_idx: 0,
            grabbed_card: None,
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
        self.description_editor = Some(textarea);
        self.mode = AppMode::Insert(InsertTarget::EditCardDescription);
    }

    pub fn finish_description_edit(&mut self) -> Option<String> {
        self.description_editor.take().map(|ta| ta.into_lines().join("\n"))
    }
}
