//! Shared syntax-highlighting engine.
//! Used by the preview (code blocks) and the editor (fenced code + Markdown).
//!
//! When the `highlight` feature (syntect) is enabled we use syntect for fenced
//! code blocks (block-level, lazily resolved) plus a lightweight Markdown line
//! highlighter. When only `lite-highlight` is enabled we fall back to a fully
//! self-contained tokenizer. Either way, the editor and the preview call the
//! SAME highlighter, so colors stay consistent between the two views.

use crate::theme::Theme;
use egui::Color32;

#[derive(Debug, Clone)]
pub struct Span {
    pub text: String,
    pub color: Color32,
    /// Style flags: bit0 = heading/strong, bit1 = italics, bit2 = underline.
    pub style: u8,
}

/// A highlighted logical line: a set of (text, color) spans.
pub type Line = Vec<Span>;

pub trait Highlighter {
    /// Highlight a single line of a fenced code block in `lang` (or plain if None).
    fn code_line(&mut self, lang: Option<&str>, line: &str) -> Line;
    /// Highlight a single line in Markdown context (editor full text).
    fn markdown_line(&mut self, line: &str) -> Line;
}

// ---------------------------------------------------------------------------
// lite-highlight: self-contained tokenizer (no syntect) — used only when the
// `highlight` feature is disabled.
// ---------------------------------------------------------------------------

#[cfg(not(feature = "highlight"))]
pub struct LiteHighlighter {
    syntax: crate::theme::SyntaxColors,
}

#[cfg(not(feature = "highlight"))]
impl LiteHighlighter {
    pub fn new(theme: &Theme) -> Self {
        Self { syntax: theme.syntax.clone() }
    }
}

#[cfg(not(feature = "highlight"))]
impl Highlighter for LiteHighlighter {
    fn code_line(&mut self, lang: Option<&str>, line: &str) -> Line {
        let _ = lang;
        let mut out: Line = Vec::new();
        let mut rest = line;
        while !rest.is_empty() {
            let (tok, color, adv) = next_code_token(rest, &self.syntax);
            out.push(Span { text: tok.clone(), color: color, style: 0 });
            let adv = adv.min(rest.len());
            rest = &rest[adv..];
        }
        out
    }

    fn markdown_line(&mut self, line: &str) -> Line {
        markdown_line_highlight(line, &self.syntax)
    }
}

#[cfg(not(feature = "highlight"))]
fn next_code_token(s: &str, sy: &crate::theme::SyntaxColors) -> (String, Color32, usize) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            let quote = c;
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
            let end = j.min(s.len());
            return (s[start..end].to_string(), sy.string, end - start);
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            return (s[start..i].to_string(), sy.number, i - start);
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return (s[i..].to_string(), sy.comment, bytes.len() - i);
        }
        if c == b'#' && (i == 0 || s[..i].chars().all(char::is_whitespace)) {
            return (s[i..].to_string(), sy.comment, bytes.len() - i);
        }
        i += 1;
    }
    (s.to_string(), Color32::TRANSPARENT, s.len())
}

fn markdown_line_highlight(line: &str, sy: &crate::theme::SyntaxColors) -> Line {
    let mut out: Line = Vec::new();
    let hash_count = line.chars().take_while(|c| *c == '#').count();
    if hash_count >= 1 && hash_count <= 6 {
        let trimmed = line.trim_start_matches('#').trim_start();
        out.push(Span { text: "#".repeat(hash_count) + " ", color: sy.markup_heading, style: 1 });
        out.push(Span { text: trimmed.to_string(), color: sy.markup_heading, style: 1 });
        return out;
    }
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(start) = rest.find('`') {
            let after = &rest[start + 1..];
            if let Some(end_rel) = after.find('`') {
                let code_span = &rest[start..start + 1 + end_rel + 1];
                out.push(Span { text: rest[..start].to_string(), color: Color32::TRANSPARENT, style: 0 });
                out.push(Span { text: code_span.to_string(), color: sy.markup_code, style: 0 });
                rest = &rest[start + code_span.len()..];
                continue;
            }
        }
        out.push(Span { text: rest.to_string(), color: Color32::TRANSPARENT, style: 0 });
        rest = "";
    }
    out
}

// ---------------------------------------------------------------------------
// syntect-based highlighter (feature = "highlight")
// ---------------------------------------------------------------------------

#[cfg(feature = "highlight")]
pub struct SyntectHighlighter {
    ss: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::Theme,
    markdown: crate::theme::SyntaxColors,
}

#[cfg(feature = "highlight")]
impl SyntectHighlighter {
    pub fn new(theme: &Theme) -> Self {
        let ss = syntect::parsing::SyntaxSet::load_defaults_newlines();
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let name = match theme.kind {
            crate::theme::ThemeKind::Light => "InspiredGitHub",
            crate::theme::ThemeKind::Dark => "base16-ocean.dark",
        };
        let th = ts.themes.get(name).unwrap().clone();
        Self { ss, theme: th, markdown: theme.syntax.clone() }
    }

    fn highlight_block(&self, lang: Option<&str>, text: &str) -> Vec<Line> {
        let syntax = lang
            .and_then(|l| self.ss.find_syntax_by_token(l))
            .unwrap_or_else(|| self.ss.find_syntax_plain_text());
        let mut hl = syntect::easy::HighlightLines::new(syntax, &self.theme);
        let mut lines = Vec::new();
        for ln in text.lines() {
            let ranges = hl.highlight_line(ln, &self.ss).unwrap_or_default();
            let spans: Vec<Span> = ranges
                .into_iter()
                .filter_map(|(style, text)| {
                    if text.is_empty() {
                        return None;
                    }
                    Some(Span { text: text.to_string(), color: syntect_color(style.foreground), style: 0 })
                })
                .collect();
            lines.push(spans);
        }
        lines
    }
}

#[cfg(feature = "highlight")]
fn syntect_color(c: syntect::highlighting::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

#[cfg(feature = "highlight")]
impl Highlighter for SyntectHighlighter {
    fn code_line(&mut self, lang: Option<&str>, line: &str) -> Line {
        // Line-oriented API used by the editor for fenced code.
        self.highlight_block(lang, line).into_iter().next().unwrap_or_default()
    }

    fn markdown_line(&mut self, line: &str) -> Line {
        markdown_line_highlight(line, &self.markdown)
    }
}

// ---------------------------------------------------------------------------
// Construct a highlighter appropriate to the enabled features
// ---------------------------------------------------------------------------

pub fn new_highlighter(theme: &Theme) -> Box<dyn Highlighter> {
    #[cfg(feature = "highlight")]
    {
        Box::new(SyntectHighlighter::new(theme))
    }
    #[cfg(not(feature = "highlight"))]
    {
        Box::new(LiteHighlighter::new(theme))
    }
}

