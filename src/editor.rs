/// Open the user's preferred editor to compose or revise text.
///
/// Editor discovery follows the conventional priority order: `$VISUAL`,
/// then `$EDITOR`, then a platform default (`nano` on Unix, `notepad` on
/// Windows).  The value is split into a program and optional arguments
/// using POSIX shell-word rules (via the `shell-words` crate), so values
/// like `vim -u NONE` or `code --wait` work correctly.
///
/// In tests, set `TD_FORCE_EDITOR=<cmd>` to bypass the real editor and
/// supply a fake one (e.g. a shell one-liner that writes known content).
use anyhow::{anyhow, Result};

/// Locate the editor binary (and any leading flags) from the environment.
///
/// Priority: `TD_FORCE_EDITOR` (tests) → `$VISUAL` → `$EDITOR` →
/// platform default.  Returns the argv list ready to pass to
/// `std::process::Command`.
pub fn find_editor() -> Result<Vec<String>> {
    // TD_FORCE_EDITOR lets tests inject a fake editor without touching the
    // real environment variables.
    let raw = std::env::var("TD_FORCE_EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_owned()
            } else {
                "nano".to_owned()
            }
        });

    let argv = shell_words::split(&raw)
        .map_err(|e| anyhow!("could not parse editor command {:?}: {}", raw, e))?;

    if argv.is_empty() {
        return Err(anyhow!(
            "editor command is empty; set $VISUAL or $EDITOR to a valid command"
        ));
    }

    Ok(argv)
}

/// Strip lines beginning with `TD: `, then extract the first non-blank line
/// as the title and everything that follows as the description.
///
/// Lines starting with `#` are intentionally preserved — they are markdown
/// headings that belong to the task content, not directives.
///
/// Returns `Err` when the result would be an empty title.
pub fn parse_result(content: &str) -> Result<(String, String)> {
    let meaningful: Vec<&str> = content.lines().filter(|l| !l.starts_with("TD: ")).collect();

    // Find the first non-blank line — that becomes the title.
    let title_pos = meaningful
        .iter()
        .position(|l| !l.trim().is_empty())
        .ok_or_else(|| anyhow!("editor returned an empty message; task creation aborted"))?;

    let title = meaningful[title_pos].trim().to_owned();

    // Everything after the title line is the description; trim surrounding
    // blank lines but preserve internal ones.
    let desc = meaningful[title_pos + 1..].join("\n").trim().to_owned();

    Ok((title, desc))
}

/// Write `template` to a temporary file, open the editor, read the
/// result back, clean up the file, and return `parse_result`.
pub fn open(template: &str) -> Result<(String, String)> {
    use std::io::Write;

    // Create a uniquely-named temp file so concurrent td invocations don't
    // collide.  The `.md` extension is a hint to editors to enable markdown
    // highlighting.
    let id = ulid::Ulid::new();
    let path = std::env::temp_dir().join(format!("td-edit-{id}.md"));

    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(template.as_bytes())?;
    }

    // Best-effort cleanup: delete the temp file even if the editor or parse
    // step fails.
    let result = (|| {
        let argv = find_editor()?;
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| anyhow!("editor command resolved to an empty list"))?;

        let status = std::process::Command::new(program)
            .args(args)
            .arg(&path)
            .status()
            .map_err(|e| anyhow!("failed to launch editor {:?}: {}", program, e))?;

        if !status.success() {
            return Err(anyhow!("editor exited with status {}", status));
        }

        let content = std::fs::read_to_string(&path)?;
        parse_result(&content)
    })();

    // Always remove the temp file, regardless of success or failure.
    let _ = std::fs::remove_file(&path);

    result
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── find_editor ───────────────────────────────────────────────────────────

    #[test]
    fn find_editor_prefers_td_force_editor() {
        // TD_FORCE_EDITOR should take precedence over VISUAL and EDITOR.
        std::env::remove_var("VISUAL");
        std::env::remove_var("EDITOR");
        std::env::set_var("TD_FORCE_EDITOR", "myfakeeditor");
        let argv = find_editor().unwrap();
        assert_eq!(argv, vec!["myfakeeditor"]);
        std::env::remove_var("TD_FORCE_EDITOR");
    }

    #[test]
    fn find_editor_prefers_visual_over_editor() {
        std::env::remove_var("TD_FORCE_EDITOR");
        std::env::set_var("VISUAL", "vis");
        std::env::set_var("EDITOR", "ed");
        let argv = find_editor().unwrap();
        assert_eq!(argv[0], "vis");
        std::env::remove_var("VISUAL");
        std::env::remove_var("EDITOR");
    }

    #[test]
    fn find_editor_falls_back_to_editor() {
        std::env::remove_var("TD_FORCE_EDITOR");
        std::env::remove_var("VISUAL");
        std::env::set_var("EDITOR", "myeditor");
        let argv = find_editor().unwrap();
        assert_eq!(argv[0], "myeditor");
        std::env::remove_var("EDITOR");
    }

    #[test]
    fn find_editor_splits_args() {
        // Values like "vim -u NONE" must be split into separate argv entries.
        std::env::remove_var("TD_FORCE_EDITOR");
        std::env::remove_var("VISUAL");
        std::env::set_var("EDITOR", "vim -u NONE");
        let argv = find_editor().unwrap();
        assert_eq!(argv, vec!["vim", "-u", "NONE"]);
        std::env::remove_var("EDITOR");
    }

    #[test]
    fn find_editor_splits_quoted_path() {
        // Paths with spaces must survive shell-word quoting.
        std::env::remove_var("TD_FORCE_EDITOR");
        std::env::set_var("VISUAL", "\"/usr/local/bin/my editor\"");
        std::env::remove_var("EDITOR");
        let argv = find_editor().unwrap();
        assert_eq!(argv, vec!["/usr/local/bin/my editor"]);
        std::env::remove_var("VISUAL");
    }

    #[test]
    fn find_editor_errors_on_unmatched_quote() {
        std::env::remove_var("TD_FORCE_EDITOR");
        std::env::remove_var("VISUAL");
        std::env::set_var("EDITOR", "vim 'unterminated");
        let result = find_editor();
        assert!(result.is_err());
        std::env::remove_var("EDITOR");
    }

    // ── parse_result ──────────────────────────────────────────────────────────

    #[test]
    fn parse_strips_td_comment_lines() {
        let content = "TD: ignore me\nMy title\nsome description";
        let (title, desc) = parse_result(content).unwrap();
        assert_eq!(title, "My title");
        assert_eq!(desc, "some description");
    }

    #[test]
    fn parse_preserves_markdown_headings() {
        // Lines starting with '#' are NOT comments — they're markdown headings
        // and must be preserved as description content.
        let content = "TD: ignore me\nMy title\n## Details\nsome description";
        let (title, desc) = parse_result(content).unwrap();
        assert_eq!(title, "My title");
        assert_eq!(desc, "## Details\nsome description");
    }

    #[test]
    fn parse_returns_empty_desc_when_only_title() {
        let content = "TD: comment\nJust a title";
        let (title, desc) = parse_result(content).unwrap();
        assert_eq!(title, "Just a title");
        assert_eq!(desc, "");
    }

    #[test]
    fn parse_trims_surrounding_blank_lines_from_desc() {
        let content = "Title\n\nParagraph one\n\nParagraph two\n";
        let (title, desc) = parse_result(content).unwrap();
        assert_eq!(title, "Title");
        // Leading/trailing blank lines stripped from description.
        assert_eq!(desc, "Paragraph one\n\nParagraph two");
    }

    #[test]
    fn parse_errors_on_only_comments() {
        let content = "TD: first comment\nTD: second comment\n";
        let result = parse_result(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_errors_on_empty_string() {
        let result = parse_result("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_errors_on_blank_lines_only() {
        let result = parse_result("   \n\n  \n");
        assert!(result.is_err());
    }
}
