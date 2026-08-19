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
use crate::theme::Theme;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextEdit, Ui};

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
                        let mut y = gutter_rect.min.y;
                        for (i, &rows) in rows_per_line.iter().enumerate() {
                            ui.painter().text(
                                Pos2::new(gutter_rect.max.x, y + row_h * 0.5),
                                Align2::RIGHT_CENTER,
                                (i + 1).to_string(),
                                mono.clone(),
                                self.theme.c.muted,
                            );
                            y += row_h * rows as f32;
                        }
                        // Gutter separator line spanning the full source height.
                        let x = gutter_rect.max.x + 4.0;
                        ui.painter().line_segment(
                            [
                                Pos2::new(x, gutter_rect.min.y),
                                Pos2::new(x, gutter_rect.max.y),
                            ],
                            Stroke::new(1.0, self.theme.c.table_border),
                        );
                    }

                    // Current-line highlight (behind the text), spanning all
                    // wrapped rows of the logical line.
                    let current_line = if highlight_on {
                        egui::TextEdit::load_state(ui.ctx(), te_id)
                            .and_then(|s| s.cursor.char_range())
                            .map(|r| count_newlines(&doc.text, r.primary.index))
                            .unwrap_or(0)
                    } else {
                        0
                    };
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
                        ui.painter().rect_filled(hl_rect, 0.0, self.theme.c.code_bg);
                    }

                    ui.style_mut().visuals.override_text_color = Some(fg);
                    ui.style_mut().visuals.selection.bg_fill = selection;
                    ui.style_mut().visuals.selection.stroke = Stroke::new(1.0, selection);

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
                        .margin(egui::Margin::symmetric(8, 6));
                    let out = te.show(ui);
                    te_changed = out.response.changed();
                    if te_changed {
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
