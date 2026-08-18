//! Minimal native Mermaid renderer: parses the common `flowchart`/`graph`
//! subset (nodes `A[label]`/`A(label)`/`A{label}`/`A((label))`, edges
//! `A --> B`, `A -- text --> B`, `A -->|text| B`, chains `A --> B --> C`)
//! and draws it with the egui painter using a layered top-down (or
//! left-right) layout. Anything outside the subset falls back to the plain
//! code-block rendering.

use crate::theme::Theme;
use egui::{pos2, vec2, Align2, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    /// Top-down (also BT is drawn top-down; the visual difference is minor).
    Vertical,
    /// Left-right (and RL).
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `A[label]`
    Box,
    /// `A(label)` — rounded
    Rounded,
    /// `A{label}` — diamond
    Diamond,
    /// `A((label))` — circle
    Circle,
}

#[derive(Debug)]
struct Node {
    label: String,
    shape: Shape,
}

#[derive(Debug)]
struct Edge {
    from: usize,
    to: usize,
    label: Option<String>,
}

#[derive(Debug)]
struct Graph {
    dir: Dir,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a mermaid diagram; returns None when the source is not a supported
/// flowchart/graph (caller then falls back to a code block).
fn parse(src: &str) -> Option<Graph> {
    let mut lines = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("%%"));

    let first = lines.next()?;
    let dir = if first.starts_with("graph") || first.starts_with("flowchart") {
        let rest = first
            .trim_start_matches("flowchart")
            .trim_start_matches("graph")
            .trim();
        match rest {
            "LR" | "RL" => Dir::Horizontal,
            // TD / TB / BT / RL-less default
            _ => Dir::Vertical,
        }
    } else {
        return None;
    };

    let mut g = Graph {
        dir,
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let mut ids: HashMap<String, usize> = HashMap::new();

    fn node_id(
        g: &mut Graph,
        ids: &mut HashMap<String, usize>,
        id: &str,
        label: Option<String>,
        shape: Option<Shape>,
    ) -> usize {
        if let Some(&i) = ids.get(id) {
            // A later explicit label/shape overrides the bare reference.
            if let Some(l) = label {
                g.nodes[i].label = l;
            }
            if let Some(s) = shape {
                g.nodes[i].shape = s;
            }
            return i;
        }
        let i = g.nodes.len();
        ids.insert(id.to_string(), i);
        g.nodes.push(Node {
            label: label.unwrap_or_else(|| id.to_string()),
            shape: shape.unwrap_or(Shape::Box),
        });
        i
    }

    /// Parse one node token like `A`, `A[label]`, `A(label)`, `A{label}`,
    /// `A((label))`. Returns (id, label, shape).
    fn parse_node(tok: &str) -> Option<(String, Option<String>, Option<Shape>)> {
        let tok = tok.trim();
        if tok.is_empty() {
            return None;
        }
        for (open, close, shape) in [
            ("((", "))", Shape::Circle),
            ("(", ")", Shape::Rounded),
            ("[", "]", Shape::Box),
            ("{", "}", Shape::Diamond),
        ] {
            if let Some(p) = tok.find(open) {
                if tok.ends_with(close) {
                    let id = tok[..p].trim().to_string();
                    if id.is_empty() {
                        return None;
                    }
                    let label = tok[p + open.len()..tok.len() - close.len()]
                        .trim()
                        .to_string();
                    return Some((id, Some(label), Some(shape)));
                }
            }
        }
        // Bare id (allow letters, digits, underscore, dash, dot).
        if tok
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Some((tok.to_string(), None, None));
        }
        None
    }

    for line in lines {
        // Skip subgraph wrappers but keep their contents.
        let line = if let Some(rest) = line.strip_prefix("subgraph") {
            rest.trim()
        } else {
            line
        };
        if line == "end" {
            continue;
        }
        // Walk the raw line, capturing node tokens and the labels between
        // arrows (`-- label -->` or `-->|label|`).
        if !line.contains("--") {
            // Node-only line (e.g. `A[label]` alone).
            if let Some((id, label, shape)) = parse_node(line) {
                node_id(&mut g, &mut ids, &id, label, shape);
            }
            continue;
        }
        let mut prev: Option<usize> = None;
        let mut pending_label: Option<String> = None;
        let mut tokens: Vec<(Option<String>, String)> = Vec::new(); // (edge label, node token)
        let mut rest = line;
        loop {
            match rest.find("--") {
                Some(pos) => {
                    tokens.push((pending_label.take(), rest[..pos].trim().to_string()));
                    rest = rest[pos + 2..].trim();
                    // Strip the arrowhead first (`-->`), then the pipe label.
                    let had_arrow = rest.starts_with('>');
                    if let Some(r) = rest.strip_prefix('>') {
                        rest = r.trim();
                    }
                    if let Some(stripped) = rest.strip_prefix('|') {
                        if let Some(end) = stripped.find('|') {
                            pending_label = Some(stripped[..end].trim().to_string());
                            rest = stripped[end + 1..].trim();
                        }
                    } else if !had_arrow && rest.contains("-->") {
                        // `-- label -->` form: label is the text before the arrow.
                        if let Some(arrow) = rest.find("-->") {
                            pending_label = Some(rest[..arrow].trim().to_string());
                            rest = rest[arrow + 3..].trim();
                        }
                    }
                    if let Some(r) = rest.strip_prefix('>') {
                        rest = r.trim();
                    }
                }
                None => {
                    tokens.push((pending_label.take(), rest.trim().to_string()));
                    break;
                }
            }
        }
        for (label, tok) in tokens {
            let Some((id, nlabel, shape)) = parse_node(&tok) else {
                continue;
            };
            let idx = node_id(&mut g, &mut ids, &id, nlabel, shape);
            if let Some(from) = prev {
                g.edges.push(Edge {
                    from,
                    to: idx,
                    label,
                });
            }
            prev = Some(idx);
        }
    }

    if g.nodes.is_empty() {
        return None;
    }
    Some(g)
}

// ---------------------------------------------------------------------------
// Layout & painting
// ---------------------------------------------------------------------------

/// Layered layout: longest-path rank per node, order within layer by first
/// appearance. Coordinates are top-down; LR transposes at paint time.
fn layout(g: &Graph, sizes: &[Vec2]) -> Vec<Pos2> {
    let n = g.nodes.len();
    // Longest-path layering (handles DAGs; cycles clamp to n iterations).
    let mut rank = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for e in &g.edges {
            if rank[e.to] < rank[e.from] + 1 {
                rank[e.to] = rank[e.from] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let layers = rank.iter().max().copied().unwrap_or(0) + 1;
    let mut order: Vec<Vec<usize>> = vec![Vec::new(); layers];
    for (i, &r) in rank.iter().enumerate() {
        order[r].push(i);
    }

    let gap_x = 24.0;
    let gap_y = 36.0;
    let mut pos = vec![pos2(0.0, 0.0); n];
    let layer_h: Vec<f32> = order
        .iter()
        .map(|l| l.iter().map(|&i| sizes[i].y).fold(0.0_f32, f32::max))
        .collect();
    let mut y = 0.0;
    for (r, layer) in order.iter().enumerate() {
        let width: f32 = layer.iter().map(|&i| sizes[i].x).sum::<f32>()
            + gap_x * layer.len().saturating_sub(1) as f32;
        let mut x = -width / 2.0;
        for &i in layer {
            pos[i] = pos2(x + sizes[i].x / 2.0, y + layer_h[r] / 2.0);
            x += sizes[i].x + gap_x;
        }
        y += layer_h[r] + gap_y;
    }
    pos
}

/// Clip the segment from `a` to `b` to the border of the node rect centered
/// at `a` (half-size `hs`), so arrows start/end at shape edges.
fn clip_to_border(a: Pos2, hs: Vec2, b: Pos2) -> Pos2 {
    let d = b - a;
    if d == Vec2::ZERO {
        return a;
    }
    let tx = if d.x.abs() > f32::EPSILON {
        hs.x / d.x.abs()
    } else {
        f32::INFINITY
    };
    let ty = if d.y.abs() > f32::EPSILON {
        hs.y / d.y.abs()
    } else {
        f32::INFINITY
    };
    let t = tx.min(ty).min(1.0);
    a + d * t
}

/// Render a mermaid diagram. Returns false when the source is not a
/// supported diagram (caller falls back to the code block).
pub fn render(ui: &mut Ui, src: &str, theme: &Theme, font_size: f32) -> bool {
    let Some(g) = parse(src) else {
        return false;
    };

    let font = FontId::proportional(font_size - 1.0);
    let pad = vec2(12.0, 8.0);
    let sizes: Vec<Vec2> = g
        .nodes
        .iter()
        .map(|n| {
            let w = ui.fonts_mut(|f| {
                f.layout_no_wrap(n.label.clone(), font.clone(), theme.c.foreground)
                    .size()
                    .x
            });
            vec2(w + 2.0 * pad.x, font_size + 2.0 * pad.y)
        })
        .collect();
    let pos = layout(&g, &sizes);

    // Canvas bounds (top-down), then transpose if horizontal.
    let mut min = pos2(f32::INFINITY, f32::INFINITY);
    let mut max = pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mapped: Vec<Pos2> = pos
        .iter()
        .map(|p| match g.dir {
            Dir::Vertical => *p,
            Dir::Horizontal => pos2(p.y, p.x),
        })
        .collect();
    let mapped_sizes: Vec<Vec2> = sizes
        .iter()
        .map(|s| match g.dir {
            Dir::Vertical => *s,
            Dir::Horizontal => vec2(s.y, s.x),
        })
        .collect();
    for (i, p) in mapped.iter().enumerate() {
        min = min.min(*p - mapped_sizes[i] / 2.0);
        max = max.max(*p + mapped_sizes[i] / 2.0);
    }
    let canvas = max - min + vec2(8.0, 8.0);
    let (rect, _) = ui.allocate_exact_size(canvas, Sense::hover());
    let origin = rect.min + vec2(4.0, 4.0) - min.to_vec2();
    let at = |p: Pos2| origin + p.to_vec2();

    let stroke_edge = Stroke::new(1.2, theme.c.muted);
    let stroke_node = Stroke::new(1.2, theme.c.table_border);

    // Edges first (under the nodes).
    for e in &g.edges {
        let a = at(mapped[e.from]);
        let b = at(mapped[e.to]);
        let start = clip_to_border(a, mapped_sizes[e.from] / 2.0, b);
        let end = clip_to_border(b, mapped_sizes[e.to] / 2.0, a);
        ui.painter().line_segment([start, end], stroke_edge);
        // Arrowhead at `end`.
        let dir = (end - start).normalized();
        let n = vec2(-dir.y, dir.x);
        let p1 = end - dir * 8.0 + n * 3.5;
        let p2 = end - dir * 8.0 - n * 3.5;
        ui.painter().add(egui::Shape::convex_polygon(
            vec![end, p1, p2],
            theme.c.muted,
            Stroke::NONE,
        ));
        if let Some(label) = &e.label {
            if !label.is_empty() {
                let mid = (start + end.to_vec2()) / 2.0;
                let galley =
                    ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), theme.c.muted));
                let lrect = Rect::from_center_size(mid, galley.size() + vec2(6.0, 4.0));
                ui.painter().rect_filled(lrect, 3.0, theme.c.background);
                ui.painter()
                    .galley(lrect.center() - galley.size() / 2.0, galley, theme.c.muted);
            }
        }
    }

    // Nodes.
    for (i, node) in g.nodes.iter().enumerate() {
        let center = at(mapped[i]);
        let nrect = Rect::from_center_size(center, mapped_sizes[i]);
        let fill = theme.c.code_bg;
        match node.shape {
            Shape::Box => {
                ui.painter()
                    .rect(nrect, 2.0, fill, stroke_node, egui::StrokeKind::Inside);
            }
            Shape::Rounded => {
                ui.painter().rect(
                    nrect,
                    nrect.height() / 2.0,
                    fill,
                    stroke_node,
                    egui::StrokeKind::Inside,
                );
            }
            Shape::Circle => {
                ui.painter().rect(
                    nrect,
                    nrect.height().min(nrect.width()) / 2.0,
                    fill,
                    stroke_node,
                    egui::StrokeKind::Inside,
                );
            }
            Shape::Diamond => {
                let pts = vec![
                    pos2(nrect.center().x, nrect.min.y),
                    pos2(nrect.max.x, nrect.center().y),
                    pos2(nrect.center().x, nrect.max.y),
                    pos2(nrect.min.x, nrect.center().y),
                ];
                ui.painter()
                    .add(egui::Shape::convex_polygon(pts, fill, stroke_node));
            }
        }
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            &node.label,
            font.clone(),
            theme.c.foreground,
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_flowchart() {
        let g = parse("graph TD\n  A[开始] --> B{判断}\n  B -->|是| C(处理)\n").unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
        assert_eq!(g.edges[1].label.as_deref(), Some("是"));
        assert_eq!(g.nodes[1].shape, Shape::Diamond);
    }

    #[test]
    fn parses_chain_and_lr() {
        let g = parse("flowchart LR\nA --> B --> C").unwrap();
        assert_eq!(g.dir, Dir::Horizontal);
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn rejects_non_flowchart() {
        assert!(parse("sequenceDiagram\nA->>B: hi").is_none());
        assert!(parse("let x = 1;").is_none());
    }
}
