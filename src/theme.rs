//! Theme system: color model, builtin themes, syntax-highlight palette.

use egui::Color32;

#[derive(Debug, Clone)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub kind: ThemeKind,
    pub c: Colors,
    pub syntax: SyntaxColors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Light,
    Dark,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // full theme palette is part of the design API; some fields render-ready only
pub struct Colors {
    pub background: Color32,
    pub foreground: Color32,
    pub heading: Color32,
    pub muted: Color32,
    pub code_fg: Color32,
    pub code_bg: Color32,
    pub blockquote_fg: Color32,
    pub blockquote_bar: Color32,
    pub link: Color32,
    pub table_border: Color32,
    pub table_header_bg: Color32,
    pub hr: Color32,
    pub selection_bg: Color32,
    pub image_bg: Color32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // syntax palette consumed by the highlighter map; lite mode uses subset
pub struct SyntaxColors {
    pub comment: Color32,
    pub keyword: Color32,
    pub string: Color32,
    pub number: Color32,
    pub function: Color32,
    pub typ: Color32,
    pub variable: Color32,
    pub operator: Color32,
    pub punctuation: Color32,
    pub constant: Color32,
    pub markup_heading: Color32,
    pub markup_link: Color32,
    pub markup_code: Color32,
}

fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Color32::from_rgb(((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8)
}

// ---------------------------------------------------------------------------
// Builtin themes
// ---------------------------------------------------------------------------

pub fn builtin(id: &str) -> Option<Theme> {
    match id {
        "github-light" => Some(github_light()),
        "github-dark" => Some(github_dark()),
        "sepia" => Some(sepia()),
        _ => None,
    }
}

pub fn builtin_ids() -> Vec<(&'static str, &'static str, ThemeKind)> {
    vec![
        ("github-light", "GitHub Light", ThemeKind::Light),
        ("github-dark", "GitHub Dark", ThemeKind::Dark),
        ("sepia", "Sepia", ThemeKind::Light),
    ]
}

fn github_light() -> Theme {
    let c = Colors {
        background: hex("#ffffff"),
        foreground: hex("#24292e"),
        heading: hex("#1f2328"),
        muted: hex("#57606a"),
        code_fg: hex("#e01e5a"),
        code_bg: hex("#f6f8fa"),
        blockquote_fg: hex("#57606a"),
        blockquote_bar: hex("#d0d7de"),
        link: hex("#0969da"),
        table_border: hex("#d8dee4"),
        table_header_bg: hex("#f6f8fa"),
        hr: hex("#d8dee4"),
        selection_bg: hex("#b6d7ff"),
        image_bg: hex("#f6f8fa"),
    };
    let syntax = SyntaxColors {
        comment: hex("#6e7781"),
        keyword: hex("#cf222e"),
        string: hex("#0a3069"),
        number: hex("#0550ae"),
        function: hex("#8250df"),
        typ: hex("#953800"),
        variable: hex("#24292e"),
        operator: hex("#000000"),
        punctuation: hex("#57606a"),
        constant: hex("#0550ae"),
        markup_heading: hex("#0550ae"),
        markup_link: hex("#0969da"),
        markup_code: hex("#e01e5a"),
    };
    Theme { id: "github-light".into(), name: "GitHub Light".into(), kind: ThemeKind::Light, c, syntax }
}

fn github_dark() -> Theme {
    let c = Colors {
        background: hex("#0d1117"),
        foreground: hex("#c9d1d9"),
        heading: hex("#f0f6fc"),
        muted: hex("#8b949e"),
        code_fg: hex("#ff7b72"),
        code_bg: hex("#161b22"),
        blockquote_fg: hex("#8b949e"),
        blockquote_bar: hex("#30363d"),
        link: hex("#58a6ff"),
        table_border: hex("#30363d"),
        table_header_bg: hex("#161b22"),
        hr: hex("#30363d"),
        selection_bg: hex("#264f78"),
        image_bg: hex("#161b22"),
    };
    let syntax = SyntaxColors {
        comment: hex("#8b949e"),
        keyword: hex("#ff7b72"),
        string: hex("#a5d6ff"),
        number: hex("#79c0ff"),
        function: hex("#d2a8ff"),
        typ: hex("#ffa657"),
        variable: hex("#c9d1d9"),
        operator: hex("#ff7b72"),
        punctuation: hex("#8b949e"),
        constant: hex("#79c0ff"),
        markup_heading: hex("#79c0ff"),
        markup_link: hex("#58a6ff"),
        markup_code: hex("#ff7b72"),
    };
    Theme { id: "github-dark".into(), name: "GitHub Dark".into(), kind: ThemeKind::Dark, c, syntax }
}

fn sepia() -> Theme {
    let c = Colors {
        background: hex("#f4ecd8"),
        foreground: hex("#5b4636"),
        heading: hex("#4a3527"),
        muted: hex("#9a8b7a"),
        code_fg: hex("#b58900"),
        code_bg: hex("#efe4c8"),
        blockquote_fg: hex("#8b7b6a"),
        blockquote_bar: hex("#c8b890"),
        link: hex("#8b5a00"),
        table_border: hex("#d6c8a8"),
        table_header_bg: hex("#efe4c8"),
        hr: hex("#d6c8a8"),
        selection_bg: hex("#dccfae"),
        image_bg: hex("#efe4c8"),
    };
    let syntax = SyntaxColors {
        comment: hex("#9a8b7a"),
        keyword: hex("#a8342a"),
        string: hex("#7a5a00"),
        number: hex("#5a5a00"),
        function: hex("#6b3fa0"),
        typ: hex("#7a4a1a"),
        variable: hex("#5b4636"),
        operator: hex("#3a2a1a"),
        punctuation: hex("#8b7b6a"),
        constant: hex("#5a5a00"),
        markup_heading: hex("#7a5a00"),
        markup_link: hex("#8b5a00"),
        markup_code: hex("#b58900"),
    };
    Theme { id: "sepia".into(), name: "Sepia".into(), kind: ThemeKind::Light, c, syntax }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct ThemeRegistry {
    pub themes: Vec<Theme>,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self { themes: builtin_ids().iter().filter_map(|(id, _, _)| builtin(id)).collect() }
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.id == id)
    }

    pub fn cycle(&self, current: &str) -> &Theme {
        let idx = self.themes.iter().position(|t| t.id == current).unwrap_or(0);
        let next = (idx + 1) % self.themes.len().max(1);
        &self.themes[next]
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
