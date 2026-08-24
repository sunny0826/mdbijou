//! Embedded UI assets required by the GPUI component icon set.
//!
//! All glyphs are verbatim Lucide icons (ISC license, lucide.dev) served on
//! the `icons/<slug>.svg` paths that gpui-component's IconName enum requests.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let asset = match path {
            "icons/folder-open.svg" => Some(FOLDER_OPEN),
            "icons/file.svg" => Some(FILE),
            "icons/undo-2.svg" => Some(UNDO),
            "icons/panel-left-close.svg" => Some(PANEL_LEFT_CLOSE),
            "icons/panel-left-open.svg" => Some(PANEL_LEFT_OPEN),
            "icons/settings-2.svg" => Some(SETTINGS),
            "icons/square-terminal.svg" => Some(TERMINAL),
            "icons/book-open.svg" => Some(BOOK_OPEN),
            "icons/folder.svg" => Some(FOLDER),
            "icons/globe.svg" => Some(GLOBE),
            "icons/heart.svg" => Some(HEART),
            "icons/info.svg" => Some(INFO),
            "icons/map.svg" => Some(MAP),
            "icons/star.svg" => Some(STAR),
            "icons/user.svg" => Some(USER),
            "icons/check.svg" => Some(CHECK),
            _ => None,
        };
        Ok(asset.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

const FOLDER_OPEN: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-folder-open"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />
</svg>"#;
const FILE: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-file"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
  <path d="M14 2v4a2 2 0 0 0 2 2h4" />
</svg>"#;
const UNDO: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-undo-2"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M9 14 4 9l5-5" />
  <path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11" />
</svg>"#;
const PANEL_LEFT_CLOSE: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-panel-left-close"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <rect width="18" height="18" x="3" y="3" rx="2" />
  <path d="M9 3v18" />
  <path d="m16 15-3-3 3-3" />
</svg>"#;
const PANEL_LEFT_OPEN: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-panel-left-open"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <rect width="18" height="18" x="3" y="3" rx="2" />
  <path d="M9 3v18" />
  <path d="m14 9 3 3-3 3" />
</svg>"#;
const SETTINGS: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-settings-2"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M14 17H5" />
  <path d="M19 7h-9" />
  <circle cx="17" cy="17" r="3" />
  <circle cx="7" cy="7" r="3" />
</svg>"#;
const TERMINAL: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-square-terminal"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="m7 11 2-2-2-2" />
  <path d="M11 13h4" />
  <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
</svg>"#;
const BOOK_OPEN: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-book-open"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M12 7v14" />
  <path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z" />
</svg>"#;
const FOLDER: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-folder"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
</svg>"#;
const GLOBE: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-globe"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <circle cx="12" cy="12" r="10" />
  <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
  <path d="M2 12h20" />
</svg>"#;
const HEART: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-heart"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" />
</svg>"#;
const INFO: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-info"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <circle cx="12" cy="12" r="10" />
  <path d="M12 16v-4" />
  <path d="M12 8h.01" />
</svg>"#;
const MAP: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-map"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M14.106 5.553a2 2 0 0 0 1.788 0l3.659-1.83A1 1 0 0 1 21 4.619v12.764a1 1 0 0 1-.553.894l-4.553 2.277a2 2 0 0 1-1.788 0l-4.212-2.106a2 2 0 0 0-1.788 0l-3.659 1.83A1 1 0 0 1 3 19.381V6.618a1 1 0 0 1 .553-.894l4.553-2.277a2 2 0 0 1 1.788 0z" />
  <path d="M15 5.764v15" />
  <path d="M9 3.236v15" />
</svg>"#;
const STAR: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-star"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z" />
</svg>"#;
const USER: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-user"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
  <circle cx="12" cy="7" r="4" />
</svg>"#;
const CHECK: &[u8] = br#"<!-- @license lucide-static v0.525.0 - ISC -->
<svg
  class="lucide lucide-check"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M20 6 9 17l-5-5" />
</svg>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplies_every_mdx_preview_icon() {
        for path in [
            "icons/folder-open.svg",
            "icons/file.svg",
            "icons/undo-2.svg",
            "icons/panel-left-close.svg",
            "icons/panel-left-open.svg",
            "icons/settings-2.svg",
            "icons/square-terminal.svg",
            "icons/book-open.svg",
            "icons/folder.svg",
            "icons/globe.svg",
            "icons/heart.svg",
            "icons/info.svg",
            "icons/map.svg",
            "icons/star.svg",
            "icons/user.svg",
            "icons/check.svg",
        ] {
            assert!(Assets.load(path).unwrap().is_some(), "missing {path}");
        }
    }
}
