//! Supported document types shared by file dialogs and tests.

pub const DOCUMENT_FILTER_NAME: &str = "Markdown / MDX";
pub const DOCUMENT_EXTENSIONS: &[&str] = &["md", "markdown", "mdx", "txt"];
pub const DEFAULT_DOCUMENT_NAME: &str = "untitled.md";

#[cfg(test)]
fn is_supported_document(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            DOCUMENT_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_mdx_documents_case_insensitively() {
        assert!(is_supported_document(std::path::Path::new("guide.mdx")));
        assert!(is_supported_document(std::path::Path::new("guide.MDX")));
    }

    #[test]
    fn rejects_unrelated_file_types() {
        assert!(!is_supported_document(std::path::Path::new("guide.tsx")));
        assert!(!is_supported_document(std::path::Path::new("guide")));
    }
}
