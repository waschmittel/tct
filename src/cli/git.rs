//! Git integration commands: `tct --git [enable|disable]`, `tct commit`, and
//! the `--git` choice of `tct init`. The logic lives in [`crate::git_sync`].

use anyhow::{Result, bail};

use crate::git_sync::{self, Outcome};
use crate::outln;
use crate::storage::paths;

/// `enable` → `Some(true)`, `disable` → `Some(false)`.
pub(super) fn parse_choice(value: Option<&str>) -> Result<bool> {
    match value {
        Some("enable") => Ok(true),
        Some("disable") => Ok(false),
        Some(other) => bail!("--git takes 'enable' or 'disable' (got '{other}')"),
        None => bail!("--git requires 'enable' or 'disable'"),
    }
}

/// `tct --git [enable|disable]`: switch commit-on-quit, or show its state.
pub(super) fn flag(rest: &[String]) -> Result<()> {
    match rest {
        [] => {
            let dir = paths::base_dir();
            match git_sync::detect(&dir) {
                Ok(()) => outln!(
                    "Git integration: {}",
                    if git_sync::is_enabled() {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                Err(why) => outln!("Git integration: disabled ({why})"),
            }
            Ok(())
        }
        [choice] => {
            let on = parse_choice(Some(choice))?;
            git_sync::set_enabled(on)?;
            outln!(
                "Git integration: {}",
                if on { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        [_, extra, ..] => bail!("unexpected argument '{extra}' after --git"),
    }
}

/// The git step of `tct init`: an explicit `choice` wins, then an existing
/// setting, then detection (enabled inside a work tree that tracks the data
/// directory). Always reports the result.
pub(super) fn init_step(choice: Option<bool>) -> Result<()> {
    let dir = paths::base_dir();
    let (on, why) = match (choice, git_sync::configured()) {
        (Some(on), _) => (on, "as requested".to_string()),
        (None, Some(on)) => (on, "already configured".to_string()),
        (None, None) => match git_sync::detect(&dir) {
            Ok(()) => (true, "inside a git repository".to_string()),
            Err(why) => (false, why),
        },
    };
    git_sync::set_enabled(on)?;
    outln!(
        "Git integration: {} ({why})",
        if on { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// `tct commit`: same commit as commit-on-quit, on request.
pub(super) fn commit(rest: &[String]) -> Result<()> {
    if let Some(extra) = rest.first() {
        bail!("commit takes no arguments (got '{extra}')");
    }
    match git_sync::commit()? {
        Outcome::Committed(subject) => outln!("Committed: {subject}"),
        Outcome::Nothing => outln!("Nothing to commit"),
    }
    Ok(())
}
