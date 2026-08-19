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
| 打开/渲染 Markdown | `mdbijou file.md`（安装为 CLI 后：`mdb file.md`）；打包为 .app 后支持双击 .md / 拖到 Dock 图标打开 |
| Mermaid 图 | ```mermaid 围栏代码块原生渲染（flowchart/graph 子集：节点形状、边标签、TD/LR），不支持的图类型回退为代码块 |
| 简单编辑器 | `Cmd+E` 切换编辑/预览，`Cmd+S` 保存 |
| 语法高亮 | Markdown 全文 + 内嵌代码块（`syntect`，可裁剪） |
| 主题切换 | 设置页（`Cmd+,`）选择主题，或 `--theme <id>`（手动选择后优先于跟随系统） |
| 字体与字号 | 设置页切换正文字体（默认/苹方/冬青黑体/宋体/黑体）、正文字号与编辑器字号 |
| 快捷键 | `Cmd+O` 打开、`Cmd+R` 重载、`Cmd+,` 设置、`Cmd+Z` 撤销等 |
| 未保存保护 | 打开新文件前弹「保存/放弃/取消」确认 |
| 安装 CLI | 设置页一键安装 `mdb` 命令到本地 PATH |

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
just install               # 安装为 `mdb` 到 PATH
just themes                # 列出主题
just size                  # 查看二进制体积
just baseline              # 体积 / 启动基线
just icon                  # 从 logo.png 生成 assets/mdbijou.icns
just bundle                # 打包 dist/mdbijou.app（含图标 + ad-hoc 签名）
just dmg                   # 打包 .app 并生成 dist/mdbijou-<version>.dmg
just clippy                # lint
just fmt                   # 格式化
```

### 打包与发布

```bash
just bundle   # 构建 release，生成 dist/mdbijou.app（图标来自 logo.png）
just dmg      # 在上述基础上再生成 dist/mdbijou-<version>.dmg
```

打包产物使用 logo.png 作为应用图标：`scripts/icon-compose.py`（Pillow）先把它合成为 Big Sur 风格的圆角矩形母图 `assets/mdbijou-icon-1024.png`，再由 `scripts/make-icon.sh` 用系统自带 `sips`/`iconutil` 生成 `assets/mdbijou.icns`（无 Pillow 时回退为直接使用 logo.png），并做 ad-hoc 签名。运行窗口图标同样来自该母图。

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
| `Cmd+S` | 保存 |
| `Cmd+O` | 打开文件 |
| `Cmd+,` | 打开设置页 |
| `Cmd+R` | 重新加载 |
| `Cmd+Z` / `Cmd+Shift+Z` | 撤销 / 重做 |

---

## 项目结构

```
src/
  main.rs        CLI 入口（--edit / --theme / --list-themes）
  app.rs         应用外壳（视图状态机 / 顶栏 / 设置页 / 快捷键 / 保存）
  macos.rs       macOS 原生窗口标题栏（红绿灯顶栏整合）+ 双击打开 .md 的 application:openFiles: 钩子
  install.rs     将当前程序安装为 `mdb` CLI
  document.rs    pulldown-cmark → IR
  render.rs      预览渲染
  mermaid.rs     Mermaid flowchart/graph 子集原生渲染
  editor.rs      简单编辑器（TextEdit + 逐行高亮 layouter，软换行无横向滚动）
  highlight.rs   共用高亮引擎（syntect）
  theme.rs       主题模型 + 内置主题
  config.rs      配置读写（~/.config/mdbijou/config.toml）
  fonts.rs       CJK 字体加载
  images.rs      图片加载与缓存（本地 + 远程）
scripts/
  install.sh     安装脚本（安装为 `mdb`）
  bench.sh       体积 / 启动基线
  make-icon.sh   logo.png → assets/mdbijou.icns 应用图标
  icon-compose.py logo.png → macOS 风格圆角矩形母图（Pillow）
  bundle.sh      打包 dist/mdbijou.app / .dmg（含 ad-hoc 签名）
justfile         命令编排
docs/            设计文档（DESIGN.md / TASKS.md）
```

## License

MIT
