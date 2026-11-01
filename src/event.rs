use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

pub enum AppEvent {
    Key(KeyEvent),
    Paste(String),
    Tick,
}

pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    pub fn next(&self) -> anyhow::Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                CrosstermEvent::Key(key) => Ok(AppEvent::Key(key)),
                CrosstermEvent::Paste(text) => Ok(AppEvent::Paste(text)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
