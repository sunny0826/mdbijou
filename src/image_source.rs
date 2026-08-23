//! Rendering-backend-neutral Markdown image source handling.

use std::path::{Path, PathBuf};

/// Resolve a local Markdown image source against the document directory.
/// Remote URLs and data URIs deliberately return `None` so each UI backend can
/// use its native network/asset loader.
pub fn resolve_local_path(base_dir: &Path, src: &str) -> Option<PathBuf> {
    let stripped = src.split(['#', '?']).next().unwrap_or(src);
    let source = stripped.trim();
    if source.is_empty()
        || source.starts_with("data:")
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("//")
    {
        return None;
    }
    let path = Path::new(source);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        Some(base_dir.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_sources_against_document_directory() {
        assert_eq!(
            resolve_local_path(Path::new("/tmp/guide"), "assets/photo.png?large"),
            Some(PathBuf::from("/tmp/guide/assets/photo.png"))
        );
    }

    #[test]
    fn leaves_remote_sources_for_the_backend_loader() {
        assert!(resolve_local_path(Path::new("/tmp"), "https://example.com/a.png").is_none());
        assert!(resolve_local_path(Path::new("/tmp"), "data:image/png;base64,AAAA").is_none());
    }
}
