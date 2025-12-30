use std::borrow::Cow;

/// Extracts a normalized extension label from a file name.
///
/// Returns the extension in lowercase with a leading dot (e.g., ".rs"),
/// or "(no ext)" for files without a valid extension.
///
/// An extension is considered valid if it:
/// - Is non-empty
/// - Is at most 10 characters long
/// - Does not contain '/'
pub fn extension_label(file_name: &str) -> Cow<'static, str> {
    match file_name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && ext.len() <= 10 && !ext.contains('/') => {
            Cow::Owned(format!(".{}", ext.to_ascii_lowercase()))
        }
        _ => Cow::Borrowed("(no ext)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_extension() {
        assert_eq!(extension_label("main.rs"), ".rs");
        assert_eq!(extension_label("logo.png"), ".png");
        assert_eq!(extension_label("README.md"), ".md");
    }

    #[test]
    fn test_no_extension() {
        assert_eq!(extension_label("Makefile"), "(no ext)");
        assert_eq!(extension_label("LICENSE"), "(no ext)");
    }

    #[test]
    fn test_hidden_files() {
        // Hidden files with an extension part are treated as having that extension
        assert_eq!(extension_label(".gitignore"), ".gitignore");
        assert_eq!(extension_label(".env"), ".env");
        // But truly extension-less hidden files remain so
        assert_eq!(extension_label("."), "(no ext)");
    }

    #[test]
    fn test_multiple_dots() {
        assert_eq!(extension_label("file.test.rs"), ".rs");
        assert_eq!(extension_label("app.config.json"), ".json");
    }

    #[test]
    fn test_case_normalization() {
        assert_eq!(extension_label("IMAGE.PNG"), ".png");
        assert_eq!(extension_label("Script.JS"), ".js");
    }

    #[test]
    fn test_long_extension_rejected() {
        // Extensions longer than 10 chars should be rejected
        assert_eq!(extension_label("file.verylongextension"), "(no ext)");
    }
}
