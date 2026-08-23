//! GPUI application shell during the renderer migration.
//!
//! It owns native interactions only; Markdown semantics remain in `Document`.

use crate::config::{self, Config, View};
use crate::document::Document;
use crate::file_types::{DEFAULT_DOCUMENT_NAME, DOCUMENT_EXTENSIONS, DOCUMENT_FILTER_NAME};
use crate::gpui_preview;
use crate::install;
use crate::theme::{self, Theme};
use crate::toc;
use gpui::prelude::FluentBuilder;
use gpui::{
    actions, div, px, size, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, Timer, Window, WindowAppearance, WindowBounds, WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    scroll::{ScrollableElement, ScrollbarShow},
    switch::Switch,
    theme::Theme as ComponentTheme,
    IconName, Root, TitleBar,
};
use std::path::PathBuf;
use std::time::Duration;

actions!(
    mdbijou,
    [
        ToggleView,
        SaveDocument,
        ReloadDocument,
        OpenDocument,
        ToggleSettings,
        ToggleToc,
        IncreasePreviewFont,
        DecreasePreviewFont,
    ]
);

pub struct GpuiMdbijouApp {
    cfg: Config,
    theme: Theme,
    document: Document,
    title: SharedString,
    view: View,
    editor: Entity<InputState>,
    show_toc: bool,
    active_toc: Option<usize>,
    pending_open: Option<PathBuf>,
    feedback: Option<SharedString>,
    preview_scroll: ScrollHandle,
    remote_images: gpui_preview::RemoteImageStore,
    remote_image_poll_task: Option<Task<()>>,
    open_rx: std::sync::mpsc::Receiver<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl GpuiMdbijouApp {
    pub fn new(
        document: Document,
        cfg: Config,
        open_rx: std::sync::mpsc::Receiver<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = theme::builtin(&cfg.theme)
            .or_else(|| theme::builtin("github-light"))
            .expect("builtin fallback theme exists");
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(if cfg.highlight { "markdown" } else { "text" })
                .line_number(cfg.show_line_numbers)
                .tab_size(gpui_component::input::TabSize {
                    tab_size: cfg.tab_size.clamp(1, 16),
                    hard_tabs: false,
                })
                .default_value(document.text.clone())
        });
        let subscriptions = vec![cx.subscribe_in(
            &editor,
            window,
            |this, editor, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.document.text = editor.read(cx).value().to_string();
                    this.document.dirty = true;
                    this.feedback = None;
                    this.auto_save(window, cx);
                    window.set_window_title(&this.window_title());
                    cx.notify();
                }
            },
        )];

        let mut app = Self {
            title: document_title(&document).into(),
            view: cfg.default_view,
            show_toc: cfg.show_toc,
            active_toc: None,
            cfg,
            theme,
            document,
            editor,
            pending_open: None,
            feedback: None,
            preview_scroll: ScrollHandle::default(),
            remote_images: gpui_preview::RemoteImageStore::default(),
            remote_image_poll_task: None,
            open_rx,
            _subscriptions: subscriptions,
        };
        app.apply_follow_system_theme(window);
        app.sync_component_theme(cx);
        window.set_window_title(&app.window_title());
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            if this.cfg.follow_system_theme {
                this.apply_follow_system_theme(window);
                this.sync_component_theme(cx);
                cx.notify();
            }
        });
        app._subscriptions.push(appearance_subscription);
        app
    }

    fn window_title(&self) -> String {
        if self.document.dirty {
            format!("{} — 未保存", self.title)
        } else {
            self.title.to_string()
        }
    }

    /// Poll the worker-owned HTTP image cache without blocking GPUI's UI
    /// thread. A completed request invalidates the preview exactly once.
    fn poll_remote_images(&mut self, cx: &mut Context<Self>) {
        self.remote_image_poll_task = None;
        if self.remote_images.poll() {
            cx.notify();
        }
        if self.remote_images.has_pending() {
            self.remote_image_poll_task = Some(cx.spawn(async move |this, cx| {
                Timer::after(Duration::from_millis(80)).await;
                if let Some(this) = this.upgrade() {
                    let _ = this.update(cx, |this, cx| this.poll_remote_images(cx));
                }
            }));
        }
    }

    fn apply_follow_system_theme(&mut self, window: &Window) {
        if !self.cfg.follow_system_theme {
            return;
        }
        let id = match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => "bijou-dark",
            _ => "bijou-light",
        };
        if let Some(theme) = theme::builtin(id) {
            self.theme = theme;
            self.cfg.theme = id.into();
        }
    }

    /// Keep gpui-component controls (buttons, editor, scrollbars) in the
    /// same Paper & Jewel palette as the document renderer. Without this,
    /// those controls silently keep GPUI's stock blue/gray theme.
    fn sync_component_theme(&self, cx: &mut Context<Self>) {
        let c = &self.theme.c;
        let colors = &mut ComponentTheme::global_mut(cx).colors;
        colors.background = theme_color(c.background);
        colors.foreground = theme_color(c.foreground);
        colors.border = theme_color(c.table_border);
        colors.accent = theme_color(c.surface_hover);
        colors.accent_foreground = theme_color(c.foreground);
        colors.primary = theme_color(c.link);
        colors.primary_hover = theme_color(c.focus);
        colors.primary_active = theme_color(c.focus);
        colors.primary_foreground = theme_color(c.background);
        colors.secondary = theme_color(c.surface);
        colors.secondary_hover = theme_color(c.surface_hover);
        colors.secondary_active = theme_color(c.surface_hover);
        colors.secondary_foreground = theme_color(c.foreground);
        colors.muted = theme_color(c.surface);
        colors.muted_foreground = theme_color(c.muted);
        colors.link = theme_color(c.link);
        colors.link_hover = theme_color(c.focus);
        colors.link_active = theme_color(c.focus);
        colors.input = theme_color(c.table_border);
        colors.caret = theme_color(c.focus);
        colors.selection = theme_color(c.selection_bg);
        colors.ring = theme_color(c.focus);
        colors.popover = theme_color(c.surface);
        colors.popover_foreground = theme_color(c.foreground);
        colors.overlay = theme_color(c.background);
        colors.title_bar = theme_color(c.surface);
        colors.title_bar_border = theme_color(c.table_border);
        colors.scrollbar = theme_color(c.surface);
        colors.scrollbar_thumb = theme_color(c.muted);
        colors.scrollbar_thumb_hover = theme_color(c.link);
        colors.success = theme_color(c.success);
        colors.success_foreground = theme_color(c.background);
        colors.danger = theme_color(c.error);
        colors.danger_foreground = theme_color(c.background);
        ComponentTheme::global_mut(cx).scrollbar_show = ScrollbarShow::Always;
    }

    fn set_document(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.title = document_title(&document).into();
        self.editor.update(cx, |editor, cx| {
            editor.set_value(document.text.clone(), window, cx);
        });
        self.document = document;
        self.view = self.cfg.default_view;
        self.active_toc = None;
        self.pending_open = None;
        window.set_window_title(&self.window_title());
        cx.notify();
    }

    fn request_open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.document.dirty {
            self.pending_open = Some(path);
            cx.notify();
        } else {
            self.apply_open(path, window, cx);
        }
    }

    fn apply_open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let document = match std::fs::read_to_string(&path) {
            Ok(text) => Document::with_path(path, text),
            Err(error) => Document::with_path(path, format!("# 无法打开文件\n\n{error}")),
        };
        self.set_document(document, window, cx);
    }

    fn open_via_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(DOCUMENT_FILTER_NAME, DOCUMENT_EXTENSIONS)
            .pick_file()
        {
            self.request_open(path, window, cx);
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.document.path.clone().or_else(|| {
            rfd::FileDialog::new()
                .add_filter(DOCUMENT_FILTER_NAME, DOCUMENT_EXTENSIONS)
                .set_file_name(DEFAULT_DOCUMENT_NAME)
                .save_file()
        });
        let Some(path) = path else {
            self.feedback = Some("保存已取消".into());
            cx.notify();
            return;
        };
        match config::atomic_write(&path, self.document.text.as_bytes()) {
            Ok(()) => {
                self.document.path = Some(path);
                self.document.dirty = false;
                self.title = document_title(&self.document).into();
                self.feedback = Some("已保存".into());
                window.set_window_title(&self.window_title());
            }
            Err(error) => self.feedback = Some(format!("保存失败：{error}").into()),
        }
        let _ = window;
        cx.notify();
    }

    fn auto_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.cfg.auto_save || !self.document.dirty {
            return;
        }
        let Some(path) = self.document.path.as_deref() else {
            return;
        };
        match config::atomic_write(path, self.document.text.as_bytes()) {
            Ok(()) => {
                self.document.dirty = false;
                self.feedback = Some("已自动保存".into());
            }
            Err(error) => self.feedback = Some(format!("自动保存失败：{error}").into()),
        }
        cx.notify();
    }

    fn toggle_view(&mut self, cx: &mut Context<Self>) {
        if self.view == View::Edit {
            self.document.reparse();
            self.view = View::Preview;
        } else {
            self.view = View::Edit;
        }
        cx.notify();
    }

    fn set_preview(&mut self, preview: bool, cx: &mut Context<Self>) {
        if preview != (self.view == View::Preview) {
            self.toggle_view(cx);
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        let app = cx.entity();
        // The settings window is created while this entity is mutably borrowed by
        // the click handler. Give it an initial snapshot so its first render does
        // not try to read `app` re-entrantly.
        let cfg = self.cfg.clone();
        let theme = self.theme.clone();
        let bounds = Bounds::centered(None, size(px(640.0), px(740.0)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(Default::default()),
                ..Default::default()
            },
            move |window, cx| {
                let settings = cx.new(|cx| SettingsWindow::new(app, cfg, theme, cx));
                cx.new(|cx| Root::new(settings, window, cx))
            },
        ) {
            self.feedback = Some(format!("无法打开设置窗口：{error}").into());
            cx.notify();
        }
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.document.dirty {
            if let Some(path) = self.document.path.clone() {
                self.apply_open(path, window, cx);
            }
        } else {
            self.feedback = Some("请先保存或放弃未保存的修改".into());
            cx.notify();
        }
    }

    fn toggle_toc(&mut self, cx: &mut Context<Self>) {
        self.show_toc = !self.show_toc;
        self.cfg.show_toc = self.show_toc;
        config::save(&self.cfg);
        cx.notify();
    }

    fn switch_theme(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let Some(theme) = theme::builtin(id) else {
            self.feedback = Some("未知主题".into());
            cx.notify();
            return;
        };
        self.theme = theme;
        self.cfg.theme = id.into();
        self.cfg.follow_system_theme = false;
        self.sync_component_theme(cx);
        config::save(&self.cfg);
        self.feedback = Some(format!("已切换到 {}", self.cfg.theme).into());
        cx.notify();
    }

    fn adjust_preview_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        let updated = (self.cfg.font_size + delta).clamp(12.0, 28.0);
        if (updated - self.cfg.font_size).abs() > f32::EPSILON {
            self.cfg.font_size = updated;
            config::save(&self.cfg);
            cx.notify();
        }
    }

    fn adjust_editor_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        let updated = (self.cfg.editor_font_size + delta).clamp(12.0, 28.0);
        if (updated - self.cfg.editor_font_size).abs() > f32::EPSILON {
            self.cfg.editor_font_size = updated;
            config::save(&self.cfg);
            cx.notify();
        }
    }

    fn adjust_content_width(&mut self, delta: f32, cx: &mut Context<Self>) {
        let updated = (self.cfg.content_width + delta).clamp(420.0, 1100.0);
        if (updated - self.cfg.content_width).abs() > f32::EPSILON {
            self.cfg.content_width = updated;
            config::save(&self.cfg);
            cx.notify();
        }
    }

    fn adjust_line_height(&mut self, delta: f32, cx: &mut Context<Self>) {
        let updated = (self.cfg.line_height + delta).clamp(1.1, 2.0);
        if (updated - self.cfg.line_height).abs() > f32::EPSILON {
            self.cfg.line_height = updated;
            config::save(&self.cfg);
            cx.notify();
        }
    }

    fn cycle_preview_font(&mut self, cx: &mut Context<Self>) {
        const FONTS: [(&str, &str); 5] = [
            ("default", "系统默认"),
            ("pingfang", "苹方"),
            ("hiragino", "冬青黑体"),
            ("songti", "宋体"),
            ("heiti", "黑体"),
        ];
        let index = FONTS
            .iter()
            .position(|(id, _)| *id == self.cfg.font_family)
            .unwrap_or(0);
        let next = FONTS[(index + 1) % FONTS.len()];
        self.cfg.font_family = next.0.into();
        self.feedback = Some(format!("正文字体：{}", next.1).into());
        config::save(&self.cfg);
        cx.notify();
    }

    fn toggle_follow_system_theme(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.cfg.follow_system_theme = !self.cfg.follow_system_theme;
        self.apply_follow_system_theme(window);
        self.sync_component_theme(cx);
        config::save(&self.cfg);
        cx.notify();
    }

    fn toggle_highlight(&mut self, cx: &mut Context<Self>) {
        self.cfg.highlight = !self.cfg.highlight;
        self.editor.update(cx, |editor, cx| {
            editor.set_highlighter(
                if self.cfg.highlight {
                    "markdown"
                } else {
                    "text"
                },
                cx,
            );
        });
        config::save(&self.cfg);
        cx.notify();
    }

    fn cycle_tab_size(&mut self, cx: &mut Context<Self>) {
        self.cfg.tab_size = match self.cfg.tab_size {
            2 => 4,
            4 => 8,
            _ => 2,
        };
        self.feedback = Some(format!("Tab 宽度：{}", self.cfg.tab_size).into());
        config::save(&self.cfg);
        // gpui-component currently exposes tab size only during InputState
        // construction. Preserve the chosen value for the next editor session.
        cx.notify();
    }

    fn install_cli(&mut self, cx: &mut Context<Self>) {
        let result = install::install_cli();
        self.feedback = Some(result.message.into());
        cx.notify();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.document.dirty {
            format!("{} · 未保存", self.title)
        } else {
            self.title.to_string()
        };
        TitleBar::new().child(
            div()
                .relative()
                .w_full()
                .h_full()
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    Button::new("open")
                                        .compact()
                                        .icon(IconName::FolderOpen)
                                        .tooltip("打开文档（⌘O）")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_via_dialog(window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("save")
                                        .compact()
                                        .icon(IconName::File)
                                        .tooltip("保存（⌘S）")
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.save(window, cx)
                                            }),
                                        ),
                                )
                                .child(
                                    Button::new("reload")
                                        .compact()
                                        .icon(IconName::Undo2)
                                        .tooltip("重新载入（⌘R）")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reload(window, cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    Button::new("toc")
                                        .compact()
                                        .icon(if self.show_toc {
                                            IconName::PanelLeftClose
                                        } else {
                                            IconName::PanelLeftOpen
                                        })
                                        .tooltip("显示或隐藏目录（⌘T）")
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.toggle_toc(cx)),
                                        ),
                                )
                                .child(
                                    Button::new("settings")
                                        .compact()
                                        .icon(IconName::Settings2)
                                        .tooltip("设置（⌘,）")
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.open_settings(cx)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .p(px(2.0))
                                        .gap(px(2.0))
                                        .rounded(px(theme::RADIUS_SM))
                                        .bg(theme_color(self.theme.c.code_bg))
                                        .border_1()
                                        .border_color(theme_color(self.theme.c.table_border))
                                        .child(view_segment(
                                            "view-edit",
                                            "编辑",
                                            self.view == View::Edit,
                                            &self.theme,
                                            cx.listener(|this, _, _, cx| {
                                                this.set_preview(false, cx)
                                            }),
                                        ))
                                        .child(view_segment(
                                            "view-preview",
                                            "预览",
                                            self.view == View::Preview,
                                            &self.theme,
                                            cx.listener(|this, _, _, cx| {
                                                this.set_preview(true, cx)
                                            }),
                                        )),
                                ),
                        ),
                ),
        )
    }

    fn toc_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .w(px(248.0))
            .h_full()
            .p(px(16.0))
            .bg(theme_color(self.theme.c.surface))
            .border_r_1()
            .border_color(theme_color(self.theme.c.table_border))
            .text_color(theme_color(self.theme.c.muted))
            .child(
                div()
                    .mb(px(12.0))
                    .px(px(8.0))
                    .text_xs()
                    .font_family("Menlo")
                    .text_color(theme_color(self.theme.c.muted))
                    .child("目录"),
            );
        for (index, location) in toc::extract_with_root_indices(&self.document.blocks)
            .into_iter()
            .enumerate()
        {
            let active = self.active_toc == Some(index);
            let indent = px(8.0 + (location.entry.level.saturating_sub(1).min(3) as f32 * 12.0));
            panel = panel.child(
                div()
                    .id(("toc-entry", index))
                    .relative()
                    .h(px(30.0))
                    .mb(px(2.0))
                    .pl(indent)
                    .pr(px(8.0))
                    .rounded(px(theme::RADIUS_SM))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .when(active, |this| {
                        this.bg(theme_color(self.theme.c.selection_bg))
                            .text_color(theme_color(self.theme.c.link))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .hover(|this| this.bg(theme_color(self.theme.c.surface_hover)))
                    .when(active, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .top(px(7.0))
                                .bottom(px(7.0))
                                .w(px(3.0))
                                .rounded(px(2.0))
                                .bg(theme_color(self.theme.c.link)),
                        )
                    })
                    .child(div().truncate().child(location.entry.title))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_toc = Some(index);
                        this.view = View::Preview;
                        this.preview_scroll
                            .scroll_to_top_of_item(location.root_block_index);
                        cx.notify();
                    })),
            );
        }
        panel
    }

    fn open_confirmation(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.pending_open.as_ref().map(|path| {
            let name = path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x0000001f))
                .child(
                    div()
                        .w(px(420.0))
                        .p(px(24.0))
                        .rounded(px(theme::RADIUS_LG))
                        .bg(theme_color(self.theme.c.background))
                        .text_color(theme_color(self.theme.c.foreground))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("未保存的更改"),
                        )
                        .child(
                            div()
                                .mt(px(10.0))
                                .text_color(theme_color(self.theme.c.muted))
                                .child(format!("当前文档有未保存的修改。是否打开“{name}”？")),
                        )
                        .child(
                            div()
                                .mt(px(20.0))
                                .flex()
                                .justify_end()
                                .gap(px(8.0))
                                .child(Button::new("cancel-open").label("取消").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.pending_open = None;
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    Button::new("discard-open")
                                        .primary()
                                        .label("放弃并打开")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            if let Some(path) = this.pending_open.clone() {
                                                this.apply_open(path, window, cx);
                                            }
                                        })),
                                ),
                        ),
                )
        })
    }
}

/// A separate native window so configuration never replaces the reading flow.
struct SettingsWindow {
    app: Entity<GpuiMdbijouApp>,
    cfg: Config,
    theme: Theme,
    _subscription: Subscription,
}

impl SettingsWindow {
    fn new(app: Entity<GpuiMdbijouApp>, cfg: Config, theme: Theme, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&app, |this, app, cx| {
            let app_state = app.read(cx);
            this.cfg = app_state.cfg.clone();
            this.theme = app_state.theme.clone();
            cx.notify();
        });
        Self {
            app,
            cfg,
            theme,
            _subscription: subscription,
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cfg = self.cfg.clone();
        let theme = self.theme.clone();
        let c = &theme.c;

        let mut themes = div().mt(px(12.0)).flex().flex_wrap().gap(px(8.0));
        for (index, (id, name, _)) in theme::builtin_ids().into_iter().enumerate() {
            let app = self.app.clone();
            themes = themes.child(Button::new(("theme", index)).label(name).on_click(
                move |_, _, cx| {
                    app.update(cx, |this, cx| this.switch_theme(id, cx));
                },
            ));
        }

        let preview_down = self.app.clone();
        let preview_up = self.app.clone();
        let editor_down = self.app.clone();
        let editor_up = self.app.clone();
        let font_family = self.app.clone();
        let width_down = self.app.clone();
        let width_up = self.app.clone();
        let line_down = self.app.clone();
        let line_up = self.app.clone();
        let auto_save = self.app.clone();
        let status_bar = self.app.clone();
        let line_numbers = self.app.clone();
        let highlight = self.app.clone();
        let tab_size = self.app.clone();
        let follow_system = self.app.clone();
        let install_cli = self.app.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme_color(c.background))
            .text_color(theme_color(c.foreground))
            .child(
                div()
                    .h(px(52.0))
                    .px(px(20.0))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme_color(c.table_border))
                    .bg(theme_color(c.surface))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("设置")),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(560.0))
                            .mx_auto()
                            .p(px(28.0))
                            .child(div().text_color(theme_color(c.muted)).child(format!(
                                "当前主题：{} · 正文 {} pt · 编辑器 {} pt",
                                cfg.theme, cfg.font_size, cfg.editor_font_size
                            )))
                            .child(
                                div()
                                    .mt(px(28.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("主题"),
                            )
                            .child(themes)
                            .child(
                                div()
                                    .mt(px(28.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("排版"),
                            )
                            .child(
                                div()
                                    .mt(px(12.0))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(8.0))
                                    .child(
                                        Button::new("preview-font-down").label("正文 A−").on_click(
                                            move |_, _, cx| {
                                                preview_down.update(cx, |this, cx| {
                                                    this.adjust_preview_font(-1.0, cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        Button::new("preview-font-up").label("正文 A+").on_click(
                                            move |_, _, cx| {
                                                preview_up.update(cx, |this, cx| {
                                                    this.adjust_preview_font(1.0, cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        Button::new("editor-font-down").label("编辑 A−").on_click(
                                            move |_, _, cx| {
                                                editor_down.update(cx, |this, cx| {
                                                    this.adjust_editor_font(-1.0, cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        Button::new("editor-font-up").label("编辑 A+").on_click(
                                            move |_, _, cx| {
                                                editor_up.update(cx, |this, cx| {
                                                    this.adjust_editor_font(1.0, cx)
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(8.0))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(8.0))
                                    .child(
                                        Button::new("font-family").label("切换正文字体").on_click(
                                            move |_, _, cx| {
                                                font_family.update(cx, |this, cx| {
                                                    this.cycle_preview_font(cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(Button::new("width-down").label("列宽 −").on_click(
                                        move |_, _, cx| {
                                            width_down.update(cx, |this, cx| {
                                                this.adjust_content_width(-60.0, cx)
                                            });
                                        },
                                    ))
                                    .child(Button::new("width-up").label("列宽 +").on_click(
                                        move |_, _, cx| {
                                            width_up.update(cx, |this, cx| {
                                                this.adjust_content_width(60.0, cx)
                                            });
                                        },
                                    ))
                                    .child(Button::new("line-down").label("行距 −").on_click(
                                        move |_, _, cx| {
                                            line_down.update(cx, |this, cx| {
                                                this.adjust_line_height(-0.1, cx)
                                            });
                                        },
                                    ))
                                    .child(Button::new("line-up").label("行距 +").on_click(
                                        move |_, _, cx| {
                                            line_up.update(cx, |this, cx| {
                                                this.adjust_line_height(0.1, cx)
                                            });
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .mt(px(28.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("文档行为"),
                            )
                            .child(
                                div()
                                    .mt(px(12.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .child(
                                        Switch::new("auto-save")
                                            .checked(cfg.auto_save)
                                            .label("自动保存")
                                            .on_click(move |value, _, cx| {
                                                auto_save.update(cx, |this, cx| {
                                                    this.cfg.auto_save = *value;
                                                    config::save(&this.cfg);
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("status-bar")
                                            .checked(cfg.show_status_bar)
                                            .label("显示状态栏")
                                            .on_click(move |value, _, cx| {
                                                status_bar.update(cx, |this, cx| {
                                                    this.cfg.show_status_bar = *value;
                                                    config::save(&this.cfg);
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("line-numbers")
                                            .checked(cfg.show_line_numbers)
                                            .label("显示行号")
                                            .on_click(move |value, window, cx| {
                                                line_numbers.update(cx, |this, cx| {
                                                    this.cfg.show_line_numbers = *value;
                                                    this.editor.update(cx, |editor, cx| {
                                                        editor.set_line_number(*value, window, cx);
                                                    });
                                                    config::save(&this.cfg);
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("highlight")
                                            .checked(cfg.highlight)
                                            .label("语法高亮")
                                            .on_click(move |value, _, cx| {
                                                highlight.update(cx, |this, cx| {
                                                    if this.cfg.highlight != *value {
                                                        this.toggle_highlight(cx);
                                                    }
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("follow-system-theme")
                                            .checked(cfg.follow_system_theme)
                                            .label("跟随系统主题")
                                            .on_click(move |value, window, cx| {
                                                follow_system.update(cx, |this, cx| {
                                                    if this.cfg.follow_system_theme != *value {
                                                        this.toggle_follow_system_theme(window, cx);
                                                    }
                                                });
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(16.0))
                                    .flex()
                                    .gap(px(8.0))
                                    .child(
                                        Button::new("tab-size")
                                            .label(format!("Tab：{}", cfg.tab_size))
                                            .on_click(move |_, _, cx| {
                                                tab_size
                                                    .update(cx, |this, cx| this.cycle_tab_size(cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("install-cli")
                                            .icon(IconName::SquareTerminal)
                                            .label("安装 mdb 命令")
                                            .on_click(move |_, _, cx| {
                                                install_cli
                                                    .update(cx, |this, cx| this.install_cli(cx));
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}

impl Render for GpuiMdbijouApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        while let Ok(path) = self.open_rx.try_recv() {
            self.request_open(path, window, cx);
        }
        let body = if self.view == View::Preview {
            div()
                .id("preview-frame")
                .relative()
                .flex_1()
                .min_h_0()
                .vertical_scrollbar(&self.preview_scroll)
                .child(
                    div()
                        .id("preview-scroll")
                        .size_full()
                        .flex()
                        .flex_col()
                        .pt(px(40.0))
                        .pb(px(96.0))
                        .track_scroll(&self.preview_scroll)
                        .overflow_y_scroll()
                        .children(gpui_preview::document_blocks(
                            &self.document,
                            &self.theme,
                            &self.cfg,
                            &mut self.remote_images,
                        )),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .p(px(16.0))
                .text_size(px(self.cfg.editor_font_size.clamp(12.0, 28.0)))
                .child(Input::new(&self.editor).h_full())
                .into_any_element()
        };
        self.poll_remote_images(cx);
        let mut main = div().flex_1().min_h_0().flex();
        if self.show_toc {
            main = main.child(self.toc_panel(cx));
        }
        main = main.child(body);
        let mut content = div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme_color(self.theme.c.background))
            .text_color(theme_color(self.theme.c.foreground))
            .child(
                div()
                    .w_full()
                    .border_b_1()
                    .border_color(theme_color(self.theme.c.table_border))
                    .child(self.toolbar(cx)),
            )
            .child(main);
        if let Some(feedback) = &self.feedback {
            content = content.child(
                div()
                    .h(px(28.0))
                    .px(px(20.0))
                    .text_sm()
                    .text_color(theme_color(self.theme.c.success))
                    .child(feedback.clone()),
            );
        }
        if self.cfg.show_status_bar {
            content = content.child(self.status_bar());
        }
        if let Some(confirmation) = self.open_confirmation(cx) {
            content = content.child(confirmation);
        }
        content
            .on_action(cx.listener(|this, _: &ToggleView, _, cx| this.toggle_view(cx)))
            .on_action(cx.listener(|this, _: &SaveDocument, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &ReloadDocument, window, cx| this.reload(window, cx)))
            .on_action(
                cx.listener(|this, _: &OpenDocument, window, cx| this.open_via_dialog(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.open_settings(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleToc, _, cx| this.toggle_toc(cx)))
            .on_action(
                cx.listener(|this, _: &IncreasePreviewFont, _, cx| {
                    this.adjust_preview_font(1.0, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &DecreasePreviewFont, _, cx| {
                this.adjust_preview_font(-1.0, cx)
            }))
    }
}

impl GpuiMdbijouApp {
    fn status_bar(&self) -> impl IntoElement {
        let words = self.document.text.split_whitespace().count();
        let lines = self.document.text.lines().count().max(1);
        div()
            .h(px(28.0))
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(theme_color(self.theme.c.table_border))
            .bg(theme_color(self.theme.c.surface))
            .text_xs()
            .text_color(theme_color(self.theme.c.muted))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .size(px(6.0))
                            .rounded(px(3.0))
                            .bg(theme_color(self.theme.c.link)),
                    )
                    .child(if self.view == View::Preview {
                        "预览"
                    } else {
                        "编辑"
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(format!("{lines} 行 · {words} 词"))
                    .child(
                        div()
                            .font_family("Menlo")
                            .text_color(theme_color(self.theme.c.muted))
                            .child(self.theme.id.clone()),
                    ),
            )
    }
}

fn document_title(document: &Document) -> String {
    document
        .path
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled.md".into())
}

/// A single segment of the view segmented control in the toolbar.
fn view_segment(
    id: &'static str,
    label: &'static str,
    active: bool,
    theme: &Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .text_sm()
        .cursor_pointer()
        .font_weight(if active {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::MEDIUM
        })
        .text_color(theme_color(if active {
            theme.c.foreground
        } else {
            theme.c.muted
        }))
        .when(active, |segment| {
            segment
                .bg(theme_color(theme.c.background))
                .border_1()
                .border_color(theme_color(theme.c.table_border))
        })
        .hover(|segment| {
            segment.bg(theme_color(if active {
                theme.c.background
            } else {
                theme.c.surface_hover
            }))
        })
        .child(label)
        .on_click(on_click)
}

fn theme_color(color: crate::color::Color) -> gpui::Hsla {
    gpui::rgba(
        ((color.r() as u32) << 24)
            | ((color.g() as u32) << 16)
            | ((color.b() as u32) << 8)
            | color.a() as u32,
    )
    .into()
}
