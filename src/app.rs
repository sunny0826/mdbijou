//! Application shell: eframe::App tying together config, document, theme,
//! highlighter, image loading, preview renderer and the simple editor, plus the
//! preview/edit view state machine and save flow.

use crate::config::{self, Config, View};
use crate::document::Document;
use crate::editor::Editor;
use crate::highlight::{self, Highlighter};
use crate::images::ImageStore;
use crate::render::RenderCtx;
use crate::theme::{Theme, ThemeRegistry};
use eframe::egui;
use egui::{Color32, RichText};
use std::path::PathBuf;
use std::time::Instant;

pub struct MdbijouApp {
    cfg: Config,
    doc: Document,
    registry: ThemeRegistry,
    theme_id: String,
    view: View,
    hl: Box<dyn Highlighter>,
    images: ImageStore,
    last_edit_saved: Instant,
    need_reparse: bool,
    /// A file the user chose to open but which we are holding for confirmation
    /// because the current document has unsaved changes.
    pending_open: Option<PathBuf>,
    /// Transient save/status feedback: (expiry, message, color).
    feedback: Option<(Instant, String, Color32)>,
}

#[derive(Debug)]
enum Cmd {
    ToggleView,
    Save,
    SaveAs,
    Reload,
    SetTheme(String),
    Open,
}

impl MdbijouApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mut cfg: Config, path: Option<PathBuf>) -> Self {
        let text = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let doc = match path {
            Some(p) => Document::with_path(p, text),
            None => Document::new(text),
        };
        let registry = ThemeRegistry::new();
        let mut theme_id = if registry.get(&cfg.theme).is_some() {
            cfg.theme.clone()
        } else {
            "github-light".into()
        };
        // Respect "follow system theme" unless the user pinned a specific theme.
        if cfg.follow_system_theme {
            let dark = cc.egui_ctx.system_theme() == Some(egui::Theme::Dark);
            theme_id = if dark {
                "github-dark".into()
            } else {
                "github-light".into()
            };
            cfg.theme = theme_id.clone();
        }
        let theme = registry.get(&theme_id).unwrap().clone();
        let hl = highlight::new_highlighter(&theme);
        let images = ImageStore::new(base_dir_for(&doc));

        // Install CJK fonts.
        let mut fonts = egui::FontDefinitions::default();
        crate::fonts::install_cjk_fonts(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);

        let view = cfg.default_view;

        Self {
            cfg,
            doc,
            registry,
            theme_id,
            view,
            hl,
            images,
            last_edit_saved: Instant::now(),
            need_reparse: false,
            pending_open: None,
            feedback: None,
        }
    }

    fn theme(&self) -> &Theme {
        self.registry
            .get(&self.theme_id)
            .unwrap_or(&self.registry.themes[0])
    }

    fn rebuild_highlighter(&mut self) {
        let theme = self.theme().clone();
        self.hl = highlight::new_highlighter(&theme);
    }

    fn switch_theme(&mut self, id: &str) {
        if self.registry.get(id).is_some() {
            self.theme_id = id.to_string();
            self.cfg.theme = id.to_string();
            // Manually picking a theme overrides system-follow.
            self.cfg.follow_system_theme = false;
            config::save(&self.cfg);
            self.rebuild_highlighter();
        }
    }

    fn load_document(&mut self, path: &PathBuf) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.doc = Document::with_path(path.clone(), text);
            }
            Err(e) => {
                self.doc = Document::with_path(path.clone(), format!("# 无法打开文件\n\n{e}"));
            }
        }
        self.images = ImageStore::new(base_dir_for(&self.doc));
    }

    fn save(&mut self) -> bool {
        let Some(path) = self.doc.path.clone() else {
            return self.save_as();
        };
        let ok = config::atomic_write(&path, self.doc.text.as_bytes()).is_ok();
        if ok {
            self.doc.dirty = false;
            self.flash("已保存", Color32::from_rgb(70, 170, 90));
        } else {
            self.flash("保存失败", Color32::from_rgb(220, 90, 70));
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
                self.flash("已保存", Color32::from_rgb(70, 170, 90));
                return true;
            }
        }
        self.flash("保存取消或失败", Color32::from_rgb(220, 90, 70));
        false
    }

    fn flash(&mut self, msg: &str, color: Color32) {
        self.feedback = Some((
            Instant::now() + std::time::Duration::from_secs(3),
            msg.to_string(),
            color,
        ));
    }

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
    fn request_open(&mut self, path: PathBuf) {
        if self.doc.dirty {
            self.pending_open = Some(path);
        } else {
            self.apply_open(path);
        }
    }

    fn apply_open(&mut self, path: PathBuf) {
        self.load_document(&path);
        self.pending_open = None;
        self.view = self.cfg.default_view;
    }

    /// Render the unsaved-changes confirmation modal (if one is pending).
    fn show_open_confirm(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_open.clone() else {
            return;
        };
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        egui::Window::new("未保存的更改")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("当前文档有未保存的修改。");
                ui.label(format!("是否先保存，再打开 “{name}”？"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("保存并打开").clicked() && self.save() {
                        self.apply_open(path.clone());
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
                    self.doc.reparse();
                    self.view = View::Preview;
                }
            }
            Cmd::Save => {
                let _ = self.save();
            }
            Cmd::SaveAs => {
                let _ = self.save_as();
            }
            Cmd::Reload => {
                if let Some(p) = self.doc.path.clone() {
                    if !self.doc.dirty {
                        self.load_document(&p);
                    }
                }
            }
            Cmd::SetTheme(id) => self.switch_theme(&id),
            Cmd::Open => self.open_via_dialog(),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let muted = self.theme().c.muted;
        let title = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
            .unwrap_or_else(|| "untitled".into());
        let dirty = self.doc.dirty;

        ui.horizontal_wrapped(|ui| {
            // --- Document title + persist state (not color-only) ---
            let (dot, state) = if dirty {
                ("●", "未保存")
            } else {
                ("○", "已保存")
            };
            let title_resp = ui.add(egui::Label::new(
                RichText::new(format!("{dot} {title} · {state}")).color(if dirty {
                    Color32::from_rgb(230, 120, 40)
                } else {
                    muted
                }),
            ));
            if let Some(p) = &self.doc.path {
                title_resp.on_hover_text(p.display().to_string());
            }
            ui.separator();

            // --- Actions, each with an accessible name + shortcut hint ---
            if ui
                .add(egui::Button::new("打开"))
                .on_hover_text("⌘O")
                .clicked()
            {
                self.dispatch(Cmd::Open);
            }
            let view_label = if self.view == View::Preview {
                "编辑"
            } else {
                "预览"
            };
            let view_hint = if self.view == View::Preview {
                "⌘E — 编辑源码"
            } else {
                "⌘E — 返回预览"
            };
            if ui
                .add(egui::Button::new(view_label))
                .on_hover_text(view_hint)
                .clicked()
            {
                self.dispatch(Cmd::ToggleView);
            }
            if ui
                .add(egui::Button::new("保存"))
                .on_hover_text("⌘S")
                .clicked()
            {
                self.dispatch(Cmd::Save);
            }
            if ui
                .add(egui::Button::new("另存"))
                .on_hover_text("⇧⌘S")
                .clicked()
            {
                self.dispatch(Cmd::SaveAs);
            }
            ui.separator();

            // --- Theme as a visible menu (UI-MD-012) ---
            let theme_name = self.theme().name.clone();
            let current = self.theme_id.clone();
            let themes: Vec<(String, String)> = self
                .registry
                .themes
                .iter()
                .map(|t| (t.id.clone(), t.name.clone()))
                .collect();
            let mut picked = current.clone();
            egui::ComboBox::from_id_salt("theme_picker")
                .selected_text(RichText::new(theme_name).color(muted))
                .width(130.0)
                .show_ui(ui, |ui| {
                    for (id, name) in &themes {
                        ui.selectable_value(&mut picked, id.clone(), name.clone());
                    }
                });
            if picked != current {
                self.dispatch(Cmd::SetTheme(picked));
            }

            // --- Save feedback ---
            if let Some((expiry, msg, color)) = &self.feedback {
                if *expiry > Instant::now() {
                    ui.label(RichText::new(msg).color(*color));
                }
            }
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
        visuals.selection.stroke = egui::Stroke::new(1.0, sel);
        visuals.widgets.hovered.weak_bg_fill = th.c.code_bg;
        ctx.set_visuals(visuals);

        // ------- top bar -------
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.top_bar(ui);
        });

        // ------- central panel: preview or edit -------
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(0, 8)),
            )
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
        if self.cfg.auto_save && self.doc.dirty && self.last_edit_saved.elapsed().as_millis() > 800
        {
            let _ = self.save();
            self.last_edit_saved = Instant::now();
        }
    }
}

impl MdbijouApp {
    fn show_preview(&mut self, ui: &mut egui::Ui) {
        if self.need_reparse {
            self.doc.reparse();
            self.need_reparse = false;
        }
        let theme = self.theme().clone();
        // Responsive reading column (UI-MD-002): never grow beyond the window,
        // keep a stable small gutter on narrow windows, center on wide ones.
        let avail = ui.available_width().max(20.0);
        let effective = self.cfg.content_width.min(avail - 2.0 * 12.0);
        let pad = ((avail - effective) * 0.5).max(12.0);

        let mut rctx = RenderCtx::new(
            &theme,
            &mut *self.hl,
            &mut self.images,
            effective,
            self.cfg.font_size,
            self.cfg.line_height,
        );

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_max_width(effective);
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
            self.last_edit_saved = Instant::now();
            self.need_reparse = true;
        }
    }
}

fn base_dir_for(doc: &Document) -> PathBuf {
    doc.path
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
