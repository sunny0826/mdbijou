//! Whitelist-based safe rendering of HTML fragments (UI-MD-011).
//!
//! `Block::Html` and `Inline::InlineHtml` are parsed with html5ever into a DOM
//! and converted to the document IR using a strict whitelist. No scripts or
//! styles are executed; only the attributes `align/href/src/alt/width/height`
//! are honored, and `href`/`src` must be http(s) or a relative path. Unknown
//! tags are stripped but their text is preserved, so no information is lost.
//! On parse failure or empty input both entry points return `None` so the
//! caller can fall back to the inert text-card rendering.

use crate::document::{Align, Block, Card, Inline, Step};
use html5ever::tendril::TendrilSink;
use html5ever::{local_name, ns, parse_document, parse_fragment, ParseOpts, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Convert a block-level HTML fragment to IR blocks. `None` when the fragment
/// is empty/whitespace-only or carries no renderable content.
pub fn html_blocks(raw: &str) -> Option<Vec<Block>> {
    if raw.trim().is_empty() {
        return None;
    }
    let dom = parse_dom(raw);
    let body = find_body(&dom.document)?;
    let mut out = Vec::new();
    walk_blocks(&body, &mut out, Align::None);
    out.retain(block_has_content);
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Convert an inline HTML fragment to IR inlines. `None` when the fragment is
/// empty/whitespace-only or carries no renderable content.
pub fn html_inlines(raw: &str) -> Option<Vec<Inline>> {
    if raw.trim().is_empty() {
        return None;
    }
    let context = QualName::new(None, ns!(html), local_name!("div"));
    let dom = parse_fragment(RcDom::default(), opts(), context, Vec::new(), false)
        .from_utf8()
        .read_from(&mut raw.as_bytes())
        .unwrap_or_else(|_| RcDom::default());
    let mut out = Vec::new();
    walk_inlines(&dom.document, &mut out);
    trim_ws_inlines(&mut out);
    if out.is_empty() || !out.iter().any(inline_has_content) {
        None
    } else {
        Some(out)
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn opts() -> ParseOpts {
    let mut o = ParseOpts::default();
    o.tree_builder.scripting_enabled = false;
    o
}

/// Parse `raw` as an HTML document (always wrapped in html/head/body). The
/// html5ever tokenizer is error-tolerant, so failure is effectively limited to
/// I/O errors on the byte stream.
fn parse_dom(raw: &str) -> RcDom {
    parse_document(RcDom::default(), opts())
        .from_utf8()
        .read_from(&mut raw.as_bytes())
        .unwrap_or_else(|_| RcDom::default())
}

fn find_body(handle: &Handle) -> Option<Handle> {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            if name.local == local_name!("body") {
                return Some(child.clone());
            }
        }
        if let Some(b) = find_body(child) {
            return Some(b);
        }
    }
    None
}

fn tag_of(handle: &Handle) -> Option<&str> {
    match &handle.data {
        NodeData::Element { name, .. } => Some(name.local.as_ref()),
        _ => None,
    }
}

/// Whitelisted attribute lookup (attributes are the only styling channel we
/// honor; everything else, including `style`, is ignored).
fn attr(handle: &Handle, name: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &handle.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|a| a.name.local.as_ref() == name)
        .map(|a| a.value.to_string())
}

// ---------------------------------------------------------------------------
// Block conversion
// ---------------------------------------------------------------------------

/// Walk the children of `handle`, emitting block-level IR. Inline/phrasing
/// content accumulates into a pending paragraph flushed at block boundaries.
fn walk_blocks(handle: &Handle, out: &mut Vec<Block>, inherited_align: Align) {
    let mut pending: Vec<Inline> = Vec::new();
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => {
                let text = collapse_ws(&contents.borrow());
                if !text.is_empty() {
                    pending.push(Inline::Text(text));
                }
            }
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                match tag {
                    "p" | "div" | "center" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    | "blockquote" | "hr" | "img" | "section" | "article" | "main" | "header"
                    | "footer" | "aside" | "nav" | "figure" | "figcaption" | "details"
                    | "summary" | "ul" | "ol" | "pre" | "table" | "cardgroup" | "card"
                    | "steps" | "step" => {
                        flush_paragraph(&mut pending, out, inherited_align);
                        emit_block_element(child, out);
                    }
                    // Phrasing elements at block level: keep as inline content.
                    "a" | "strong" | "b" | "em" | "i" | "span" | "code" | "br" => {
                        push_inline_node(child, &mut pending);
                    }
                    // Never executed nor rendered.
                    "script" | "style" => {}
                    // Unknown block tag: strip tags, keep the text.
                    _ => {
                        flush_paragraph(&mut pending, out, inherited_align);
                        let text = collapse_ws(&element_text(child));
                        if !text.is_empty() {
                            out.push(Block::Paragraph {
                                inlines: vec![Inline::Text(text)],
                                align: inherited_align,
                            });
                        }
                    }
                }
            }
            // Comments, doctypes and processing instructions are invisible.
            _ => {}
        }
    }
    flush_paragraph(&mut pending, out, inherited_align);
}

/// Emit the IR for a single whitelisted block-level HTML or MDX element.
fn emit_block_element(handle: &Handle, out: &mut Vec<Block>) {
    let Some(tag) = tag_of(handle) else {
        return;
    };
    match tag {
        "p" | "div" | "center" | "section" | "article" | "main" | "header" | "footer" | "aside"
        | "nav" | "figure" | "figcaption" | "details" => {
            let my_align = align_attr(handle, tag);
            walk_blocks(handle, out, my_align);
        }
        "summary" => {
            let inlines = collect_inlines(handle);
            if inlines.iter().any(inline_has_content) {
                out.push(Block::Paragraph {
                    inlines: vec![Inline::Strong(inlines)],
                    align: Align::None,
                });
            }
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse().unwrap_or(1);
            let inlines = collect_inlines(handle);
            if inlines.iter().any(inline_has_content) {
                out.push(Block::Heading {
                    level,
                    inlines,
                    align: align_attr(handle, tag),
                });
            }
        }
        "blockquote" => {
            let mut inner = Vec::new();
            walk_blocks(handle, &mut inner, Align::None);
            inner.retain(block_has_content);
            if !inner.is_empty() {
                out.push(Block::BlockQuote { blocks: inner });
            }
        }
        "hr" => out.push(Block::ThematicBreak),
        "img" => emit_image(handle, out, Align::None),
        "ul" => emit_list(handle, out, false),
        "ol" => emit_list(handle, out, true),
        "pre" => emit_preformatted(handle, out),
        "table" => emit_table(handle, out),
        "cardgroup" => emit_card_group(handle, out),
        "card" => {
            if let Some(card) = parse_card(handle) {
                out.push(Block::CardGroup {
                    columns: 1,
                    cards: vec![card],
                });
            }
        }
        "steps" => emit_steps(handle, out),
        "step" => {
            if let Some(step) = parse_step(handle) {
                out.push(Block::Steps { items: vec![step] });
            }
        }
        _ => {}
    }
}

fn emit_list(handle: &Handle, out: &mut Vec<Block>, ordered: bool) {
    let items: Vec<Vec<Block>> = child_elements(handle, "li")
        .into_iter()
        .map(|item| parse_embedded_markdown(&item))
        .filter(|blocks| !blocks.is_empty())
        .collect();
    if !items.is_empty() {
        let start = if ordered {
            attr(handle, "start")
                .and_then(|value| value.parse().ok())
                .unwrap_or(1)
        } else {
            1
        };
        out.push(Block::List {
            ordered,
            start,
            items,
        });
    }
}

fn emit_preformatted(handle: &Handle, out: &mut Vec<Block>) {
    let text = element_text(handle).trim_matches('\n').to_string();
    if text.is_empty() {
        return;
    }
    let lang = child_elements(handle, "code")
        .first()
        .and_then(|code| attr(code, "class"))
        .and_then(|class| {
            class
                .split_whitespace()
                .find_map(|name| name.strip_prefix("language-").map(str::to_string))
        });
    out.push(Block::CodeBlock { lang, text });
}

fn emit_table(handle: &Handle, out: &mut Vec<Block>) {
    let mut row_handles = Vec::new();
    collect_descendant_elements(handle, "tr", &mut row_handles);
    let mut header = Vec::new();
    let mut rows = Vec::new();

    for row in row_handles {
        let children = row.children.borrow();
        let has_header_cells = children.iter().any(|cell| tag_of(cell) == Some("th"));
        let cells: Vec<Vec<Inline>> = children
            .iter()
            .filter(|cell| matches!(tag_of(cell), Some("th" | "td")))
            .map(collect_inlines)
            .collect();
        if cells.is_empty() {
            continue;
        }
        if header.is_empty() && has_header_cells {
            header = cells;
        } else {
            rows.push(cells);
        }
    }

    if header.is_empty() && !rows.is_empty() {
        header = rows.remove(0);
    }
    if !header.is_empty() {
        let align = vec![Align::None; header.len()];
        out.push(Block::Table {
            header,
            align,
            rows,
        });
    }
}

fn emit_card_group(handle: &Handle, out: &mut Vec<Block>) {
    let cards: Vec<Card> = child_elements(handle, "card")
        .into_iter()
        .filter_map(|card| parse_card(&card))
        .collect();
    if cards.is_empty() {
        return;
    }
    let columns = attr(handle, "cols")
        .as_deref()
        .and_then(parse_jsx_usize)
        .unwrap_or(2)
        .clamp(1, 4);
    out.push(Block::CardGroup { columns, cards });
}

fn parse_card(handle: &Handle) -> Option<Card> {
    let title = attr(handle, "title")?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let icon = attr(handle, "icon").filter(|icon| !icon.trim().is_empty());
    let href = attr(handle, "href").filter(|href| is_allowed_url(href));
    Some(Card {
        title,
        icon,
        href,
        blocks: parse_embedded_markdown(handle),
    })
}

fn emit_steps(handle: &Handle, out: &mut Vec<Block>) {
    let items: Vec<Step> = child_elements(handle, "step")
        .into_iter()
        .filter_map(|step| parse_step(&step))
        .collect();
    if !items.is_empty() {
        out.push(Block::Steps { items });
    }
}

fn parse_step(handle: &Handle) -> Option<Step> {
    let title = attr(handle, "title")?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some(Step {
        title,
        blocks: parse_embedded_markdown(handle),
    })
}

fn parse_embedded_markdown(handle: &Handle) -> Vec<Block> {
    let source = dedent(&element_text(handle));
    crate::document::parse_fragment(&source)
}

fn child_elements(handle: &Handle, tag: &str) -> Vec<Handle> {
    handle
        .children
        .borrow()
        .iter()
        .filter(|child| tag_of(child) == Some(tag))
        .cloned()
        .collect()
}

fn collect_descendant_elements(handle: &Handle, tag: &str, out: &mut Vec<Handle>) {
    for child in handle.children.borrow().iter() {
        if tag_of(child) == Some(tag) {
            out.push(child.clone());
        } else {
            collect_descendant_elements(child, tag, out);
        }
    }
}

fn parse_jsx_usize(value: &str) -> Option<usize> {
    value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim()
        .parse()
        .ok()
}

/// Alignment of a block element: its `align` attribute, with `<center>`
/// defaulting to center alignment.
fn align_attr(handle: &Handle, tag: &str) -> Align {
    match parse_align(attr(handle, "align").as_deref()) {
        Some(a) => a,
        None if tag == "center" => Align::Center,
        None => Align::None,
    }
}

fn emit_image(handle: &Handle, out: &mut Vec<Block>, align: Align) {
    let alt = attr(handle, "alt").unwrap_or_default();
    match attr(handle, "src").filter(|s| is_allowed_url(s)) {
        Some(src) => {
            let width = attr(handle, "width").and_then(|w| w.parse::<f32>().ok());
            out.push(Block::Paragraph {
                inlines: vec![Inline::Image { src, alt, width }],
                align,
            });
        }
        None => {
            // No usable source: keep the alt text so nothing is lost.
            let alt = alt.trim().to_string();
            if !alt.is_empty() {
                out.push(Block::Paragraph {
                    inlines: vec![Inline::Text(alt)],
                    align,
                });
            }
        }
    }
}

/// Flush pending inline content as a paragraph, trimming boundary whitespace.
fn flush_paragraph(pending: &mut Vec<Inline>, out: &mut Vec<Block>, align: Align) {
    if pending.is_empty() {
        return;
    }
    trim_ws_inlines(pending);
    while matches!(pending.last(), Some(Inline::HardBreak)) {
        pending.pop();
    }
    if !pending.is_empty() && pending.iter().any(inline_has_content) {
        out.push(Block::Paragraph {
            inlines: std::mem::take(pending),
            align,
        });
    } else {
        pending.clear();
    }
}

fn block_has_content(b: &Block) -> bool {
    match b {
        Block::Paragraph { inlines, .. } => inlines.iter().any(inline_has_content),
        Block::Heading { inlines, .. } => inlines.iter().any(inline_has_content),
        Block::BlockQuote { blocks } => blocks.iter().any(block_has_content),
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Inline conversion
// ---------------------------------------------------------------------------

/// Walk the children of `handle`, pushing inline IR (text, styles, links).
fn walk_inlines(handle: &Handle, out: &mut Vec<Inline>) {
    for child in handle.children.borrow().iter() {
        push_inline_node(child, out);
    }
}

fn push_inline_node(child: &Handle, out: &mut Vec<Inline>) {
    match &child.data {
        NodeData::Text { contents } => {
            let text = collapse_ws(&contents.borrow());
            if !text.is_empty() {
                out.push(Inline::Text(text));
            }
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            match tag {
                "a" => {
                    let children = collect_inlines(child);
                    match attr(child, "href").filter(|d| is_allowed_url(d)) {
                        Some(dest) => out.push(Inline::Link { dest, children }),
                        // Anchor without a usable href: transparent wrapper.
                        None => out.extend(children),
                    }
                }
                "strong" | "b" => {
                    let children = collect_inlines(child);
                    if children.iter().any(inline_has_content) {
                        out.push(Inline::Strong(children));
                    }
                }
                "em" | "i" => {
                    let children = collect_inlines(child);
                    if children.iter().any(inline_has_content) {
                        out.push(Inline::Emphasis(children));
                    }
                }
                "code" | "kbd" | "samp" => {
                    let text = collapse_ws(&element_text(child));
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        out.push(Inline::Code(text));
                    }
                }
                "br" => out.push(Inline::HardBreak),
                // Transparent containers: pass children through unchanged.
                "span" | "html" | "body" | "small" | "mark" | "u" | "sub" | "sup" | "var" => {
                    walk_inlines(child, out)
                }
                "del" | "s" => {
                    let children = collect_inlines(child);
                    if children.iter().any(inline_has_content) {
                        out.push(Inline::Strikethrough(children));
                    }
                }
                "img" => {
                    let alt = attr(child, "alt").unwrap_or_default();
                    match attr(child, "src").filter(|s| is_allowed_url(s)) {
                        Some(src) => {
                            let width = attr(child, "width").and_then(|w| w.parse::<f32>().ok());
                            out.push(Inline::Image { src, alt, width });
                        }
                        None => {
                            let alt = alt.trim().to_string();
                            if !alt.is_empty() {
                                out.push(Inline::Text(alt));
                            }
                        }
                    }
                }
                // Never executed nor rendered.
                "script" | "style" => {}
                // Unknown inline tag: strip tags, keep the text.
                _ => {
                    let text = collapse_ws(&element_text(child));
                    if !text.is_empty() {
                        out.push(Inline::Text(text));
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_inlines(handle: &Handle) -> Vec<Inline> {
    let mut out = Vec::new();
    walk_inlines(handle, &mut out);
    out
}

/// Concatenated raw text of all descendant text nodes (tags stripped).
fn element_text(handle: &Handle) -> String {
    let mut out = String::new();
    collect_text(handle, &mut out);
    out
}

fn collect_text(handle: &Handle, out: &mut String) {
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { .. } => collect_text(child, out),
            _ => {}
        }
    }
}

fn dedent(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let first = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(first, |index| index + 1);
    let lines = &lines[first..last];
    let indentation = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|line| {
            let mut bytes = 0usize;
            for character in line.chars().take(indentation) {
                if matches!(character, ' ' | '\t') {
                    bytes += character.len_utf8();
                } else {
                    break;
                }
            }
            &line[bytes..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Trim leading/trailing whitespace-only text nodes (boundary whitespace).
fn trim_ws_inlines(inlines: &mut Vec<Inline>) {
    while let Some(Inline::Text(t)) = inlines.first() {
        if t.trim().is_empty() {
            inlines.remove(0);
        } else {
            break;
        }
    }
    while let Some(Inline::Text(t)) = inlines.last() {
        if t.trim().is_empty() {
            inlines.pop();
        } else {
            break;
        }
    }
}

fn inline_has_content(i: &Inline) -> bool {
    match i {
        Inline::Text(s) => !s.trim().is_empty(),
        Inline::Code(s) => !s.trim().is_empty(),
        Inline::Image { .. } => true,
        Inline::Link { children, .. }
        | Inline::Strong(children)
        | Inline::Emphasis(children)
        | Inline::Strikethrough(children) => children.iter().any(inline_has_content),
        _ => false,
    }
}

/// Collapse any run of whitespace (including newlines and `&nbsp;`) to a
/// single space, trimming at the ends. A whitespace-only node still yields a
/// single space so gaps between inline elements survive.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws_pending = false;
    let mut any = false;
    for c in s.chars() {
        if c.is_whitespace() {
            ws_pending = true;
        } else {
            if ws_pending && !out.is_empty() {
                out.push(' ');
            }
            ws_pending = false;
            out.push(c);
            any = true;
        }
    }
    if !any {
        " ".to_string()
    } else {
        out
    }
}

fn parse_align(s: Option<&str>) -> Option<Align> {
    match s?.trim().to_ascii_lowercase().as_str() {
        "center" => Some(Align::Center),
        "left" => Some(Align::Left),
        "right" => Some(Align::Right),
        _ => None,
    }
}

/// A `href`/`src` is only honored when it is http(s) or a relative path.
/// Anything else (javascript:, data:, mailto:, protocol-relative, …) is
/// rejected so no active content can sneak in.
fn is_allowed_url(u: &str) -> bool {
    let u = u.trim();
    if u.is_empty() {
        return false;
    }
    if u.starts_with("http://") || u.starts_with("https://") {
        return true;
    }
    if u.starts_with("//") {
        return false;
    }
    if let Some((scheme, _)) = u.split_once(':') {
        let is_scheme = !scheme.is_empty()
            && scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if is_scheme {
            return false;
        }
    }
    true // relative path
}

// ---------------------------------------------------------------------------
// Tests: whitelist tag -> Block/Inline conversion snapshots
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(raw: &str) -> Vec<Block> {
        html_blocks(raw).unwrap_or_default()
    }

    fn inlines(raw: &str) -> Vec<Inline> {
        html_inlines(raw).unwrap_or_default()
    }

    fn plain(inlines: &[Inline]) -> String {
        inlines.iter().map(inline_plain).collect()
    }

    fn inline_plain(i: &Inline) -> String {
        match i {
            Inline::Text(s) => s.clone(),
            Inline::Code(s) => s.clone(),
            Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => plain(v),
            Inline::Link { children, .. } => plain(children),
            Inline::Image { alt, .. } => alt.clone(),
            Inline::SoftBreak | Inline::HardBreak => " ".to_string(),
            Inline::InlineHtml(s) | Inline::Math(s) => s.clone(),
            Inline::FootnoteRef(s) => format!("[^{s}]"),
        }
    }

    fn block_text(blocks: &[Block]) -> String {
        let mut out = String::new();
        for b in blocks {
            match b {
                Block::Paragraph { inlines, .. } | Block::Heading { inlines, .. } => {
                    out.push_str(&plain(inlines));
                }
                Block::BlockQuote { blocks } => out.push_str(&block_text(blocks)),
                Block::CardGroup { cards, .. } => {
                    for card in cards {
                        out.push_str(&card.title);
                        out.push_str(&block_text(&card.blocks));
                    }
                }
                Block::Steps { items } => {
                    for step in items {
                        out.push_str(&step.title);
                        out.push_str(&block_text(&step.blocks));
                    }
                }
                _ => {}
            }
        }
        out
    }

    // README.md header: the motivating real-world fragment.
    #[test]
    fn readme_centered_image_paragraph() {
        let raw = concat!(
            "<p align=\"center\">\n",
            "   <a><img src=\"assets/mdbijou-icon-1024.png\" width=\"120\" alt=\"mdbijou logo\"></a>\n",
            "</p>\n"
        );
        let bs = blocks(raw);
        assert_eq!(bs.len(), 1);
        let Block::Paragraph { inlines, align } = &bs[0] else {
            panic!("expected centered paragraph")
        };
        assert_eq!(*align, Align::Center);
        assert!(matches!(
            inlines.as_slice(),
            [Inline::Image { src, alt, width }]
                if src == "assets/mdbijou-icon-1024.png"
                    && alt == "mdbijou logo"
                    && *width == Some(120.0)
        ));
    }

    #[test]
    fn readme_centered_heading() {
        let bs = blocks(r#"<h1 align="center">MDbijou</h1>"#);
        let Block::Heading {
            level,
            inlines,
            align,
        } = &bs[0]
        else {
            panic!("expected heading")
        };
        assert_eq!(*level, 1);
        assert_eq!(*align, Align::Center);
        assert_eq!(plain(inlines), "MDbijou");
    }

    #[test]
    fn readme_centered_strong_paragraph() {
        let raw = r#"<p align="center"><strong>A simple Markdown reader and editor.</strong></p>"#;
        let bs = blocks(raw);
        let Block::Paragraph { inlines, align } = &bs[0] else {
            panic!("expected paragraph")
        };
        assert_eq!(*align, Align::Center);
        assert!(matches!(inlines.as_slice(), [Inline::Strong(_)]));
        assert_eq!(plain(inlines), "A simple Markdown reader and editor.");
    }

    #[test]
    fn inline_styles_and_links() {
        let ins = inlines(
            r#"<a href="https://example.com">link</a> <b>bold</b> <i>em</i> <code>c</code>"#,
        );
        assert!(ins
            .iter()
            .any(|i| matches!(i, Inline::Link { dest, .. } if dest == "https://example.com")));
        assert!(ins.iter().any(|i| matches!(i, Inline::Strong(_))));
        assert!(ins.iter().any(|i| matches!(i, Inline::Emphasis(_))));
        assert!(ins.iter().any(|i| matches!(i, Inline::Code(c) if c == "c")));
        // Inter-element spaces are preserved.
        assert_eq!(plain(&ins), "link bold em c");
    }

    #[test]
    fn inline_html_span_strips_style_and_keeps_text() {
        let ins = inlines(r#"<span style="color: red">red text</span>"#);
        assert_eq!(plain(&ins), "red text");
        assert!(ins.iter().all(|i| !matches!(i, Inline::InlineHtml(_))));
    }

    #[test]
    fn hard_break_converts() {
        let ins = inlines(r#"a<br>b"#);
        assert!(ins.iter().any(|i| matches!(i, Inline::HardBreak)));
        assert_eq!(plain(&ins), "a b");
    }

    #[test]
    fn hr_becomes_thematic_break() {
        let bs = blocks(r#"<div>before</div><hr><p>after</p>"#);
        assert!(bs.iter().any(|b| matches!(b, Block::ThematicBreak)));
    }

    #[test]
    fn blockquote_nests_paragraph() {
        let bs = blocks(r#"<blockquote><p>quoted text</p></blockquote>"#);
        let Block::BlockQuote { blocks } = &bs[0] else {
            panic!("expected blockquote")
        };
        assert!(matches!(&blocks[0], Block::Paragraph { .. }));
        assert_eq!(block_text(blocks), "quoted text");
    }

    #[test]
    fn unknown_tags_strip_but_keep_text() {
        let bs = blocks(r#"<div><blink>blinking text</blink><u>under</u></div>"#);
        assert_eq!(block_text(&bs), "blinking textunder");
    }

    #[test]
    fn script_and_style_are_dropped() {
        let bs = blocks(r#"<script>alert("x")</script><style>p { color: red }</style><p>ok</p>"#);
        assert_eq!(block_text(&bs), "ok");
    }

    #[test]
    fn dangerous_urls_are_ignored() {
        let ins = inlines(r#"<a href="javascript:alert(1)">x</a>"#);
        assert_eq!(plain(&ins), "x");
        assert!(ins.iter().all(|i| !matches!(i, Inline::Link { .. })));
        let ins = inlines(r#"<a href="data:text/html,hi">d</a>"#);
        assert!(ins.iter().all(|i| !matches!(i, Inline::Link { .. })));
        let ins = inlines(r#"<a href="//evil.com">e</a>"#);
        assert!(ins.iter().all(|i| !matches!(i, Inline::Link { .. })));
        // Relative and http(s) links are allowed.
        let ins = inlines(r#"<a href="LICENSE">rel</a>"#);
        assert!(ins
            .iter()
            .any(|i| matches!(i, Inline::Link { dest, .. } if dest == "LICENSE")));
    }

    #[test]
    fn img_without_src_falls_back_to_alt_text() {
        let ins = inlines(r#"<img alt="logo" width="100">"#);
        assert!(ins
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t == "logo")));
    }

    #[test]
    fn empty_and_whitespace_input_returns_none() {
        assert!(html_blocks("").is_none());
        assert!(html_blocks("   \n  ").is_none());
        assert!(html_inlines("").is_none());
        assert!(html_inlines("<!-- only a comment -->").is_none());
    }

    #[test]
    fn missing_image_width_is_none() {
        let ins = inlines(r#"<img src="a.png" alt="a">"#);
        assert!(matches!(
            ins.as_slice(),
            [Inline::Image { width: None, .. }]
        ));
    }

    #[test]
    fn heading_levels_map_1_to_6() {
        for lvl in 1..=6 {
            let raw = format!("<h{lvl}>title</h{lvl}>");
            let bs = blocks(&raw);
            let Block::Heading { level, inlines, .. } = &bs[0] else {
                panic!("expected h{lvl}")
            };
            assert_eq!(*level, lvl);
            assert_eq!(plain(inlines), "title");
        }
    }

    #[test]
    fn html_entities_are_decoded() {
        let ins = inlines(r#"a &amp; b &lt;c&gt; &nbsp; d"#);
        assert_eq!(plain(&ins), "a & b <c> d");
    }

    #[test]
    fn mdx_card_group_preserves_attributes_and_markdown_body() {
        let bs = blocks(
            r#"<CardGroup cols={2}>
  <Card title="Essentials" icon="book-open" href="/essentials">
    Start with **your first memory**.
  </Card>
</CardGroup>"#,
        );
        let Block::CardGroup { columns, cards } = &bs[0] else {
            panic!("expected card group")
        };
        assert_eq!(*columns, 2);
        assert_eq!(cards[0].title, "Essentials");
        assert_eq!(cards[0].icon.as_deref(), Some("book-open"));
        assert_eq!(cards[0].href.as_deref(), Some("/essentials"));
        assert!(matches!(
            &cards[0].blocks[0],
            Block::Paragraph { inlines, .. }
                if inlines.iter().any(|inline| matches!(inline, Inline::Strong(_)))
        ));
    }

    #[test]
    fn mdx_steps_preserve_titles_and_fenced_code() {
        let bs = blocks(
            r#"<Steps>
  <Step title="Ask Mem directly">
    Ask this:

    ```text
    What did we decide?
    ```
  </Step>
</Steps>"#,
        );
        let Block::Steps { items } = &bs[0] else {
            panic!("expected steps")
        };
        assert_eq!(items[0].title, "Ask Mem directly");
        assert!(items[0].blocks.iter().any(
            |block| matches!(block, Block::CodeBlock { text, .. } if text == "What did we decide?")
        ));
    }

    #[test]
    fn semantic_html_lists_and_preformatted_code_convert() {
        let bs = blocks(
            r#"<section><ul><li>one</li><li><strong>two</strong></li></ul><pre><code class="language-rust">fn main() {}</code></pre></section>"#,
        );
        assert!(matches!(
            &bs[0],
            Block::List { ordered: false, items, .. } if items.len() == 2
        ));
        assert!(matches!(
            &bs[1],
            Block::CodeBlock { lang: Some(lang), text } if lang == "rust" && text == "fn main() {}"
        ));
    }

    #[test]
    fn semantic_html_table_converts_to_native_table() {
        let bs = blocks(
            r#"<table><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody><tr><td>alpha</td><td><strong>one</strong></td></tr></tbody></table>"#,
        );
        let Block::Table { header, rows, .. } = &bs[0] else {
            panic!("expected table")
        };
        assert_eq!(plain(&header[0]), "Name");
        assert_eq!(plain(&header[1]), "Value");
        assert_eq!(plain(&rows[0][0]), "alpha");
        assert!(matches!(rows[0][1].as_slice(), [Inline::Strong(_)]));
    }

    #[test]
    fn readme_header_end_to_end_through_markdown_parse() {
        let src = concat!(
            "<p align=\"center\">\n",
            "   <a><img src=\"assets/mdbijou-icon-1024.png\" width=\"120\" alt=\"mdbijou logo\"></a>\n",
            "</p>\n\n",
            "<h1 align=\"center\">MDbijou</h1>\n",
            "<p align=\"center\"><strong>A simple Markdown reader and editor.</strong></p>\n",
        );
        let doc = crate::document::Document::new(src.to_string());
        let html: Vec<&str> = doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Html(raw) => Some(raw.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            html.len(),
            2,
            "README header yields two HTML blocks: {html:?}"
        );
        let converted: Vec<Vec<Block>> = html
            .iter()
            .map(|raw| html_blocks(raw).expect("converts"))
            .collect();
        assert!(matches!(
            &converted[0][0],
            Block::Paragraph { align: Align::Center, inlines }
                if matches!(inlines.as_slice(), [Inline::Image { width: Some(120.0), .. }])
        ));
        // `<h1>` and the following `<p>` land in one HTML block and must split.
        assert_eq!(converted[1].len(), 2);
        assert!(matches!(
            &converted[1][0],
            Block::Heading {
                level: 1,
                align: Align::Center,
                ..
            }
        ));
        assert!(matches!(
            &converted[1][1],
            Block::Paragraph { align: Align::Center, inlines }
                if matches!(inlines.as_slice(), [Inline::Strong(_)])
        ));
    }
}
