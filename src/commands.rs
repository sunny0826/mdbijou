//! Backend-neutral application commands and file-transition decisions.
//!
//! Native front ends dispatch the same commands, while UI-specific code owns
//! shortcuts, menus and dialogs.

/// User-intent commands shared by the egui and GPUI shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ToggleView,
    Save,
    Reload,
    Open,
    ToggleSettings,
    ToggleToc,
}

/// The safe next step when the user asks to replace the current document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenDisposition {
    ApplyImmediately,
    ConfirmDiscardChanges,
}

/// Keep unsaved work intact until the user explicitly confirms a replacement.
pub fn open_disposition(document_is_dirty: bool) -> OpenDisposition {
    if document_is_dirty {
        OpenDisposition::ConfirmDiscardChanges
    } else {
        OpenDisposition::ApplyImmediately
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_document_opens_without_confirmation() {
        assert_eq!(open_disposition(false), OpenDisposition::ApplyImmediately);
    }

    #[test]
    fn dirty_document_requires_confirmation() {
        assert_eq!(
            open_disposition(true),
            OpenDisposition::ConfirmDiscardChanges
        );
    }
}
