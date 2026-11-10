//! Output-path handling shared by the export dialog and the CLI: default
//! file names, resolving what the user typed, and listing what could come
//! next while they type.

use std::path::{Path, PathBuf};

const EXTENSION: &str = "pdf";
const MAX_SLUG: usize = 60;

pub fn default_file_name(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(MAX_SLUG).collect();
    let slug = slug.trim_end_matches('-');
    format!(
        "{}.{EXTENSION}",
        if slug.is_empty() { "card" } else { slug }
    )
}

/// Where `typed` points. Relative paths start at `cwd`, `~` is the home
/// directory, a directory (or a trailing `/`) gets `file_name` inside it,
/// and a name without extension gets `.pdf`.
pub fn resolve(typed: &str, cwd: &Path, file_name: &str) -> PathBuf {
    let typed = typed.trim();
    let expanded = match typed.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => dirs::home_dir()
            .map(|home| home.join(rest.trim_start_matches('/')))
            .unwrap_or_else(|| PathBuf::from(typed)),
        _ => PathBuf::from(typed),
    };
    let mut path = cwd.join(expanded);
    if typed.is_empty() || typed.ends_with('/') || path.is_dir() {
        path.push(file_name);
    } else if path.extension().is_none() {
        path.set_extension(EXTENSION);
    }
    path
}

/// `dir` as typed into the input: absolute, `~` for the home directory, trailing `/`.
pub fn display_dir(dir: &Path) -> String {
    let shown = match dirs::home_dir()
        .and_then(|home| dir.strip_prefix(home).ok().map(Path::to_path_buf))
    {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    };
    if shown.ends_with('/') {
        shown
    } else {
        format!("{shown}/")
    }
}

/// The directory one level above the typed directory `dir` (which ends in `/` or is empty).
pub fn parent_dir(dir: &str) -> String {
    let trimmed = dir.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        _ if trimmed.is_empty() && dir.starts_with('/') => "/".to_string(),
        _ if trimmed.is_empty() || trimmed == "~" || trimmed.ends_with("..") => format!("{dir}../"),
        Some(("", _)) => "/".to_string(),
        Some((up, _)) => format!("{up}/"),
        None => String::new(),
    }
}

/// One row of the directory listing under the path input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
}

/// Directories and existing PDFs that continue `typed`: those in the
/// directory the input names whose name starts with what is typed after its
/// last `/`. Hidden entries only show when the typed name starts with `.`.
pub fn suggestions(typed: &str, cwd: &Path) -> Vec<Entry> {
    let (dir, prefix) = split_dir(typed);
    let dir = resolve_dir(dir, cwd);
    let prefix_lower = prefix.to_lowercase();
    let mut entries: Vec<Entry> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let is_dir = e.path().is_dir();
            let is_pdf = Path::new(&name)
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION));
            let visible = !name.starts_with('.') || prefix.starts_with('.');
            (visible && (is_dir || is_pdf) && name.to_lowercase().starts_with(&prefix_lower))
                .then_some(Entry { name, is_dir })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    if prefix.is_empty() || prefix == "." {
        entries.insert(
            0,
            Entry {
                name: "..".into(),
                is_dir: true,
            },
        );
    }
    entries
}

/// Split at the last `/`: the directory part (with its slash) and the name typed after it.
pub fn split_dir(typed: &str) -> (&str, &str) {
    typed.rsplit_once('/').map_or(("", typed), |(_, name)| {
        typed.split_at(typed.len() - name.len())
    })
}

fn resolve_dir(dir: &str, cwd: &Path) -> PathBuf {
    if dir.is_empty() {
        return cwd.to_path_buf();
    }
    resolve(dir, cwd, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_slugs() {
        assert_eq!(default_file_name("Fix the Bug!"), "fix-the-bug.pdf");
        assert_eq!(default_file_name("Größe & Maß"), "größe-maß.pdf");
        assert_eq!(default_file_name("***"), "card.pdf");
        assert_eq!(default_file_name(&"a".repeat(200)).len(), MAX_SLUG + 4);
    }

    #[test]
    fn typed_paths_resolve_against_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path();
        std::fs::create_dir(cwd.join("sub")).unwrap();
        assert_eq!(resolve("", cwd, "c.pdf"), cwd.join("c.pdf"));
        assert_eq!(resolve("out", cwd, "c.pdf"), cwd.join("out.pdf"));
        assert_eq!(resolve("x.pdf", cwd, "c.pdf"), cwd.join("x.pdf"));
        assert_eq!(resolve("sub", cwd, "c.pdf"), cwd.join("sub/c.pdf"));
        assert_eq!(resolve("new/", cwd, "c.pdf"), cwd.join("new/c.pdf"));
        assert_eq!(
            resolve("/abs/x.pdf", cwd, "c.pdf"),
            PathBuf::from("/abs/x.pdf")
        );
    }

    #[test]
    fn suggestions_list_dirs_and_pdfs_filtered_by_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path();
        std::fs::create_dir(cwd.join("docs")).unwrap();
        std::fs::create_dir(cwd.join("data")).unwrap();
        std::fs::create_dir(cwd.join(".hidden")).unwrap();
        std::fs::write(cwd.join("a.pdf"), b"").unwrap();
        std::fs::write(cwd.join("b.txt"), b"").unwrap();
        let names = |typed: &str| -> Vec<String> {
            suggestions(typed, cwd)
                .into_iter()
                .map(|e| e.name)
                .collect()
        };
        assert_eq!(names(""), ["..", "data", "docs", "a.pdf"]);
        assert_eq!(names("d"), ["data", "docs"]);
        assert_eq!(names("docs/"), [".."]);
        assert_eq!(names("."), ["..", ".hidden"]);
        assert_eq!(names("nope/x"), Vec::<String>::new());
    }

    #[test]
    fn parent_dir_pops_one_segment() {
        assert_eq!(parent_dir("a/b/"), "a/");
        assert_eq!(parent_dir("a/"), "");
        assert_eq!(parent_dir("/a/"), "/");
        assert_eq!(parent_dir("/"), "/");
        assert_eq!(parent_dir(""), "../");
        assert_eq!(parent_dir("../"), "../../");
        assert_eq!(parent_dir("~/"), "~/../");
    }

    #[test]
    fn display_dir_ends_with_a_slash_and_abbreviates_home() {
        assert_eq!(display_dir(Path::new("/tmp/x")), "/tmp/x/");
        let home = dirs::home_dir().unwrap();
        assert_eq!(display_dir(&home), "~/");
        assert_eq!(display_dir(&home.join("a")), "~/a/");
    }

    #[test]
    fn split_dir_keeps_the_slash() {
        assert_eq!(split_dir("a/b/c"), ("a/b/", "c"));
        assert_eq!(split_dir("c"), ("", "c"));
        assert_eq!(split_dir("a/"), ("a/", ""));
    }
}
