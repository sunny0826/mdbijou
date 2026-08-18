//! Preview renderer: map the IR (Block/Inline) onto egui widgets.

use crate::document::{Align, Block, Document, Inline};
use crate::highlight::Highlighter;
use crate::theme::Theme;
use egui::{text::LayoutJob, text::TextFormat, Color32, FontId, RichText, Stroke, Ui};

/// Shared state needed across block rendering.
pub struct RenderCtx<'a> {
    pub theme: &'a Theme,
    pub hl: &'a mut dyn Highlighter,
    pub content_width: f32,
    pub font_size: f32,
}

impl<'a> RenderCtx<'a> {
    pub fn new(theme: &'a Theme, hl: &'a mut dyn Highlighter, content_width: f32, font_size: f32) -> Self {
        Self { theme, hl, content_width, font_size }
    }
}

/// Render the whole document into `ui` (typical scroll area).
pub fn render_document(ui: &mut Ui, doc: &Document, ctx: &mut RenderCtx) {
    for block in &doc.blocks {
        render_block(ui, block, ctx);
    }
}

pub fn render_blocks(ui: &mut Ui, blocks: &[Block], ctx: &mut RenderCtx) {
    for block in blocks {
        render_block(ui, block, ctx);
    }
}

pub fn render_block(ui: &mut Ui, block: &Block, ctx: &mut RenderCtx) {
    match block {
        Block::Heading { level, inlines } => {
            let size = match level {
                1 => 28.0,
                2 => 24.0,
                3 => 20.0,
                4 => 18.0,
                _ => 16.0,
            };
            ui.add_space(if *level <= 2 { 14.0 } else { 8.0 });
            ui.label(
                RichText::new(inline_text(inlines))
                    .size(size)
                    .strong()
                    .color(ctx.theme.c.heading),
            );
            // heading rule for h1/h2
            if *level <= 2 {
                let y = ui.cursor().top();
                let w = ui.available_width().min(ctx.content_width - 8.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 2.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, ctx.theme.c.hr);
                let _ = y;
            }
            ui.add_space(4.0);
        }
        Block::Paragraph { inlines } => {
            ui.add_space(5.0);
            render_inlines(ui, inlines, ctx);
            ui.add_space(5.0);
        }
        Block::CodeBlock { lang, text } => {
            ui.add_space(8.0);
            render_code_block(ui, lang.as_deref(), text, ctx);
            ui.add_space(8.0);
        }
        Block::BlockQuote { blocks } => {
            ui.add_space(6.0);
            let frame = egui::Frame::new()
                .fill(ctx.theme.c.code_bg)
                .inner_margin(egui::Margin::symmetric(12, 8));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let bar_w = 3.0;
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_w, ui.available_height().max(1.0)),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(bar_rect, 0.0, ctx.theme.c.blockquote_bar);
                    ui.vertical(|ui| {
                        render_blocks(ui, blocks, ctx);
                    });
                });
            });
            ui.add_space(6.0);
        }
        Block::List { ordered, items } => {
            ui.add_space(4.0);
            render_list(ui, *ordered, items, ctx);
            ui.add_space(6.0);
        }
        Block::TaskList { checked, items } => {
            ui.add_space(4.0);
            render_task_list(ui, checked, items, ctx);
            ui.add_space(6.0);
        }
        Block::Table { header, align, rows } => {
            ui.add_space(8.0);
            render_table(ui, header, align, rows, ctx);
            ui.add_space(8.0);
        }
        Block::ThematicBreak => {
            ui.add_space(10.0);
            let w = ui.available_width().min(ctx.content_width);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 2.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, ctx.theme.c.hr);
            ui.add_space(10.0);
        }
        Block::Image { src, alt } => {
            ui.add_space(8.0);
            let _ = alt;
            ui.label(RichText::new(format!("🖼 [image] {src}")).color(ctx.theme.c.muted).italics());
            ui.add_space(6.0);
        }
        Block::Html(_) => {
            // Safe fallback: raw HTML is not executed.
        }
    }
}

fn render_list(ui: &mut Ui, ordered: bool, items: &[Vec<Block>], ctx: &mut RenderCtx) {
    for (idx, item) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            let marker = if ordered { format!("{}.", idx + 1) } else { "•".to_string() };
            ui.add_space(8.0);
            ui.label(RichText::new(marker).color(ctx.theme.c.muted));
            ui.vertical(|ui| {
                render_blocks(ui, item, ctx);
            });
        });
    }
}

fn render_task_list(ui: &mut Ui, checked: &[bool], items: &[Vec<Block>], ctx: &mut RenderCtx) {
    for (idx, item) in items.iter().enumerate() {
        let chk = checked.get(idx).copied().unwrap_or(false);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let boxed = if chk {
                RichText::new("☑").color(ctx.theme.c.link)
            } else {
                RichText::new("☐").color(ctx.theme.c.muted)
            };
            ui.label(boxed);
            ui.vertical(|ui| {
                render_blocks(ui, item, ctx);
            });
        });
    }
}

fn render_table(
    ui: &mut Ui,
    header: &[Vec<Inline>],
    align: &[Align],
    rows: &[Vec<Vec<Inline>>],
    ctx: &mut RenderCtx,
) {
    if header.is_empty() {
        return;
    }
    let _ = align;
    egui::Grid::new(egui::Id::new("md_table"))
        .striped(true)
        .spacing(egui::vec2(12.0, 6.0))
        .min_col_width(40.0)
        .show(ui, |ui| {
            for cell in header {
                ui.label(
                    RichText::new(inline_text(cell))
                        .strong()
                        .color(ctx.theme.c.heading),
                );
            }
            ui.end_row();
            for row in rows {
                for cell in row {
                    ui.label(RichText::new(inline_text(cell)).color(ctx.theme.c.foreground));
                }
                ui.end_row();
            }
        });
}

pub fn render_code_block(ui: &mut Ui, lang: Option<&str>, text: &str, ctx: &mut RenderCtx) {
    let frame = egui::Frame::new()
        .fill(ctx.theme.c.code_bg)
        .stroke(Stroke::new(1.0, ctx.theme.c.table_border))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(12, 8));
    frame.show(ui, |ui| {
        if let Some(lang) = lang {
            ui.label(RichText::new(lang).small().color(ctx.theme.c.muted));
            ui.add_space(2.0);
        }
        egui::ScrollArea::horizontal().show(ui, |ui| {
            let mono = FontId::monospace(ctx.font_size - 1.0);
            let fg = ctx.theme.c.foreground;
            for line in text.split('\n') {
                let spans = ctx.hl.code_line(lang, line);
                if spans.iter().all(|s| s.text.is_empty()) {
                    ui.add(egui::Label::new(RichText::new(" ")).selectable(false));
                    continue;
                }
                let job = spans_to_job(&spans, mono.clone(), fg);
                ui.add(egui::Label::new(job).selectable(true));
            }
        });
    });
}

fn spans_to_job(spans: &[crate::highlight::Span], mono: FontId, default: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for sp in spans {
        let color = if sp.color == Color32::TRANSPARENT { default } else { sp.color };
        job.append(&sp.text, 0.0, TextFormat { font_id: mono.clone(), color, ..Default::default() });
    }
    job
}

/// Render a run of inline nodes as wrapped label(s); links are clickable.
pub fn render_inlines(ui: &mut Ui, inlines: &[Inline], ctx: &mut RenderCtx) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        for inl in inlines {
            render_inline(ui, inl, ctx);
        }
    });
}

fn render_inline(ui: &mut Ui, inl: &Inline, ctx: &mut RenderCtx) {
    let size = ctx.font_size;
    match inl {
        Inline::Text(s) => {
            ui.add(egui::Label::new(RichText::new(s).size(size).color(ctx.theme.c.foreground)));
        }
        Inline::SoftBreak => {
            ui.add_space(2.0);
        }
        Inline::Code(s) => {
            let rt = RichText::new(format!("`{s}`"))
                .monospace()
                .size(size - 1.0)
                .color(ctx.theme.c.code_fg)
                .background_color(ctx.theme.c.code_bg);
            ui.add(egui::Label::new(rt));
        }
        Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => {
            let text = inline_text(v);
            let mut rt = RichText::new(text).size(size).color(ctx.theme.c.foreground);
            if matches!(inl, Inline::Strong(_)) {
                rt = rt.strong();
            }
            if matches!(inl, Inline::Emphasis(_)) {
                rt = rt.italics();
            }
            if matches!(inl, Inline::Strikethrough(_)) {
                rt = rt.strikethrough();
            }
            ui.add(egui::Label::new(rt));
        }
        Inline::Link { dest, children } => {
            let txt = inline_text(children);
            let resp = ui.add(
                egui::Link::new(RichText::new(txt).color(ctx.theme.c.link).size(size)),
            );
            if resp.clicked() {
                open_url(dest);
            }
            resp.on_hover_text(dest);
        }
        Inline::Image { src, alt } => {
            let _ = alt;
            ui.label(RichText::new(format!("🖼 {src}")).color(ctx.theme.c.muted).italics());
        }
    }
}

fn inline_text(inlines: &[Inline]) -> String {
    inlines.iter().filter_map(inline_leaf).collect()
}

fn inline_leaf(i: &Inline) -> Option<String> {
    match i {
        Inline::Text(s) => Some(s.clone()),
        Inline::Code(s) => Some(format!("`{s}`")),
        Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => Some(inline_text(v)),
        Inline::Link { children, .. } => Some(inline_text(children)),
        Inline::Image { alt, .. } => Some(alt.clone()),
        Inline::SoftBreak => Some(" ".to_string()),
    }
}

fn open_url(url: &str) {
    // macOS `open`; fall back to xdg-open on other unix.
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn();
}
