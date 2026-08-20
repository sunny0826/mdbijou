//! Simple Markdown source editor: edit/preview toggle target.
//!
//! Built on egui's native `TextEdit` with a custom layouter that syntax-highlights
//! each line (Markdown for non-fenced lines, the fenced language inside code
//! fences). The editor soft-wraps to the window width (no horizontal
//! scrollbar); line numbers, the gutter separator and the current-line
//! highlight track the wrapped row heights so they stay aligned
//! (UI-MD-013); `highlight` / `tab_size` config take real effect (UI-MD-015)
//! and the layout is built once per frame without cloning the whole document.

use crate::config::Config;
use crate::document::Document;
use crate::highlight::Highlighter;
use crate::theme::{Metrics, Theme};
use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::text_edit::TextEditState;
use egui::{Color32, CornerRadius, FontId, Margin, Pos2, Rect, Sense, Stroke, TextEdit, Ui};
use egui_phosphor::regular;

/// Whether a line is Markdown or part of a fenced code block.
#[derive(Debug, Clone)]
enum LineRole {
    Markdown,
    /// A fenced-code line with the fence language (or None if plain fence).
    Code {
        lang: Option<String>,
    },
}

/// Compute the role of each line (fence-aware).
fn line_roles(text: &str) -> Vec<LineRole> {
    let mut roles = Vec::new();
    let mut in_fence = false;
    let mut fence_lang: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if !in_fence {
                let lang = trimmed[3..].trim().to_string();
                let lang = if lang.is_empty() { None } else { Some(lang) };
                roles.push(LineRole::Code { lang: lang.clone() });
                fence_lang = lang;
                in_fence = true;
            } else {
                roles.push(LineRole::Markdown);
                in_fence = false;
                fence_lang = None;
            }
        } else if in_fence {
            roles.push(LineRole::Code {
                lang: fence_lang.clone(),
            });
        } else {
            roles.push(LineRole::Markdown);
        }
    }
    roles
}

pub struct Editor<'a> {
    pub cfg: &'a Config,
    pub theme: &'a Theme,
}

pub struct EditorResult {
    pub changed: bool,
    /// 1-based (line, column) of the cursor, when a cursor exists.
    pub cursor: Option<(usize, usize)>,
}

impl<'a> Editor<'a> {
    pub fn new(cfg: &'a Config, theme: &'a Theme) -> Self {
        Self { cfg, theme }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        doc: &mut Document,
        hl: &mut dyn Highlighter,
    ) -> EditorResult {
        let fg = self.theme.c.foreground;
        let bg = self.theme.c.background;
        let selection = self.theme.c.selection_bg;
        let font_size = self.cfg.editor_font_size;
        let mono = FontId::monospace(font_size);
        // Shared line height so line numbers stay aligned with source rows.
        let row_h = (font_size * self.cfg.line_height).max(font_size + 4.0);

        let highlight_on = self.cfg.highlight;

        let te_id = ui.id().with("md_editor_source");
        let mut te_changed = false;

        // ---- floating Markdown shortcut bar (Phase 2b) ----
        let metrics = Metrics::scaled(ui.ctx().pixels_per_point());
        // Wrap in a Frame that feels like paper: surface fill, hairline hr,
        // radius_md, symmetric padding, shadow_sm.
        let mut pending_insert: Option<String> = None;
        egui::Frame::new()
            .fill(self.theme.c.surface)
            .stroke(Stroke::new(metrics.hairline, self.theme.c.hr))
            .corner_radius(CornerRadius::same(metrics.radius_md as u8))
            .inner_margin(Margin::symmetric(6, 4))
            .shadow(metrics.shadow_sm)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                    let btns: [(&str, &str, &str); 5] = [
                        (regular::TEXT_H_ONE, "插入标题 (# )", "# "),
                        (regular::TEXT_B, "插入加粗 (**)", "**粗体**"),
                        (regular::TEXT_ITALIC, "插入斜体 (*)", "*斜体*"),
                        (
                            regular::LINK,
                            "插入链接 []()",
                            "[文本](https://example.com)",
                        ),
                        (regular::CODE, "插入行内代码 ``", "`代码`"),
                    ];
                    for (glyph, tip, snippet) in btns {
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(4),
                                self.theme.c.code_bg,
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(4),
                                self.theme.c.code_bg.gamma_multiply(0.9),
                            );
                        }
                        let icon_color = if resp.hovered() {
                            fg
                        } else {
                            self.theme.c.muted
                        };
                        crate::app::paint_optical_centered_text(
                            ui.painter(),
                            rect,
                            glyph,
                            FontId::proportional(12.0),
                            icon_color,
                        );
                        if resp.clicked() {
                            pending_insert = Some(snippet.to_string());
                        }
                        if resp.hovered() {
                            resp.on_hover_ui(|ui| {
                                let font = egui::FontId::proportional(12.0);
                                let col = ui.visuals().text_color();
                                let g = ui.fonts_mut(|f| {
                                    f.layout_no_wrap(tip.to_owned(), font.clone(), col)
                                });
                                let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                                crate::app::paint_optical_centered_text(
                                    ui.painter(),
                                    r,
                                    tip,
                                    font,
                                    col,
                                );
                            });
                        }
                    }
                });
            });
        if let Some(snippet) = pending_insert {
            let total_chars = doc.text.chars().count();
            let raw_idx = egui::TextEdit::load_state(ui.ctx(), te_id)
                .and_then(|s| s.cursor.char_range())
                .map(|r| r.primary.index)
                .unwrap_or(total_chars);
            let char_idx = raw_idx.min(total_chars);
            let byte_idx = doc
                .text
                .char_indices()
                .nth(char_idx)
                .map(|(i, _)| i)
                .unwrap_or(doc.text.len());
            debug_assert!(doc.text.is_char_boundary(byte_idx));
            doc.text.insert_str(byte_idx, &snippet);
            doc.dirty = true;
            te_changed = true;
            // Move cursor to end of inserted snippet.
            let new_char_idx = char_idx + snippet.chars().count();
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), te_id) {
                let cc = CCursor::new(new_char_idx);
                state.cursor.set_char_range(Some(CCursorRange::one(cc)));
                state.store(ui.ctx(), te_id);
                ui.ctx().memory_mut(|m| m.request_focus(te_id));
            } else {
                // No prior state: create one with cursor at new position.
                let mut state = TextEditState::default();
                let cc = CCursor::new(new_char_idx);
                state.cursor.set_char_range(Some(CCursorRange::one(cc)));
                state.store(ui.ctx(), te_id);
                ui.ctx().memory_mut(|m| m.request_focus(te_id));
            }
        }

        ui.add_space(6.0);

        // Wrap width actually used by the TextEdit galley in the previous
        // frame; used to predict wrapped row heights for the gutter.
        let wrap_id = te_id.with("wrap_w");
        let prev_wrap_w = ui
            .ctx()
            .data_mut(|d| d.get_temp::<f32>(wrap_id))
            .unwrap_or(0.0);

        // Wrapped row count per logical line, for the gutter and the
        // current-line highlight. Heights are integer multiples of `row_h`.
        let display_lines: Vec<String> = doc
            .text
            .split('\n')
            .map(|l| {
                if self.cfg.tab_size > 0 {
                    l.replace('\t', &" ".repeat(self.cfg.tab_size))
                } else {
                    l.to_string()
                }
            })
            .collect();
        let mut rows_per_line: Vec<usize> = Vec::with_capacity(display_lines.len());
        for line in &display_lines {
            rows_per_line.push(line_rows(ui, line, &mono, row_h, prev_wrap_w));
        }

        // Vertically center the source when it is shorter than the viewport;
        // taller documents stay top-aligned and scrollable.
        let total_rows: usize = rows_per_line.iter().sum::<usize>().max(1);
        let content_h = total_rows as f32 * row_h + 12.0; // + TextEdit vertical margins
        let top_pad = ((ui.available_height() - content_h) / 2.0).max(0.0);

        // Current line for gutter highlight and background highlight.
        let current_line = if highlight_on {
            egui::TextEdit::load_state(ui.ctx(), te_id)
                .and_then(|s| s.cursor.char_range())
                .map(|r| count_newlines(&doc.text, r.primary.index))
                .unwrap_or(0)
        } else {
            0
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(top_pad);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 2.0);

                    if self.cfg.show_line_numbers {
                        let gutter_w = 44.0;
                        let (gutter_rect, _) = ui.allocate_exact_size(
                            egui::vec2(gutter_w, row_h * total_rows as f32),
                            Sense::hover(),
                        );
                        let mono_gutter = FontId::monospace(12.0);
                        let muted_soft = self.theme.c.muted.gamma_multiply(0.75);
                        let mut y = gutter_rect.min.y;
                        for (i, &rows) in rows_per_line.iter().enumerate() {
                            let is_current = highlight_on && i == current_line;
                            let color = if is_current { fg } else { muted_soft };
                            let text = (i + 1).to_string();
                            let galley = ui.fonts_mut(|f| {
                                f.layout_no_wrap(text.clone(), mono_gutter.clone(), color)
                            });
                            let mut shift_y = 0.0;
                            if let Some(placed) = galley.rows.first() {
                                let ink = placed.row.visuals.mesh_bounds;
                                if ink.is_finite() && ink.height() > 0.0 {
                                    let ink_center_y = placed.pos.y + ink.center().y;
                                    shift_y = galley.rect.center().y - ink_center_y;
                                }
                            }
                            let slot_center_y = y + row_h * rows as f32 * 0.5;
                            let gy = slot_center_y - galley.size().y / 2.0 + shift_y;
                            let gx = gutter_rect.max.x - galley.size().x;
                            ui.painter().galley(egui::pos2(gx, gy), galley, color);
                            y += row_h * rows as f32;
                        }
                        // Gutter separator line spanning the full source height.
                        let x = gutter_rect.max.x + 4.0;
                        ui.painter().line_segment(
                            [
                                Pos2::new(x, gutter_rect.min.y),
                                Pos2::new(x, gutter_rect.max.y),
                            ],
                            Stroke::new(metrics.hairline, self.theme.c.hr.gamma_multiply(0.7)),
                        );
                    }

                    // Current-line highlight (behind the text), spanning all
                    // wrapped rows of the logical line.
                    if highlight_on {
                        let y = ui.cursor().top()
                            + row_h
                                * rows_per_line[..current_line.min(rows_per_line.len())]
                                    .iter()
                                    .sum::<usize>() as f32;
                        let h =
                            row_h * rows_per_line.get(current_line).copied().unwrap_or(1) as f32;
                        let hl_rect = Rect::from_min_size(
                            Pos2::new(ui.cursor().min.x, y),
                            egui::vec2(ui.available_width(), h),
                        );
                        ui.painter().rect_filled(
                            hl_rect,
                            CornerRadius::same(3),
                            self.theme.c.code_bg.gamma_multiply(0.6),
                        );
                    }

                    ui.style_mut().visuals.override_text_color = Some(fg);
                    ui.style_mut().visuals.selection.bg_fill = selection;
                    ui.style_mut().visuals.selection.stroke = Stroke::new(1.0, fg);

                    let ctx2 = ui.ctx().clone();
                    let mut layouter = |ui: &Ui,
                                        buf: &dyn egui::TextBuffer,
                                        wrap_w: f32|
                     -> std::sync::Arc<egui::Galley> {
                        // Record the real wrap width so the next frame's
                        // gutter can predict wrapped row heights exactly.
                        ctx2.data_mut(|d| d.insert_temp(wrap_id, wrap_w));
                        let job = editor_job(
                            buf.as_str(),
                            hl,
                            fg,
                            mono.clone(),
                            row_h,
                            self.cfg.tab_size,
                            highlight_on,
                            wrap_w,
                        );
                        ui.fonts_mut(|f| f.layout_job(job))
                    };

                    let te = TextEdit::multiline(&mut doc.text)
                        .font(mono.clone())
                        .desired_width(ui.available_width())
                        .layouter(&mut layouter)
                        .frame(true)
                        .background_color(bg)
                        .text_color(fg)
                        .margin(Margin::symmetric(10, 8));
                    let out = te.show(ui);
                    if out.response.changed() {
                        te_changed = true;
                    }
                    if te_changed && !doc.dirty {
                        doc.dirty = true;
                    }
                    if out.response.changed() {
                        doc.dirty = true;
                    }
                });
            });
        let cursor = egui::TextEdit::load_state(ui.ctx(), te_id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.primary.index)
            .map(|idx| {
                let line = count_newlines(&doc.text, idx);
                let mut col = 0;
                for c in doc.text.chars().take(idx) {
                    if c == '\n' {
                        col = 0;
                    } else {
                        col += 1;
                    }
                }
                (line + 1, col + 1)
            });
        EditorResult {
            changed: te_changed,
            cursor,
        }
    }
}

/// How many lines precede `char_idx` (for current-line highlight).
fn count_newlines(text: &str, char_idx: usize) -> usize {
    text.chars().take(char_idx).filter(|c| *c == '\n').count()
}

/// Predict how many visual rows a logical line occupies after soft-wrapping
/// to `wrap_w`. egui caches galleys, so re-measuring unchanged lines is cheap.
fn line_rows(ui: &Ui, text: &str, mono: &FontId, row_h: f32, wrap_w: f32) -> usize {
    if wrap_w <= 1.0 {
        return 1;
    }
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_w;
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: mono.clone(),
            line_height: Some(row_h),
            ..Default::default()
        },
    );
    ui.fonts_mut(|f| f.layout_job(job)).rows.len().max(1)
}

/// Build the highlighted layout job for the whole buffer.
#[allow(clippy::too_many_arguments)]
fn editor_job(
    text: &str,
    hl: &mut dyn Highlighter,
    fg: Color32,
    mono: FontId,
    line_h: f32,
    tab_size: usize,
    highlight_on: bool,
    wrap_w: f32,
) -> LayoutJob {
    let roles = line_roles(text);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_w.max(20.0);
    for (idx, line) in lines.iter().enumerate() {
        let role = roles.get(idx).cloned().unwrap_or(LineRole::Markdown);
        let display = if tab_size > 0 {
            line.replace('\t', &" ".repeat(tab_size))
        } else {
            line.to_string()
        };
        if highlight_on {
            match role {
                LineRole::Markdown => {
                    for sp in hl.markdown_line(&display) {
                        append_span(
                            &mut job,
                            &sp.text,
                            if sp.color == Color32::TRANSPARENT {
                                fg
                            } else {
                                sp.color
                            },
                            &mono,
                            line_h,
                            sp.style,
                        );
                    }
                }
                LineRole::Code { lang } => {
                    for sp in hl.code_line(lang.as_deref(), &display) {
                        append_span(
                            &mut job,
                            &sp.text,
                            if sp.color == Color32::TRANSPARENT {
                                fg
                            } else {
                                sp.color
                            },
                            &mono,
                            line_h,
                            sp.style,
                        );
                    }
                }
            }
        } else {
            job.append(
                &display,
                0.0,
                TextFormat {
                    font_id: mono.clone(),
                    line_height: Some(line_h),
                    color: fg,
                    ..Default::default()
                },
            );
        }
        if idx < lines.len() - 1 {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: mono.clone(),
                    line_height: Some(line_h),
                    color: fg,
                    ..Default::default()
                },
            );
        }
    }
    job
}

fn append_span(
    job: &mut LayoutJob,
    text: &str,
    color: Color32,
    mono: &FontId,
    line_h: f32,
    style_flags: u8,
) {
    if text.is_empty() {
        return;
    }
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: mono.clone(),
            line_height: Some(line_h),
            color,
            italics: style_flags & 2 != 0,
            underline: if style_flags & 4 != 0 {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_text(input: &str, highlight_on: bool) -> String {
        let th = crate::theme::builtin("github-light").expect("builtin theme");
        let mut hl = crate::highlight::new_highlighter(&th);
        let job = editor_job(
            input,
            hl.as_mut(),
            th.c.foreground,
            FontId::monospace(14.0),
            20.0,
            4,
            highlight_on,
            800.0,
        );
        job.text
    }

    #[test]
    fn editor_job_preserves_fullwidth_punctuation_markdown() {
        let input = "你好，世界！这是测试（中文）【标点】《引号》“双引”‘单引’。；：、？";
        assert_eq!(job_text(input, true), input);
    }

    #[test]
    fn editor_job_preserves_fullwidth_punctuation_plain() {
        let input = "，。！？（）【】《》“”''；：、";
        assert_eq!(job_text(input, false), input);
    }

    #[test]
    fn editor_job_preserves_fullwidth_in_fenced_code() {
        let input = "```rust\nlet s = \"你好，世界！\";\n```";
        assert_eq!(job_text(input, true), input);
    }

    #[test]
    fn editor_job_preserves_fullwidth_across_multiple_lines() {
        let input = "第一行，标点！\n第二行（中文）？\n第三行《引号》";
        assert_eq!(job_text(input, true), input);
    }
}
