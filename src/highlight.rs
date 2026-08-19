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
    /// Highlight an entire code block in `lang`, keeping cross-line syntax state
    /// (multi-line strings, block comments) consistent (UI-MD-008).
    fn code_block(&mut self, lang: Option<&str>, text: &str) -> Vec<Line>;
    /// Highlight a single line in Markdown context (editor full text).
    fn markdown_line(&mut self, line: &str) -> Line;
}

/// Round `idx` down to the nearest UTF-8 character boundary in `s`.
///
/// Every index produced by [`str::find`] on an ASCII pattern is already a char
/// boundary, but defensive rounding guarantees we never slice through the
/// middle of a multi-byte character (e.g. full-width punctuation), which would
/// panic. Clamps to `s.len()` and never drops characters.
fn char_boundary(s: &str, idx: usize) -> usize {
    let idx = idx.min(s.len());
    if s.is_char_boundary(idx) {
        idx
    } else {
        s[..idx]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
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
        Self {
            syntax: theme.syntax.clone(),
        }
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
            out.push(Span {
                text: tok.clone(),
                color: color,
                style: 0,
            });
            let adv = char_boundary(rest, adv);
            rest = &rest[adv..];
        }
        out
    }

    fn code_block(&mut self, lang: Option<&str>, text: &str) -> Vec<Line> {
        text.lines().map(|l| self.code_line(lang, l)).collect()
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
            let end = char_boundary(s, j.min(s.len()));
            if start > 0 {
                // Emit the plain prefix first; the string token is handled on
                // the next call so no bytes are dropped or duplicated.
                return (s[..start].to_string(), Color32::TRANSPARENT, start);
            }
            return (s[start..end].to_string(), sy.string, end);
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let end = char_boundary(s, i);
            if start > 0 {
                return (s[..start].to_string(), Color32::TRANSPARENT, start);
            }
            return (s[start..end].to_string(), sy.number, end);
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            if i > 0 {
                return (s[..i].to_string(), Color32::TRANSPARENT, i);
            }
            return (s[i..].to_string(), sy.comment, bytes.len());
        }
        if c == b'#' && (i == 0 || s[..char_boundary(s, i)].chars().all(char::is_whitespace)) {
            if i > 0 {
                return (s[..i].to_string(), Color32::TRANSPARENT, i);
            }
            return (s[i..].to_string(), sy.comment, bytes.len());
        }
        i += 1;
    }
    (s.to_string(), Color32::TRANSPARENT, s.len())
}

fn markdown_line_highlight(line: &str, sy: &crate::theme::SyntaxColors) -> Line {
    let mut out: Line = Vec::new();
    let hash_count = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hash_count) {
        let trimmed = line.trim_start_matches('#').trim_start();
        out.push(Span {
            text: "#".repeat(hash_count) + " ",
            color: sy.markup_heading,
            style: 1,
        });
        out.push(Span {
            text: trimmed.to_string(),
            color: sy.markup_heading,
            style: 1,
        });
        return out;
    }
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(start) = rest.find('`') {
            let after = &rest[char_boundary(rest, start + 1)..];
            if let Some(end_rel) = after.find('`') {
                let code_end = char_boundary(rest, start + 1 + end_rel + 1);
                out.push(Span {
                    text: rest[..start].to_string(),
                    color: Color32::TRANSPARENT,
                    style: 0,
                });
                out.push(Span {
                    text: rest[start..code_end].to_string(),
                    color: sy.markup_code,
                    style: 0,
                });
                rest = &rest[code_end..];
                continue;
            }
        }
        out.push(Span {
            text: rest.to_string(),
            color: Color32::TRANSPARENT,
            style: 0,
        });
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
        Self {
            ss,
            theme: th,
            markdown: theme.syntax.clone(),
        }
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
                    Some(Span {
                        text: text.to_string(),
                        color: syntect_color(style.foreground),
                        style: 0,
                    })
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
        self.highlight_block(lang, line)
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    fn code_block(&mut self, lang: Option<&str>, text: &str) -> Vec<Line> {
        self.highlight_block(lang, text)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn syntax() -> crate::theme::SyntaxColors {
        crate::theme::builtin("github-light")
            .expect("builtin theme")
            .syntax
    }

    fn joined(spans: &Line) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn markdown_line_highlight_preserves_fullwidth_punctuation() {
        let line = "你好，世界！测试（中文）【标点】《引号》“双引”‘单引’。；：、？";
        assert_eq!(joined(&markdown_line_highlight(line, &syntax())), line);
    }

    #[test]
    fn markdown_line_highlight_keeps_backtick_spans_byte_safe() {
        // Backticks around CJK must not truncate the multi-byte content.
        let line = "代码`你好，世界！`标点，后。";
        assert_eq!(joined(&markdown_line_highlight(line, &syntax())), line);
    }

    #[test]
    fn markdown_line_highlight_heading_preserves_cjk() {
        let line = "## 标题，含标点！";
        assert_eq!(joined(&markdown_line_highlight(line, &syntax())), line);
    }

    #[cfg(not(feature = "highlight"))]
    #[test]
    fn lite_code_line_preserves_fullwidth_punctuation() {
        let th = crate::theme::builtin("github-light").unwrap();
        let mut hl = LiteHighlighter::new(&th);
        let line = "print('你好，世界！')";
        assert_eq!(joined(&hl.code_line(Some("py"), line)), line);
    }
}
