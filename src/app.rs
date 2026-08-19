//! Application shell: eframe::App tying together config, document, theme,
//! highlighter, image loading, preview renderer and the simple editor, plus the
//! preview/edit view state machine, the traffic-light toolbar, and the settings
//! page.

use crate::config::{self, Config, View};
use crate::document::Document;
use crate::editor::Editor;
use crate::highlight::{self, Highlighter};
use crate::images::ImageStore;
use crate::install;
use crate::render::RenderCtx;
use crate::theme::{Metrics, Theme, ThemeRegistry};
use eframe::egui;
use egui::{Color32, RichText};
use egui_phosphor::regular;
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
    /// Whether the settings page is open.
    show_settings: bool,
    /// Result of the last CLI-install attempt, shown in the settings page.
    cli_status: Option<install::InstallResult>,
    /// 1-based cursor (line, column) in edit mode, for the status bar.
    cursor: Option<(usize, usize)>,
    /// File paths sent by the OS (Finder double-click, `open`, Dock drop).
    open_rx: std::sync::mpsc::Receiver<PathBuf>,
}

#[derive(Debug)]
enum Cmd {
    ToggleView,
    Save,
    Reload,
    Open,
    ToggleSettings,
}

impl MdbijouApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        mut cfg: Config,
        path: Option<PathBuf>,
        open_rx: std::sync::mpsc::Receiver<PathBuf>,
    ) -> Self {
        crate::macos::configure_title_bar(cc);

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

        // Install the chosen body font, the CJK fallback and the icon font.
        cc.egui_ctx
            .set_fonts(crate::fonts::build_fonts(&cfg.font_family));

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
            show_settings: false,
            cli_status: None,
            cursor: None,
            open_rx,
        }
    }

    fn theme(&self) -> &Theme {
        self.registry
            .get(&self.theme_id)
            .unwrap_or(&self.registry.themes[0])
    }

    fn metrics(&self, ctx: &egui::Context) -> Metrics {
        Metrics::scaled(ctx.pixels_per_point())
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

    /// Re-derive the light/dark theme from the system preference.
    fn apply_follow_system(&mut self, ctx: &egui::Context) {
        let dark = ctx.system_theme() == Some(egui::Theme::Dark);
        let id = if dark { "github-dark" } else { "github-light" };
        if self.registry.get(id).is_some() {
            self.theme_id = id.to_string();
            self.cfg.theme = id.to_string();
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
            self.flash("已保存", self.theme().c.success);
        } else {
            self.flash("保存失败", self.theme().c.error);
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
                self.flash("已保存", self.theme().c.success);
                return true;
            }
        }
        self.flash("保存取消或失败", self.theme().c.error);
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
            Cmd::Reload => {
                if let Some(p) = self.doc.path.clone() {
                    if !self.doc.dirty {
                        self.load_document(&p);
                    }
                }
            }
            Cmd::Open => self.open_via_dialog(),
            Cmd::ToggleSettings => self.show_settings = !self.show_settings,
        }
    }

    /// The traffic-light toolbar: hover-revealed action icons and an
    /// edit/preview switch pinned to the top-right.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        let fg = theme.c.foreground;
        let muted = theme.c.muted;

        // The whole bar is draggable (empty areas) so the window can still be
        // moved now that the native title bar is transparent.
        let bar_rect = ui.max_rect();
        let drag_resp = ui.interact(bar_rect, ui.id().with("titlebar_drag"), egui::Sense::drag());
        if drag_resp.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // Hairline separating the bar from the content below.
        ui.painter().hline(
            bar_rect.left()..=bar_rect.right(),
            bar_rect.bottom() - 0.5,
            egui::Stroke::new(1.0, theme.c.hr),
        );

        // Open/save icons fade in while the pointer is over the bar; the
        // settings gear is always visible.
        let bar_hovered = ui.rect_contains_pointer(bar_rect);
        let reveal = ui
            .ctx()
            .animate_bool(ui.id().with("icon_reveal"), bar_hovered);

        // Action icons optically matched to the traffic-light buttons: 13pt
        // Phosphor glyphs in 22pt slots, centered on the traffic lights.
        let light_y = crate::macos::traffic_light_center();
        let icon_area_w = 3.0 * 22.0 + 2.0 * 4.0;
        let icon_y = bar_rect.top() + light_y;
        let mut icon_x = bar_rect.left() + crate::macos::traffic_light_pad() + 2.0;
        let mut clicked_icon: Option<usize> = None;
        for (idx, (glyph, tooltip)) in [
            (regular::FOLDER_OPEN, "打开 (⌘O)"),
            (regular::FLOPPY_DISK, "保存 (⌘S)"),
            (regular::GEAR_SIX, "设置 (⌘,)"),
        ]
        .into_iter()
        .enumerate()
        {
            // All icons hide until the bar is hovered; the gear stays visible
            // while the settings page is open as a close affordance.
            let fade = if idx == 2 && self.show_settings {
                1.0
            } else {
                reveal
            };
            let rect = egui::Rect::from_center_size(
                egui::pos2(icon_x + 11.0, icon_y),
                egui::vec2(22.0, 22.0),
            );
            icon_x += 26.0;
            if fade <= 0.0 {
                continue;
            }
            let resp = ui.interact(
                rect,
                ui.id().with(("toolbar_icon", idx)),
                egui::Sense::click(),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter().circle_filled(
                    rect.center(),
                    11.0,
                    ui.visuals()
                        .widgets
                        .hovered
                        .weak_bg_fill
                        .gamma_multiply(fade),
                );
            }
            let base = if idx == 2 && self.show_settings {
                fg
            } else {
                muted
            };
            let color = (if resp.hovered() { fg } else { base }).gamma_multiply(fade);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                egui::FontId::proportional(13.0),
                color,
            );
            if resp.clicked() {
                clicked_icon = Some(idx);
            }
            resp.on_hover_text(tooltip);
        }
        match clicked_icon {
            Some(0) => self.dispatch(Cmd::Open),
            Some(1) => self.dispatch(Cmd::Save),
            Some(2) => self.dispatch(Cmd::ToggleSettings),
            _ => {}
        }

        // Feedback (save status) and view switch are drawn against the bar's
        // geometric center: egui row layout aligns children by content height
        // (and galleys carry ascender/descender padding), which left the text
        // sitting visibly high in the bar.
        let center_y = bar_rect.center().y;
        let feedback = match &self.feedback {
            Some((expiry, msg, color)) if *expiry > Instant::now() => Some((
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(msg.clone(), egui::FontId::proportional(12.0), *color)
                }),
                *color,
            )),
            _ => None,
        };
        let feedback_w = feedback
            .as_ref()
            .map(|(g, _)| g.size().x + 8.0)
            .unwrap_or(0.0);

        // Center the feedback group horizontally.
        let total_w = feedback_w;
        let min_left = crate::macos::traffic_light_pad() + 2.0 + icon_area_w + 8.0;
        let start_x =
            (bar_rect.left() + (bar_rect.width() - total_w) / 2.0).max(bar_rect.left() + min_left);

        if let Some((g, color)) = feedback {
            let pos = egui::pos2(start_x, center_y) - egui::vec2(0.0, g.size().y / 2.0);
            ui.painter().galley(pos, g, color);
        }

        // Edit/preview segmented switch, pinned to the top-right.
        let switch_rect = egui::Rect::from_center_size(
            egui::pos2(bar_rect.right() - 10.0 - 46.0, center_y),
            egui::vec2(92.0, 22.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(switch_rect), |ui| {
            if self.view_switch(ui, &theme) {
                self.dispatch(Cmd::ToggleView);
            }
        });
    }

    /// Xcode-style segmented control for switching between preview and edit.
    /// Returns true when the user asked to toggle the view.
    fn view_switch(&mut self, ui: &mut egui::Ui, theme: &Theme) -> bool {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(92.0, 22.0), egui::Sense::hover());
        let painter = ui.painter();
        let metrics = self.metrics(ui.ctx());
        let rounding = egui::CornerRadius::same(metrics.radius_lg as u8);
        painter.rect_filled(rect, rounding, theme.c.code_bg);
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, theme.c.hr),
            egui::StrokeKind::Inside,
        );

        let half = rect.width() / 2.0;
        let preview_rect =
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + half, rect.max.y));
        let edit_rect =
            egui::Rect::from_min_max(egui::pos2(rect.min.x + half, rect.min.y), rect.max);
        let is_preview = self.view == View::Preview;
        let sel_rect = if is_preview { preview_rect } else { edit_rect };
        let sel_rounding = egui::CornerRadius::same(metrics.radius_md as u8);
        painter.rect_filled(sel_rect.shrink(2.0), sel_rounding, theme.c.background);
        painter.rect_stroke(
            sel_rect.shrink(2.0),
            sel_rounding,
            egui::Stroke::new(1.0, theme.c.hr),
            egui::StrokeKind::Inside,
        );

        let font = egui::FontId::proportional(12.0);
        let fg = theme.c.foreground;
        let muted = theme.c.muted;
        let (preview_color, edit_color) = if is_preview { (fg, muted) } else { (muted, fg) };
        paint_optical_centered_text(painter, preview_rect, "预览", font.clone(), preview_color);
        paint_optical_centered_text(painter, edit_rect, "编辑", font, edit_color);

        let preview_resp = ui.interact(
            preview_rect,
            ui.id().with("seg_preview"),
            egui::Sense::click(),
        );
        let edit_resp = ui.interact(edit_rect, ui.id().with("seg_edit"), egui::Sense::click());
        let hovered = preview_resp.hovered() || edit_resp.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            let hint = if is_preview {
                "切换到编辑 (⌘E)"
            } else {
                "切换到预览 (⌘E)"
            };
            preview_resp.clone().on_hover_text(hint);
        }
        (preview_resp.clicked() && !is_preview) || (edit_resp.clicked() && is_preview)
    }

    /// Settings page (opened with ⌘,): a centered modal card over a dimmed
    /// backdrop. Closed with Esc, the ✕ button, or a click on the backdrop.
    fn show_settings(&mut self, ctx: &egui::Context) {
        let fade = ctx.animate_bool(egui::Id::new("settings_fade"), self.show_settings);
        if fade <= 0.0 {
            return;
        }
        if self.show_settings
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.show_settings = false;
        }

        let theme = self.theme().clone();
        let fg = theme.c.foreground;
        let muted = theme.c.muted;
        let metrics = self.metrics(ctx);
        let accent = theme.c.link;
        let themes: Vec<(String, String, Color32)> = self
            .registry
            .themes
            .iter()
            .map(|t| (t.id.clone(), t.name.clone(), t.c.link))
            .collect();
        let mut follow = self.cfg.follow_system_theme;
        let mut picked: Option<String> = None;
        let mut do_follow = false;
        let mut do_install = false;
        let mut font_picked: Option<String> = None;
        let mut font_size_changed = false;
        let mut editor_size_changed = false;
        let mut close = false;

        // Dimmed backdrop; clicking it closes the settings page.
        let screen = ctx.input(|i| i.content_rect());
        let backdrop = egui::Area::new(egui::Id::new("settings_backdrop"))
            .order(egui::Order::Middle)
            .interactable(true)
            .show(ctx, |ui| {
                let resp = ui.allocate_rect(screen, egui::Sense::click());
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    Color32::from_black_alpha((90.0 * fade) as u8),
                );
                resp
            });
        if backdrop.inner.clicked() && self.show_settings {
            close = true;
        }

        egui::Area::new(egui::Id::new("settings_card"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_opacity(fade);
                egui::Frame::new()
                    .fill(theme.c.background)
                    .corner_radius(metrics.radius_lg)
                    .stroke(egui::Stroke::new(1.0, theme.c.hr))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .inner_margin(egui::Margin::same(22))
                    .show(ui, |ui| {
                        ui.set_width(400.0);

                        // Header: icon + title + close button.
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(regular::GEAR_SIX).size(15.0).color(muted));
                            ui.label(RichText::new("设置").size(15.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(regular::X).size(13.0).color(muted),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("关闭 (Esc)")
                                        .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });

                        ui.add_space(18.0);
                        settings_section(ui, regular::PALETTE, "外观", muted);
                        ui.add_space(6.0);
                        settings_group(ui, &theme, |ui| {
                            ui.add_space(8.0);
                            settings_row(ui, "跟随系统主题", fg, |ui| {
                                let mut f = follow;
                                if toggle_switch(ui, &mut f, accent, muted) {
                                    follow = f;
                                    do_follow = true;
                                }
                            });
                            ui.add_space(6.0);
                            ui.add_enabled_ui(!follow, |ui| {
                                hairline(ui, theme.c.hr);
                                ui.add_space(8.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for (id, name, chip_accent) in &themes {
                                        let selected = self.theme_id == *id;
                                        let mut job = egui::text::LayoutJob::default();
                                        job.append(
                                            "●  ",
                                            0.0,
                                            egui::TextFormat {
                                                font_id: egui::FontId::proportional(10.0),
                                                color: *chip_accent,
                                                ..Default::default()
                                            },
                                        );
                                        job.append(
                                            name,
                                            0.0,
                                            egui::TextFormat {
                                                font_id: egui::FontId::proportional(12.0),
                                                color: if selected { fg } else { muted },
                                                ..Default::default()
                                            },
                                        );
                                        let capsule = egui::Button::new(job)
                                            .fill(if selected {
                                                chip_accent.gamma_multiply(0.15)
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                if selected {
                                                    chip_accent.gamma_multiply(0.6)
                                                } else {
                                                    fg.gamma_multiply(0.15)
                                                },
                                            ))
                                            .corner_radius(10.0);
                                        if ui.add(capsule).clicked() {
                                            picked = Some(id.clone());
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                            });
                            ui.add_space(2.0);
                        });

                        ui.add_space(16.0);
                        settings_section(ui, regular::TEXT_AA, "字体", muted);
                        ui.add_space(6.0);
                        settings_group(ui, &theme, |ui| {
                            ui.add_space(8.0);
                            settings_row(ui, "正文字体", fg, |ui| {
                                let current = crate::fonts::BODY_FONTS
                                    .iter()
                                    .find(|f| f.id == self.cfg.font_family)
                                    .map(|f| f.name)
                                    .unwrap_or("默认");
                                let shown = fit_combo_text(ui, current, 130.0);
                                egui::ComboBox::from_id_salt("body_font")
                                    .width(170.0)
                                    .selected_text(RichText::new(shown).size(12.0))
                                    .show_ui(ui, |ui| {
                                        for f in crate::fonts::BODY_FONTS {
                                            if ui
                                                .selectable_label(
                                                    self.cfg.font_family == f.id,
                                                    RichText::new(f.name).size(12.0),
                                                )
                                                .clicked()
                                            {
                                                font_picked = Some(f.id.to_string());
                                            }
                                        }
                                    });
                            });
                            ui.add_space(6.0);
                            hairline(ui, theme.c.hr);
                            ui.add_space(8.0);
                            settings_row(ui, "正文字号", fg, |ui| {
                                ui.label(
                                    RichText::new(format!("{:.1}", self.cfg.font_size))
                                        .size(12.0)
                                        .color(muted),
                                );
                                let w = (ui.available_width() - 8.0).max(60.0);
                                if ui
                                    .add_sized(
                                        [w, 18.0],
                                        egui::Slider::new(&mut self.cfg.font_size, 12.0..=24.0)
                                            .step_by(0.5)
                                            .show_value(false),
                                    )
                                    .changed()
                                {
                                    font_size_changed = true;
                                }
                            });
                            ui.add_space(6.0);
                            hairline(ui, theme.c.hr);
                            ui.add_space(8.0);
                            settings_row(ui, "编辑器字号", fg, |ui| {
                                ui.label(
                                    RichText::new(format!("{:.1}", self.cfg.editor_font_size))
                                        .size(12.0)
                                        .color(muted),
                                );
                                let w = (ui.available_width() - 8.0).max(60.0);
                                if ui
                                    .add_sized(
                                        [w, 18.0],
                                        egui::Slider::new(
                                            &mut self.cfg.editor_font_size,
                                            11.0..=22.0,
                                        )
                                        .step_by(0.5)
                                        .show_value(false),
                                    )
                                    .changed()
                                {
                                    editor_size_changed = true;
                                }
                            });
                            ui.add_space(8.0);
                        });

                        ui.add_space(16.0);
                        settings_section(ui, regular::TERMINAL, "命令行工具", muted);
                        ui.add_space(6.0);
                        settings_group(ui, &theme, |ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("把 mdbijou 安装为 `mdb` 命令，加入你的 PATH。")
                                    .size(12.0)
                                    .color(muted),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if let Some(res) = &self.cli_status {
                                    let (mark, color) = if res.ok {
                                        (regular::CHECK_CIRCLE, Color32::from_rgb(70, 170, 90))
                                    } else {
                                        (regular::X_CIRCLE, Color32::from_rgb(220, 90, 70))
                                    };
                                    ink_centered_label(
                                        ui,
                                        &format!("{mark}  {}", res.message),
                                        egui::FontId::proportional(12.0),
                                        color,
                                        26.0,
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("安装 CLI 到本地").clicked() {
                                            do_install = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(8.0);
                        });
                    });
            });

        if close {
            self.show_settings = false;
        }

        if do_follow {
            self.cfg.follow_system_theme = follow;
            if follow {
                self.apply_follow_system(ctx);
            }
            config::save(&self.cfg);
        }
        if let Some(id) = picked {
            self.switch_theme(&id);
        }
        if let Some(id) = font_picked {
            self.cfg.font_family = id;
            ctx.set_fonts(crate::fonts::build_fonts(&self.cfg.font_family));
            config::save(&self.cfg);
        }
        if font_size_changed || editor_size_changed {
            config::save(&self.cfg);
        }
        if do_install {
            self.cli_status = Some(install::install_cli());
        }
    }
}

// ---------------------------------------------------------------------------
// Settings card helpers
// ---------------------------------------------------------------------------

/// Paint `text` centered inside `rect` by the glyphs' *ink* bounds rather than
/// the galley's geometric box. The galley box is derived from font metrics
/// (ascent/descent of the whole font set), so with CJK fallback fonts in the
/// stack the box extends well above the visible glyphs and naive
/// `Align2::CENTER_CENTER` placement renders the text too high.
fn paint_optical_centered_text(
    painter: &egui::Painter,
    rect: egui::Rect,
    text: &str,
    font: egui::FontId,
    color: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font, color);
    let mut shift = egui::Vec2::ZERO;
    if let Some(placed) = galley.rows.first() {
        let ink = placed.row.visuals.mesh_bounds;
        if ink.is_finite() && ink.width() > 0.0 && ink.height() > 0.0 {
            let ink_center = placed.pos + ink.center().to_vec2();
            shift = galley.rect.center() - ink_center;
        }
    }
    let pos = rect.center() - galley.size() / 2.0 + shift;
    painter.galley(pos, galley, color);
}

/// Muted 12pt section caption with a Phosphor icon, used inside the settings
/// card.
fn settings_section(ui: &mut egui::Ui, icon: &str, text: &str, muted: Color32) {
    ui.label(
        RichText::new(format!("{icon}  {text}"))
            .size(12.0)
            .strong()
            .color(muted),
    );
}

/// Grouped-panel container (macOS System Settings style): a rounded, subtly
/// filled box holding one settings section's rows.
fn settings_group(ui: &mut egui::Ui, theme: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme.c.code_bg)
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(12, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Ink-optically center `text` within a fixed `height`-tall slot sized to the
/// text. Uses a fixed height (not available-height) so it can never distort the
/// layout, while CJK glyphs land on the true vertical center of the slot.
fn ink_centered_label(
    ui: &mut egui::Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    height: f32,
) {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), color));
    let w = galley.size().x;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, height), egui::Sense::hover());
    paint_optical_centered_text(ui.painter(), rect, text, font, color);
}

/// One settings row: label on the left, controls right-aligned.
fn settings_row(ui: &mut egui::Ui, label: &str, fg: Color32, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        // Fixed-height slot (matches the tallest control) keeps the label from
        // sitting high when a taller control later grows the row.
        ink_centered_label(ui, label, egui::FontId::proportional(13.0), fg, 26.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

/// Truncate `text` with an ellipsis so its 12pt layout fits within `max_w`,
/// keeping a closed combo box a fixed width regardless of the option length.
fn fit_combo_text(ui: &mut egui::Ui, text: &str, max_w: f32) -> String {
    let font = egui::FontId::proportional(12.0);
    let text_w = |s: &str| {
        ui.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE))
            .size()
            .x
    };
    if text_w(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s: String = chars[..mid].iter().collect();
        if text_w(&s) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut s: String = chars[..lo.saturating_sub(1)].iter().collect();
    s.push('…');
    s
}

/// Full-width 1px separator used between rows inside a settings group.
fn hairline(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, color),
    );
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let a = egui::ecolor::Rgba::from(a);
    let b = egui::ecolor::Rgba::from(b);
    Color32::from(a * (1.0 - t) + b * t)
}

/// macOS-style pill toggle switch. Returns true when the value was toggled.
fn toggle_switch(ui: &mut egui::Ui, on: &mut bool, accent: Color32, muted: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(36.0, 20.0), egui::Sense::click());
    let changed = resp.clicked();
    if changed {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let pill = rect.shrink(1.0);
    let bg = lerp_color(muted.gamma_multiply(0.30), accent, t);
    ui.painter()
        .rect_filled(pill, egui::CornerRadius::same(10), bg);
    let r = 7.5;
    let travel = pill.width() - 6.0 - 2.0 * r;
    let x = pill.left() + 3.0 + r + travel * t;
    ui.painter()
        .circle_filled(egui::pos2(x, pill.center().y), r, Color32::WHITE);
    changed
}

impl eframe::App for MdbijouApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ------- files sent by the OS (Finder double-click / `open` / Dock) ---
        let mut got_open = false;
        while let Ok(path) = self.open_rx.try_recv() {
            self.request_open(path);
            got_open = true;
        }
        if got_open {
            ctx.request_repaint();
        }

        // ------- keyboard shortcuts -------
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::E)) {
            self.dispatch(Cmd::ToggleView);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.dispatch(Cmd::Save);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R)) {
            self.dispatch(Cmd::Reload);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.dispatch(Cmd::Open);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.dispatch(Cmd::ToggleSettings);
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
        visuals.window_corner_radius = egui::CornerRadius::same(10);

        // Polish egui widgets (buttons, combo boxes, sliders) to match the
        // theme: rounded corners from the design tokens, surface fills, and an
        // accent-tinted hover/active state.
        let m = self.metrics(ctx);
        let accent = th.c.link;
        let rounded_md = egui::CornerRadius::same(m.radius_md as u8);
        // A neutral "control" fill distinct from the code_bg group background so
        // slider rails stay visible; also used as the default button fill.
        let control_bg = match th.kind {
            crate::theme::ThemeKind::Light => Color32::from_rgb(0xe9, 0xe9, 0xe9),
            crate::theme::ThemeKind::Dark => Color32::from_rgb(0x3c, 0x3c, 0x3c),
        };
        visuals.widgets.noninteractive = egui::style::WidgetVisuals {
            bg_fill: th.c.code_bg,
            weak_bg_fill: Color32::TRANSPARENT,
            bg_stroke: egui::Stroke::new(1.0, th.c.table_border),
            corner_radius: egui::CornerRadius::same(m.radius_sm as u8),
            fg_stroke: egui::Stroke::new(1.0, fg),
            expansion: 0.0,
        };
        visuals.widgets.inactive = egui::style::WidgetVisuals {
            bg_fill: control_bg,
            weak_bg_fill: Color32::TRANSPARENT,
            bg_stroke: egui::Stroke::new(1.0, th.c.table_border),
            corner_radius: rounded_md,
            fg_stroke: egui::Stroke::new(1.0, fg),
            expansion: 0.0,
        };
        visuals.widgets.hovered = egui::style::WidgetVisuals {
            bg_fill: lerp_color(control_bg, accent, 0.08),
            weak_bg_fill: accent.gamma_multiply(0.10),
            bg_stroke: egui::Stroke::new(1.0, accent.gamma_multiply(0.6)),
            corner_radius: rounded_md,
            fg_stroke: egui::Stroke::new(1.0, fg),
            expansion: 0.0,
        };
        visuals.widgets.active = egui::style::WidgetVisuals {
            bg_fill: lerp_color(control_bg, accent, 0.14),
            weak_bg_fill: accent.gamma_multiply(0.16),
            bg_stroke: egui::Stroke::new(1.0, accent),
            corner_radius: rounded_md,
            fg_stroke: egui::Stroke::new(1.0, fg),
            expansion: 0.0,
        };
        visuals.widgets.open = egui::style::WidgetVisuals {
            bg_fill: control_bg,
            weak_bg_fill: accent.gamma_multiply(0.08),
            bg_stroke: egui::Stroke::new(1.0, accent.gamma_multiply(0.5)),
            corner_radius: rounded_md,
            fg_stroke: egui::Stroke::new(1.0, fg),
            expansion: 0.0,
        };
        ctx.set_visuals(visuals);

        // More breathing room around interactive controls.
        ctx.style_mut(|s| {
            s.spacing.button_padding = egui::vec2(8.0, 4.0);
            s.spacing.interact_size.y = 26.0;
            s.spacing.slider_rail_height = 4.0;
        });

        // ------- top bar (traffic-light toolbar) -------
        // Height is twice the measured traffic-light center so the buttons
        // have equal space above and below.
        let light_y = crate::macos::traffic_light_center();
        egui::TopBottomPanel::top("top")
            .exact_height(2.0 * light_y)
            .frame(egui::Frame::new().fill(bg).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
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

        // ------- status bar -------
        self.status_bar(ctx);

        // ------- settings page -------
        self.show_settings(ctx);

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
            Metrics::scaled(ui.ctx().pixels_per_point()),
        );

        // Vertically center the document when it is shorter than the
        // viewport; taller documents stay top-aligned and scrollable. The
        // content height is measured during rendering and stored for the next
        // frame (one frame of lag, then stable).
        let viewport_h = ui.available_height();
        let content_h_id = ui.id().with("preview_content_h");
        let stored_h = ui
            .ctx()
            .data_mut(|d| d.get_temp::<f32>(content_h_id))
            .unwrap_or(0.0);
        let top_pad = ((viewport_h - stored_h) / 2.0).max(0.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(top_pad);
                let content_top = ui.cursor().min.y;
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_max_width(effective);
                        crate::render::render_document(ui, &self.doc, &mut rctx);
                    });
                });
                let measured = ui.cursor().min.y - content_top;
                ui.ctx()
                    .data_mut(|d| d.insert_temp(content_h_id, measured.max(0.0)));
            });
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        let mut editor = Editor::new(&self.cfg, &theme);
        let res = editor.show(ui, &mut self.doc, &mut *self.hl);
        self.cursor = res.cursor;
        if res.changed {
            self.last_edit_saved = Instant::now();
            self.need_reparse = true;
        }
    }

    /// Bottom 24px status bar: file path, word/char count, cursor (edit mode),
    /// save feedback on the left; font size and theme quick switch on the right.
    fn status_bar(&mut self, ctx: &egui::Context) {
        if !self.cfg.show_status_bar {
            return;
        }
        let theme = self.theme().clone();
        let fg = theme.c.foreground;
        let muted = theme.c.muted;
        let hairline = self.metrics(ctx).hairline;
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .frame(
                egui::Frame::new()
                    .fill(theme.c.surface)
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(ctx, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(
                    bar.left()..=bar.right(),
                    bar.top(),
                    egui::Stroke::new(hairline, theme.c.hr),
                );
                let inner = ui.available_rect_before_wrap();
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        match &self.doc.path {
                            Some(p) => {
                                let resp = ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(p.display().to_string())
                                                .size(11.0)
                                                .color(muted),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                    .on_hover_text("在 Finder 中显示");
                                if resp.clicked() {
                                    reveal_in_finder(p);
                                }
                            }
                            None => {
                                ui.label(RichText::new("未命名").size(11.0).color(muted));
                            }
                        }
                        let words = self.doc.text.split_whitespace().count();
                        let chars = self.doc.text.chars().count();
                        ui.label(
                            RichText::new(format!("{words} 词 · {chars} 字符"))
                                .size(11.0)
                                .color(muted),
                        );
                        if self.view == View::Edit {
                            if let Some((line, col)) = self.cursor {
                                ui.label(
                                    RichText::new(format!("{line}:{col}")).size(11.0).color(fg),
                                );
                            }
                        }
                        if let Some((expiry, msg, color)) = &self.feedback {
                            if *expiry > Instant::now() {
                                ui.label(RichText::new(msg).size(11.0).color(*color));
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(6.0);
                            let minus = ui
                                .add(
                                    egui::Button::new(RichText::new("A-").size(11.0).color(muted))
                                        .frame(false),
                                )
                                .on_hover_text("减小字号");
                            if minus.clicked() {
                                self.adjust_font_size(-1.0);
                            }
                            let plus = ui
                                .add(
                                    egui::Button::new(RichText::new("A+").size(11.0).color(muted))
                                        .frame(false),
                                )
                                .on_hover_text("增大字号");
                            if plus.clicked() {
                                self.adjust_font_size(1.0);
                            }
                            let th = ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(self.theme().name.clone()).size(11.0),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("切换主题");
                            if th.clicked() {
                                self.cycle_theme();
                            }
                        });
                    });
                });
            });
    }

    fn adjust_font_size(&mut self, delta: f32) {
        let new = (self.cfg.font_size + delta).clamp(12.0, 28.0);
        if (new - self.cfg.font_size).abs() > 0.01 {
            self.cfg.font_size = new;
            config::save(&self.cfg);
        }
    }

    fn cycle_theme(&mut self) {
        let idx = self
            .registry
            .themes
            .iter()
            .position(|t| t.id == self.theme_id)
            .unwrap_or(0);
        let next = self.registry.themes[(idx + 1) % self.registry.themes.len()]
            .id
            .clone();
        self.switch_theme(&next);
    }
}

fn base_dir_for(doc: &Document) -> PathBuf {
    doc.path
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[cfg(target_os = "macos")]
fn reveal_in_finder(path: &std::path::Path) {
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
}

#[cfg(not(target_os = "macos"))]
fn reveal_in_finder(_path: &std::path::Path) {}
