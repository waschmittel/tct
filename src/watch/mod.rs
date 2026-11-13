//! File watching for the data directory (ADR-0021): a native watcher
//! (`notify`: FSEvents/inotify/…) tells the app when to reload, so external
//! changes show up at once instead of on the next poll. Polling stays as a
//! slow safety net and as the fallback whenever the watcher cannot start or
//! dies.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use notify::RecommendedWatcher;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

/// Collapses the burst of events one save causes (`.tmp` write + rename).
const DEBOUNCE: Duration = Duration::from_millis(200);

/// What the watcher has seen since the last [`Watcher::poll`].
#[derive(Debug, PartialEq, Eq)]
pub enum Poll {
    Idle,
    /// Something under the data directory changed.
    Changed,
    /// The watcher is dead or reported an error — fall back to polling.
    Failed,
}

pub struct Watcher {
    // Held to keep the watcher thread alive; dropping stops it.
    _debouncer: Debouncer<RecommendedWatcher>,
    rx: Receiver<DebounceEventResult>,
}

impl Watcher {
    /// Watches `dir` recursively. `None` when the platform watcher cannot be
    /// set up (inotify limits, …) — the caller then keeps polling.
    pub fn start(dir: &Path) -> Option<Self> {
        Self::try_start(dir).ok()
    }

    fn try_start(dir: &Path) -> notify::Result<Self> {
        let (tx, rx) = channel();
        let mut debouncer = new_debouncer(DEBOUNCE, tx)?;
        debouncer
            .watcher()
            .watch(dir, notify::RecursiveMode::Recursive)?;
        Ok(Self {
            _debouncer: debouncer,
            rx,
        })
    }

    /// Test hook: make the next `poll` report a dead watcher.
    #[cfg(test)]
    pub fn kill_for_test(&mut self) {
        let (tx, rx) = channel();
        drop(tx);
        self.rx = rx;
    }

    /// Drains pending events without blocking.
    pub fn poll(&self) -> Poll {
        let mut seen = Poll::Idle;
        loop {
            match self.rx.try_recv() {
                Ok(Ok(events)) if !events.is_empty() => seen = Poll::Changed,
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => return Poll::Failed,
                Err(TryRecvError::Empty) => return seen,
            }
        }
    }
}

#[cfg(test)]
mod tests;
