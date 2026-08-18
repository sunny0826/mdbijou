//! Application shell: eframe::App tying together config, document, theme,
//! highlighter, preview renderer and the simple editor, plus the
//! preview/edit view state machine and save flow.

use crate::config::{self, Config, View};
use crate::document::Document;
use crate::editor::Editor;
use crate::highlight::{self, Highlighter};
use crate::render::RenderCtx;
use crate::theme::{Theme, ThemeRegistry};
use eframe::egui;
use egui::{Color32, RichText};

pub struct MdbijouApp {
    cfg: Config,
    doc: Document,
    registry: ThemeRegistry,
    theme_id: String,
    view: View,
    hl: Box<dyn Highlighter>,
    last_edit_saved: std::time::Instant,
    need_reparse: bool,
    /// A file the user chose to open but which we are holding for confirmation
    /// because the current document has unsaved changes.
    pending_open: Option<std::path::PathBuf>,
}

#[derive(Debug)]
enum Cmd {
    ToggleView,
    Save,
    SaveAs,
    Reload,
    CycleTheme,
    Open,
}

impl MdbijouApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config, path: Option<std::path::PathBuf>) -> Self {
        let text = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let doc = match path {
            Some(p) => Document::with_path(p, text),
            None => Document::new(text),
        };
        let registry = ThemeRegistry::new();
        let theme_id = if registry.get(&cfg.theme).is_some() { cfg.theme.clone() } else { "github-light".into() };
        let theme = registry.get(&theme_id).unwrap().clone();
        let hl = highlight::new_highlighter(&theme);

        // Install CJK fonts.
        let mut fonts = egui::FontDefinitions::default();
        crate::fonts::install_cjk_fonts(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);

        // Default view from config, overridable via CLI handled during run (we
        // keep config's view; CLI --edit is passed as cfg.default_view).
        let view = cfg.default_view;

        Self {
            cfg,
            doc,
            registry,
            theme_id,
            view,
            hl,
            last_edit_saved: std::time::Instant::now(),
            need_reparse: false,
            pending_open: None,
        }
    }

    fn theme(&self) -> &Theme {
        self.registry.get(&self.theme_id).unwrap_or(&self.registry.themes[0])
    }

    fn rebuild_highlighter(&mut self) {
        let theme = self.theme().clone();
        self.hl = highlight::new_highlighter(&theme);
    }

    fn switch_theme(&mut self, id: &str) {
        if self.registry.get(id).is_some() {
            self.theme_id = id.to_string();
            self.cfg.theme = id.to_string();
            let _ = config::save(&self.cfg);
            self.rebuild_highlighter();
        }
    }

    fn load_document(&mut self, path: &std::path::Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.doc = Document::with_path(path.to_path_buf(), text);
            }
            Err(e) => {
                self.doc = Document::with_path(path.to_path_buf(), format!("# 无法打开文件\n\n{e}"));
            }
        }
    }

    fn save(&mut self) -> bool {
        let Some(path) = self.doc.path.clone() else {
            return self.save_as();
        };
        let ok = config::atomic_write(&path, self.doc.text.as_bytes()).is_ok();
        if ok {
            self.doc.dirty = false;
        }
        ok
    }

    fn save_as(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .set_file_name("untitled.md")
            .save_file()
        {
            let ok = config::atomic_write(&path, self.doc.text.as_bytes()).is_ok();
            if ok {
                self.doc.path = Some(path);
                self.doc.dirty = false;
                return true;
            }
        }
        false
    }

    /// Show the native open-file dialog, then (if the current document is
    /// dirty) defer to the unsaved-changes confirmation.
    fn open_via_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .set_directory(std::env::current_dir().unwrap_or_default())
            .pick_file()
        {
            self.request_open(path);
        }
    }

    /// Offer to open `path`; if the document has unsaved edits, hold it for
    /// confirmation instead of silently discarding (design §8.5).
    fn request_open(&mut self, path: std::path::PathBuf) {
        if self.doc.dirty {
            self.pending_open = Some(path);
        } else {
            self.apply_open(path);
        }
    }

    fn apply_open(&mut self, path: std::path::PathBuf) {
        self.load_document(&path);
        self.pending_open = None;
        self.view = self.cfg.default_view;
    }

    /// Render the unsaved-changes confirmation modal (if one is pending).
    fn show_open_confirm(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_open.clone() else { return };
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        egui::Window::new("未保存的更改")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("当前文档有未保存的修改。"));
                ui.label(format!("是否先保存，再打开 “{name}”？"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("保存并打开").clicked() {
                        if self.save() {
                            self.apply_open(path.clone());
                        }
                    }
                    if ui.button("放弃并打开").clicked() {
                        self.apply_open(path.clone());
                    }
                    if ui.button("取消").clicked() {
                        self.pending_open = None;
                    }
                });
            });
    }

    fn dispatch(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::ToggleView => {
                if self.view == View::Preview {
                    self.view = View::Edit;
                } else {
                    // switch to preview -> reparse from buffer
                    self.doc.reparse();
                    self.view = View::Preview;
                }
            }
            Cmd::Save => {
                self.save();
            }
            Cmd::SaveAs => {
                self.save_as();
            }
            Cmd::Reload => {
                if let Some(p) = self.doc.path.clone() {
                    let dirty = self.doc.dirty;
                    if !dirty {
                        self.load_document(&p);
                    }
                }
            }
            Cmd::CycleTheme => {
                let next = self.registry.cycle(&self.theme_id).id.clone();
                self.switch_theme(&next);
            }
            Cmd::Open => {
                self.open_via_dialog();
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let muted = self.theme().c.muted;
        let theme_id = self.theme_id.clone();
        let dirty = self.doc.dirty;
        let title = self
            .doc
            .path
            .as_ref()
            .map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default())
            .unwrap_or_else(|| "untitled".into());

        ui.horizontal_wrapped(|ui| {
            let title_resp = ui.add(
                egui::Label::new(
                    RichText::new(format!("{} {}", if dirty { "●" } else { "○" }, title))
                        .color(if dirty { Color32::from_rgb(230, 120, 40) } else { muted }),
                ),
            );
            if let Some(p) = &self.doc.path {
                title_resp.on_hover_text(p.display().to_string());
            }
            ui.separator();

            if ui.button("打开").clicked() {
                self.dispatch(Cmd::Open);
            }
            let view_label = if self.view == View::Preview { "编辑" } else { "预览" };
            if ui.button(view_label).clicked() {
                self.dispatch(Cmd::ToggleView);
            }
            if ui.button("保存").clicked() {
                self.dispatch(Cmd::Save);
            }
            if ui.button("另存").clicked() {
                self.dispatch(Cmd::SaveAs);
            }
            if ui.button("主题").clicked() {
                self.dispatch(Cmd::CycleTheme);
            }
            ui.separator();
            ui.label(RichText::new(&theme_id).small().color(muted));
        });
    }
}

impl eframe::App for MdbijouApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ------- keyboard shortcuts -------
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::E)) {
            self.dispatch(Cmd::ToggleView);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            let shift = ctx.input_mut(|i| i.modifiers.shift);
            if shift {
                self.dispatch(Cmd::SaveAs);
            } else {
                self.dispatch(Cmd::Save);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.dispatch(Cmd::CycleTheme);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R)) {
            self.dispatch(Cmd::Reload);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.dispatch(Cmd::Open);
        }

        // ------- visual theme (egui) derived from our theme -------
        let th = self.theme();
        let kind = th.kind;
        let bg = th.c.background;
        let fg = th.c.foreground;
        let sel = th.c.selection_bg;
        let mut visuals = match kind {
            crate::theme::ThemeKind::Light => egui::Visuals::light(),
            crate::theme::ThemeKind::Dark => egui::Visuals::dark(),
        };
        visuals.panel_fill = bg;
        visuals.window_fill = bg;
        visuals.override_text_color = Some(fg);
        visuals.selection.bg_fill = sel;
        ctx.set_visuals(visuals);

        // ------- top bar -------
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.top_bar(ui);
        });

        // ------- central panel: preview or edit -------
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg))
            .show(ctx, |ui| {
                if self.view == View::Preview {
                    self.show_preview(ui);
                } else {
                    self.show_editor(ui);
                }
            });

        // ------- unsaved-changes confirm (if a file open is pending) -------
        self.show_open_confirm(ctx);

        // ------- auto-save -------
        if self.cfg.auto_save && self.doc.dirty {
            if self.last_edit_saved.elapsed().as_millis() > 800 {
                self.save();
                self.last_edit_saved = std::time::Instant::now();
            }
        }
    }
}

impl MdbijouApp {
    fn show_preview(&mut self, ui: &mut egui::Ui) {
        // Deferred reparse if needed.
        if self.need_reparse {
            self.doc.reparse();
            self.need_reparse = false;
        }
        let theme = self.theme().clone();
        let width = self.cfg.content_width;
        let font_size = self.cfg.font_size;
        let mut rctx = RenderCtx::new(&theme, &mut *self.hl, width, font_size);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Center the content column.
            let avail = ui.available_width();
            let pad = ((avail - width) * 0.5).max(12.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_min_width(width);
                    crate::render::render_document(ui, &self.doc, &mut rctx);
                });
            });
        });
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        let mut editor = Editor::new(&self.cfg, &theme);
        let res = editor.show(ui, &mut self.doc, &mut *self.hl);
        if res.changed {
            self.last_edit_saved = std::time::Instant::now();
        }
    }
}
