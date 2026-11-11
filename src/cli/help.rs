//! `tct --help [topic]` — overview, reference topics and the embedded
//! README / user guide (with `--toc` and `--section`).
//!
//! The two documents are `include_str!`d from the repository, so the binary
//! always carries exactly the docs it was built from (agents can read them
//! offline). The Dockerfile must `COPY` both files.

use anyhow::{Result, bail};

const README: &str = include_str!("../../README.md");
const USER_GUIDE: &str = include_str!("../../docs/user-guide.md");

const OVERVIEW: &str = "\
tct - Terminal Card Tracker

A keyboard-driven TUI Kanban board. Run without arguments to open the TUI.

USAGE:
    tct                              Open the TUI board selector
    tct --board <name>               Open TUI directly on a board (partial, case-insensitive)
    tct init [--git enable|disable]  Create ./.tct (or $TCT_DATA_DIR) with a README, like git init
    tct --git [enable|disable]       Commit .tct when the TUI quits (or show the setting)
    tct commit                       Commit .tct to git now
    tct <COMMAND> [ARGS] [FLAGS]     Run a CLI command (no TUI)
    tct --help [TOPIC]               Show help (see below), also `tct help [TOPIC]`
    tct --version, -V                Print the version and exit

HELP TOPICS:
    tct --help cli                   All FLAGS and COMMANDS
    tct --help examples              Copy-paste example workflow
    tct --help env                   Data directory (STORAGE) and ENVIRONMENT variables
    tct --help readme                The README (installation, storage, CLI usage, PDF export)
    tct --help user-guide            The complete user guide (TUI, editor, CLI, workflows, formats)

    readme and user-guide are meant for users and AI agents alike: print them in full, or
    narrow down with
        --toc                        Only the table of contents
        --section <heading>          Only that section (case-insensitive, partial match)
    e.g. `tct --help user-guide --toc`, `tct --help user-guide --section \"CLI Reference\"`.

Website: https://waschmittel.github.io/tct/
Source:  https://github.com/waschmittel/tct
";

const CLI: &str = "\
FLAGS (apply to any command):
    --by-id                          Match all identifier arguments by exact ID instead of name

COMMANDS:
  boards                              List active boards
  boards --archived                   List archived boards
  boards --create <name>              Create a new board
  boards --archive <name>             Archive a board
  boards --restore <name>             Restore an archived board
  boards --delete <name>              Permanently delete an archived board

  lists <board>                       List all active lists on a board
  lists <board> --archived            List archived lists
  lists <board> --create <name>       Create a list
  lists <board> --rename <list> <name>  Rename a list
  lists <board> --archive <list>      Archive a list
  lists <board> --restore <list>      Restore an archived list
  lists <board> --delete <list>       Permanently delete an archived list and its cards

  cards <board>                       List active cards grouped by list
  cards <board> --archived            List archived cards
  cards <board> --list <list>         List active cards in a specific list
  cards <board> --show <card>         Show full card detail
  cards <board> --export <card>       Export a card as PDF/A to <card-title>.pdf in the current directory
    --out <path>                        File, or directory to put the file in (default: current directory)
    --style <mono|color>                Black and gray, for monochrome printers (default), or color
  cards <board> --create <list> <title>  Create a card in a list
  cards <board> --edit <card>         Edit card fields
    --title <text>                      New title
    --description <text>                New description
    --due <YYYY-MM-DD|none>             Set or clear due date
  cards <board> --archive <card>      Archive a card
  cards <board> --restore <card>      Restore an archived card to the first list
  cards <board> --delete <card>       Permanently delete an archived card

  checklist <board> <card>            Show checklist
  checklist <board> <card> --add <text>     Add a checklist item
  checklist <board> <card> --toggle <n>     Toggle item n (1-based index)
  checklist <board> <card> --delete <n>     Delete item n (1-based index)

  labels <board>                      List all labels
  labels <board> --create <name>      Create a label
  labels <board> --delete <label>     Delete a label (removes from all cards)
  labels <board> --assign <card> <label>   Assign a label to a card
  labels <board> --remove <card> <label>   Remove a label from a card

  search <query>                      Search cards across all boards (case-insensitive substring)
  search <query> --board <name>       Limit search to boards matching name (repeatable)
  search <query> --list <name>        Limit search to lists matching name
  search <query> --regex              Treat query as a regular expression
  search <query> --archived           Include archived cards in results

  commit                              Commit the data directory (and only it) to git, now

GIT:
    The CLI never commits on its own. `tct commit` (or `C` in the TUI board view) always works
    when the data directory is inside a git repository; only .tct is committed, whatever else
    is staged. The message names the boards, lists and cards that changed (titles, not ids).
    `tct --git enable` additionally commits when the TUI quits; `tct --git` shows the setting.
    `tct init` enables it when the new .tct is inside a git repository (override with --git
    enable|disable) and says what it chose. The setting is local git config (tct.autocommit),
    never a file in .tct. Commits skip hooks and signing, and are refused during a merge,
    rebase, cherry-pick or revert.

IDs are shown in listings as [xxxxxxxx]. Pass --by-id to match by ID instead of name.
Multiple name matches or a missing ID result in an error.
";

const ENV: &str = "\
STORAGE:
    By default data is stored in ~/.tct/. If a .tct/ directory exists in the current
    working directory or any of its parents, that directory is used instead.
    Override with the TCT_DATA_DIR environment variable (see ENVIRONMENT).
    Only ~/.tct is created implicitly (with a README); any other missing data directory needs `tct init`.
    tct exits with an error if the current directory no longer exists.

ENVIRONMENT:
    TCT_DATA_DIR       Complete path of the data directory (not its parent). Wins over the
                       .tct search and ~/.tct; may be a symlink. Must exist (run `tct init`).
    TCT_SKIP_CORRUPT   Set to any value other than empty or 0 to skip unreadable board/card
                       files and load the rest, instead of failing. Single-file loads still fail.
    TCT_WEB            Set by the Docker/ttyd image. PDF export streams the file to the browser
                       instead of asking for a path; nothing is written on the server.
    TERM, COLORTERM    Pick the color tier: COLORTERM=truecolor|24bit gives full color,
                       otherwise 256 colors; TERM=linux gets 16 colors and console-safe glyphs.
";

const EXAMPLES: &str = "\
EXAMPLES (a typical workflow from scratch):

    # Create a board and lists
    tct boards --create \"My Project\"
    tct lists \"My Project\" --create \"To Do\"
    tct lists \"My Project\" --create \"Done\"

    # Add and view cards
    tct cards \"My Project\" --create \"To Do\" \"Fix login bug\"
    tct cards \"My Project\"

    # Edit a card
    tct cards \"My Project\" --edit \"Fix login\" --due 2099-12-31

    # Checklist and labels
    tct checklist \"My Project\" \"Fix login\" --add \"Reproduce the bug\"
    tct checklist \"My Project\" \"Fix login\" --toggle 1
    tct labels \"My Project\" --create \"bug\"
    tct labels \"My Project\" --assign \"Fix login\" \"bug\"

    # Search across all boards, export as PDF, archive
    tct search login --board project
    tct cards \"My Project\" --export \"Fix login\" --out /tmp --style color
    tct cards \"My Project\" --archive \"Fix login\"

    # Unambiguous addressing by ID (IDs are shown in listings as [xxxxxxxx])
    tct boards
    tct lists a1b2c3d4 --by-id

More: tct --help user-guide --section \"Workflows\"
";

/// Help topic: name and what it prints.
enum Topic {
    Cli,
    Examples,
    Env,
    /// An embedded markdown document.
    Doc(&'static str),
}

const TOPICS: &[(&str, Topic)] = &[
    ("cli", Topic::Cli),
    ("examples", Topic::Examples),
    ("env", Topic::Env),
    ("readme", Topic::Doc(README)),
    ("user-guide", Topic::Doc(USER_GUIDE)),
];

fn topic_names() -> String {
    TOPICS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A markdown heading: level (1-6), title, and its line index.
struct Heading {
    level: usize,
    title: String,
    line: usize,
}

/// Headings outside fenced code blocks (shell comments start with `#` too).
fn headings(doc: &str) -> Vec<Heading> {
    let mut fence: Option<char> = None;
    let mut out = Vec::new();
    for (line, text) in doc.lines().enumerate() {
        let t = text.trim_start();
        if let Some(c) = t.chars().next().filter(|c| matches!(c, '`' | '~'))
            && t.starts_with(&c.to_string().repeat(3))
        {
            fence = match fence {
                None => Some(c),
                Some(open) if open == c => None,
                other => other,
            };
            continue;
        }
        if fence.is_some() || text.starts_with(' ') {
            continue;
        }
        let level = text.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level)
            && let Some(title) = text[level..].strip_prefix(' ')
        {
            out.push(Heading {
                level,
                title: title.trim().trim_end_matches('#').trim().to_string(),
                line,
            });
        }
    }
    out
}

fn toc(doc: &str) -> String {
    let hs = headings(doc);
    let base = hs.iter().map(|h| h.level).min().unwrap_or(1);
    hs.iter()
        .map(|h| format!("{}{}\n", "  ".repeat(h.level - base), h.title))
        .collect()
}

/// `Parent > Title` for a heading (to tell equal titles apart).
fn path(hs: &[Heading], h: &Heading) -> String {
    let parent = hs
        .iter()
        .rev()
        .find(|p| p.line < h.line && p.level < h.level && p.level > 1);
    match parent {
        Some(p) => format!("{} > {}", path(hs, p), h.title),
        None => h.title.clone(),
    }
}

/// The section whose heading matches `query` (exact, else unique partial,
/// case-insensitive): from its heading up to the next heading of the same
/// or a higher level.
fn section(doc: &str, query: &str) -> Result<String> {
    let hs = headings(doc);
    let q = query.to_lowercase();
    let exact: Vec<_> = hs.iter().filter(|h| h.title.to_lowercase() == q).collect();
    let hits = if exact.is_empty() {
        hs.iter()
            .filter(|h| h.title.to_lowercase().contains(&q))
            .collect()
    } else {
        exact
    };
    let h = match hits.as_slice() {
        [] => bail!("No section matches '{query}'. Try --toc to list the headings."),
        [h] => h,
        many => {
            let titles: Vec<_> = many.iter().map(|h| format!("  {}", path(&hs, h))).collect();
            bail!(
                "Multiple sections match '{query}':\n{}\nBe more specific, e.g. a parent section.",
                titles.join("\n")
            );
        }
    };
    let end = hs
        .iter()
        .find(|n| n.line > h.line && n.level <= h.level)
        .map_or(usize::MAX, |n| n.line);
    let lines: Vec<_> = doc.lines().skip(h.line).take(end - h.line).collect();
    Ok(format!("{}\n", lines.join("\n").trim_end()))
}

fn print(text: &str) {
    crate::outln!("{}", text.trim_end());
}

/// Handle `tct --help [topic] [--toc] [--section <heading>]` (also `-h`,
/// `tct help …`). `args` are all CLI args; everything after the help word
/// is the topic and its options.
pub fn run(args: &[String]) -> Result<()> {
    let at = args
        .iter()
        .position(|a| a == "--help" || a == "-h")
        .unwrap_or(0);
    let mut rest = args[at + 1..].iter();
    let (mut topic, mut toc_only, mut sect) = (None, false, None);
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--toc" => toc_only = true,
            "--section" => match rest.next() {
                Some(s) => sect = Some(s.as_str()),
                None => bail!("--section requires a heading argument"),
            },
            t if !t.starts_with('-') && topic.is_none() => topic = Some(t),
            other => bail!("Unexpected help argument '{other}'. Run 'tct --help' for topics."),
        }
    }
    let Some(name) = topic else {
        if toc_only || sect.is_some() {
            bail!("--toc and --section need a document topic: readme, user-guide");
        }
        print(OVERVIEW);
        return Ok(());
    };
    let Some((_, kind)) = TOPICS.iter().find(|(n, _)| *n == name) else {
        bail!("Unknown help topic '{name}'. Topics: {}", topic_names());
    };
    match kind {
        Topic::Doc(doc) => match (toc_only, sect) {
            (true, Some(_)) => bail!("--toc and --section cannot be combined"),
            (true, None) => print(&toc(doc)),
            (false, Some(s)) => print(&section(doc, s)?),
            (false, None) => print(doc),
        },
        _ if toc_only || sect.is_some() => {
            bail!("--toc and --section only apply to readme and user-guide")
        }
        Topic::Cli => print(CLI),
        Topic::Examples => print(EXAMPLES),
        Topic::Env => print(ENV),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Title\n\nintro\n\n## A\n\n```sh\n# not a heading\n```\n\n### A1\n\ntext\n\n## B\n\n~~~\n## fenced\n~~~\n\n## A\n\n    ## indented\n";

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn headings_skip_fences_and_indented_code() {
        let t: Vec<_> = headings(DOC).into_iter().map(|h| h.title).collect();
        assert_eq!(t, ["Title", "A", "A1", "B", "A"]);
    }

    #[test]
    fn toc_indents_by_level() {
        assert_eq!(toc(DOC), "Title\n  A\n    A1\n  B\n  A\n");
        assert_eq!(toc("no headings"), "");
    }

    #[test]
    fn section_runs_to_next_heading_of_same_or_higher_level() {
        let a1 = section(DOC, "a1").unwrap();
        assert_eq!(a1, "### A1\n\ntext\n");
        let b = section(DOC, "b").unwrap();
        assert!(b.starts_with("## B") && b.contains("## fenced") && !b.contains("## A"));
        assert!(section(DOC, "title").unwrap().contains("indented"));
    }

    #[test]
    fn section_exact_beats_partial_and_ambiguity_is_an_error() {
        let doc = "## Cards\n\nx\n\n## Cards and more\n\ny\n";
        assert_eq!(section(doc, "cards").unwrap(), "## Cards\n\nx\n");
        let e = section(DOC, "a").unwrap_err().to_string();
        assert!(e.contains("Multiple sections"), "{e}");
        let e = section(DOC, "zzz").unwrap_err().to_string();
        assert!(e.contains("--toc"), "{e}");
    }

    #[test]
    fn ambiguous_equal_titles_show_their_parents() {
        let doc = "# T\n\n## X\n\n### Cards\n\n## Y\n\n### Cards\n";
        let e = section(doc, "cards").unwrap_err().to_string();
        assert!(e.contains("X > Cards") && e.contains("Y > Cards"), "{e}");
    }

    #[test]
    fn every_topic_runs_and_is_listed_in_the_overview() {
        for (name, _) in TOPICS {
            assert!(OVERVIEW.contains(&format!("--help {name}")), "{name}");
            run(&s(&["--help", name])).unwrap();
        }
        run(&s(&["--help"])).unwrap();
    }

    #[test]
    fn overview_links_website_and_repo() {
        assert!(OVERVIEW.contains("https://waschmittel.github.io/tct/"));
        assert!(OVERVIEW.contains("https://github.com/waschmittel/tct"));
    }

    #[test]
    fn bad_arguments_are_errors() {
        for args in [
            &["--help", "nope"][..],
            &["--help", "--toc"],
            &["--help", "cli", "--toc"],
            &["--help", "readme", "--toc", "--section", "x"],
            &["--help", "readme", "--section"],
            &["--help", "readme", "--bogus"],
        ] {
            assert!(run(&s(args)).is_err(), "{args:?}");
        }
    }

    #[test]
    fn embedded_docs_have_headings_and_the_user_guide_cli_section() {
        assert!(toc(README).lines().count() > 3);
        assert!(section(USER_GUIDE, "CLI Reference").unwrap().len() > 500);
    }
}
