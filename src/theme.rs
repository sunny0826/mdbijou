//! Theme system: color model, builtin themes, syntax-highlight palette, and
//! shared spacing/rounding design tokens.

use crate::color::Color;

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
    pub background: Color,
    pub foreground: Color,
    pub heading: Color,
    pub muted: Color,
    pub code_fg: Color,
    pub code_bg: Color,
    pub blockquote_fg: Color,
    pub blockquote_bar: Color,
    pub quote_bg: Color,
    pub link: Color,
    pub table_border: Color,
    pub table_header_bg: Color,
    pub stripe_bg: Color,
    pub hr: Color,
    pub selection_bg: Color,
    pub image_bg: Color,
    /// Elevated surface (slightly lighter/darker than background): status bar,
    /// sidebars, floating toolbars.
    pub surface: Color,
    /// Hovered surface variant (one step above `surface`).
    pub surface_hover: Color,
    /// Keyboard focus ring / accent ring.
    pub focus: Color,
    /// Positive feedback (save success etc.).
    pub success: Color,
    /// Negative feedback (save failure etc.).
    pub error: Color,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // syntax palette consumed by the highlighter map; lite mode uses subset
pub struct SyntaxColors {
    pub comment: Color,
    pub keyword: Color,
    pub string: Color,
    pub number: Color,
    pub function: Color,
    pub typ: Color,
    pub variable: Color,
    pub operator: Color,
    pub punctuation: Color,
    pub constant: Color,
    pub markup_heading: Color,
    pub markup_link: Color,
    pub markup_code: Color,
}

fn hex(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Color::from_rgb(
        ((v >> 16) & 0xff) as u8,
        ((v >> 8) & 0xff) as u8,
        (v & 0xff) as u8,
    )
}

// ---------------------------------------------------------------------------
// Design tokens (shared spacing / rounding language)
// ---------------------------------------------------------------------------

/// Small radius: inline chips, segmented control segments, small controls.
pub const RADIUS_SM: f32 = 6.0;
/// Medium radius: code blocks, tables, callouts.
pub const RADIUS_MD: f32 = 8.0;
/// Large radius: cards, dialogs, floating surfaces.
pub const RADIUS_LG: f32 = 12.0;
/// Hairline border width used across chrome and components.
pub const BORDER_WIDTH: f32 = 1.0;

// ---------------------------------------------------------------------------
// Builtin themes
// ---------------------------------------------------------------------------

pub fn builtin(id: &str) -> Option<Theme> {
    match id {
        "github-light" => Some(github_light()),
        "github-dark" => Some(github_dark()),
        "sepia" => Some(sepia()),
        "bijou-light" => Some(bijou_light()),
        "bijou-dark" => Some(bijou_dark()),
        _ => None,
    }
}

pub fn builtin_ids() -> Vec<(&'static str, &'static str, ThemeKind)> {
    vec![
        ("github-light", "GitHub Light", ThemeKind::Light),
        ("github-dark", "GitHub Dark", ThemeKind::Dark),
        ("sepia", "Sepia", ThemeKind::Light),
        ("bijou-light", "Bijou Light", ThemeKind::Light),
        ("bijou-dark", "Bijou Dark", ThemeKind::Dark),
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
        quote_bg: hex("#f6f8fa"),
        link: hex("#0969da"),
        table_border: hex("#d8dee4"),
        stripe_bg: hex("#f6f8fa"),
        table_header_bg: hex("#f6f8fa"),
        hr: hex("#d8dee4"),
        selection_bg: hex("#b6d7ff"),
        image_bg: hex("#f6f8fa"),
        surface: hex("#f6f8fa"),
        surface_hover: hex("#eff2f5"),
        focus: hex("#0969da"),
        success: Color::from_rgb(70, 170, 90),
        error: Color::from_rgb(220, 90, 70),
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
    Theme {
        id: "github-light".into(),
        name: "GitHub Light".into(),
        kind: ThemeKind::Light,
        c,
        syntax,
    }
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
        quote_bg: hex("#161b22"),
        link: hex("#58a6ff"),
        table_border: hex("#30363d"),
        table_header_bg: hex("#161b22"),
        stripe_bg: hex("#1c2128"),
        hr: hex("#30363d"),
        selection_bg: hex("#3a6ea5"),
        image_bg: hex("#161b22"),
        surface: hex("#161b22"),
        surface_hover: hex("#1e242d"),
        focus: hex("#58a6ff"),
        success: Color::from_rgb(63, 185, 80),
        error: Color::from_rgb(248, 81, 73),
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
    Theme {
        id: "github-dark".into(),
        name: "GitHub Dark".into(),
        kind: ThemeKind::Dark,
        c,
        syntax,
    }
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
        quote_bg: hex("#efe4c8"),
        link: hex("#8b5a00"),
        table_border: hex("#d6c8a8"),
        table_header_bg: hex("#efe4c8"),
        stripe_bg: hex("#eadfc0"),
        hr: hex("#d6c8a8"),
        selection_bg: hex("#cbb893"),
        image_bg: hex("#efe4c8"),
        surface: hex("#efe4c8"),
        surface_hover: hex("#e8ddbc"),
        focus: hex("#8b5a00"),
        success: Color::from_rgb(70, 170, 90),
        error: Color::from_rgb(220, 90, 70),
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
    Theme {
        id: "sepia".into(),
        name: "Sepia".into(),
        kind: ThemeKind::Light,
        c,
        syntax,
    }
}

fn bijou_light() -> Theme {
    let c = Colors {
        // Warm paper canvas; chrome surfaces sit one step whiter than the
        // reading column (paper + card metaphor).
        background: hex("#F7F6F3"),
        foreground: hex("#1A1E23"),
        heading: hex("#11151A"),
        muted: hex("#6B7785"),
        code_fg: hex("#1A1E23"),
        code_bg: hex("#F2EFE6"),
        blockquote_fg: hex("#6B7785"),
        blockquote_bar: hex("#0A7D6B"),
        quote_bg: hex("#FDFCF8"),
        link: hex("#0A7D6B"),
        table_border: hex("#E8E0D0"),
        table_header_bg: hex("#F2EFE6"),
        stripe_bg: hex("#F9F6F0"),
        hr: hex("#E8E0D0"),
        selection_bg: hex("#B8E6DD"),
        image_bg: hex("#F2EFE6"),
        surface: hex("#FCFCF9"),
        surface_hover: hex("#F1EDE4"),
        focus: hex("#0A7D6B"),
        success: hex("#2DA44E"),
        error: hex("#CF222E"),
    };
    let syntax = SyntaxColors {
        comment: hex("#8A9AAD"),
        keyword: hex("#B42318"),
        string: hex("#7A5A00"),
        number: hex("#0A7D6B"),
        function: hex("#6B4CA8"),
        typ: hex("#8B5A00"),
        variable: hex("#1A1E23"),
        operator: hex("#1A1E23"),
        punctuation: hex("#6B7785"),
        constant: hex("#0A7D6B"),
        markup_heading: hex("#0A7D6B"),
        markup_link: hex("#0A7D6B"),
        markup_code: hex("#1A1E23"),
    };
    Theme {
        id: "bijou-light".into(),
        name: "Bijou Light".into(),
        kind: ThemeKind::Light,
        c,
        syntax,
    }
}

fn bijou_dark() -> Theme {
    let c = Colors {
        background: hex("#121416"),
        foreground: hex("#E6E8EB"),
        heading: hex("#F0F3F6"),
        muted: hex("#8B95A1"),
        code_fg: hex("#E6E8EB"),
        code_bg: hex("#1E242B"),
        blockquote_fg: hex("#8B95A1"),
        blockquote_bar: hex("#2DD4BF"),
        quote_bg: hex("#181D22"),
        link: hex("#2DD4BF"),
        table_border: hex("#2A3038"),
        table_header_bg: hex("#1E242B"),
        stripe_bg: hex("#1A1F26"),
        hr: hex("#2A3038"),
        selection_bg: hex("#1E5A52"),
        image_bg: hex("#1E242B"),
        surface: hex("#1B1F24"),
        surface_hover: hex("#242A32"),
        focus: hex("#2DD4BF"),
        success: hex("#2DD4BF"),
        error: hex("#F85149"),
    };
    let syntax = SyntaxColors {
        comment: hex("#6B7A8A"),
        keyword: hex("#FF7B8A"),
        string: hex("#7EE0D1"),
        number: hex("#5EEAD4"),
        function: hex("#C4A6FF"),
        typ: hex("#FFD580"),
        variable: hex("#E6E8EB"),
        operator: hex("#2DD4BF"),
        punctuation: hex("#8B95A1"),
        constant: hex("#5EEAD4"),
        markup_heading: hex("#5EEAD4"),
        markup_link: hex("#2DD4BF"),
        markup_code: hex("#E6E8EB"),
    };
    Theme {
        id: "bijou-dark".into(),
        name: "Bijou Dark".into(),
        kind: ThemeKind::Dark,
        c,
        syntax,
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct ThemeRegistry {
    pub themes: Vec<Theme>,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self {
            themes: builtin_ids()
                .iter()
                .filter_map(|(id, _, _)| builtin(id))
                .collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.id == id)
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
