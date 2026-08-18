# mdbijou

轻量级原生 Markdown 阅读器 + 简单编辑器（Rust / macOS）。

- **原生 GUI**：基于 `eframe`/`egui`（OpenGL `glow` 后端），**不集成 webview**，二进制小、启动快。
- **阅读优先**：默认进入渲染预览，CommonMark + GFM（表格、任务列表、删除线、脚注）。
- **编辑从简**：`Cmd+E` 一键切换「编辑/预览」单视图，源编辑器带**语法高亮**（Markdown 全文 + 围栏代码块）。
- **主题 3 套**：`github-light` / `github-dark` / `sepia`，切换即时生效。
- **CJK 友好**：自动加载 macOS 中文字体（PingFang SC）。

---

## 功能

| 功能 | 说明 |
| --- | --- |
| 打开/渲染 Markdown | `mdbijou file.md` |
| 简单编辑器 | `Cmd+E` 切换编辑/预览，`Cmd+S` 保存、`Cmd+Shift+S` 另存 |
| 语法高亮 | Markdown 全文 + 内嵌代码块（`syntect`，可裁剪） |
| 主题切换 | `Cmd+T` 循环 / `--theme <id>` |
| 快捷键 | `Cmd+O` 打开、`Cmd+R` 重载、`Cmd+Z` 撤销等 |
| 未保存保护 | 打开新文件前弹「保存/放弃/取消」确认 |

## 依赖与体积

- 发布构建约 **5.3 MB**（`opt-level="z"` + `lto="fat"` + `strip`）。
- 冷启动以「启动至首帧」计约 **~15 ms**（Apple Silicon）。

---

## 编译与运行

前置：Rust 工具链（rustc/cargo ≥ 1.85）、[just](https://github.com/casey/just)（或用 `cargo` 原生命令）。

```bash
# 一键构建 + 运行（默认打开 sample.md）
just

# 或带文件参数
just path/to/file.md
```

常用 `just` 命令：

```bash
just build                 # 调试构建
just release               # 正式发布构建（~5.3 MB）
just check                 # 快速类型检查
just preview file.md       # 阅读预览视图
just edit file.md          # 直接进编辑器（带高亮）
just run-release file.md   # 直接跑 release 产物
just install               # 安装到 PATH
just themes                # 列出主题
just size                  # 查看二进制体积
just baseline              # 体积 / 启动基线
just clippy                # lint
just fmt                   # 格式化
```

不用 `just` 的原生 `cargo` 命令：

```bash
cargo build --release
./target/release/mdbijou sample.md
```

> 体积敏感时可关闭语法高亮：
> `cargo build --release --no-default-features --features "editor,remote-images"`

---

## 使用示例

```bash
just edit sample.md        # 编辑模式，含代码高亮
just preview --theme github-dark sample.md   # 深色主题预览
just --list-themes         # 查看全部主题
```

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Cmd+E` | 编辑 / 预览 切换 |
| `Cmd+S` / `Cmd+Shift+S` | 保存 / 另存 |
| `Cmd+O` | 打开文件 |
| `Cmd+T` | 循环切换主题 |
| `Cmd+R` | 重新加载 |
| `Cmd+Z` / `Cmd+Shift+Z` | 撤销 / 重做 |

---

## 项目结构

```
src/
  main.rs        CLI 入口（--edit / --theme / --list-themes）
  app.rs         应用外壳（视图状态机 / 快捷键 / 保存 / 打开确认）
  document.rs    pulldown-cmark → IR
  render.rs      预览渲染
  editor.rs      简单编辑器（TextEdit + 逐行高亮 layouter）
  highlight.rs   共用高亮引擎（syntect）
  theme.rs       主题模型 + 内置主题
  config.rs      配置读写（~/.config/mdbijou/config.toml）
  fonts.rs       CJK 字体加载
scripts/
  install.sh     安装脚本
  bench.sh       体积 / 启动基线
justfile         命令编排
docs/            设计文档（DESIGN.md / TASKS.md）
```

## 设计文档

- [DESIGN.md](docs/DESIGN.md) — 架构与设计决议
- [TASKS.md](docs/TASKS.md) — 任务拆分

## License

MIT
