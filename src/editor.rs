//! Simple Markdown source editor: edit/preview toggle target.
//!
//! Built on egui's native `TextEdit` with a custom layouter that syntax-highlights
//! each line (Markdown for non-fenced lines, the fenced language inside code
//! fences) — the design's "TextEdit + highlight layering" fallback. Uses the same
//! highlighter as the preview so colors stay consistent.

use crate::config::Config;
use crate::document::Document;
use crate::highlight::Highlighter;
use crate::theme::Theme;
use egui::{text::LayoutJob, text::TextFormat, Align, Color32, FontId, RichText, TextEdit, Ui};

/// Whether a line is Markdown or part of a fenced code block.
#[derive(Debug, Clone)]
enum LineRole {
    Markdown,
    /// A fenced-code line with the fence language (or None if plain fence).
    Code { lang: Option<String> },
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
            roles.push(LineRole::Code { lang: fence_lang.clone() });
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

        // Build the layout job for the whole buffer, line by line.
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _w: f32| -> std::sync::Arc<egui::Galley> {
            let text = buf.as_str();
            let roles = line_roles(text);
            let lines: Vec<&str> = text.split('\n').collect();
            let mono = FontId::monospace(font_size);
            let mut job = LayoutJob::default();
            for (idx, line) in lines.iter().enumerate() {
                let role = roles.get(idx).cloned().unwrap_or(LineRole::Markdown);
                match role {
                    LineRole::Markdown => {
                        for sp in hl.markdown_line(line) {
                            let color = if sp.color == Color32::TRANSPARENT { fg } else { sp.color };
                            let style = style_for_md(&sp.text, color, &mono, sp.style);
                            job.append(&sp.text, 0.0, style);
                        }
                    }
                    LineRole::Code { lang } => {
                        for sp in hl.code_line(lang.as_deref(), line) {
                            let color = if sp.color == Color32::TRANSPARENT { fg } else { sp.color };
                            let style = style_for_md(&sp.text, color, &mono, sp.style);
                            job.append(&sp.text, 0.0, style);
                        }
                    }
                }
                if idx < lines.len() - 1 {
                    job.append("\n", 0.0, TextFormat { font_id: mono.clone(), color: fg, ..Default::default() });
                }
            }
            ui.fonts_mut(|f| f.layout_job(job))
        };

        let prev = doc.text.clone();

        let scroll_output = egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                    if self.cfg.show_line_numbers {
                        let line_count = doc.text.lines().count().max(1);
                        ui.allocate_ui_with_layout(
                            egui::vec2(38.0, ui.available_height()),
                            egui::Layout::top_down(Align::Min),
                            |ui| {
                                for n in 1..=line_count {
                                    ui.label(
                                        RichText::new(n.to_string())
                                            .monospace()
                                            .size(font_size)
                                            .color(self.theme.c.muted),
                                    );
                                }
                            },
                        );
                    }

                    ui.style_mut().visuals.override_text_color = Some(fg);
                    ui.style_mut().visuals.selection.bg_fill = selection;
                    ui.style_mut().visuals.selection.stroke = egui::Stroke::new(1.0, selection);

                    let te = TextEdit::multiline(&mut doc.text)
                        .font(FontId::monospace(font_size))
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter)
                        .frame(true)
                        .background_color(bg)
                        .text_color(fg)
                        .margin(egui::Margin::symmetric(8, 6));
                    ui.add(te)
                })
                .inner
            });
        let response = scroll_output.inner;

        let changed = response.changed() || doc.text != prev;
        if changed {
            doc.dirty = true;
        }
        EditorResult { changed }
    }
}

/// Translate a highlighter span into a TextFormat honoring its style flags.
fn style_for_md(text: &str, color: Color32, mono: &FontId, style_flags: u8) -> TextFormat {
    let mut tf = TextFormat {
        font_id: mono.clone(),
        color,
        background: Color32::TRANSPARENT,
        ..Default::default()
    };
    if style_flags & 1 != 0 {
        // used by markdown heading segments for extra emphasis
        tf.color = color;
    }
    if style_flags & 2 != 0 {
        tf.italics = true;
    }
    if style_flags & 4 != 0 {
        tf.underline = egui::Stroke::new(1.0, color);
    }
    let _ = text;
    tf
}
