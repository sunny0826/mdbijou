//! GPUI preview elements backed by MDbijou's existing document IR.
//!
//! All Markdown blocks use MDbijou's native renderer so the preview stays
//! faithful to the selected document theme and extension rendering.

use crate::color::Color;
use crate::config::Config;
use crate::document::{Align, Block, Document, Inline};
use crate::html;
use crate::image_source::resolve_local_path;
use crate::{mermaid, theme::Theme};
use gpui::{
    div, img, prelude::FluentBuilder, px, AnyElement, Font, FontFallbacks, FontStyle, FontWeight,
    HighlightStyle, InteractiveElement, IntoElement, ObjectFit, ParentElement, StrikethroughStyle,
    Styled, StyledImage, StyledText, UnderlineStyle,
};
use gpui_component::{Icon, IconName};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;
use std::sync::{mpsc, Arc};
use std::time::Duration;

const MAX_MERMAID_IMAGES: usize = 32;
const MAX_REMOTE_IMAGES: usize = 64;

/// A small, application-owned cache for remote images.  GPUI's generic image
/// asset loader is normally convenient, but its asynchronous URI path can
/// fail after HTTP redirects on macOS.  Keeping the fetch and decode here
/// gives Markdown previews a dependable path and keeps completed images
/// stable across rerenders.
pub struct RemoteImageStore {
    sender: mpsc::Sender<(String, Result<Vec<u8>, String>)>,
    receiver: mpsc::Receiver<(String, Result<Vec<u8>, String>)>,
    images: HashMap<String, RemoteImageState>,
}

enum RemoteImageState {
    Loading,
    Ready(Arc<gpui::Image>),
    Failed(String),
}

pub enum RemoteImageStatus {
    Loading,
    Ready(Arc<gpui::Image>),
    Failed(String),
}

impl Default for RemoteImageStore {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            images: HashMap::new(),
        }
    }
}

impl RemoteImageStore {
    pub fn image_for(&mut self, url: &str) -> RemoteImageStatus {
        match self.images.get(url) {
            Some(RemoteImageState::Loading) => RemoteImageStatus::Loading,
            Some(RemoteImageState::Ready(image)) => RemoteImageStatus::Ready(image.clone()),
            Some(RemoteImageState::Failed(reason)) => RemoteImageStatus::Failed(reason.clone()),
            None => {
                self.images
                    .insert(url.to_owned(), RemoteImageState::Loading);
                let sender = self.sender.clone();
                let url = url.to_owned();
                std::thread::spawn(move || {
                    let _ = sender.send((url.clone(), fetch_remote_image(&url)));
                });
                RemoteImageStatus::Loading
            }
        }
    }

    /// Returns whether an image changed state and the preview needs repainting.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((url, result)) = self.receiver.try_recv() {
            let state = match result.and_then(|bytes| image_from_bytes(&bytes)) {
                Ok(image) => RemoteImageState::Ready(image),
                Err(error) => {
                    eprintln!("mdbijou: remote image failed for {url}: {error}");
                    RemoteImageState::Failed(error)
                }
            };
            if self.images.len() >= MAX_REMOTE_IMAGES {
                self.images.clear();
            }
            self.images.insert(url, state);
            changed = true;
        }
        changed
    }

    pub fn has_pending(&self) -> bool {
        self.images
            .values()
            .any(|state| matches!(state, RemoteImageState::Loading))
    }
}

fn fetch_remote_image(url: &str) -> Result<Vec<u8>, String> {
    let proxy_error = match fetch_with_reqwest(url) {
        Ok(bytes) => return Ok(bytes),
        Err(error) => error,
    };

    // Some shell sessions keep a localhost proxy variable after the proxy
    // application has stopped. curl can bypass it without waiting for a
    // second Rust client connection timeout.
    fetch_with_curl(url).map_err(|curl_error| format!("proxy: {proxy_error}; curl: {curl_error}"))
}

fn fetch_with_reqwest(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("mdbijou/0.0.2")
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?;
    client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        )
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("{error:?}"))
        .and_then(|response| {
            response
                .bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(|error| format!("{error:?}"))
        })
}

fn fetch_with_curl(url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("/usr/bin/curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            "20",
            "--noproxy",
            "*",
            "--user-agent",
            "mdbijou/0.0.2",
            "--header",
            "Accept: image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
            "--",
            url,
        ])
        .output()
        .map_err(|error| format!("start: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().into())
    }
}

fn image_from_bytes(bytes: &[u8]) -> Result<Arc<gpui::Image>, String> {
    let format = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        gpui::ImageFormat::Png
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        gpui::ImageFormat::Jpeg
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        gpui::ImageFormat::Gif
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        gpui::ImageFormat::Webp
    } else if bytes.starts_with(b"BM") {
        gpui::ImageFormat::Bmp
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        gpui::ImageFormat::Tiff
    } else if std::str::from_utf8(bytes)
        .ok()
        .is_some_and(|text| text.trim_start().starts_with('<'))
    {
        gpui::ImageFormat::Svg
    } else {
        return Err("unsupported image format".into());
    };
    Ok(Arc::new(gpui::Image::from_bytes(format, bytes.to_vec())))
}

thread_local! {
    /// GPUI's image loader treats each newly allocated `Image` as a new asset.
    /// Keeping the image itself stable prevents reload/flicker while the parent
    /// document rerenders during scrolling or window resizing.
    static MERMAID_IMAGES: RefCell<HashMap<u64, Arc<gpui::Image>>> = RefCell::new(HashMap::new());
}

/// Render all parsed blocks in a readable, scrollable-paper-friendly column.
pub fn document_blocks(
    document: &Document,
    theme: &Theme,
    cfg: &Config,
    remote_images: &mut RemoteImageStore,
) -> Vec<AnyElement> {
    let base_dir = document
        .path
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."));
    let mut elements = Vec::with_capacity(document.blocks.len());
    for block in &document.blocks {
        // The outer, indexed child is deliberately retained: the app's
        // ScrollHandle targets these top-level blocks for TOC navigation.
        // Padding sits outside the constrained reading column so it stays
        // symmetrical even when the window becomes narrow.
        elements.push(
            div()
                .w_full()
                .px(px(48.0))
                .flex()
                .justify_center()
                .font_family(preview_font_family(&cfg.font_family))
                .text_size(px(cfg.font_size.clamp(12.0, 28.0)))
                .line_height(px((cfg.font_size * cfg.line_height).clamp(16.0, 52.0)))
                .text_color(theme_color(theme.c.foreground))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .max_w(px(cfg.content_width.clamp(420.0, 1100.0)))
                        .child(block_element(block, base_dir, theme, remote_images)),
                )
                .into_any_element(),
        );
    }
    elements
}

fn preview_font_family(id: &str) -> &'static str {
    match id {
        "pingfang" => "PingFang SC",
        "hiragino" => "Hiragino Sans GB",
        "songti" => "Songti SC",
        "heiti" => "STHeiti",
        _ => "PingFang SC",
    }
}

/// Editorial display face for headings: a serif with CJK fallback so Latin
/// and Chinese headings share one typographic voice. New York ships with macOS.
fn heading_font() -> Font {
    let mut font = gpui::font("New York");
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "Songti SC".into(),
        "PingFang SC".into(),
    ]));
    font
}

fn block_element(
    block: &Block,
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    match block {
        Block::Heading {
            level,
            inlines,
            align,
        } => {
            let (size, line, weight, margin_top) = match level {
                1 => (32.0, 42.0, FontWeight::BOLD, 36.0),
                2 => (25.0, 34.0, FontWeight::SEMIBOLD, 30.0),
                3 => (20.0, 28.0, FontWeight::SEMIBOLD, 24.0),
                _ => (17.0, 24.0, FontWeight::SEMIBOLD, 20.0),
            };
            let element = div()
                .mt(px(margin_top))
                .mb(px(10.0))
                .font(heading_font())
                .font_weight(weight)
                .text_size(px(size))
                .line_height(px(line))
                .text_color(theme_color(theme.c.heading))
                .child(inline_text(inlines, theme));
            text_align(element, *align).into_any_element()
        }
        Block::Paragraph { inlines, align } => {
            if let [Inline::Image { src, alt, width }] = inlines.as_slice() {
                image_element(base_dir, src, alt, *width, *align, theme, remote_images)
            } else {
                text_align(
                    div().mb(px(16.0)).child(inline_text(inlines, theme)),
                    *align,
                )
                .into_any_element()
            }
        }
        Block::CodeBlock { lang, text } => {
            if lang.as_deref() == Some("mermaid") {
                mermaid_element(text, theme)
                    .unwrap_or_else(|| code_block_element(lang.as_deref(), text, theme))
            } else {
                code_block_element(lang.as_deref(), text, theme)
            }
        }
        Block::BlockQuote { blocks } => {
            nested_blocks(blocks, base_dir, theme, remote_images, |content| {
                content
                    .mb(px(18.0))
                    .pl(px(18.0))
                    .border_l(px(3.0))
                    .border_color(theme_color(theme.c.blockquote_bar))
                    .text_color(theme_color(theme.c.blockquote_fg))
            })
        }
        Block::List {
            ordered,
            start,
            items,
        } => list_element(*ordered, *start, items, base_dir, theme, remote_images),
        Block::TaskList { checked, items } => {
            let mut list = div().mb(px(18.0)).flex().flex_col().gap(px(8.0));
            for (index, item) in items.iter().enumerate() {
                let done = checked.get(index).copied().unwrap_or(false);
                list = list.child(list_row(
                    task_checkbox(done, theme).into_any_element(),
                    item,
                    base_dir,
                    theme,
                    remote_images,
                ));
            }
            list.into_any_element()
        }
        Block::Table { header, rows, .. } => {
            let mut table = div()
                .mb(px(18.0))
                .border_1()
                .border_color(theme_color(theme.c.table_border))
                .rounded(px(8.0))
                .overflow_hidden();
            if !header.is_empty() {
                table = table.child(table_row(header, true, theme));
            }
            for row in rows {
                table = table.child(table_row(row, false, theme));
            }
            table.into_any_element()
        }
        Block::CardGroup { columns, cards } => {
            card_group_element(*columns, cards, base_dir, theme, remote_images)
        }
        Block::Steps { items } => steps_element(items, base_dir, theme, remote_images),
        Block::ThematicBreak => div()
            .my(px(24.0))
            .h(px(1.0))
            .bg(theme_color(theme.c.hr))
            .into_any_element(),
        Block::Html(source) => html::html_blocks(source)
            .map(|blocks| nested_blocks(&blocks, base_dir, theme, remote_images, |content| content))
            .unwrap_or_else(|| inert_block("HTML", source, theme)),
        Block::Footnote { label, blocks } => {
            nested_blocks(blocks, base_dir, theme, remote_images, |content| {
                content
                    .mt(px(16.0))
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(theme_color(theme.c.quote_bg))
                    .text_sm()
                    .child(format!("[^{label}]"))
            })
        }
    }
}

fn text_align(element: gpui::Div, align: Align) -> gpui::Div {
    match align {
        Align::Left => element.text_left(),
        Align::Center => element.text_center(),
        Align::Right => element.text_right(),
        Align::None => element,
    }
}

fn card_group_element(
    columns: usize,
    cards: &[crate::document::Card],
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    let mut group = div().mb(px(22.0)).flex().flex_wrap().gap(px(12.0));
    for (index, card) in cards.iter().enumerate() {
        let mut body = div().mt(px(8.0));
        for block in &card.blocks {
            body = body.child(block_element(block, base_dir, theme, remote_images));
        }
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme_color(theme.c.heading));
        if let Some(icon) = card.icon.as_deref().and_then(mdx_icon) {
            header = header.child(
                div()
                    .size(px(28.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(theme_color(theme.c.surface))
                    .child(
                        Icon::new(icon)
                            .size_4()
                            .text_color(theme_color(theme.c.link)),
                    ),
            );
        }
        header = header.child(card.title.clone());
        let card_element = div()
            .id(("mdx-card", index))
            .flex_1()
            .min_w(px(220.0))
            .p(px(16.0))
            .rounded(px(crate::theme::RADIUS_LG))
            .bg(theme_color(theme.c.surface))
            .border_1()
            .border_color(theme_color(theme.c.table_border))
            .child(header)
            .child(body);
        group = group.child(if columns <= 1 {
            card_element.w_full()
        } else {
            card_element
        });
    }
    group.into_any_element()
}

fn mdx_icon(name: &str) -> Option<IconName> {
    Some(match name {
        "book-open" => IconName::BookOpen,
        "file" => IconName::File,
        "folder" => IconName::Folder,
        "globe" => IconName::Globe,
        "heart" => IconName::Heart,
        "info" => IconName::Info,
        "map" => IconName::Map,
        "star" => IconName::Star,
        "terminal" | "square-terminal" => IconName::SquareTerminal,
        "user" => IconName::User,
        _ => return None,
    })
}

fn steps_element(
    items: &[crate::document::Step],
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    let mut steps = div().mb(px(22.0)).flex().flex_col().gap(px(16.0));
    for (index, step) in items.iter().enumerate() {
        let mut content = div().flex_1().min_w_0();
        content = content.child(
            div()
                .mb(px(6.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme_color(theme.c.heading))
                .child(step.title.clone()),
        );
        for block in &step.blocks {
            content = content.child(block_element(block, base_dir, theme, remote_images));
        }
        steps = steps.child(
            div()
                .id(("mdx-step", index))
                .flex()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(26.0))
                        .h(px(26.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(13.0))
                        .bg(theme_color(theme.c.selection_bg))
                        .text_color(theme_color(theme.c.link))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child((index + 1).to_string()),
                )
                .child(content),
        );
    }
    steps.into_any_element()
}

fn nested_blocks(
    blocks: &[Block],
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
    style: impl FnOnce(gpui::Div) -> gpui::Div,
) -> AnyElement {
    let mut content = style(div());
    for block in blocks {
        content = content.child(block_element(block, base_dir, theme, remote_images));
    }
    content.into_any_element()
}

fn list_element(
    ordered: bool,
    start: u64,
    items: &[Vec<Block>],
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    let mut list = div().mb(px(18.0)).flex().flex_col().gap(px(8.0));
    for (index, item) in items.iter().enumerate() {
        let marker = if ordered {
            div().child(format!("{}.", start + index as u64))
        } else {
            div().child("•")
        };
        list = list.child(list_row(
            marker.into_any_element(),
            item,
            base_dir,
            theme,
            remote_images,
        ));
    }
    list.into_any_element()
}

/// A drawn checkbox (empty or checked with a check mark) for task lists.
fn task_checkbox(checked: bool, theme: &Theme) -> impl IntoElement {
    div()
        .size(px(16.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(theme_color(if checked {
            theme.c.link
        } else {
            theme.c.table_border
        }))
        .bg(theme_color(if checked {
            theme.c.link
        } else {
            theme.c.background
        }))
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |box_| {
            box_.child(
                Icon::new(IconName::Check)
                    .size_3p5()
                    .text_color(theme_color(theme.c.background)),
            )
        })
}

fn list_row(
    marker: AnyElement,
    blocks: &[Block],
    base_dir: &Path,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    let mut content = div().flex_1();
    for block in blocks {
        content = content.child(block_element(block, base_dir, theme, remote_images));
    }
    div()
        .flex()
        .gap(px(10.0))
        .child(
            div()
                .w(px(24.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme_color(theme.c.link))
                .child(marker),
        )
        .child(content)
        .into_any_element()
}

fn table_row(cells: &[Vec<crate::document::Inline>], header: bool, theme: &Theme) -> AnyElement {
    let mut row = div()
        .flex()
        .border_b_1()
        .border_color(theme_color(theme.c.table_border))
        .when(header, |row| {
            row.bg(theme_color(theme.c.table_header_bg))
                .font_weight(FontWeight::SEMIBOLD)
        });
    for cell in cells {
        row = row.child(div().flex_1().p(px(10.0)).child(inline_text(cell, theme)));
    }
    row.into_any_element()
}

fn inert_block(kind: &str, source: &str, theme: &Theme) -> AnyElement {
    div()
        .mb(px(16.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(theme_color(theme.c.quote_bg))
        .text_color(theme_color(theme.c.muted))
        .child(format!("{kind}: {source}"))
        .into_any_element()
}

fn image_element(
    base_dir: &Path,
    src: &str,
    alt: &str,
    width: Option<f32>,
    align: Align,
    theme: &Theme,
    remote_images: &mut RemoteImageStore,
) -> AnyElement {
    let muted = theme_color(theme.c.muted);
    let image_bg = theme_color(theme.c.image_bg);
    let border = theme_color(theme.c.table_border);
    let image = if is_remote_url(src) {
        match remote_images.image_for(src) {
            RemoteImageStatus::Ready(image) => img(image)
                .max_w_full()
                .rounded(px(crate::theme::RADIUS_MD))
                .border_1()
                .border_color(border)
                .into_any_element(),
            RemoteImageStatus::Loading => image_loading(muted),
            RemoteImageStatus::Failed(reason) => {
                remote_image_fallback(alt, &reason, muted, image_bg)
            }
        }
    } else {
        let source = resolve_local_path(base_dir, src)
            .map(gpui::ImageSource::from)
            .unwrap_or_else(|| gpui::ImageSource::from(src.to_owned()));
        img(source)
            .max_w_full()
            .rounded(px(crate::theme::RADIUS_MD))
            .border_1()
            .border_color(border)
            .with_loading(move || image_loading(muted))
            .with_fallback({
                let alt = alt.to_string();
                move || image_fallback(&alt, muted, image_bg)
            })
            .into_any_element()
    };
    let image = if let Some(width) = width {
        div().w(px(width.max(1.0))).child(image).into_any_element()
    } else {
        image
    };
    let container = div().mb(px(18.0)).flex();
    let container = match align {
        Align::Center => container.justify_center(),
        Align::Right => container.justify_end(),
        Align::Left | Align::None => container.justify_start(),
    };
    container.child(image).into_any_element()
}

fn is_remote_url(src: &str) -> bool {
    src.starts_with("https://") || src.starts_with("http://")
}

fn image_loading(muted: gpui::Hsla) -> AnyElement {
    div()
        .h(px(120.0))
        .flex()
        .items_center()
        .justify_center()
        .text_color(muted)
        .child("正在载入图片…")
        .into_any_element()
}

fn image_fallback(alt: &str, muted: gpui::Hsla, image_bg: gpui::Hsla) -> AnyElement {
    div()
        .h(px(96.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(image_bg)
        .text_color(muted)
        .child(if alt.is_empty() {
            "图片无法载入".into()
        } else {
            format!("图片无法载入：{alt}")
        })
        .into_any_element()
}

fn remote_image_fallback(
    alt: &str,
    reason: &str,
    muted: gpui::Hsla,
    image_bg: gpui::Hsla,
) -> AnyElement {
    div()
        .min_h(px(96.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(image_bg)
        .text_color(muted)
        .child(if alt.is_empty() {
            "图片无法载入".into()
        } else {
            format!("图片无法载入：{alt}")
        })
        .child(div().mt(px(6.0)).text_sm().child(format!("原因：{reason}")))
        .into_any_element()
}

fn code_block_element(lang: Option<&str>, text: &str, theme: &Theme) -> AnyElement {
    let code = text.trim_end_matches('\n');
    let highlights = code_highlight_styles(lang, code, theme);
    div()
        .mb(px(18.0))
        .rounded(px(crate::theme::RADIUS_MD))
        .border_1()
        .border_color(theme_color(theme.c.table_border))
        .bg(theme_color(theme.c.code_bg))
        .overflow_hidden()
        .child(
            div()
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme_color(theme.c.table_border))
                .bg(theme_color(theme.c.surface))
                .child(
                    div()
                        .font_family("Menlo")
                        .text_xs()
                        .text_color(theme_color(theme.c.muted))
                        .child(lang.unwrap_or("text").to_string()),
                ),
        )
        .child(
            div()
                .p(px(16.0))
                .font_family("Menlo")
                .text_size(px(13.0))
                .line_height(px(20.0))
                .text_color(theme_color(theme.c.code_fg))
                .child(StyledText::new(code.to_owned()).with_highlights(highlights)),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Preview code-block syntax highlighting
//
// A small, allocation-light byte scanner that maps tokens onto the active
// theme's `SyntaxColors`. It intentionally does not pull in a full tree-sitter
// grammar: preview code stays fast to rebuild on every rerender and shares the
// same palette as the rest of the document.
// ---------------------------------------------------------------------------

const COMMON_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "return",
    "break",
    "continue",
    "true",
    "false",
    "null",
    "undefined",
    "new",
    "class",
    "function",
    "var",
    "import",
    "export",
    "from",
    "default",
    "as",
    "try",
    "catch",
    "finally",
    "throw",
    "this",
    "typeof",
    "instanceof",
    "void",
    "delete",
    "switch",
    "case",
    "do",
    "in",
    "of",
    "static",
    "const",
    "let",
    "async",
    "await",
];

const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "const", "pub", "use", "mod", "struct", "enum", "impl", "trait", "match",
    "async", "await", "move", "ref", "self", "Self", "super", "type", "where", "loop", "dyn",
    "unsafe", "extern", "crate", "return", "if", "else", "for", "while", "break", "continue", "in",
    "true", "false", "as",
];

const PY_KEYWORDS: &[&str] = &[
    "def", "class", "return", "if", "elif", "else", "for", "while", "break", "continue", "pass",
    "import", "from", "as", "try", "except", "finally", "raise", "with", "lambda", "yield",
    "global", "nonlocal", "del", "not", "and", "or", "is", "in", "assert", "async", "await",
    "True", "False", "None",
];

fn language_keywords(lang: Option<&str>) -> &'static [&'static str] {
    match lang.map(|name| name.to_ascii_lowercase()).as_deref() {
        Some("rs" | "rust") => RUST_KEYWORDS,
        Some("py" | "python") => PY_KEYWORDS,
        Some("js" | "javascript" | "ts" | "typescript" | "jsx" | "tsx") => COMMON_KEYWORDS,
        _ => COMMON_KEYWORDS,
    }
}

fn code_highlight_styles(
    lang: Option<&str>,
    code: &str,
    theme: &Theme,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let syntax = &theme.syntax;
    let keywords = language_keywords(lang);
    let bytes = code.as_bytes();
    let n = bytes.len();
    let mut styles = Vec::new();
    let push = |styles: &mut Vec<(std::ops::Range<usize>, HighlightStyle)>,
                start: usize,
                end: usize,
                color: Color| {
        if start < end {
            styles.push((
                start..end,
                HighlightStyle {
                    color: Some(theme_color(color)),
                    ..Default::default()
                },
            ));
        }
    };

    let mut i = 0;
    while i < n {
        let b = bytes[i];
        // Line comments.
        if b == b'/' && i + 1 < n && bytes[i + 1] == b'/'
            || (matches!(
                lang.map(|name| name.to_ascii_lowercase()).as_deref(),
                Some("py" | "python" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml")
            ) && b == b'#')
        {
            let start = i;
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            push(&mut styles, start, i, syntax.comment);
            continue;
        }
        // Block comments.
        if b == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(n);
            push(&mut styles, start, i, syntax.comment);
            continue;
        }
        // Strings (single, double, backtick) with backslash escapes.
        if b == b'"' || b == b'\'' || b == b'`' {
            let quote = b;
            let start = i;
            i += 1;
            while i < n {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(n);
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            push(&mut styles, start, i, syntax.string);
            continue;
        }
        // Numbers (decimals, hex prefixes, floats).
        if b.is_ascii_digit() || (b == b'.' && i + 1 < n && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
            {
                i += 1;
            }
            push(&mut styles, start, i, syntax.number);
            continue;
        }
        // Identifiers: keywords, function calls, type-like names.
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &code[start..i];
            if keywords.contains(&word) {
                push(&mut styles, start, i, syntax.keyword);
            } else {
                // Allow a trailing macro bang (`println!`) before the call
                // paren so macro invocations read as function calls.
                let mut next = i;
                while next < n && (bytes[next] == b' ' || bytes[next] == b'\t') {
                    next += 1;
                }
                if next < n && bytes[next] == b'!' {
                    next += 1;
                    while next < n && (bytes[next] == b' ' || bytes[next] == b'\t') {
                        next += 1;
                    }
                }
                if next < n && bytes[next] == b'(' {
                    push(&mut styles, start, i, syntax.function);
                } else if word.starts_with(|c: char| c.is_uppercase()) {
                    push(&mut styles, start, i, syntax.typ);
                }
            }
            continue;
        }
        // Operators / punctuation runs.
        if matches!(
            b,
            b'=' | b'+'
                | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'<'
                | b'>'
                | b'!'
                | b'&'
                | b'|'
                | b'^'
                | b'~'
                | b'?'
                | b':'
                | b'@'
        ) {
            let start = i;
            while i < n
                && matches!(
                    bytes[i],
                    b'=' | b'+'
                        | b'-'
                        | b'*'
                        | b'/'
                        | b'%'
                        | b'<'
                        | b'>'
                        | b'!'
                        | b'&'
                        | b'|'
                        | b'^'
                        | b'~'
                        | b'?'
                        | b':'
                        | b'@'
                )
            {
                i += 1;
            }
            push(&mut styles, start, i, syntax.operator);
            continue;
        }
        i += 1;
    }
    styles
}

fn mermaid_element(source: &str, theme: &Theme) -> Option<AnyElement> {
    let image = cached_mermaid_image(source, theme)?;
    Some(
        div()
            .mb(px(18.0))
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(theme_color(theme.c.code_bg))
            .overflow_hidden()
            .child(
                div().w_full().flex().justify_center().child(
                    img(image)
                        .w_full()
                        .max_w(px(680.0))
                        .object_fit(ObjectFit::Contain),
                ),
            )
            .into_any_element(),
    )
}

fn cached_mermaid_image(source: &str, theme: &Theme) -> Option<Arc<gpui::Image>> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    theme.id.hash(&mut hasher);
    16.0_f32.to_bits().hash(&mut hasher);
    let key = hasher.finish();

    if let Some(image) = MERMAID_IMAGES.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(image);
    }

    let png = mermaid::render_png(source, theme, 16.0)?;
    let image = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png));
    MERMAID_IMAGES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_MERMAID_IMAGES {
            cache.clear();
        }
        cache.insert(key, image.clone());
    });
    Some(image)
}

fn inline_text(inlines: &[Inline], theme: &Theme) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for inline in inlines {
        append_inline(
            inline,
            &mut text,
            &mut highlights,
            HighlightStyle::default(),
            theme,
        );
    }
    StyledText::new(text).with_highlights(highlights)
}

fn append_inline(
    inline: &Inline,
    text: &mut String,
    highlights: &mut Vec<(std::ops::Range<usize>, HighlightStyle)>,
    style: HighlightStyle,
    theme: &Theme,
) {
    match inline {
        Inline::Text(value) | Inline::Math(value) => append_styled(value, text, highlights, style),
        Inline::InlineHtml(value) => {
            if let Some(inlines) = html::html_inlines(value) {
                append_children(&inlines, text, highlights, style, theme);
            }
        }
        Inline::Code(value) => append_styled(
            value,
            text,
            highlights,
            style.highlight(HighlightStyle {
                color: Some(theme_color(theme.c.code_fg)),
                background_color: Some(theme_color(theme.c.code_bg)),
                ..Default::default()
            }),
        ),
        Inline::SoftBreak | Inline::HardBreak => append_styled("\n", text, highlights, style),
        Inline::Strong(children) => append_children(
            children,
            text,
            highlights,
            style.highlight(HighlightStyle {
                font_weight: Some(FontWeight::SEMIBOLD),
                ..Default::default()
            }),
            theme,
        ),
        Inline::Emphasis(children) => append_children(
            children,
            text,
            highlights,
            style.highlight(HighlightStyle {
                font_style: Some(FontStyle::Italic),
                ..Default::default()
            }),
            theme,
        ),
        Inline::Strikethrough(children) => append_children(
            children,
            text,
            highlights,
            style.highlight(HighlightStyle {
                strikethrough: Some(StrikethroughStyle::default()),
                ..Default::default()
            }),
            theme,
        ),
        Inline::Link { children, .. } => append_children(
            children,
            text,
            highlights,
            style.highlight(HighlightStyle {
                color: Some(theme_color(theme.c.link)),
                underline: Some(UnderlineStyle::default()),
                ..Default::default()
            }),
            theme,
        ),
        Inline::Image { alt, .. } => append_styled(alt, text, highlights, style),
        Inline::FootnoteRef(label) => {
            append_styled(&format!("[^{label}]"), text, highlights, style)
        }
    }
}

fn append_children(
    children: &[Inline],
    text: &mut String,
    highlights: &mut Vec<(std::ops::Range<usize>, HighlightStyle)>,
    style: HighlightStyle,
    theme: &Theme,
) {
    for child in children {
        append_inline(child, text, highlights, style, theme);
    }
}

fn theme_color(color: crate::color::Color) -> gpui::Hsla {
    gpui::rgba(
        ((color.r() as u32) << 24)
            | ((color.g() as u32) << 16)
            | ((color.b() as u32) << 8)
            | color.a() as u32,
    )
    .into()
}

fn append_styled(
    value: &str,
    text: &mut String,
    highlights: &mut Vec<(std::ops::Range<usize>, HighlightStyle)>,
    style: HighlightStyle,
) {
    let start = text.len();
    text.push_str(value);
    if start != text.len() && style != HighlightStyle::default() {
        highlights.push((start..text.len(), style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    #[test]
    fn recognizes_remote_http_sources() {
        assert!(is_remote_url("https://picsum.photos/400/200"));
        assert!(is_remote_url("http://example.test/image.png"));
        assert!(!is_remote_url("images/local.png"));
    }

    #[test]
    #[ignore = "requires external network access"]
    fn fetches_and_identifies_redirecting_jpeg() {
        let bytes = fetch_remote_image("https://picsum.photos/400/200").unwrap();
        assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
        assert!(image_from_bytes(&bytes).is_ok());
    }

    #[test]
    #[ignore = "requires external network access and macOS curl"]
    fn curl_fallback_fetches_redirecting_jpeg() {
        let bytes = fetch_with_curl("https://picsum.photos/400/200").unwrap();
        assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
    }

    fn bijou_theme() -> Theme {
        crate::theme::builtin("bijou-light").expect("bijou-light theme exists")
    }

    fn styled_at(styles: &[(Range<usize>, HighlightStyle)], pos: usize) -> Option<HighlightStyle> {
        styles
            .iter()
            .find(|(range, _)| range.start <= pos && pos < range.end)
            .map(|(_, style)| *style)
    }

    #[test]
    fn highlights_comments_and_strings() {
        let theme = bijou_theme();
        let code = "// note\nlet s = \"hi\"; /* block */";
        let styles = code_highlight_styles(Some("rust"), code, &theme);
        let comment = theme_color(theme.syntax.comment);
        let string = theme_color(theme.syntax.string);
        assert_eq!(styled_at(&styles, 3).unwrap().color, Some(comment));
        assert_eq!(styled_at(&styles, 17).unwrap().color, Some(string));
        assert_eq!(styled_at(&styles, 27).unwrap().color, Some(comment));
        // Every range must respect UTF-8 boundaries of the source.
        for (range, _) in &styles {
            assert!(code.is_char_boundary(range.start));
            assert!(code.is_char_boundary(range.end));
        }
    }

    #[test]
    fn highlights_keywords_numbers_and_functions() {
        let theme = bijou_theme();
        let code = "fn main() { let x = 42; println!(\"v\"); }";
        let styles = code_highlight_styles(Some("rust"), code, &theme);
        let keyword = theme_color(theme.syntax.keyword);
        let function = theme_color(theme.syntax.function);
        let number = theme_color(theme.syntax.number);
        assert_eq!(styled_at(&styles, 0).unwrap().color, Some(keyword)); // fn
        assert_eq!(styled_at(&styles, 13).unwrap().color, Some(keyword)); // let
        assert_eq!(styled_at(&styles, 21).unwrap().color, Some(number)); // 42
        assert_eq!(styled_at(&styles, 27).unwrap().color, Some(function)); // println
    }

    #[test]
    fn highlights_uppercase_identifiers_as_types() {
        let theme = bijou_theme();
        let code = "let name: String = \"x\";";
        let styles = code_highlight_styles(Some("rust"), code, &theme);
        let typ = theme_color(theme.syntax.typ);
        assert_eq!(styled_at(&styles, 11).unwrap().color, Some(typ));
    }

    #[test]
    fn handles_cjk_inside_strings_without_boundary_breakage() {
        let theme = bijou_theme();
        let code = "println!(\"你好，世界\");";
        let styles = code_highlight_styles(Some("rust"), code, &theme);
        let string = theme_color(theme.syntax.string);
        assert_eq!(styled_at(&styles, 9).unwrap().color, Some(string));
        for (range, _) in &styles {
            assert!(code.is_char_boundary(range.start));
            assert!(code.is_char_boundary(range.end));
        }
    }

    #[test]
    fn handles_unterminated_string_without_panicking() {
        let theme = bijou_theme();
        let code = "let s = \"oops";
        let styles = code_highlight_styles(Some("rust"), code, &theme);
        let string = theme_color(theme.syntax.string);
        assert_eq!(styled_at(&styles, 8).unwrap().color, Some(string));
    }
}
