//! Table of contents: extract heading anchors from the parsed document IR.
//!
//! Headings are collected in document order, recursing through nested
//! containers (block quotes, lists, task lists, footnotes and HTML-derived
//! blocks) so the TOC panel mirrors exactly what the preview renders. Each
//! entry gets a stable, unique anchor via [`slugify`].

use crate::document::Block;
use crate::render::inline_text;
use std::collections::HashMap;

/// One TOC row: heading level (1..=6), flattened title text, and a stable
/// unique anchor (see [`slugify`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

/// Extract every heading from `blocks`, in document order, assigning each a
/// unique anchor (duplicate titles get `-2`, `-3`, … suffixes).
pub fn extract(blocks: &[Block]) -> Vec<TocEntry> {
    let mut out = Vec::new();
    let mut used = HashMap::new();
    walk(blocks, &mut out, &mut used);
    out
}

fn walk(blocks: &[Block], out: &mut Vec<TocEntry>, used: &mut HashMap<String, usize>) {
    for block in blocks {
        match block {
            Block::Heading { level, inlines, .. } => {
                let title = inline_text(inlines);
                let anchor = unique_anchor(&slugify(&title), used);
                out.push(TocEntry {
                    level: *level,
                    title,
                    anchor,
                });
            }
            Block::BlockQuote { blocks } => walk(blocks, out, used),
            Block::List { items, .. } => {
                for item in items {
                    walk(item, out, used);
                }
            }
            Block::TaskList { items, .. } => {
                for item in items {
                    walk(item, out, used);
                }
            }
            Block::CardGroup { cards, .. } => {
                for card in cards {
                    walk(&card.blocks, out, used);
                }
            }
            Block::Steps { items } => {
                for step in items {
                    walk(&step.blocks, out, used);
                }
            }
            Block::Footnote { blocks, .. } => walk(blocks, out, used),
            // HTML fragments are converted to IR the same way the preview
            // renders them, so traversal order stays identical.
            Block::Html(raw) => {
                if let Some(blocks) = crate::html::html_blocks(raw) {
                    walk(&blocks, out, used);
                }
            }
            _ => {}
        }
    }
}

/// Slugify a heading title into an anchor: lowercase, spaces → `-`, keep CJK
/// (and all other letters/digits), strip punctuation. Empty results fall back
/// to `"section"` so every heading still gets a usable anchor.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        if lower.is_alphanumeric() || matches!(lower, '-' | '_') {
            if matches!(lower, '-' | '_') {
                // Literal dashes/underscores act as separators too.
                if !out.is_empty() && !out.ends_with('-') {
                    out.push('-');
                }
            } else {
                out.push(lower);
            }
        } else if lower == ' ' && !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        // Punctuation is dropped entirely (GitHub-style).
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

/// Make `slug` unique across a document by appending `-2`, `-3`, … on repeat.
fn unique_anchor(slug: &str, used: &mut HashMap<String, usize>) -> String {
    let n = used.entry(slug.to_owned()).or_insert(0);
    *n += 1;
    if *n == 1 {
        slug.to_owned()
    } else {
        format!("{slug}-{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Align, Document, Inline};

    fn heading(level: u8, title: &str) -> Block {
        Block::Heading {
            level,
            inlines: vec![Inline::Text(title.to_string())],
            align: Align::None,
        }
    }

    #[test]
    fn slugify_lowercases_and_replaces_spaces() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("C++ Guide"), "c-guide");
        assert_eq!(slugify("A--B"), "a-b");
    }

    #[test]
    fn slugify_keeps_cjk() {
        assert_eq!(slugify("你好，世界！"), "你好世界");
        assert_eq!(slugify("安装指南"), "安装指南");
        assert_eq!(slugify(" 前后空格 "), "前后空格");
    }

    #[test]
    fn slugify_strips_punctuation_and_falls_back() {
        assert_eq!(slugify("Hello, (World)!"), "hello-world");
        assert_eq!(slugify("!!!"), "section"); // nothing left -> fallback
        assert_eq!(slugify(""), "section");
    }

    #[test]
    fn slugify_flattens_inline_style_markers_away() {
        assert_eq!(slugify("**Bold** & *Em*"), "bold-em");
    }

    #[test]
    fn extract_deduplicates_duplicate_titles() {
        let blocks = vec![
            heading(1, "Intro"),
            heading(2, "Intro"),
            heading(3, "Intro"),
        ];
        let toc = extract(&blocks);
        let anchors: Vec<&str> = toc.iter().map(|e| e.anchor.as_str()).collect();
        assert_eq!(anchors, vec!["intro", "intro-2", "intro-3"]);
    }

    #[test]
    fn extract_recurses_into_nested_blocks() {
        let blocks = vec![
            heading(1, "Top"),
            Block::BlockQuote {
                blocks: vec![heading(2, "Quoted")],
            },
            Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![heading(3, "Listed")]],
            },
            Block::TaskList {
                checked: vec![false],
                items: vec![vec![heading(4, "Tasked")]],
            },
            Block::Footnote {
                label: "n1".into(),
                blocks: vec![heading(5, "Footed")],
            },
            heading(6, "Bottom"),
        ];
        let toc = extract(&blocks);
        let titles: Vec<&str> = toc.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Top", "Quoted", "Listed", "Tasked", "Footed", "Bottom"]
        );
        let levels: Vec<u8> = toc.iter().map(|e| e.level).collect();
        assert_eq!(levels, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn extract_cjk_through_real_parse() {
        let doc = Document::new(
            "# 第一章 概述\n\n## 第一章 概述\n\n### 你好，世界！\n\n#### **加粗** 标题\n"
                .to_string(),
        );
        let toc = extract(&doc.blocks);
        assert_eq!(toc.len(), 4);
        assert_eq!(toc[0].title, "第一章 概述");
        assert_eq!(toc[0].anchor, "第一章-概述");
        assert_eq!(toc[1].anchor, "第一章-概述-2");
        assert_eq!(toc[2].anchor, "你好世界");
        // Inline styles are flattened away by inline_text.
        assert_eq!(toc[3].title, "加粗 标题");
        assert_eq!(toc[3].anchor, "加粗-标题");
    }

    #[test]
    fn extract_skips_non_heading_blocks() {
        let blocks = vec![
            Block::Paragraph {
                inlines: vec![Inline::Text("not a heading".into())],
                align: Align::None,
            },
            Block::ThematicBreak,
            Block::CodeBlock {
                lang: Some("rust".into()),
                text: "# also not a heading".into(),
            },
        ];
        assert!(extract(&blocks).is_empty());
    }
}
