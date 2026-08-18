//! Document model: pulldown-cmark events -> intermediate representation (IR).
//! The IR is consumed by both the render (preview) and edit views.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Image block variant reserved; images currently rendered as inline
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    CodeBlock { lang: Option<String>, text: String },
    BlockQuote { blocks: Vec<Block> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    TaskList { checked: Vec<bool>, items: Vec<Vec<Block>> },
    Table { header: Vec<Vec<Inline>>, align: Vec<Align>, rows: Vec<Vec<Vec<Inline>>> },
    ThematicBreak,
    Image { src: String, alt: String },
    Html(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Code(String),
    Link { dest: String, children: Vec<Inline> },
    Image { src: String, alt: String },
    Strikethrough(Vec<Inline>),
    SoftBreak,
}

/// A parsed Markdown document. `text` is the raw source (used by the editor).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // `parse_error` reserved for diagnostics
pub struct Document {
    pub path: Option<PathBuf>,
    pub text: String,
    pub blocks: Vec<Block>,
    pub dirty: bool,
    pub parse_error: Option<String>,
}

impl Document {
    pub fn new(text: String) -> Self {
        let blocks = parse(&text);
        Document { path: None, text, blocks, dirty: false, parse_error: None }
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
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    CodeBlock { lang: Option<String>, buf: String },
    BlockQuote { inner: Vec<Block>, para: Option<Vec<Inline>> },
    List {
        ordered: bool,
        items: Vec<(Option<bool>, Vec<Block>)>,
        cur: Vec<Block>,
        cur_task: Option<bool>,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
        cur_row: Vec<Vec<Inline>>,
        cur_cell: Vec<Inline>,
        in_head: bool,
    },
    Inline(InlineFrame),
}

enum InlineFrame {
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link { dest: String, children: Vec<Inline> },
    Image { src: String, alt: String },
    Raw(Vec<Inline>),
}

fn parse(text: &str) -> Vec<Block> {
    let parser = Parser::new_ext(text, mk_options());
    let mut stack: Vec<Ctx> = vec![Ctx::Root];
    let mut out: Vec<Block> = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let lvl = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    stack.push(Ctx::Heading { level: lvl, inlines: Vec::new() });
                }
                Tag::Paragraph => stack.push(Ctx::Paragraph { inlines: Vec::new() }),
                Tag::CodeBlock(kind) => {
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
                    stack.push(Ctx::CodeBlock { lang, buf: String::new() });
                }
                Tag::BlockQuote(_) => stack.push(Ctx::BlockQuote { inner: Vec::new(), para: None }),
                Tag::List(opt) => stack.push(Ctx::List {
                    ordered: opt.is_some(),
                    items: Vec::new(),
                    cur: Vec::new(),
                    cur_task: None,
                }),
                Tag::Item => {
                    if let Some(last) = stack.last_mut() {
                        if let Ctx::List { items, cur, cur_task, .. } = last {
                            let task = cur_task.take();
                            if !cur.is_empty() || task.is_some() {
                                items.push((task, std::mem::take(cur)));
                            }
                        }
                    }
                }
                Tag::Table(align) => {
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
                        cur_cell: Vec::new(),
                        in_head: true,
                    });
                }
                Tag::TableHead => set_table_phase(&mut stack, true),
                Tag::TableRow => finish_table_row(&mut stack),
                Tag::TableCell => {
                    if let Some(Ctx::Table { cur_cell, .. }) = stack.last_mut() {
                        cur_cell.clear();
                    }
                }
                Tag::Emphasis => stack.push(Ctx::Inline(InlineFrame::Emphasis(Vec::new()))),
                Tag::Strong => stack.push(Ctx::Inline(InlineFrame::Strong(Vec::new()))),
                Tag::Strikethrough => {
                    stack.push(Ctx::Inline(InlineFrame::Strikethrough(Vec::new())))
                }
                Tag::Link { dest_url, .. } => stack.push(Ctx::Inline(InlineFrame::Link {
                    dest: dest_url.to_string(),
                    children: Vec::new(),
                })),
                Tag::Image { dest_url, .. } => stack.push(Ctx::Inline(InlineFrame::Image {
                    src: dest_url.to_string(),
                    alt: String::new(),
                })),
                _ => stack.push(Ctx::Inline(InlineFrame::Raw(Vec::new()))),
            },
            Event::End(_) => pull_one(&mut stack, &mut out),
            Event::Text(t) => {
                let s = t.to_string();
                push_text(&mut stack, &mut out, s);
            }
            Event::SoftBreak | Event::HardBreak => {
                push_inline(&mut stack, &mut out, Inline::SoftBreak);
            }
            Event::Rule => emit_block(&mut stack, &mut out, Block::ThematicBreak),
            Event::TaskListMarker(checked) => {
                if let Some(last) = stack.last_mut() {
                    if let Ctx::List { cur_task, .. } = last {
                        *cur_task = Some(checked);
                    }
                }
            }
            Event::Html(t) => emit_block(&mut stack, &mut out, Block::Html(t.to_string())),
            Event::InlineHtml(t) => {
                push_inline(&mut stack, &mut out, Inline::Code(t.to_string()));
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                push_inline(&mut stack, &mut out, Inline::Code(t.to_string()));
            }
            Event::FootnoteReference(name) => {
                push_inline(&mut stack, &mut out, Inline::Code(name.to_string()));
            }
            _ => {}
        }
    }
    out
}

fn set_table_phase(stack: &mut Vec<Ctx>, param: bool) {
    if let Some(Ctx::Table { in_head, .. }) = stack.last_mut() {
        *in_head = param;
    }
}

fn finish_table_row(stack: &mut Vec<Ctx>) {
    // Pull the current row's cells: flush cur_row if non-empty.
    match stack.last_mut() {
        Some(Ctx::Table { in_head, header, rows, cur_row, cur_cell, .. }) => {
            if !cur_cell.is_empty() {
                cur_row.push(std::mem::take(cur_cell));
            }
            if !cur_row.is_empty() {
                let row = std::mem::take(cur_row);
                if *in_head {
                    *header = row;
                } else {
                    rows.push(row);
                }
            }
        }
        _ => {}
    }
}

fn emit_block(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, b: Block) {
    match stack.last_mut() {
        Some(Ctx::List { cur, .. }) => cur.push(b),
        Some(Ctx::BlockQuote { inner, .. }) => inner.push(b),
        _ => out.push(b),
    }
}

fn push_text(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, s: String) {
    match stack.last_mut() {
        Some(Ctx::Heading { inlines, .. }) => inlines.push(Inline::Text(s)),
        Some(Ctx::Paragraph { inlines, .. }) => inlines.push(Inline::Text(s)),
        Some(Ctx::CodeBlock { buf, .. }) => buf.push_str(&s),
        Some(Ctx::Table { cur_cell, .. }) => cur_cell.push(Inline::Text(s)),
        Some(Ctx::Inline(frame)) => push_text_to_inline_frame(frame, s),
        Some(Ctx::BlockQuote { para, .. }) => {
            let p = para.get_or_insert_with(Vec::new);
            p.push(Inline::Text(s));
        }
        _ => out.push(Block::Paragraph { inlines: vec![Inline::Text(s)] }),
    }
}

fn push_text_to_inline_frame(frame: &mut InlineFrame, s: String) {
    match frame {
        InlineFrame::Strong(v) => v.push(Inline::Text(s)),
        InlineFrame::Emphasis(v) => v.push(Inline::Text(s)),
        InlineFrame::Strikethrough(v) => v.push(Inline::Text(s)),
        InlineFrame::Link { children, .. } => children.push(Inline::Text(s)),
        InlineFrame::Image { alt, .. } => alt.push_str(&s),
        InlineFrame::Raw(v) => v.push(Inline::Text(s)),
    }
}

fn push_inline(stack: &mut Vec<Ctx>, out: &mut Vec<Block>, i: Inline) {
    match stack.last_mut() {
        Some(Ctx::Heading { inlines, .. }) => inlines.push(i),
        Some(Ctx::Paragraph { inlines, .. }) => inlines.push(i),
        Some(Ctx::Table { cur_cell, .. }) => cur_cell.push(i),
        Some(Ctx::Inline(frame)) => push_inline_to_inline_frame(frame, i),
        Some(Ctx::BlockQuote { para, .. }) => {
            let p = para.get_or_insert_with(Vec::new);
            p.push(i);
        }
        _ => out.push(Block::Paragraph { inlines: vec![i] }),
    }
}

fn push_inline_to_inline_frame(frame: &mut InlineFrame, i: Inline) {
    match frame {
        InlineFrame::Strong(v) => v.push(i),
        InlineFrame::Emphasis(v) => v.push(i),
        InlineFrame::Strikethrough(v) => v.push(i),
        InlineFrame::Link { children, .. } => children.push(i),
        InlineFrame::Image { alt, .. } => alt.push_str(&inline_to_plain(&i)),
        InlineFrame::Raw(v) => v.push(i),
    }
}

fn inline_to_plain(i: &Inline) -> String {
    match i {
        Inline::Text(s) => s.clone(),
        Inline::Code(s) => s.clone(),
        Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => {
            v.iter().map(inline_to_plain).collect()
        }
        Inline::Link { children, .. } => children.iter().map(inline_to_plain).collect(),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::SoftBreak => " ".to_string(),
    }
}

fn pull_one(stack: &mut Vec<Ctx>, out: &mut Vec<Block>) {
    // Pop the top frame. `TagEnd` for the top-most element.
    let top = match stack.pop() {
        Some(t) => t,
        None => return,
    };
    match top {
        Ctx::Heading { level, inlines } => emit_block(stack, out, Block::Heading { level, inlines }),
        Ctx::Paragraph { inlines } => emit_block(stack, out, Block::Paragraph { inlines }),
        Ctx::CodeBlock { lang, buf } => {
            emit_block(stack, out, Block::CodeBlock { lang, text: buf.trim_end().to_string() })
        }
        Ctx::Table { align, header, rows, cur_cell, .. } => {
            // rows already flushed via TableRow ends; ignore a trailing stray cell.
            let _ = cur_cell;
            emit_block(stack, out, Block::Table { header, align, rows });
        }
        Ctx::BlockQuote { mut inner, para } => {
            if let Some(p) = para {
                inner.push(Block::Paragraph { inlines: p });
            }
            emit_block(stack, out, Block::BlockQuote { blocks: inner });
        }
        Ctx::List { ordered, mut items, cur, cur_task } => {
            if !cur.is_empty() || cur_task.is_some() {
                items.push((cur_task, cur));
            }
            // Decide task list vs plain list
            let is_task = items.iter().any(|(t, _)| t.is_some());
            if is_task {
                let checked: Vec<bool> = items.iter().map(|(t, _)| t.unwrap_or(false)).collect();
                let blocks: Vec<Vec<Block>> = items.into_iter().map(|(_, b)| b).collect();
                emit_block(stack, out, Block::TaskList { checked, items: blocks });
            } else {
                let items: Vec<Vec<Block>> = items.into_iter().map(|(_, b)| b).collect();
                emit_block(stack, out, Block::List { ordered, items });
            }
        }
        Ctx::Inline(frame) => {
            let inline = finish_inline_frame(frame);
            push_inline(stack, out, inline);
        }
        Ctx::Root => {}
    }
}

fn finish_inline_frame(frame: InlineFrame) -> Inline {
    match frame {
        InlineFrame::Strong(v) => Inline::Strong(dedup_text(v)),
        InlineFrame::Emphasis(v) => Inline::Emphasis(dedup_text(v)),
        InlineFrame::Strikethrough(v) => Inline::Strikethrough(dedup_text(v)),
        InlineFrame::Link { dest, children } => Inline::Link { dest, children: dedup_text(children) },
        InlineFrame::Image { src, alt } => Inline::Image { src, alt },
        InlineFrame::Raw(v) => Inline::Text(
            v.into_iter()
                .filter_map(|i| match i {
                    Inline::Text(s) => Some(s),
                    _ => None,
                })
                .collect(),
        ),
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
