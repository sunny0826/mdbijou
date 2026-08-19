//! Mermaid diagram rendering via `merman` (headless Mermaid.js-parity
//! renderer, pinned to mermaid@11.12.3) → SVG → `usvg`/`resvg` rasterization
//! → egui texture. Diagrams get the official Mermaid look (rounded nodes,
//! shadows, gradients, theme-adaptive colors) instead of the previous
//! hand-drawn approximation. Unsupported or unparseable sources return
//! `false` so the caller falls back to the plain code block.

use crate::theme::{Theme, ThemeKind};
use egui::{vec2, Color32, ColorImage, Margin, Stroke, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

/// Rendered-texture cache, keyed by a hash of (source, theme, font size).
/// `TextureHandle` is `Send + Sync`, so a process-wide cache is safe; the
/// handle is freed automatically when evicted and the context is dropped.
struct Cache {
    map: HashMap<u64, (TextureHandle, Vec2)>,
    order: VecDeque<u64>,
}

const CACHE_CAP: usize = 64;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            map: HashMap::new(),
            order: VecDeque::new(),
        })
    })
}

fn cache_key(src: &str, theme: &Theme, font_size: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    theme.id.hash(&mut h);
    font_size.to_bits().hash(&mut h);
    h.finish()
}

/// Render a mermaid diagram. Returns `false` when the source is not a
/// supported diagram (caller falls back to the code block).
pub fn render(ui: &mut Ui, src: &str, theme: &Theme, font_size: f32) -> bool {
    let key = cache_key(src, theme, font_size);
    let cache = cache();
    if let Some((tex, size)) = cache.lock().unwrap().map.get(&key) {
        show_image(ui, tex, *size, theme);
        return true;
    }

    let Some(svg) = render_svg(src, theme, font_size) else {
        return false;
    };
    let Some((img, size)) = rasterize(&svg) else {
        return false;
    };

    let tex = ui
        .ctx()
        .load_texture(format!("mermaid-{key}"), img, TextureOptions::LINEAR);

    let mut guard = cache.lock().unwrap();
    guard.map.insert(key, (tex.clone(), size));
    guard.order.push_back(key);
    while guard.order.len() > CACHE_CAP {
        if let Some(old) = guard.order.pop_front() {
            guard.map.remove(&old);
        }
    }
    drop(guard);

    show_image(ui, &tex, size, theme);
    true
}

/// Render the source to a resvg-safe SVG string via merman. Returns `None`
/// when the source is not a recognized diagram or fails to parse.
fn render_svg(src: &str, theme: &Theme, font_size: f32) -> Option<String> {
    let cfg = theme_config(theme, font_size);
    let renderer = merman::render::HeadlessRenderer::new()
        .with_site_config(cfg)
        .with_strict_parsing()
        .with_diagram_id(&format!("mermaid-{:x}", cache_key(src, theme, font_size)));
    renderer
        .render_svg_resvg_safe_sync(src)
        .ok()
        .flatten()
        .map(|svg| strip_svg_background(&svg))
}

/// Mermaid hardcodes `background-color: white` on the SVG root (parity with
/// mermaid.js), ignoring the theme background. Make it transparent so the
/// surrounding card (`code_bg`) shows through with its rounded corners.
fn strip_svg_background(svg: &str) -> String {
    svg.replace("background-color:white", "background-color:transparent")
        .replace("background-color: white", "background-color: transparent")
}

/// Map the app `Theme` onto Mermaid `themeVariables` so diagrams adapt to the
/// current light/dark palette. The flowchart's hardcoded white background is
/// stripped to transparent later (see `strip_svg_background`) so the
/// surrounding card (`code_bg`) shows through with its rounded corners.
fn theme_config(theme: &Theme, font_size: f32) -> merman::MermaidConfig {
    let mut cfg = merman::MermaidConfig::empty_object();
    let dark = theme.kind == ThemeKind::Dark;
    cfg.set_value(
        "theme",
        serde_json::Value::String(if dark { "dark" } else { "default" }.into()),
    );
    let mut tv = |path: &str, c: Color32| {
        cfg.set_value(
            &format!("themeVariables.{path}"),
            serde_json::Value::String(color32_to_css(c)),
        );
    };
    let node_fill = mix(theme.c.background, theme.c.link, 0.08);
    tv("background", theme.c.background);
    tv("primaryColor", node_fill);
    tv("primaryTextColor", theme.c.foreground);
    tv("primaryBorderColor", theme.c.link);
    // Flowchart nodes read these specific theme variables (not the generic ones).
    tv("mainBkg", node_fill);
    tv("nodeBorder", theme.c.link);
    tv("textColor", theme.c.foreground);
    tv("lineColor", theme.c.muted);
    tv("edgeLabelBackground", theme.c.code_bg);
    cfg.set_value(
        "themeVariables.fontFamily",
        serde_json::Value::String("sans-serif".into()),
    );
    cfg.set_value(
        "themeVariables.fontSize",
        serde_json::Value::String(format!("{font_size}px")),
    );
    cfg
}

fn color32_to_css(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Linear blend of two colors by `t` in [0, 1].
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t).round() as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t).round() as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t).round() as u8,
    )
}

/// Rasterize an SVG string into a premultiplied-RGBA `ColorImage` at 2x (for
/// retina crispness) plus the logical display size.
fn rasterize(svg: &str) -> Option<(ColorImage, Vec2)> {
    let svg = fix_double_escaped_xml_entities(svg);
    let mut fontdb = resvg::usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let opt = resvg::usvg::Options {
        fontdb: std::sync::Arc::new(fontdb),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &opt).ok()?;

    let size = tree.size();
    let scale = 2.0;
    let w = (size.width() * scale).ceil() as u32;
    let h = (size.height() * scale).ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    let mut pm = pixmap.as_mut();
    resvg::render(&tree, transform, &mut pm);

    let img = ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data());
    Some((img, vec2(size.width(), size.height())))
}

/// Undo double-escaped XML entities that some Mermaid label pipelines emit
/// (e.g. `&amp;lt;` → `&lt;`), so `usvg` parses the markup correctly.
fn fix_double_escaped_xml_entities(svg: &str) -> String {
    svg.replace("&amp;lt;", "&lt;")
        .replace("&amp;gt;", "&gt;")
        .replace("&amp;quot;", "&quot;")
        .replace("&amp;#39;", "&#39;")
        .replace("&amp;amp;", "&amp;")
}

/// Show the cached texture inside a rounded card with horizontal scrolling so
/// wide (LR) diagrams scroll instead of being squeezed by the parent.
fn show_image(ui: &mut Ui, tex: &TextureHandle, size: Vec2, theme: &Theme) {
    let frame = egui::Frame::new()
        .fill(theme.c.code_bg)
        .stroke(Stroke::new(1.0, theme.c.table_border))
        .corner_radius(4.0)
        .inner_margin(Margin::symmetric(8, 8));
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.add(egui::Image::new(tex).max_size(size));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    fn light() -> Theme {
        theme::builtin("github-light").unwrap()
    }

    fn dark() -> Theme {
        theme::builtin("github-dark").unwrap()
    }

    #[test]
    fn renders_flowchart_to_svg() {
        let svg = render_svg(
            "graph TD\nA[Start] --> B{Check}\nB -->|Yes| C[Done]",
            &light(),
            16.0,
        );
        let svg = svg.expect("flowchart should render");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Start"));
    }

    #[test]
    fn renders_lr_flowchart() {
        let svg = render_svg("flowchart LR\nA --> B --> C", &dark(), 16.0);
        assert!(svg.is_some());
    }

    #[test]
    fn rejects_non_mermaid() {
        assert!(render_svg("let x = 1;", &light(), 16.0).is_none());
        assert!(render_svg("", &light(), 16.0).is_none());
        assert!(render_svg("   \n  ", &light(), 16.0).is_none());
    }

    #[test]
    fn theme_config_adapts_to_dark() {
        let cfg = theme_config(&dark(), 16.0);
        assert_eq!(cfg.get_str("theme"), Some("dark"));
        assert_eq!(cfg.get_str("themeVariables.textColor"), Some("#c9d1d9"));
        assert_eq!(cfg.get_str("themeVariables.nodeBorder"), Some("#58a6ff"));
        assert_eq!(cfg.get_str("themeVariables.background"), Some("#0d1117"));
        assert_eq!(cfg.get_str("themeVariables.fontSize"), Some("16px"));
    }

    #[test]
    fn theme_config_adapts_to_light() {
        let cfg = theme_config(&light(), 14.0);
        assert_eq!(cfg.get_str("theme"), Some("default"));
        assert_eq!(cfg.get_str("themeVariables.textColor"), Some("#24292e"));
        assert_eq!(cfg.get_str("themeVariables.fontSize"), Some("14px"));
    }

    #[test]
    fn fixes_double_escaped_entities() {
        assert_eq!(
            fix_double_escaped_xml_entities("&amp;lt;b&amp;gt;x&amp;lt;/b&amp;gt;"),
            "&lt;b&gt;x&lt;/b&gt;"
        );
        assert_eq!(
            fix_double_escaped_xml_entities("a &amp;amp; b"),
            "a &amp; b"
        );
    }

    #[test]
    fn strips_hardcoded_svg_background() {
        assert_eq!(
            strip_svg_background(r#"<svg style="max-width:130px;background-color:white">"#),
            r#"<svg style="max-width:130px;background-color:transparent">"#
        );
        assert_eq!(
            strip_svg_background(r#"<svg style="background-color: white">"#),
            r#"<svg style="background-color: transparent">"#
        );
    }

    #[test]
    fn rasterizes_simple_svg() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
            <rect x="0" y="0" width="100" height="50" fill="#ff0000"/>
        </svg>"##;
        let (img, size) = rasterize(svg).expect("svg should rasterize");
        assert_eq!(size, vec2(100.0, 50.0));
        assert_eq!(img.size, [200, 100]);
        assert!(img.pixels.iter().any(|p| p.r() == 255 && p.g() == 0));
    }

    #[test]
    fn full_pipeline_renders_cjk_flowchart() {
        // End-to-end: merman SVG -> usvg (system fonts) -> resvg raster.
        let src = "graph TD\n  A[开始] --> B{判断}\n  B -->|是| C(处理)\n  C --> D[结束]";
        let svg = render_svg(src, &light(), 16.0).expect("flowchart should render");
        let (img, size) = rasterize(&svg).expect("svg should rasterize");
        assert!(size.x > 0.0 && size.y > 0.0);
        // Some non-transparent pixels must be present (nodes + text drawn).
        assert!(img.pixels.iter().any(|p| p.a() > 0));
    }
}
