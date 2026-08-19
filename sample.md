# Sample

A **simple** Markdown reader and *editor* with ~~strikethrough~~ and syntax highlighting.

## Lists

- First item
- Second item
  1. Nested ordered
  2. Continue

- [x] Completed task
- [ ] Pending task

## Code Block (Rust)

```rust
fn main() {
    let name = "mdbijou";
    println!("Hello, {name}!"); // comment
    for i in 0..3 {
        println!("{i}");
    }
}
```

## Blockquote

> This is a blockquote.
> Multi-line quote with English typesetting.

## Table

| Feature   | Status | Priority |
| --------- | ------ | -------- |
| Rendering | ✅     | P0       |
| Editor    | ✅     | P0       |

## Links

Visit [mdbijou](https://github.com/sunny0826/mdbijou).

---

End of body.

## HTML & Images

This is an HTML paragraph: <span style="color: red">red text</span> and <b>bold</b>.

<div>
  <p>Block-level HTML content (rendered via the whitelist: headings, paragraphs, links, images).</p>
</div>

<h3 align="center">Centered HTML Heading</h3>
<p align="center">Centered paragraph with a <a href="https://github.com/sunny0826/mdbijou">link</a>.</p>

![Remote sample image](https://picsum.photos/400/200)

## Mermaid Example

```mermaid
graph TD
  A[Start] --> B{Condition?}
  B -->|Yes| C(Process)
  B -->|No| D[End]
  C --> D
```

```mermaid
flowchart LR
  User --> Frontend((Web)) --> Backend{API}
  Backend --> Database[(Storage)]
```
