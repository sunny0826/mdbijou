//! Document model: pulldown-cmark events -> intermediate representation (IR).
//! The IR is consumed by both the render (preview) and edit views.
//!
//! The builder keeps a stack of context frames. Every `Event::Start(Tag)`
//! pushes exactly one frame and every `Event::End(TagEnd)` pops the matching
//! one and completes that context (UI-MD-001), so nested lists, task items,
//! tables and quotes are no longer flattened or mis-attached.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
// Names intentionally mirror pulldown-cmark's `CodeBlock`/`BlockQuote` tags.
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    CodeBlock {
        lang: Option<String>,
        text: String,
    },
    BlockQuote {
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Block>>,
    },
    TaskList {
        checked: Vec<bool>,
        items: Vec<Vec<Block>>,
    },
    Table {
        header: Vec<Vec<Inline>>,
        align: Vec<Align>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    ThematicBreak,
    /// Block-level HTML, kept verbatim but never executed (UI-MD-011).
    Html(String),
    /// Footnote definition degraded to a visible reference block (UI-MD-011).
    Footnote {
        label: String,
        blocks: Vec<Block>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
// `InlineHtml` intentionally mirrors the source tag name.
#[allow(clippy::enum_variant_names)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        dest: String,
        children: Vec<Inline>,
    },
    Image {
        src: String,
        alt: String,
    },
    SoftBreak,
    HardBreak,
    /// Inline HTML kept verbatim but rendered as inert text (UI-MD-011).
    InlineHtml(String),
    /// Footnote reference degraded to a visible `[n]` marker (UI-MD-011).
    FootnoteRef(String),
    /// Inline/display math (not enabled by default) rendered as inert text.
    Math(String),
}

/// A parsed Markdown document. `text` is the raw source (used by the editor).
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub text: String,
    pub blocks: Vec<Block>,
    pub dirty: bool,
    #[allow(dead_code)] // reserved for future diagnostics; parse currently never fails
    pub parse_error: Option<String>,
}

impl Document {
    pub fn new(text: String) -> Self {
        let blocks = parse(&text);
        Document {
            path: None,
            text,
            blocks,
            dirty: false,
            parse_error: None,
        }
    }

    pub fn with_path(path: PathBuf, text: String) -> Self {
        let mut d = Self::new(text);
        d.path = Some(path);
        d
    }

    /// Re-parse from the current in-memory `text`. Used when switching to preview.
    pub fn reparse(&mut self) {
        self.blocks = parse(&self.text);
    }
}

fn mk_options() -> Options {
    let mut o = Options::empty();
    o.insert(Options::ENABLE_TABLES);
    o.insert(Options::ENABLE_TASKLISTS);
    o.insert(Options::ENABLE_STRIKETHROUGH);
    o.insert(Options::ENABLE_FOOTNOTES);
    o
}

// ---------------------------------------------------------------------------
// Intermediate builder state
// ---------------------------------------------------------------------------

enum Ctx {
    Root,
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    CodeBlock {
        lang: Option<String>,
        buf: String,
    },
    BlockQuote {
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<(Option<bool>, Vec<Block>)>,
    },
    Item {
        blocks: Vec<Block>,
        task: Option<bool>,
        cur_para: Option<Vec<Inline>>,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
        cur_row: Vec<Vec<Inline>>,
        in_head: bool,
    },
    Cell {
        inlines: Vec<Inline>,
    },
    HtmlBlock {
        buf: String,
    },
    Footnote {
        label: String,
        blocks: Vec<Block>,
        cur_para: Option<Vec<Inline>>,
    },
    // inline frames
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link {
        dest: String,
        children: Vec<Inline>,
    },
    Image {
        src: String,
        alt: String,
    },
}

fn parse(text: &str) -> Vec<Block> {
    let parser = Parser::new_ext(text, mk_options());
    let mut stack: Vec<Ctx> = vec![Ctx::Root];
    let mut out: Vec<Block> = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(tag) => start_tag(&mut stack, tag),
            Event::End(end) => end_tag(&mut stack, &mut out, end),
            Event::Text(t) => {
                // Code-block content goes straight into the code buffer; all
                // other text becomes an inline node.
                match stack.last_mut() {
                    Some(Ctx::CodeBlock { buf, .. }) => buf.push_str(&t),
                    _ => sink_inline(&mut stack, &mut out, Inline::Text(t.to_string())),
                }
            }
            Event::Code(t) => sink_inline(&mut stack, &mut out, Inline::Code(t.to_string())),
            Event::SoftBreak => sink_inline(&mut stack, &mut out, Inline::SoftBreak),
            Event::HardBreak => sink_inline(&mut stack, &mut out, Inline::HardBreak),
            Event::Rule => emit_block(&mut stack, &mut out, Block::ThematicBreak),
            Event::TaskListMarker(checked) => {
                if let Some(Ctx::Item { task, .. }) = stack.last_mut() {
                    *task = Some(checked);
                }
            }
            Event::InlineHtml(t) => {
                sink_inline(&mut stack, &mut out, Inline::InlineHtml(t.to_string()))
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                sink_inline(&mut stack, &mut out, Inline::Math(t.to_string()))
            }
            Event::FootnoteReference(name) => {
                sink_inline(&mut stack, &mut out, Inline::FootnoteRef(name.to_string()))
            }
            Event::Html(t) => {
                // Block-level HTML. If we're inside an open HtmlBlock frame,
                // accumulate into it; otherwise emit it directly.
                match stack.last_mut() {
                    Some(Ctx::HtmlBlock { buf, .. }) => buf.push_str(&t),
                    _ => emit_block(&mut stack, &mut out, Block::Html(t.to_string())),
                }
            }
        }
    }
    out
}

fn start_tag(stack: &mut Vec<Ctx>, tag: Tag<'_>) {
    match tag {
        Tag::Heading { level, .. } => {
            stack.push(Ctx::Heading {
                level: level_num(level),
                inlines: Vec::new(),
            });
        }
        Tag::Paragraph => stack.push(Ctx::Paragraph {
            inlines: Vec::new(),
        }),
        Tag::CodeBlock(kind) => {
            // A block-level construct closes any tight-item implicit paragraph.
            flush_item_para(stack);
            let lang = match kind {
                CodeBlockKind::Fenced(l) => {
                    let s = l.to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                }
                CodeBlockKind::Indented => None,
            };
            stack.push(Ctx::CodeBlock {
                lang,
                buf: String::new(),
            });
        }
        Tag::BlockQuote(_) => {
            flush_item_para(stack);
            stack.push(Ctx::BlockQuote { blocks: Vec::new() });
        }
        Tag::List(num) => {
            flush_item_para(stack);
            stack.push(Ctx::List {
                ordered: num.is_some(),
                start: num.unwrap_or(1),
                items: Vec::new(),
            });
        }
        Tag::Item => {
            flush_item_para(stack);
            stack.push(Ctx::Item {
                blocks: Vec::new(),
                task: None,
                cur_para: None,
            });
        }
        Tag::Table(align) => {
            flush_item_para(stack);
            let align = align
                .iter()
                .map(|a| match a {
                    Alignment::None => Align::None,
                    Alignment::Left => Align::Left,
                    Alignment::Center => Align::Center,
                    Alignment::Right => Align::Right,
                })
                .collect();
            stack.push(Ctx::Table {
                align,
                header: Vec::new(),
                rows: Vec::new(),
                cur_row: Vec::new(),
                in_head: true,
            });
        }
        Tag::TableHead => {
            // Marks the phase; body rows follow after it closes.
            if let Some(Ctx::Table {
                in_head, cur_row, ..
            }) = stack.last_mut()
            {
                *in_head = true;
                cur_row.clear();
            }
        }
        Tag::TableRow => {
            if let Some(Ctx::Table { cur_row, .. }) = stack.last_mut() {
                cur_row.clear();
            }
        }
        Tag::TableCell => stack.push(Ctx::Cell {
            inlines: Vec::new(),
        }),
        Tag::FootnoteDefinition(label) => {
            flush_item_para(stack);
            stack.push(Ctx::Footnote {
                label: label.to_string(),
                blocks: Vec::new(),
                cur_para: None,
            });
        }
        Tag::Emphasis => stack.push(Ctx::Emphasis(Vec::new())),
        Tag::Strong => stack.push(Ctx::Strong(Vec::new())),
        Tag::Strikethrough => stack.push(Ctx::Strikethrough(Vec::new())),
        Tag::Link { dest_url, .. } => stack.push(Ctx::Link {
            dest: dest_url.to_string(),
            children: Vec::new(),
        }),
        Tag::Image { dest_url, .. } => stack.push(Ctx::Image {
            src: dest_url.to_string(),
            alt: String::new(),
        }),
        Tag::HtmlBlock => stack.push(Ctx::HtmlBlock { buf: String::new() }),
        // Unsupported span/block tags degrade safely (no stack frame pushed, so
        // the corresponding End is a no-op too — structure is not corrupted).
        _ => {}
    }
}

fn end_tag(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, end: TagEnd) {
    match end {
        TagEnd::Heading(_) => pop_into(stack, out, |c| match c {
            Ctx::Heading { level, inlines } => Some(Block::Heading { level, inlines }),
            _ => None,
        }),
        TagEnd::Paragraph => pop_into(stack, out, |c| match c {
            Ctx::Paragraph { inlines } => Some(Block::Paragraph { inlines }),
            _ => None,
        }),
        TagEnd::CodeBlock => pop_into(stack, out, |c| match c {
            Ctx::CodeBlock { lang, buf } => Some(Block::CodeBlock {
                lang,
                text: buf.trim_end().to_string(),
            }),
            _ => None,
        }),
        TagEnd::BlockQuote(_) => pop_into(stack, out, |c| match c {
            Ctx::BlockQuote { blocks } => Some(Block::BlockQuote { blocks }),
            _ => None,
        }),
        TagEnd::List(_) => {
            if let Some(Ctx::List {
                ordered,
                start,
                items,
            }) = stack.pop()
            {
                // A trailing item may still hold content if something went wrong.
                let is_task = items.iter().any(|(t, _)| t.is_some());
                if is_task {
                    let checked: Vec<bool> =
                        items.iter().map(|(t, _)| t.unwrap_or(false)).collect();
                    let blocks: Vec<Vec<Block>> = items.into_iter().map(|(_, b)| b).collect();
                    emit_block(
                        stack,
                        out,
                        Block::TaskList {
                            checked,
                            items: blocks,
                        },
                    );
                } else {
                    let items: Vec<Vec<Block>> = items.into_iter().map(|(_, b)| b).collect();
                    emit_block(
                        stack,
                        out,
                        Block::List {
                            ordered,
                            start,
                            items,
                        },
                    );
                }
                let _ = (ordered, start);
            }
        }
        TagEnd::Item => {
            if let Some(Ctx::Item {
                mut blocks,
                task,
                cur_para,
            }) = stack.pop()
            {
                if let Some(p) = cur_para {
                    if !p.is_empty() {
                        blocks.push(Block::Paragraph { inlines: p });
                    }
                }
                // Attach to the enclosing list.
                match stack.last_mut() {
                    Some(Ctx::List { items, .. }) => items.push((task, blocks)),
                    _ => {
                        // Safety fallback: if not inside a list, emit loose items.
                        for b in blocks {
                            emit_block(stack, out, b);
                        }
                    }
                }
            }
        }
        TagEnd::Table => {
            finish_table_row(stack);
            if let Some(Ctx::Table {
                align,
                header,
                rows,
                ..
            }) = stack.pop()
            {
                emit_block(
                    stack,
                    out,
                    Block::Table {
                        header,
                        align,
                        rows,
                    },
                );
            }
        }
        TagEnd::TableHead => {
            // Flush the header cells while still in the head phase, then mark
            // the phase done so subsequent rows become body rows.
            finish_table_row(stack);
            if let Some(Ctx::Table { in_head, .. }) = stack.last_mut() {
                *in_head = false;
            }
        }
        TagEnd::TableRow => finish_table_row(stack),
        TagEnd::TableCell => {
            if let Some(Ctx::Cell { inlines }) = stack.pop() {
                if let Some(Ctx::Table { cur_row, .. }) = stack.last_mut() {
                    cur_row.push(inlines);
                }
            }
        }
        TagEnd::FootnoteDefinition => {
            if let Some(Ctx::Footnote {
                label,
                mut blocks,
                cur_para,
            }) = stack.pop()
            {
                if let Some(p) = cur_para {
                    if !p.is_empty() {
                        blocks.push(Block::Paragraph { inlines: p });
                    }
                }
                emit_block(stack, out, Block::Footnote { label, blocks });
            }
        }
        TagEnd::HtmlBlock => pop_into(stack, out, |c| match c {
            Ctx::HtmlBlock { buf } => Some(Block::Html(buf)),
            _ => None,
        }),
        TagEnd::Emphasis => finish_inline(stack, out, |c| match c {
            Ctx::Emphasis(v) => Inline::Emphasis(dedup_text(v)),
            _ => Inline::Text(String::new()),
        }),
        TagEnd::Strong => finish_inline(stack, out, |c| match c {
            Ctx::Strong(v) => Inline::Strong(dedup_text(v)),
            _ => Inline::Text(String::new()),
        }),
        TagEnd::Strikethrough => finish_inline(stack, out, |c| match c {
            Ctx::Strikethrough(v) => Inline::Strikethrough(dedup_text(v)),
            _ => Inline::Text(String::new()),
        }),
        TagEnd::Link => finish_inline(stack, out, |f| match f {
            Ctx::Link { dest, children } => Inline::Link {
                dest,
                children: dedup_text(children),
            },
            _ => Inline::Text(String::new()),
        }),
        TagEnd::Image => finish_inline(stack, out, |f| match f {
            Ctx::Image { src, alt } => Inline::Image { src, alt },
            _ => Inline::Text(String::new()),
        }),
        // Safe degradation for unsupported tags (already pushed no frame).
        _ => {}
    }
}

/// Pop the top frame and, if it matches `f`, emit the produced block.
fn pop_into(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, f: impl FnOnce(Ctx) -> Option<Block>) {
    if let Some(top) = stack.pop() {
        if let Some(b) = f(top) {
            emit_block(stack, out, b);
        }
    }
}

/// Pop an inline frame and sink the produced `Inline` into the new top.
fn finish_inline(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, f: impl FnOnce(Ctx) -> Inline) {
    if let Some(top) = stack.pop() {
        let inl = f(top);
        sink_inline(stack, out, inl);
    }
}

fn finish_table_row(stack: &mut [Ctx]) {
    if let Some(Ctx::Table {
        in_head,
        header,
        rows,
        cur_row,
        ..
    }) = stack.last_mut()
    {
        if !cur_row.is_empty() {
            let row = std::mem::take(cur_row);
            if *in_head {
                *header = row;
            } else {
                rows.push(row);
            }
        }
    }
}

fn level_num(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// If the top frame is a tight list item / footnote with an open implicit
/// paragraph, close it into a `Paragraph` block before a block-level frame is
/// pushed (or before a new item starts).
fn flush_item_para(stack: &mut [Ctx]) {
    match stack.last_mut() {
        Some(Ctx::Item {
            blocks, cur_para, ..
        }) => {
            if let Some(p) = cur_para.take() {
                if !p.is_empty() {
                    blocks.push(Block::Paragraph { inlines: p });
                }
            }
        }
        Some(Ctx::Footnote {
            blocks, cur_para, ..
        }) => {
            if let Some(p) = cur_para.take() {
                if !p.is_empty() {
                    blocks.push(Block::Paragraph { inlines: p });
                }
            }
        }
        _ => {}
    }
}

fn emit_block(stack: &mut [Ctx], out: &mut Vec<Block>, b: Block) {
    match stack.last_mut() {
        Some(Ctx::BlockQuote { blocks, .. }) => blocks.push(b),
        Some(Ctx::Item {
            blocks, cur_para, ..
        }) => {
            if let Some(p) = cur_para.take() {
                if !p.is_empty() {
                    blocks.push(Block::Paragraph { inlines: p });
                }
            }
            blocks.push(b);
        }
        Some(Ctx::Footnote {
            blocks, cur_para, ..
        }) => {
            if let Some(p) = cur_para.take() {
                if !p.is_empty() {
                    blocks.push(Block::Paragraph { inlines: p });
                }
            }
            blocks.push(b);
        }
        _ => out.push(b),
    }
}

fn sink_inline(stack: &mut [Ctx], out: &mut Vec<Block>, inl: Inline) {
    let mut hit = false;
    if let Some(top) = stack.last_mut() {
        match top {
            Ctx::Heading { inlines, .. } => {
                inlines.push(inl.clone());
                hit = true;
            }
            Ctx::Paragraph { inlines, .. } => {
                inlines.push(inl.clone());
                hit = true;
            }
            Ctx::Cell { inlines, .. } => {
                inlines.push(inl.clone());
                hit = true;
            }
            Ctx::Item { cur_para, .. } => {
                cur_para.get_or_insert_with(Vec::new).push(inl.clone());
                hit = true;
            }
            Ctx::Footnote { cur_para, .. } => {
                cur_para.get_or_insert_with(Vec::new).push(inl.clone());
                hit = true;
            }
            Ctx::Strong(v) => {
                v.push(inl.clone());
                hit = true;
            }
            Ctx::Emphasis(v) => {
                v.push(inl.clone());
                hit = true;
            }
            Ctx::Strikethrough(v) => {
                v.push(inl.clone());
                hit = true;
            }
            Ctx::Link { children, .. } => {
                children.push(inl.clone());
                hit = true;
            }
            Ctx::Image { alt, .. } => {
                alt.push_str(&inline_plain(&inl));
                hit = true;
            }
            _ => {}
        }
    }
    if !hit {
        out.push(Block::Paragraph { inlines: vec![inl] });
    }
}

fn inline_plain(i: &Inline) -> String {
    match i {
        Inline::Text(s) => s.clone(),
        Inline::Code(s) => s.clone(),
        Inline::SoftBreak | Inline::HardBreak => " ".into(),
        Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => {
            v.iter().map(inline_plain).collect()
        }
        Inline::Link { children, .. } => children.iter().map(inline_plain).collect(),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::InlineHtml(s) | Inline::Math(s) => s.clone(),
        Inline::FootnoteRef(s) => format!("[^{s}]"),
    }
}

/// Merge adjacent `Text` segments (pulldown-cmark can split text across events).
fn dedup_text(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::with_capacity(inlines.len());
    for i in inlines {
        match (out.last_mut(), &i) {
            (Some(Inline::Text(prev)), Inline::Text(s)) => prev.push_str(s),
            _ => out.push(i),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests: structural assertions on the IR (UI-MD-003)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_md(s: &str) -> Vec<Block> {
        parse(s)
    }

    fn para_text(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph { inlines } => Some(inline_plain_all(inlines)),
                _ => None,
            })
            .collect()
    }

    fn inline_plain_all(inlines: &[Inline]) -> String {
        inlines.iter().map(inline_plain).collect()
    }

    #[test]
    fn parses_six_heading_levels() {
        let md = "# h1\n## h2\n### h3\n#### h4\n##### h5\n###### h6\n";
        let blocks = parse_md(md);
        let levels: Vec<u8> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::Heading { level, .. } => Some(*level),
                _ => None,
            })
            .collect();
        assert_eq!(levels, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn preserves_strong_emphasis_strike_and_code() {
        let blocks = parse_md("**bold** *em* ~~strike~~ `code`");
        assert_eq!(blocks.len(), 1);
        let Block::Paragraph { inlines } = &blocks[0] else {
            panic!("expected paragraph")
        };
        assert!(inlines.iter().any(|i| matches!(i, Inline::Strong(_))));
        assert!(inlines.iter().any(|i| matches!(i, Inline::Emphasis(_))));
        assert!(inlines
            .iter()
            .any(|i| matches!(i, Inline::Strikethrough(_))));
        assert!(inlines
            .iter()
            .any(|i| matches!(i, Inline::Code(c) if c == "code")));
    }

    #[test]
    fn preserves_markdown_bullet_list_markers() {
        let blocks = parse_md("- 第一项\n- 第二项\n");
        assert_eq!(blocks.len(), 1);
        let Block::List { ordered, items, .. } = &blocks[0] else {
            panic!("expected list")
        };
        assert!(!*ordered);
        assert_eq!(items.len(), 2);
        assert_eq!(para_text(&items[0]), vec!["第一项".to_string()]);
        assert_eq!(para_text(&items[1]), vec!["第二项".to_string()]);
    }

    #[test]
    fn nests_ordered_list_under_second_item_with_start() {
        let md = "- 第一项\n- 第二项\n  1. 嵌套\n  2. 继续\n";
        let blocks = parse_md(md);
        let Block::List { items, .. } = &blocks[0] else {
            panic!()
        };
        assert_eq!(items.len(), 2);
        // Second item contains a nested ordered list.
        let inner = &items[1];
        let found = inner
            .iter()
            .find_map(|b| match b {
                Block::List {
                    ordered,
                    start,
                    items,
                } => Some((
                    *ordered,
                    *start,
                    items.len(),
                    items.iter().map(|it| para_text(it)).collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .expect("nested ordered list missing");
        assert!(found.0);
        assert_eq!(found.1, 1); // restarts numbering from 1
        assert_eq!(found.2, 2);
        assert_eq!(
            found.3,
            vec![vec!["嵌套".to_string()], vec!["继续".to_string()]]
        );
    }

    #[test]
    fn preserves_ordered_start_number() {
        let blocks = parse_md("3. three\n4. four\n");
        let Block::List {
            ordered,
            start,
            items,
        } = &blocks[0]
        else {
            panic!()
        };
        assert!(*ordered);
        assert_eq!(*start, 3);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn preserves_task_list_state() {
        let blocks = parse_md("- [x] 已完成\n- [ ] 待办\n");
        let Block::TaskList { checked, items } = &blocks[0] else {
            panic!("expected task list")
        };
        assert_eq!(checked, &vec![true, false]);
        assert_eq!(para_text(&items[0]), vec!["已完成".to_string()]);
        assert_eq!(para_text(&items[1]), vec!["待办".to_string()]);
    }

    #[test]
    fn distinguishes_task_list_from_plain_list() {
        let md = "- plain\n- [ ] task\n";
        let blocks = parse_md(md);
        // Because one item is a task, the whole list becomes a TaskList.
        assert!(matches!(&blocks[0], Block::TaskList { .. }));
    }

    #[test]
    fn supports_multi_paragraph_and_multi_level_items() {
        let md = "- first paragraph\n\n  second paragraph\n  - sub one\n  - sub two\n";
        let Block::List { items, .. } = &parse_md(md)[0] else {
            panic!()
        };
        let item = &items[0];
        let kinds: Vec<&str> = item
            .iter()
            .map(|b| match b {
                Block::Paragraph { .. } => "p",
                Block::List { .. } => "list",
                Block::TaskList { .. } => "task",
                _ => "?",
            })
            .collect();
        assert_eq!(kinds, vec!["p", "p", "list"]);
    }

    #[test]
    fn parses_quote_with_paragraphs_and_nested_list() {
        let md = "> quote text\n> - a\n> - b\n>\n> more\n";
        let Block::BlockQuote { blocks } = &parse_md(md)[0] else {
            panic!("expected quote")
        };
        let kinds: Vec<&str> = blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph { .. } => "p",
                Block::List { .. } => "list",
                _ => "?",
            })
            .collect();
        assert_eq!(kinds, vec!["p", "list", "p"]);
    }

    #[test]
    fn parses_table_header_alignment_and_empty_cells() {
        let md = "| a | b | c |\n| :--- | :---: | ---: |\n| 1 | 2 | 3 |\n| x | | y |\n";
        let Block::Table {
            header,
            align,
            rows,
        } = &parse_md(md)[0]
        else {
            panic!("expected table")
        };
        assert_eq!(header.len(), 3);
        assert_eq!(align, &vec![Align::Left, Align::Center, Align::Right]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].len(), 3);
        assert!(rows[1][1].is_empty()); // empty cell preserved
    }

    #[test]
    fn parses_fenced_and_indented_code_blocks_and_unknown_lang() {
        let blocks = parse_md("```rust\nfn main() {}\n```\n\n```\nplain\n```\n\n    indented\n");
        let langs: Vec<Option<String>> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::CodeBlock { lang, text } => {
                    assert!(!text.is_empty());
                    Some(lang.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(langs, vec![Some("rust".into()), None, None]);
    }

    #[test]
    fn captures_links_images_and_breaks() {
        let blocks = parse_md("[link](https://egui.rs) ![alt](img.png)  \nnext line");
        let Block::Paragraph { inlines } = &blocks[0] else {
            panic!()
        };
        assert!(inlines
            .iter()
            .any(|i| matches!(i, Inline::Link { dest, .. } if dest == "https://egui.rs")));
        assert!(inlines
            .iter()
            .any(|i| matches!(i, Inline::Image { src, alt } if src == "img.png" && alt == "alt")));
        assert!(inlines.contains(&Inline::HardBreak));
    }

    #[test]
    fn soft_break_stays_soft_and_hard_break_is_hard() {
        let md = "one\ntwo  \nthree";
        let Block::Paragraph { inlines } = &parse_md(md)[0] else {
            panic!()
        };
        assert!(inlines.contains(&Inline::SoftBreak));
        assert!(inlines.contains(&Inline::HardBreak));
    }

    #[test]
    fn preserves_html_and_footnote_degradation() {
        let md =
            "text with <span>html</span>\n\n<div>block</div>\n\nnote[^1]\n\n[^1]: definition\n";
        let blocks = parse_md(md);
        // Inline HTML is kept as an InlineHtml segment.
        assert!(blocks.iter().any(|b| match b {
            Block::Paragraph { inlines } =>
                inlines.iter().any(|i| matches!(i, Inline::InlineHtml(_))),
            _ => false,
        }));
        // Block HTML is preserved verbatim.
        assert!(blocks.iter().any(|b| matches!(b, Block::Html(_))));
        // Footnote definition is represented.
        assert!(blocks.iter().any(|b| matches!(b, Block::Footnote { .. })));
    }

    #[test]
    fn table_is_not_swallowed_by_following_heading() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |\n\n## 链接\n";
        let blocks = parse_md(md);
        assert!(matches!(&blocks[0], Block::Table { .. }));
        assert!(matches!(&blocks[1], Block::Heading { level: 2, .. }));
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn tolerant_unclosed_fence_and_emphasis() {
        // Unclosed code fence: pulses a code block to EOF without panicking.
        let blocks = parse_md("```rust\nfn main() {\n");
        assert!(blocks.len() == 1);
        assert!(matches!(&blocks[0], Block::CodeBlock { .. }));
        // Unclosed emphasis: safely degrades to plain text.
        let blocks2 = parse_md("a *b");
        assert!(!blocks2.is_empty());
    }

    #[test]
    fn thematic_break_parsed() {
        let blocks = parse_md("---\n");
        assert!(blocks.iter().any(|b| matches!(b, Block::ThematicBreak)));
    }
}
