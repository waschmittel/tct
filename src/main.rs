mod alert;
mod app;
mod board_directory;
mod board_editor;
mod cli;
mod command;
mod dialog;
mod event;
mod export;
mod footnote;
mod git_sync;
mod input;
mod insert;
mod model;
mod storage;
mod term_caps;
mod term_out;
mod text_diff;
mod ui;

#[cfg(test)]
mod test_support;

use std::time::Duration;

use app::App;
use event::{AppEvent, EventHandler};

/// Build-stamped version: a release tag (set by `build.rs`) or a
/// `snap-<date>` for untagged builds.
pub const VERSION: &str = env!("TCT_VERSION");

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Version flag (check before anything else so it always works)
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tct {VERSION}");
        return Ok(());
    }

    // Help flag (check before anything else so it always works)
    if args.iter().any(|a| a == "--help" || a == "-h")
        || args.first().map(|a| a.as_str()) == Some("help")
    {
        if let Err(e) = cli::help::run(&args) {
            errln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Everything runs from a working directory that must exist — except
    // `init` when `TCT_DATA_DIR` names the target anyway. Otherwise `init`
    // targets `./.tct` (never an ancestor's, unlike the lookup).
    let is_init = args.first().map(String::as_str) == Some("init");
    let env_dir = std::env::var_os("TCT_DATA_DIR");
    if !is_init || env_dir.is_none() {
        let cwd = std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("current working directory is not accessible: {e}"))?;
        if is_init {
            // SAFETY: still single-threaded — no threads have been spawned yet.
            unsafe { std::env::set_var("TCT_DATA_DIR", cwd.join(".tct")) };
        }
    }

    // Subcommand mode: first arg is a non-flag word
    if args.first().map(|a| !a.starts_with('-')).unwrap_or(false) {
        if let Err(e) = cli::run(&args) {
            errln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // `tct --git [enable|disable]`
    if args.first().map(String::as_str) == Some("--git") {
        if let Err(e) = cli::git_flag(&args[1..]) {
            errln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // TUI mode — resolve optional --board flag before initialising the terminal
    let open_board_id = cli::resolve_board_flag(&args)?;

    let mut terminal = ratatui::init();
    // The Linux console has no alternate screen buffer, so entering it does
    // not blank the screen — without a forced clear, the diff renderer
    // never repaints "empty" cells and stale shell output shines through.
    terminal.clear()?;
    let mut app = App::new(open_board_id)?;
    // Only the real TUI entry consults the environment — App::new defaults
    // to full caps so tests and goldens stay deterministic.
    app.caps = term_caps::TermCaps::detect();
    if std::env::var_os("TCT_WEB").is_some() {
        app.web_sink = Some(Box::new(std::io::stdout()));
    }
    let events = EventHandler::new(Duration::from_millis(250));

    let result = run_app(&mut terminal, &mut app, &events);

    ratatui::restore();
    // Commit-on-quit (`tct --git enable`); a failure must not eat the result.
    if let Some(e) = git_sync::commit_on_quit() {
        errln!("Warning: {e}");
    }
    result
}

fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    events: &EventHandler,
) -> anyhow::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, app))?;
        match events.next()? {
            AppEvent::Key(key) => input::handle_input(app, key)?,
            AppEvent::Paste(text) => input::handle_paste(app, text)?,
            AppEvent::Tick => app.on_tick(),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
