//! Preview renderer: map the IR (Block/Inline) onto egui widgets.
//!
//! Paragraphs and headings are drawn as a single rich-text `LayoutJob` so that
//! CJK and inline styles wrap naturally (UI-MD-004); links within the job are
//! made clickable via character-cursor hit testing. Lists, tables, quotes and
//! images are handled as dedicated widgets.

use crate::document::{Align, Block, Inline};
use crate::highlight::{Highlighter, Line};
use crate::images::ImageStore;
use crate::theme::{Metrics, Theme};
use egui::text::{CCursor, LayoutJob, TextFormat};
use egui::{pos2, vec2, Color32, CursorIcon, FontId, Rect, RichText, Sense, Stroke, Ui};
use std::time::{Duration, Instant};

/// Shared state needed across block rendering.
pub struct RenderCtx<'a> {
    pub theme: &'a Theme,
    pub hl: &'a mut dyn Highlighter,
    pub images: &'a mut ImageStore,
    /// Effective content-column width (already clamped for narrow windows).
    pub content_width: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub metrics: Metrics,
    /// Headings rendered this pass, in document order: (TOC anchor, on-screen
    /// rect). Populated when `toc_entries` is non-empty (i.e. the TOC panel is
    /// active); the preview side panel uses it to scroll to a heading.
    pub heading_anchors: Vec<(String, egui::Rect)>,
    /// TOC entries for the document being rendered, in document order. The
    /// heading branch consumes them with a cursor so anchors match `toc::extract`
    /// exactly even though rendering and extraction run separately.
    pub toc_entries: Vec<crate::toc::TocEntry>,
    /// Cursor into `toc_entries` for the next heading encountered.
    pub toc_cursor: usize,
    /// Internal anchor clicked during this render pass, if any. Set by
    /// `paint_job` when a `dest` starting with `#` is clicked; the preview
    /// host (`MdbijouApp`) drains it to trigger `pending_toc_anchor` scrolling.
    pub clicked_anchor: Option<String>,
}

impl<'a> RenderCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        theme: &'a Theme,
        hl: &'a mut dyn Highlighter,
        images: &'a mut ImageStore,
        content_width: f32,
        font_size: f32,
        line_height: f32,
        metrics: Metrics,
    ) -> Self {
        Self {
            theme,
            hl,
            images,
            content_width,
            font_size,
            line_height,
            metrics,
            heading_anchors: Vec::new(),
            toc_entries: Vec::new(),
            toc_cursor: 0,
            clicked_anchor: None,
        }
    }

    fn body_font(&self) -> FontId {
        FontId::proportional(self.font_size)
    }

    fn heading_size(&self, level: u8) -> f32 {
        // Scale headings with the configured body size (16pt baseline).
        let k = self.font_size / 16.0;
        let base = match level {
            1 => 28.0,
            2 => 23.0,
            3 => 20.0,
            4 => 17.5,
            5 => 16.0,
            _ => 15.0,
        };
        base * k
    }

    fn line_h(&self, size: f32) -> f32 {
        (size * self.line_height).max(size + 4.0)
    }
}

/// Render the whole document into `ui` (typical scroll area).
pub fn render_document(ui: &mut Ui, doc: &crate::document::Document, ctx: &mut RenderCtx) {
    ctx.heading_anchors.clear();
    ctx.toc_cursor = 0;
    ctx.clicked_anchor = None;
    render_blocks(ui, &doc.blocks, ctx, 0);
}

pub fn render_blocks(ui: &mut Ui, blocks: &[Block], ctx: &mut RenderCtx, depth: usize) {
    for block in blocks {
        render_block(ui, block, ctx, depth);
    }
}

pub fn render_block(ui: &mut Ui, block: &Block, ctx: &mut RenderCtx, depth: usize) {
    match block {
        Block::Heading {
            level,
            inlines,
            align,
        } => {
            let size = ctx.heading_size(*level);
            let color = ctx.theme.c.heading;
            let (job, links, strikes) =
                build_inline_job(inlines, ctx, color, FontId::proportional(size));
            // Paper & Jewel: H1 24/12, H2 20/10, H3+ 14/8 — use Metrics tokens where available.
            let top_space = match *level {
                1 => 24.0, // metrics.space_lg + 4
                2 => 20.0, // metrics.space_lg
                _ => 14.0, // metrics.space_md + 2
            };
            ui.add_space(top_space);
            let rect = paint_job(ui, job, links, strikes, color, *align, ctx);
            if *level == 1 {
                let anchor_id =
                    ui.id()
                        .with(("heading-anchor", rect.min.x.to_bits(), rect.min.y.to_bits()));
                let resp = ui.interact(rect, anchor_id, Sense::hover());
                let hovered = resp.hovered();
                let alpha = if hovered { 1.0 } else { 0.35 };
                let anchor_color = ctx.theme.c.muted.gamma_multiply(alpha);
                let font = FontId::proportional(11.0);
                let galley =
                    ui.fonts_mut(|f| f.layout_no_wrap("§".to_owned(), font.clone(), anchor_color));
                let gw = galley.size().x;
                let anchor_rect = egui::Rect::from_min_size(
                    pos2(rect.max.x - 6.0 - gw, rect.center().y - 7.0),
                    egui::vec2(gw, 14.0),
                );
                let mut shift = egui::Vec2::ZERO;
                if let Some(placed) = galley.rows.first() {
                    let ink = placed.row.visuals.mesh_bounds;
                    if ink.is_finite() && ink.width() > 0.0 && ink.height() > 0.0 {
                        let ink_center = placed.pos + ink.center().to_vec2();
                        shift = galley.rect.center() - ink_center;
                    }
                }
                let y = anchor_rect.center().y - galley.size().y / 2.0 + shift.y;
                let x = anchor_rect.max.x - galley.size().x;
                ui.painter().galley(egui::pos2(x, y), galley, anchor_color);
                if hovered {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
            // Record the heading's on-screen rect under its TOC anchor so the
            // TOC panel can scroll the preview to it. Entries are consumed in
            // document order, matching `toc::extract`'s traversal.
            if let Some(entry) = ctx.toc_entries.get(ctx.toc_cursor) {
                ctx.heading_anchors.push((entry.anchor.clone(), rect));
            }
            ctx.toc_cursor += 1;
            if *level <= 2 {
                ui.add_space(8.0);
                let hairline_h = ctx.metrics.hairline.max(1.0);
                let hr_color = ctx.theme.c.hr.gamma_multiply(0.7);
                let (line_rect, _) = ui.allocate_exact_size(
                    vec2(ui.available_width().min(ctx.content_width), hairline_h),
                    Sense::hover(),
                );
                ui.painter().rect_filled(line_rect, 1.0, hr_color);
                let bottom_extra = match *level {
                    1 => 4.0,
                    2 => 2.0,
                    _ => 6.0,
                };
                ui.add_space(bottom_extra);
            } else {
                ui.add_space(8.0);
            }
        }
        Block::Paragraph { inlines, align } => {
            ui.add_space(7.0); // metrics.space_sm - 1 (paper breathing)
                               // Standalone image paragraph -> real image block.
            if matches!(inlines.as_slice(), [Inline::Image { .. }]) {
                if let Inline::Image { src, alt, width } = &inlines[0] {
                    if *align == Align::Center {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            render_image(ui, src, alt, *width, ctx);
                        });
                    } else {
                        render_image(ui, src, alt, *width, ctx);
                    }
                }
            } else {
                let (job, links, strikes) =
                    build_inline_job(inlines, ctx, ctx.theme.c.foreground, ctx.body_font());
                paint_job(ui, job, links, strikes, ctx.theme.c.foreground, *align, ctx);
            }
            ui.add_space(7.0);
        }
        Block::CodeBlock { lang, text } => {
            ui.add_space(ctx.metrics.space_sm);
            let is_mermaid = lang
                .as_deref()
                .is_some_and(|l| l.eq_ignore_ascii_case("mermaid"));
            if !is_mermaid || !crate::mermaid::render(ui, text, ctx.theme, ctx.font_size) {
                render_code_block(ui, lang.as_deref(), text, ctx);
            }
            ui.add_space(ctx.metrics.space_sm);
        }
        Block::BlockQuote { blocks } => {
            ui.add_space(6.0);
            render_blockquote(ui, blocks, ctx, depth);
            ui.add_space(6.0);
        }
        Block::List {
            ordered,
            start,
            items,
        } => {
            ui.add_space(ctx.metrics.space_xs);
            render_list(ui, *ordered, *start, items, ctx, depth);
            ui.add_space(7.0);
        }
        Block::TaskList { checked, items } => {
            ui.add_space(ctx.metrics.space_xs);
            render_task_list(ui, checked, items, ctx, depth);
            ui.add_space(7.0);
        }
        Block::Table {
            header,
            align,
            rows,
        } => {
            ui.add_space(ctx.metrics.space_sm);
            render_table(ui, header, align, rows, ctx);
            ui.add_space(ctx.metrics.space_sm);
        }
        Block::ThematicBreak => {
            ui.add_space(14.0);
            let hairline_h = ctx.metrics.hairline.max(1.0);
            let avail_w = ui.available_width().min(ctx.content_width);
            let line_w = (avail_w * 0.6).max(80.0);
            let pad = ((avail_w - line_w) / 2.0).max(24.0);
            // Centered 60% width solid hr with breathing room; gradient fade approximated by centering.
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let (rect, _) = ui.allocate_exact_size(vec2(line_w, hairline_h), Sense::hover());
                ui.painter().rect_filled(rect, 1.0, ctx.theme.c.hr);
            });
            ui.add_space(14.0);
        }
        Block::Html(raw) => match crate::html::html_blocks(raw) {
            Some(blocks) => render_blocks(ui, &blocks, ctx, depth),
            None => {
                ui.add_space(6.0);
                // Render HTML inertly: extract the text content (never execute).
                let text = html_text(raw);
                egui::Frame::new()
                    .fill(ctx.theme.c.code_bg)
                    .stroke(Stroke::new(1.0, ctx.theme.c.table_border))
                    .corner_radius(ctx.metrics.radius_sm)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(RichText::new("HTML").small().color(ctx.theme.c.muted));
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(text)
                                .size(ctx.font_size)
                                .color(ctx.theme.c.foreground),
                        );
                    });
                ui.add_space(6.0);
            }
        },
        Block::Footnote { label, blocks } => {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 14.0);
                ui.label(
                    RichText::new(format!("^{label}"))
                        .small()
                        .color(ctx.theme.c.muted),
                );
                ui.vertical(|ui| {
                    render_blocks(ui, blocks, ctx, 0);
                });
            });
            ui.add_space(6.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Rich text (single layout job)
// ---------------------------------------------------------------------------

/// A clickable link span expressed in character offsets into the job text.
struct LinkSpan {
    start: usize,
    end: usize,
    url: String,
}

/// A strikethrough span in character offsets, drawn manually at the row's
/// optical center (see `Fmt::to_format` for why).
struct StrikeSpan {
    start: usize,
    end: usize,
    size: f32,
    color: Color32,
}

#[derive(Clone)]
struct Fmt {
    font: FontId,
    color: Color32,
    bg: Color32,
    italic: bool,
    underline: bool,
}

impl Fmt {
    // NOTE: strikethrough is NOT expressed via `TextFormat.strikethrough`.
    // egui places decoration lines at the center of the glyph's *logical*
    // rect, which derives from the font's ascent/descent metrics. With CJK
    // fallback fonts (PingFang's ascent ≈ 1.16em) the line lands visibly
    // below the glyph's optical middle. We therefore record strike spans and
    // draw the line ourselves at the row's visual center (see paint_job).
    fn to_format(&self, line_h: f32) -> TextFormat {
        TextFormat {
            font_id: self.font.clone(),
            line_height: Some(line_h),
            color: self.color,
            background: self.bg,
            expand_bg: 1.5,
            italics: self.italic,
            underline: if self.underline {
                Stroke::new(1.0, self.color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        }
    }
}

fn base_fmt(font: FontId, color: Color32) -> Fmt {
    Fmt {
        font,
        color,
        bg: Color32::TRANSPARENT,
        italic: false,
        underline: false,
    }
}

/// Build a `LayoutJob` (plus link spans) from an inline node list.
fn build_inline_job(
    inlines: &[Inline],
    ctx: &RenderCtx,
    color: Color32,
    font: FontId,
) -> (LayoutJob, Vec<LinkSpan>, Vec<StrikeSpan>) {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    let mut strikes = Vec::new();
    let mut offset = 0usize;
    for inl in inlines {
        append_inline(
            &mut job,
            &mut links,
            &mut strikes,
            &mut offset,
            inl,
            ctx,
            &base_fmt(font.clone(), color),
        );
    }
    (job, links, strikes)
}

fn append_text(job: &mut LayoutJob, offset: &mut usize, text: &str, fmt: &Fmt, line_h: f32) {
    job.append(text, 0.0, fmt.to_format(line_h));
    *offset += text.chars().count();
}

fn append_inline(
    job: &mut LayoutJob,
    links: &mut Vec<LinkSpan>,
    strikes: &mut Vec<StrikeSpan>,
    offset: &mut usize,
    inl: &Inline,
    ctx: &RenderCtx,
    f: &Fmt,
) {
    let line_h = ctx.line_h(f.font.size);
    match inl {
        Inline::Text(s) => append_text(job, offset, s, f, line_h),
        Inline::SoftBreak => append_text(job, offset, " ", f, line_h),
        Inline::HardBreak => append_text(job, offset, "\n", f, line_h),
        Inline::Code(s) => {
            let mut cf = f.clone();
            cf.font = FontId::monospace(f.font.size - 1.0);
            cf.bg = ctx.theme.c.code_bg;
            cf.color = ctx.theme.c.code_fg;
            append_text(job, offset, s, &cf, line_h);
        }
        Inline::Strong(children) => {
            let mut sf = f.clone();
            sf.color = ctx.theme.c.heading;
            for c in children {
                append_inline(job, links, strikes, offset, c, ctx, &sf);
            }
        }
        Inline::Emphasis(children) => {
            let mut ef = f.clone();
            ef.italic = true;
            for c in children {
                append_inline(job, links, strikes, offset, c, ctx, &ef);
            }
        }
        Inline::Strikethrough(children) => {
            let start = *offset;
            for c in children {
                append_inline(job, links, strikes, offset, c, ctx, f);
            }
            if *offset > start {
                strikes.push(StrikeSpan {
                    start,
                    end: *offset,
                    size: f.font.size,
                    color: f.color,
                });
            }
        }
        Inline::Link { dest, children } => {
            let lf = Fmt {
                color: ctx.theme.c.link,
                underline: true,
                ..f.clone()
            };
            let start = *offset;
            for c in children {
                append_inline(job, links, strikes, offset, c, ctx, &lf);
            }
            links.push(LinkSpan {
                start,
                end: *offset,
                url: dest.clone(),
            });
        }
        Inline::Image { alt, .. } => {
            // Inline (non-standalone) image: textual fallback.
            let mf = Fmt {
                color: ctx.theme.c.muted,
                italic: true,
                ..f.clone()
            };
            append_text(job, offset, &format!("🖼 {alt}"), &mf, line_h);
        }
        Inline::InlineHtml(s) => {
            // Convert whitelisted inline HTML to styled inlines; degrade to
            // monospace text when nothing renderable remains.
            if let Some(inlines) = crate::html::html_inlines(s) {
                for inl in &inlines {
                    append_inline(job, links, strikes, offset, inl, ctx, f);
                }
            } else {
                let mf = Fmt {
                    color: ctx.theme.c.code_fg,
                    font: FontId::monospace(f.font.size - 1.0),
                    ..f.clone()
                };
                append_text(job, offset, s, &mf, line_h);
            }
        }
        Inline::Math(s) => {
            let mf = Fmt {
                color: ctx.theme.c.muted,
                ..f.clone()
            };
            append_text(job, offset, s, &mf, line_h);
        }
        Inline::FootnoteRef(s) => {
            let mf = Fmt {
                color: ctx.theme.c.muted,
                bg: ctx.theme.c.code_bg,
                font: FontId::monospace(f.font.size - 1.0),
                ..f.clone()
            };
            append_text(job, offset, &format!("[^{s}]"), &mf, line_h);
        }
    }
}

fn extract_internal_anchor(dest: &str) -> Option<String> {
    let trimmed = dest.trim();
    let stripped = trimmed.strip_prefix('#')?;
    let raw = stripped.trim();
    if raw.is_empty() {
        return None;
    }
    let decoded = percent_decode(raw);
    let anchor = decoded.trim().to_string();
    if anchor.is_empty() {
        None
    } else {
        Some(anchor)
    }
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.as_bytes().iter().copied().peekable();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next();
            let lo = bytes.next();
            if let (Some(hi), Some(lo)) = (hi, lo) {
                let hex = [hi, lo];
                if let Ok(hex_str) = std::str::from_utf8(&hex) {
                    if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                        out.push(val as char);
                        continue;
                    }
                }
                out.push('%');
                out.push(hi as char);
                out.push(lo as char);
            } else {
                out.push('%');
                if let Some(hi) = hi {
                    out.push(hi as char);
                }
                if let Some(lo) = lo {
                    out.push(lo as char);
                }
            }
        } else {
            out.push(b as char);
        }
    }
    out
}

/// Layout and paint a rich-text job, wiring up clickable link spans and
/// manually-drawn strikethrough lines. `align` shifts single-line jobs
/// horizontally (wrapped text stays left-aligned within the block). Returns
/// the on-screen rect of the painted text.
fn paint_job(
    ui: &mut Ui,
    mut job: LayoutJob,
    links: Vec<LinkSpan>,
    strikes: Vec<StrikeSpan>,
    default: Color32,
    align: Align,
    ctx: &mut RenderCtx,
) -> Rect {
    let wrap_width = ui.available_width().max(20.0);
    job.wrap.max_width = wrap_width;
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let alloc_w = match align {
        Align::Center | Align::Right => wrap_width,
        _ => galley.size().x,
    };
    let (rect, _) = ui.allocate_exact_size(vec2(alloc_w, galley.size().y), Sense::hover());
    let offset = match align {
        Align::Center => ((alloc_w - galley.size().x) / 2.0).max(0.0),
        Align::Right => (alloc_w - galley.size().x).max(0.0),
        _ => 0.0,
    };
    ui.painter()
        .galley(rect.min + vec2(offset, 0.0), galley.clone(), default);

    // Strikethrough: a line through the *optical* middle of the glyphs, i.e.
    // the center of the row's ink bounds (`mesh_bounds` is the union of the
    // tight glyph sprites). Metric-based positioning (egui's built-in strike
    // sits at the logical box center) lands below the visual middle with CJK
    // fallback fonts whose ascent is much larger than the latin one.
    if !strikes.is_empty() {
        let mut row_start = 0usize;
        for placed in &galley.rows {
            let row_chars = placed.row.glyphs.len() + usize::from(placed.row.ends_with_newline);
            let row_end = row_start + row_chars;
            if placed.row.glyphs.is_empty() {
                row_start = row_end;
                continue;
            }
            let ink = placed.row.visuals.mesh_bounds;
            let strike_y = if ink.is_finite() && ink.height() > 0.0 {
                rect.min.y + placed.pos.y + ink.center().y
            } else {
                rect.min.y + placed.rect().center().y
            };
            for st in &strikes {
                let s = st.start.max(row_start);
                let e = st.end.min(row_end);
                if s >= e {
                    continue;
                }
                let sx = row_glyph_x(placed, s - row_start, rect.min.x + offset);
                let ex = row_glyph_x(placed, e - row_start, rect.min.x + offset);
                ui.painter().line_segment(
                    [pos2(sx, strike_y), pos2(ex, strike_y)],
                    Stroke::new((st.size / 12.0).max(1.0), st.color),
                );
            }
            row_start = row_end;
        }
    }

    for link in &links {
        if link.start > link.end || link.end > galley.text().chars().count() {
            continue;
        }
        let s = galley.pos_from_cursor(CCursor::new(link.start));
        let e = galley.pos_from_cursor(CCursor::new(link.end));
        // A union rect; for links that wrap this may span a couple of rows, but
        // it keeps links reliably clickable.
        let hit = Rect::from_min_max(
            pos2(rect.min.x + offset + s.min.x, rect.min.y + s.min.y),
            pos2(rect.min.x + offset + e.max.x, rect.min.y + e.max.y),
        );
        let id = ui.id().with(("link", &link.url, link.start));
        let resp = ui.interact(hit, id, Sense::click());
        if resp.clicked() {
            if let Some(anchor) = extract_internal_anchor(&link.url) {
                ctx.clicked_anchor = Some(anchor);
            } else {
                open_url(&link.url);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }
    rect
}

// ---------------------------------------------------------------------------
// Code blocks (multi-line highlight, UI-MD-008)
// ---------------------------------------------------------------------------

fn render_code_block(ui: &mut Ui, lang: Option<&str>, text: &str, ctx: &mut RenderCtx) {
    let body_font = FontId::monospace(ctx.font_size - 1.0);
    let body_height = (ctx.font_size - 1.0) * ctx.line_height;
    let outer = egui::Frame::new()
        .fill(ctx.theme.c.code_bg)
        .stroke(Stroke::new(1.0, ctx.theme.c.table_border))
        .corner_radius(ctx.metrics.radius_md)
        .shadow(ctx.metrics.shadow_sm)
        .inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 8,
        });
    outer.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let header_h = 32.0;
        let header_bg = ctx.theme.c.surface_hover.gamma_multiply(0.9);
        let lang_label = lang
            .map(|l| l.to_ascii_uppercase())
            .unwrap_or_else(|| "CODE".to_string());
        egui::Frame::new()
            .fill(header_bg)
            .corner_radius(egui::CornerRadius {
                nw: ctx.metrics.radius_md as u8,
                ne: ctx.metrics.radius_md as u8,
                sw: 0,
                se: 0,
            })
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_max_height(header_h);
                ui.set_min_height(header_h);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(lang_label)
                            .size(10.0)
                            .color(ctx.theme.c.muted)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let copy_id = egui::Id::new(("code_copy", text));
                        let copied_until = ui
                            .ctx()
                            .data_mut(|d| d.get_temp::<Instant>(copy_id))
                            .unwrap_or_else(|| Instant::now() - Duration::from_secs(10));
                        let is_copied = Instant::now() < copied_until;
                        if is_copied {
                            let remaining = copied_until.saturating_duration_since(Instant::now());
                            ui.ctx().request_repaint_after(remaining);
                        }
                        let label = if is_copied { "已复制 ✔" } else { "复制" };
                        let icon = if is_copied {
                            egui_phosphor::regular::CHECK
                        } else {
                            egui_phosphor::regular::COPY
                        };
                        let txt_color = if is_copied {
                            ctx.theme.c.success
                        } else {
                            ctx.theme.c.muted
                        };
                        let btn = egui::Button::new(
                            RichText::new(format!("{icon} {label}"))
                                .size(11.0)
                                .color(txt_color),
                        )
                        .frame(false);
                        let w = if is_copied { 82.0 } else { 64.0 };
                        let resp = ui.add_sized(vec2(w, 22.0), btn);
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            ui.ctx().copy_text(text.to_owned());
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(copy_id, Instant::now() + Duration::from_secs(2))
                            });
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
        let hairline = ctx.metrics.hairline.max(1.0);
        let (sep_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hairline), Sense::hover());
        ui.painter().rect_filled(sep_rect, 0.0, ctx.theme.c.hr);
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let fg = ctx.theme.c.foreground;
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let lines: Vec<Line> = ctx.hl.code_block(lang, text);
                    for spans in lines {
                        if spans.iter().all(|s| s.text.is_empty()) {
                            ui.add(
                                egui::Label::new(RichText::new(" ").monospace()).selectable(false),
                            );
                            continue;
                        }
                        let mut job = LayoutJob::default();
                        job.wrap.max_width = f32::INFINITY;
                        for sp in spans {
                            let color = if sp.color == Color32::TRANSPARENT {
                                fg
                            } else {
                                sp.color
                            };
                            job.append(
                                &sp.text,
                                0.0,
                                TextFormat {
                                    font_id: body_font.clone(),
                                    line_height: Some(body_height),
                                    color,
                                    italics: sp.style & 2 != 0,
                                    ..Default::default()
                                },
                            );
                        }
                        ui.add(egui::Label::new(job).selectable(true));
                    }
                });
            });
    });
}

// ---------------------------------------------------------------------------
// Lists & task lists (UI-MD-005)
// ---------------------------------------------------------------------------

fn render_list(
    ui: &mut Ui,
    ordered: bool,
    start: u64,
    items: &[Vec<Block>],
    ctx: &mut RenderCtx,
    depth: usize,
) {
    let indent = 14.0 * depth as f32;
    for (i, item) in items.iter().enumerate() {
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            ui.allocate_ui_with_layout(
                vec2(20.0, ctx.line_h(ctx.font_size)),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.add_space(7.0);
                    if ordered {
                        let marker = format!("{}.", start + i as u64);
                        ui.label(RichText::new(marker).size(12.0).color(ctx.theme.c.muted));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(20.0, ctx.line_h(ctx.font_size) - 2.0),
                            Sense::hover(),
                        );
                        let center = pos2(rect.center().x, rect.min.y + 9.0);
                        ui.painter().circle_filled(center, 2.5, ctx.theme.c.link);
                    }
                },
            );
            ui.vertical(|ui| {
                render_blocks(ui, item, ctx, depth + 1);
            });
        });
        if i + 1 < items.len() {
            ui.add_space(4.0);
        }
    }
}

fn render_task_list(
    ui: &mut Ui,
    checked: &[bool],
    items: &[Vec<Block>],
    ctx: &mut RenderCtx,
    depth: usize,
) {
    let indent = 14.0 * depth as f32;
    for (i, item) in items.iter().enumerate() {
        let chk = checked.get(i).copied().unwrap_or(false);
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            ui.allocate_ui_with_layout(
                vec2(20.0, ctx.line_h(ctx.font_size)),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.add_space(7.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    if chk {
                        ui.painter()
                            .rect_filled(rect, 4.0, ctx.theme.c.link.gamma_multiply(0.12));
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            Stroke::new(1.4, ctx.theme.c.link),
                            egui::StrokeKind::Inside,
                        );
                        let galley = ui.fonts_mut(|f| {
                            f.layout_no_wrap(
                                "✔".to_owned(),
                                FontId::proportional(10.0),
                                ctx.theme.c.link,
                            )
                        });
                        let mut shift = egui::Vec2::ZERO;
                        if let Some(placed) = galley.rows.first() {
                            let ink = placed.row.visuals.mesh_bounds;
                            if ink.is_finite() && ink.height() > 0.0 {
                                let ink_center = placed.pos + ink.center().to_vec2();
                                shift = galley.rect.center() - ink_center;
                            }
                        }
                        let pos = rect.center() - galley.size() / 2.0 + shift;
                        ui.painter().galley(pos, galley, ctx.theme.c.link);
                        let shadow_rect = rect.expand(0.5);
                        ui.painter().rect_stroke(
                            shadow_rect,
                            4.0,
                            Stroke::new(1.0, Color32::from_black_alpha(12)),
                            egui::StrokeKind::Inside,
                        );
                    } else {
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            Stroke::new(1.4, ctx.theme.c.muted.gamma_multiply(0.6)),
                            egui::StrokeKind::Inside,
                        );
                    }
                },
            );
            ui.vertical(|ui| {
                render_blocks(ui, item, ctx, depth + 1);
            });
        });
        if i + 1 < items.len() {
            ui.add_space(4.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Blockquote (UI-MD-009)
// ---------------------------------------------------------------------------

fn render_blockquote(ui: &mut Ui, blocks: &[Block], ctx: &mut RenderCtx, depth: usize) {
    let frame = egui::Frame::new()
        .fill(ctx.theme.c.quote_bg)
        .corner_radius(ctx.metrics.radius_md)
        .inner_margin(egui::Margin::symmetric(16, 10));
    let resp = frame.show(ui, |ui| {
        render_blocks(ui, blocks, ctx, depth);
    });
    let rect = resp.response.rect;
    ui.painter().rect_filled(
        Rect::from_min_max(rect.min, pos2(rect.min.x + 3.5, rect.max.y)),
        1.5,
        ctx.theme.c.blockquote_bar,
    );
    let inner_shadow = Rect::from_min_max(
        pos2(rect.min.x + 3.5, rect.min.y),
        pos2(rect.min.x + 4.5, rect.max.y),
    );
    ui.painter()
        .rect_filled(inner_shadow, 0.0, Color32::from_black_alpha(6));
}

// ---------------------------------------------------------------------------
// Tables (UI-MD-006)
// ---------------------------------------------------------------------------

fn render_table(
    ui: &mut Ui,
    header: &[Vec<Inline>],
    align: &[Align],
    rows: &[Vec<Vec<Inline>>],
    ctx: &mut RenderCtx,
) {
    let ncols = header
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if ncols == 0 {
        return;
    }
    let font = FontId::proportional(ctx.font_size);
    let pad_x = 7.0;
    let pad_y = 5.0;
    let row_h = ctx.line_h(ctx.font_size) + 2.0 * pad_y;
    let mut col_w = vec![0f32; ncols];
    let measure = |ui: &mut Ui, t: &str| -> f32 {
        ui.fonts_mut(|f| {
            let galley = f.layout_no_wrap(t.to_string(), font.clone(), ctx.theme.c.foreground);
            galley.size().x
        })
    };
    for (i, h) in header.iter().enumerate() {
        col_w[i] = col_w[i].max(measure(ui, &inline_text(h)));
    }
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            col_w[i] = col_w[i].max(measure(ui, &inline_text(cell)));
        }
    }
    let cell_w: Vec<f32> = col_w.iter().map(|w| w + 2.0 * pad_x).collect();
    let total_w: f32 = cell_w.iter().sum();
    let avail = ui.available_width().max(20.0);
    let needs_scroll = total_w > avail;
    let total_h = (1 + rows.len()) as f32 * row_h;
    let outer = egui::Frame::new()
        .corner_radius(ctx.metrics.radius_md)
        .stroke(Stroke::new(1.0, ctx.theme.c.table_border))
        .shadow(ctx.metrics.shadow_sm)
        .inner_margin(egui::Margin::same(0));
    let paint_content = |ui: &mut Ui, alloc_w: f32| {
        let origin = ui.cursor().min;
        let (table_rect, _) = ui.allocate_exact_size(vec2(alloc_w, total_h), Sense::hover());
        let mut x_offsets: Vec<f32> = Vec::with_capacity(ncols);
        let mut x = 0.0;
        for w in &cell_w {
            x_offsets.push(x);
            x += w;
        }
        let mut y = origin.y;
        paint_table_row(
            ui, ctx, header, align, origin.x, y, &x_offsets, &cell_w, row_h, true, false,
        );
        y += row_h;
        for (ri, row) in rows.iter().enumerate() {
            paint_table_row(
                ui,
                ctx,
                row,
                align,
                origin.x,
                y,
                &x_offsets,
                &cell_w,
                row_h,
                false,
                ri % 2 == 1,
            );
            y += row_h;
        }
        let right = origin.x + total_w;
        let accent_line = Rect::from_min_max(
            pos2(origin.x, origin.y + row_h - 2.0),
            pos2(right, origin.y + row_h),
        );
        ui.painter()
            .rect_filled(accent_line, 0.0, ctx.theme.c.link.gamma_multiply(0.05));
        ui.painter().line_segment(
            [
                pos2(origin.x, origin.y + row_h),
                pos2(right, origin.y + row_h),
            ],
            Stroke::new(1.0, ctx.theme.c.table_border),
        );
        let _ = table_rect;
    };
    if needs_scroll {
        // Keep viewport at `avail` so `max = total_w - viewport` is exact; otherwise
        // the default auto_shrink makes the thumb stop short and the last column
        // stays clipped.
        outer.show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(format!("table-{}-{}", header.len(), rows.len()))
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| paint_content(ui, total_w));
        });
    } else {
        outer.show(ui, |ui| paint_content(ui, total_w));
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_table_row(
    ui: &mut Ui,
    ctx: &RenderCtx,
    cells: &[Vec<Inline>],
    align: &[Align],
    origin_x: f32,
    top_y: f32,
    x_offsets: &[f32],
    cell_w: &[f32],
    row_h: f32,
    is_header: bool,
    striped: bool,
) {
    let header_font = FontId::proportional(12.0);
    let body_font = FontId::proportional(ctx.font_size);
    let is_hdr = is_header;
    for (i, xo) in x_offsets.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(origin_x + xo, top_y), vec2(cell_w[i], row_h));
        let bg = if is_hdr {
            ctx.theme.c.table_header_bg
        } else if striped {
            ctx.theme.c.surface_hover.gamma_multiply(0.4)
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg);
        }
        let text = cells.get(i).map(|c| inline_text(c)).unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        let a = align.get(i).copied().unwrap_or(Align::None);
        let font = if is_hdr { &header_font } else { &body_font };
        let text_w = ui.fonts_mut(|f| {
            let g = f.layout_no_wrap(text.clone(), font.clone(), ctx.theme.c.foreground);
            g.size().x
        });
        let text_h = font.size;
        let y = top_y + (row_h - text_h) / 2.0 - 1.0;
        let x = match a {
            Align::Right => xo + cell_w[i] - 7.0 - text_w,
            Align::Center => xo + (cell_w[i] - text_w) / 2.0,
            _ => xo + 7.0,
        };
        let color = if is_hdr {
            ctx.theme.c.heading
        } else {
            ctx.theme.c.foreground
        };
        let rich = if is_hdr {
            RichText::new(&text).size(12.0).strong().color(color)
        } else {
            RichText::new(&text).size(ctx.font_size).color(color)
        };
        let _ = rich;
        ui.painter().text(
            pos2(origin_x + x, y),
            egui::Align2::LEFT_TOP,
            text,
            font.clone(),
            color,
        );
    }
}

// ---------------------------------------------------------------------------
// Images (UI-MD-010)
// ---------------------------------------------------------------------------

fn render_image(ui: &mut Ui, src: &str, alt: &str, width: Option<f32>, ctx: &mut RenderCtx) {
    let avail = ui.available_width().max(20.0);
    let max_w = avail.min(ctx.content_width);
    match ctx.images.texture_for(ui.ctx(), src) {
        Some((tex, natural)) => {
            let ratio = (natural.y / natural.x.max(1.0)).clamp(0.1, 2.0);
            let w = match width {
                Some(w) => w.min(max_w).max(1.0),
                None => natural.x.min(max_w),
            };
            let h = (w * ratio).max(1.0);
            let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let shadow_rect = rect.expand(1.0);
            ui.painter().rect_filled(
                shadow_rect,
                ctx.metrics.radius_md,
                Color32::from_black_alpha(10),
            );
            ui.painter()
                .rect_filled(rect, ctx.metrics.radius_md, ctx.theme.c.image_bg);
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.painter().rect_stroke(
                rect,
                ctx.metrics.radius_md,
                Stroke::new(1.0, ctx.theme.c.table_border.gamma_multiply(0.2)),
                egui::StrokeKind::Inside,
            );
            if !alt.is_empty() {
                ui.add_space(6.0);
                ui.allocate_ui_with_layout(
                    vec2(w, 14.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(alt)
                                .size(11.0)
                                .italics()
                                .color(ctx.theme.c.muted),
                        );
                    },
                );
            }
        }
        None => {
            let (state, fg) = if ctx.images.is_pending(src) {
                ("加载中…", ctx.theme.c.muted)
            } else {
                ("图片加载失败", ctx.theme.c.hr)
            };
            let is_remote = src.starts_with("http://") || src.starts_with("https://");
            egui::Frame::new()
                .fill(ctx.theme.c.image_bg)
                .stroke(Stroke::new(1.0, ctx.theme.c.table_border))
                .corner_radius(ctx.metrics.radius_md)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🖼").size(16.0).color(ctx.theme.c.muted));
                        ui.label(RichText::new(state).color(fg));
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(if is_remote {
                                format!("远程 {src}")
                            } else {
                                src.to_string()
                            })
                            .small()
                            .color(ctx.theme.c.muted),
                        );
                    });
                    if !alt.is_empty() {
                        ui.label(
                            RichText::new(alt)
                                .small()
                                .italics()
                                .color(ctx.theme.c.muted),
                        );
                    }
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn inline_text(inlines: &[Inline]) -> String {
    inlines.iter().filter_map(inline_leaf).collect()
}

fn inline_leaf(i: &Inline) -> Option<String> {
    match i {
        Inline::Text(s) => Some(s.clone()),
        Inline::Code(s) => Some(s.clone()),
        Inline::Strong(v) | Inline::Emphasis(v) | Inline::Strikethrough(v) => Some(inline_text(v)),
        Inline::Link { children, .. } => Some(inline_text(children)),
        Inline::Image { alt, .. } => Some(alt.clone()),
        Inline::SoftBreak | Inline::HardBreak => Some(" ".to_string()),
        Inline::InlineHtml(s) | Inline::Math(s) => Some(s.clone()),
        Inline::FootnoteRef(s) => Some(format!("[^{s}]")),
    }
}

/// Extract the visible text content from a raw HTML fragment (strip tags and
/// decode the most common entities). No scripting is executed.
fn html_text(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

/// X coordinate (absolute) of character `idx` within a galley row; an index
/// past the last glyph resolves to the row's trailing edge.
fn row_glyph_x(placed: &egui::epaint::text::PlacedRow, idx: usize, galley_x: f32) -> f32 {
    let glyphs = &placed.row.glyphs;
    if glyphs.is_empty() {
        return galley_x + placed.pos.x;
    }
    if idx < glyphs.len() {
        galley_x + glyphs[idx].pos.x
    } else {
        let last = &glyphs[glyphs.len() - 1];
        galley_x + last.pos.x + last.advance_width
    }
}

fn open_url(url: &str) {
    let lower = url.trim_start().to_ascii_lowercase();
    let allowed = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:");
    if !allowed {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").args(["--", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open")
        .args(["--", url])
        .spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
}
