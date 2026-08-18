# 轻量 Markdown 阅读器 + 简单编辑器（`mdbijou`）设计文档

| 项目 | 内容 |
| --- | --- |
| 文档版本 | v0.2 |
| 状态 | 已决议（进入实现）；v0.2 新增「简单编辑器 + 代码高亮」 |
| 目标平台 | macOS（优先），Linux / Windows 为次要目标 |
| 技术栈 | Rust |
| 核心约束 | 不集成 webview；二进制体积尽可能小；启动尽可能快 |

---

## 目录

1. [项目概述](#1-项目概述)
2. [目标与非目标](#2-目标与非目标)
3. [需求分析](#3-需求分析)
4. [技术选型](#4-技术选型)
5. [总体架构](#5-总体架构)
6. [模块详细设计](#6-模块详细设计)
7. [主题系统设计](#7-主题系统设计)
8. [关键流程](#8-关键流程)
9. [性能与体积优化](#9-性能与体积优化)
10. [macOS 平台适配](#10-macos-平台适配)
11. [测试策略](#11-测试策略)
12. [里程碑与路线图](#12-里程碑与路线图)
13. [风险与缓解](#13-风险与缓解)
14. [附录：依赖清单与体积估算](#14-附录依赖清单与体积估算)

---

## 1. 项目概述

`mdbijou` 是一个面向 macOS 的本地 Markdown 阅读器 + **简单编辑器**，用 Rust 编写，采用**原生 GUI 渲染**（非 webview），目标是成为「双击即开、秒级启动、体积可控」的轻量 Markdown 工具：既能**阅读**渲染后的文档，也能在**源码编辑器**中直接修改（编辑器含语法高亮，尤其是代码块）。

它区别于 Typora / Obsidian / VS Code Markdown Preview 等产品的地方在于：

- **阅读优先、编辑从简**：默认进入阅读视图（渲染预览），按需切换到纯源码编辑器；编辑器刻意保持「简单」（无分屏、无复杂 IDE 功能），聚焦 Markdown 与代码高亮的流畅体验。
- **不用 webview**：所有内容由 Rust 侧原生绘制，避免嵌入 Chromium / WKWebView 带来的体积与启动开销。
- **编辑/预览同窗切换**：同一窗口在「编辑（源码 + 高亮）」与「预览（渲染）」之间一键切换，轻量且专一。
- **极致轻量**：单一可执行文件（可选打包为 `.app`），冷启动接近原生文本应用。

---

## 2. 目标与非目标

### 2.1 目标（In scope）

1. 打开并渲染本地 Markdown（CommonMark + GFM 常用扩展：表格、任务列表、删除线、脚注等）。
2. 通过 CLI 直接打开文件：`mdbijou path/to/file.md`。
3. 支持切换渲染主题（亮 / 暗 / 自定义）。
4. **简单源码编辑器**（v0.2）：在「编辑」视图下直接编辑 Markdown 源码，支持基础编辑（光标、选择、撤销/重做、保存）与语法高亮（Markdown 全文 + 内嵌代码块）。
5. macOS 原生体验：Finder 双击、`open -a` 打开、系统菜单、基础快捷键。
6. 二进制小、启动快、内存占用低。

### 2.2 非目标（Out of scope，首版明确不做）

1. **不追求完整 IDE 级编辑器**：不做分屏双栏、多光标、查找/替换、Markdown 所见即所得（WYSIWYG）实时双向编辑。源码编辑与渲染预览通过**单视图切换**，而非并排实时同步。
2. **不执行内嵌 HTML / 脚本**：编辑器与预览均不执行 HTML/JS；HTML 块以安全方式降级显示或忽略。
3. **插件体系、同步、多标签页**：不在首版范围。
4. **跨平台全量适配**：macOS 优先，其余平台保证「可编译、可运行」即可，不做深度打磨。

> 非目标并不是永久拒绝，而是为避免首版范围蔓延，后续版本按需引入。

---

## 3. 需求分析

### 3.1 功能需求（FR）

| 编号 | 需求 | 优先级 |
| --- | --- | --- |
| FR-1 | CLI 打开本地 `.md` / `.markdown` 文件 | P0 |
| FR-2 | 渲染 CommonMark + GFM 常用语法 | P0 |
| FR-3 | 主题切换（内置 ≥3 套，含亮/暗），快捷键 + CLI 参数 | P0 |
| FR-4 | macOS Finder 双击 / `open -a` 打开文件 | P1 |
| FR-5 | 中英文混排、CJK 字体正确渲染 | P0 |
| FR-6 | 代码块语法高亮（可延迟/可选加载） | P1 |
| FR-7 | 文件外部变更时自动重载（watch） | P1 |
| FR-8 | 用户自定义主题（配置文件） | P1 |
| FR-9 | 记住上次阅读位置/字号/主题等偏好 | P2 |
| FR-10 | 跟随系统深色模式自动切换主题（可选） | P2 |
| FR-11 | 本地相对路径与远程（http/https）图片加载，异步 + 缓存 | P1 |
| FR-12 | **简单源码编辑器（编辑/预览单视图切换）** | P0 |
| FR-13 | **编辑器基础编辑：光标、选区、撤销/重做、跳转** | P0 |
| FR-14 | **编辑器语法高亮：Markdown 全文 + 内嵌代码块** | P0 |
| FR-15 | **编辑后保存（`Cmd+S`）/ 另存，含未保存提示** | P0 |
| FR-16 | **编辑视图与状态标记（已修改/未修改），修改后预览同步** | P1 |

### 3.2 非功能需求（NFR）

| 编号 | 需求 | 目标值 | 说明 |
| --- | --- | --- | --- |
| NFR-1 | 二进制体积 | Release ≤ 10 MB（理想 < 5 MB） | strip + LTO 后实测 |
| NFR-2 | 冷启动时间 | < 150 ms（Apple Silicon） | 用 `hyperfine` 基准 |
| NFR-3 | 内存占用 | 静态文档 < 80 MB | 避免无界缓存 |
| NFR-4 | 无 webview 依赖 | 硬约束 | 不引入 `wry` / `tauri` / `webview` |
| NFR-5 | 平台 | macOS 优先 | macOS 13+（Ventura 起） |
| NFR-6 | 可维护性 | 模块解耦、平台层抽象 | 便于后续扩展 Linux/Win |

---

## 4. 技术选型

### 4.1 约束推导

「无 webview + Rust + 体积小 + 启动快」四条约束叠加后，可选方案被收敛到 **原生 GUI 工具库**。核心决策点有三个：

1. **窗口/渲染层**：用什么 GUI 框架与图形后端。
2. **文本整形与字体**：如何正确排版（尤其 CJK）。
3. **Markdown 解析**：解析器选型。

### 4.2 窗口/渲染层对比

| 方案 | 体积 | 启动 | 富文本/排版 | 维护/生态 | 结论 |
| --- | --- | --- | --- | --- | --- |
| **eframe/egui（glow 后端）** | 中（5–12 MB） | 快（<150ms） | 需借助 `egui_commonmark` 或自绘块布局 | 活跃、文档丰富 | ✅ **推荐** |
| iced | 中偏大 | 中 | 富文本/文档流弱，需大量自研 | 活跃 | ❌ 不适合文档阅读 |
| slint | 中 | 快 | 声明式 UI，文档流布局不匹配 | 活跃 | ❌ 不适合富文本阅读 |
| fltk-rs | 极小 | 极快 | 富文本弱、外观非原生 | 一般 | ❌ 文本渲染能力不足 |
| winit + softbuffer + cosmic-text（纯自绘） | 极小（1–3 MB） | 极快 | 完全可控，但滚动/表格/高亮需全手写 | 各组件活跃 | ⚠️ 极致体积的备选/演进方向 |
| egui（wgpu 后端） | 大（>20 MB） | 中 | 同 egui | 活跃 | ❌ wgpu 显著增大体积 |

**决策**：主推 **`eframe`（`glow` 后端）+ `egui`**，理由是它在「开发效率 / 体积 / 启动 / 文本能力」四者间平衡最好，且 `egui_commonmark` 提供了成熟的 Markdown→egui 渲染实现，避免从零手写全部排版逻辑。

- 使用 `glow`（OpenGL）而非 `wgpu`：OpenGL 在 macOS 上虽已被标记 deprecated 但仍可用（兼容到 4.1），关键是**二进制体积远小于 wgpu**，启动路径更短。
- **备选路线**：若后续对体积有更激进要求，可演进到 `winit + softbuffer + cosmic-text` 纯 CPU 自绘（见 [13.风险与缓解](#13-风险与缓解)），作为独立实验分支而非首版目标。

### 4.3 文本整形与字体

| 关注点 | 结论 |
| --- | --- |
| CJK 支持 | **硬需求**（目标用户读写中文）。渲染层必须加载系统中文字体（macOS 上 PingFang SC），并保证整形正确。 |
| 整形引擎 | egui 较新版本（0.29+）通过 `epaint`/`parley`（底层 `cosmic-text` + `rustybuzz`）改进了复杂文本整形。首版**实测验证 CJK 渲染质量**，若不达标则回退到 `cosmic-text` 直接排版。 |
| 等宽字体 | 代码块使用 SF Mono（macOS 系统字体）。 |
| 字体策略 | **不内嵌字体文件**（省体积），启动时从 `/System/Library/Fonts` 懒加载 + `memmap` + 缓存。 |

### 4.4 Markdown 解析器

| 解析器 | 说明 | 结论 |
| --- | --- | --- |
| **`pulldown-cmark`** | 拉取式、CommonMark 标准、GFM 表格/任务列表/删除线/脚注、依赖极简、性能好 | ✅ **推荐** |
| `comrak` | pulldown-cmark 的封装，功能多但更重（内置 syntect 高亮） | ❌ 更重 |
| `markdown`（jsx/自研） | 非标准、功能不全 | ❌ |

`pulldown-cmark` 提供的是**事件流**（`Event`），天然适合我们「事件 → 渲染块」的流水线，且不绑定任何 HTML 输出，符合「无 webview」的诉求。

### 4.5 语法高亮（预览代码块 + 编辑器共用引擎）

代码高亮覆盖两处：**预览视图中渲染的代码块**（FR-6）与**编辑视图中 Markdown 全文 / 内嵌代码块**（FR-14）。二者共享同一高亮引擎，避免维护两套。

- 方案 A：`syntect`——功能全、语法与主题丰富、质量高；缺点：+数 MB 体积、启动需加载语法集。
- 方案 B：轻量正则/关键字高亮——体积小、启动快；缺点：质量一般、覆盖弱。
- 决策：**采用 `syntect` 作为统一高亮引擎**，但用「按需 + 缓存 + 懒加载」策略控制成本：
  - `feature = "highlight"` 门控（v0.2 起默认**开启**，因为编辑器代码高亮为核心需求），但**语法集与主题在首次出现相应语言时才加载并缓存**，不随启动全量加载；
  - 仅预置少量常用语法（`Markdown` + 常见语言如 Rust/Python/JS/JSON/TOML/C 等），用户可配置扩展，避免打包全部 300+ 语法；
  - 高亮结果按「行/块 + 语言 + 主题」做 LRU 缓存，长文档不重算已稳定区域。
- 若后续对体积极度敏感，可降级到方案 B（`feature = "lite-highlight"`），接口上对渲染与编辑器透明。

### 4.6 其余依赖选型

| 用途 | 库 | 说明 |
| --- | --- | --- |
| CLI 解析 | `clap`（derive，最小 features）或 `lexopt` | 追求极致体积时用 `lexopt`（零 proc-macro） |
| 配置/主题序列化 | `serde` + `toml` | 配置与主题均为 TOML，可读性好 |
| 配置目录定位 | `directories` | 遵循 XDG / macOS 约定 |
| 文件监听 | `notify` | 文件变更自动重载 |
| 日志 | `log` + `env_logger`（可裁剪） | 仅诊断用，release 可禁用 |
| macOS 事件桥 | `objc2` / `objc2-app-kit` | 捕获 `open` 文件事件（见 §10） |
| 图片解码 | `image`（最小 features，仅 png/jpeg/gif/webp） | 本地图片解码 |
| HTTP 客户端 | `ureq`（阻塞 + rustls，最小 features） | 远程 http/https 图片拉取 |

### 4.7 编辑器文本层选型

「简单编辑器」需要一个**原生文本编辑部件**（光标、选区、撤销、IME/CJK 输入）。在 eframe/egui 生态中评估：

| 方案 | 说明 | 结论 |
| --- | --- | --- |
| **`egui` 原生 `TextEdit`** | egui 内置多行文本编辑，支持光标/选区/撤销、CJK 输入；但**原生不提供语法高亮**，需自建「分片高亮渲染」。 | 可行（需叠加高亮分片），做底层 |
| **`egui_code_editor`（社区）** | 基于 egui 的开源代码编辑器，提供行号、**语法高亮**（集成 syntect）；维护活跃度中等。 | ⚠️ 优先候选，需实测 CJK/体积 |
| **`egui_epaint` + 自绘文本层** | 用 `TextEdit` 做输入底层，叠加高亮分片绘制（`TextFormat`/`RichText`）。 | 回退方案，可控但工作量较大 |

**决策**：优先评估 **`egui_code_editor`（`syntect` 高亮）**；若其 CJK / 主题集成 / 体积不达标（尤其中文输入与撤销栈），**回退到「`TextEdit` 底层 + 高亮分片自绘」**（方案 B）。

- 选型验证点（M6 之前必做）：中文输入（拼音/I 型随打随现）、撤销/重做栈、选区高亮与主题色注入、长文档滚动性能。
- 编辑器高亮与预览高亮（§4.5）**共用 `syntect` 缓存与主题映射**，保持颜色一致。
- **排版一致性**：编辑视图使用与阅读正文一致的等宽/正文字体与字号，切换视图时排版尽量接近。

---

## 5. 总体架构

### 5.1 分层架构

```
┌───────────────────────────────────────────────────────────┐
│                      CLI 入口 (main)                       │
│     lexopt/clap 解析 → 路径 / 主题 / 字号 / watch / 视图     │
└───────────────────────────┬───────────────────────────────┘
                            │  AppConfig + 文件路径
┌───────────────────────────▼───────────────────────────────┐
│                     应用外壳 (App)                          │
│    eframe::App 实现：状态机 / 生命周期 / 命令分发 / 单实例    │
│    视图状态（预览 ‖ 编辑） / 脏标记 / 保存流程              │
└──────┬──────────────────┬──────────────────┬───────────────┘
       │                  │                  │
┌──────▼───────┐  ┌───────▼────────┐  ┌──────▼────────┐
│  文档模型      │  │   渲染层        │  │  平台适配层    │
│  Document     │  │  Renderer      │  │  Platform     │
│ (解析+缓存)    │  │ (egui 块组件)   │  │ (macOS 事件/  │
└──────┬───────┘  └───────┬────────┘  │  单实例/菜单)  │
       │                  │           └───────────────┘
┌──────▼───────┐  ┌───────▼────────┐  ┌───────────────┐
│  主题系统      │  │  字体/文本引擎   │  │   编辑器       │
│  Theme        │  │  Fonts(CJK)    │  │  Editor        │
│  (+高亮配色)   │  └────────────────┘  │  (TextEdit/    │
└──────────────┘                        │   egui_code_editor │
                                       │  + syntect 高亮) │
                                       └────────────────┘
```

> 编辑/预览共享同一份 `Document` 源文本：切换视图不丢失编辑内容，仅在「编辑 → 预览」时用编辑缓冲重新解析文档（见 §8.5）。

### 5.2 模块划分（Crate 内部模块）

| 模块 | 职责 | 关键依赖 |
| --- | --- | --- |
| `cli` | 参数解析、`--help/--version/--list-themes`、`--edit`（直接进入编辑视图） | `clap`/`lexopt` |
| `config` | 加载/保存 `config.toml`（主题、字号、行高、列宽、视图偏好、自动保存） | `serde`+`toml`、`directories` |
| `document` | 文件读取、Markdown 解析为「文档树/块序列」、增量重载、**编辑缓冲↔IR 同步** | `pulldown-cmark` |
| `theme` | 主题模型、内置主题注册、自定义主题扫描、切换、**高亮配色映射** | `serde`+`toml` |
| `highlight` | **共用高亮引擎**：`syntect` 语法集/主题懒加载 + LRU 缓存；输出供渲染与编辑器消费 | `syntect`（`highlight` feature 内） |
| `editor` | **简单编辑器**：文本编辑部件（光标/选区/撤销/保存）、Markdown 全文 + 代码块高亮、脏标记、编辑→预览重解析触发 | `egui` `TextEdit` / `egui_code_editor`、`highlight` |
| `render` | egui 绘制：块渲染、滚动、图片、表格、代码块（含高亮） | `egui`、`egui_commonmark` 或自绘 |
| `fonts` | 系统字体发现、加载、回退链、缓存（正文/等宽） | `egui` fonts、`memmap2` |
| `watch` | 文件监听、防抖、重载触发 | `notify` |
| `platform` | 平台抽象 trait + macOS 实现（open 事件、单实例、菜单、快捷键） | `objc2`（仅 macOS） |
| `app` | `eframe::App` 实现，串联以上模块，维护「预览/编辑」视图状态机与保存流程 | `eframe` |

### 5.3 依赖方向（规则）

- 依赖**单向**：`app` → 各领域模块；`render` 只依赖 `document` + `theme` + `fonts` + `highlight`，不反向依赖 `app`。
- `editor` 依赖 `document`（源文本/IR）`+ highlight`（高亮）`+ theme`，**不依赖 `render`**，保证「编辑」「预览」两个视图可独立演进。
- `highlight` 是**共享公共模块**：`render` 与 `editor` 都消费它，二者不互相依赖。
- `platform` 通过 **trait 抽象**暴露能力（如 `FileOpenEvents`、`SingleInstance`），核心逻辑不感知 macOS 细节。
- `document`（解析）与 `render`（绘制）**解耦**：先产出中间「块」结构，渲染层消费，便于替换渲染后端（egui ↔ 自绘）。
- 编辑缓冲与 IR 的同步只发生在**视图切换或显式解析**时，编辑器常驻不阻塞解析线程。

---

## 6. 模块详细设计

### 6.1 `document`：文档模型

Markdown 不直接渲染 `Event` 流，而是先转换为**中间表示（IR）**，好处是：可缓存、可增量更新、渲染层解耦。

```rust
// 概念性 IR（示意，非最终代码）
pub enum Block {
    Heading { level: u8, text: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    CodeBlock { lang: Option<String>, text: String },
    Blockquote { blocks: Vec<Block> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    Table { header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    ThematicBreak,
    Image { src: PathBuf, alt: String },
    Html(String), // 降级：安全过滤后忽略或纯文本显示
}

pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Code(String),
    Link { dest: String, children: Vec<Inline> },
    Image { src: String, alt: String },
    Strikethrough(Vec<Inline>),
}

pub struct Document {
    pub path: PathBuf,
    pub blocks: Vec<Block>,
    // 元信息：解析耗时、块数量等（用于诊断）
}
```

- `Document::parse(path)`：读取 → `pulldown-cmark` 事件流 → `Event` 归约为 `Vec<Block>`。
- **缓存策略**：文档解析结果缓存；文件 `mtime` 变化触发 `watch` 重新解析（见 §6.5）。
- **大文件**：首版整文件解析（`pulldown-cmark` 本身很快）；渲染侧做**可视区裁剪**，不做解析侧分片，避免复杂度。

### 6.2 `render`：渲染层

- 基于 `egui`，用 `ScrollArea::vertical()` 承载文档内容，列宽默认约 `720px` 居中，模拟阅读排版。
- 渲染优先级：首版直接评估 `egui_commonmark`（成熟、支持主题色注入）；若其**维护停滞或 CJK/表格覆盖不足**，则退回「自绘块组件」（`pulldown-cmark` IR → egui `Label`/`RichText`/`Grid` 手写映射），IR 设计（§6.1）已为此预留。
- 关键渲染点：
  - **标题层级**：`RichText::heading()` 或自定义字号/加粗映射。
  - **代码块**：`Frame` + 等宽字体 + 高亮（复用 `highlight` 模块，§6.8）；横向滚动。
  - **表格**：`egui::Grid` 或自定义 `TableBuilder`，处理对齐与边框。
  - **图片**：`egui::Image`；本地相对路径直接解码，远程 http/https 图片由 `ureq` 异步拉取 + LRU 缓存，失败显示占位。
  - **链接**：悬停显示 URL，`Cmd+Click` 用 `open` 打开系统默认应用（经 `platform` 层）。
- **可视区裁剪（虚拟化）**：长文档按滚动位置只布局/绘制可见块，其余跳过，保证万行文档滚动流畅。`egui` 的 `ScrollArea::show_rows`（按行虚拟化）可作为实现参考。

### 6.3 `fonts`：字体引擎

- 启动时扫描 `~/Library/Fonts` 与 `/System/Library/Fonts`，建立字体回退链：
  1. 正文：PingFang SC（中文）/ SF Pro（拉丁）。
  2. 等宽：SF Mono / Menlo。
- 字体以 `memmap` 映射并按需懒加载到 egui 字体图集（`FontDefinitions`），**不随程序分发字体文件**。
- 提供 `reload_fonts()` 供用户更换字体后热生效。

### 6.4 `theme`：主题系统

详见 [§7 主题系统设计](#7-主题系统设计)。

### 6.5 `watch`：文件监听

- 用 `notify` 监听当前文件所在目录，`DebouncedEvent`（如 250ms 防抖）触发文档重载。
- 重载后**保持滚动位置与阅读进度**（按块/行锚点恢复，而非绝对像素）。
- 监听失败（文件被删除、权限）时降级为手动刷新（`Cmd+R`）。
- **与编辑器交互**：若文件被外部修改而编辑视图有未保存的本地编辑缓冲，先弹「以磁盘版本刷新 / 保留本地编辑」选择，避免静默覆盖用户输入；程序自身 `Cmd+S` 写回产生的事件被忽略（见 §8.5）。

### 6.6 `config`：配置

默认配置路径：`~/.config/mdbijou/config.toml`（遵循 XDG 约定）；主题目录 `~/.config/mdbijou/themes/`。

```toml
# config.toml 示例
theme = "github-light"       # 默认主题 id
font_size = 16               # 正文字号（pt）
line_height = 1.5            # 行高倍率
content_width = 720          # 阅读列宽（px）
watch = true                 # 文件监听
follow_system_theme = false  # 跟随系统深色模式

# —— 编辑器（v0.2）——
default_view = "preview"     # preview | edit，启动进入的视图
editor_font_size = 15        # 编辑视图字号（pt）
show_line_numbers = true     # 编辑视图是否显示行号
highlight = true             # 语法高亮开关（feature 已编译时）
tab_size = 4                 # 制表符宽度
auto_save = false            # 编辑后是否自动保存（见 §8.5）
```

### 6.7 `editor`：简单编辑器

编辑视图在一个**单视图 Tab** 中承载，核心状态机：`Preview ⇄ Edit`，共享同一 `Document`（源文本 + 解析缓存）。

**关键设计点：**

1. **底层文本部件**：优先 `egui_code_editor`；否则回退 `TextEdit` 多行 + 高亮分片自绘（见 §4.7）。必须支持：光标移动/选择、撤销/重做、**CJK 输入法**、行号、Tab/空格缩进（`tab_size`）。
2. **语法高亮（FR-14）**：对编辑器全文按 Markdown 行做高亮——标题/加粗/斜体/链接/行内代码等用 Markdown 语法，**```` ```lang ```` 围栏内的代码用目标语言语法**（复用 `highlight` 模块，见 §6.8）。高亮按「可见行窗口」计算并缓存，长文档滚动不重算整文。
3. **脏标记（FR-16）**：`Document.dirty` 在编辑缓冲变化时置位；标题栏显示「•」与「未修改」；切换预览 / 打开新文件 / 退出时若有未保存修改弹确认。
4. **编辑 → 预览同步（FR-16）**：切到预览时用编辑缓冲重新解析（`pulldown-cmark` 极快）；**切回编辑不重解析**，保留 IR 缓存与滚动位置。
5. **撤销/重做**：依赖文本部件的撤销栈；若回退到自绘方案，自行维护命令式撤销栈。
6. **保存（FR-15）**：`Cmd+S` 写回原路径（原子写：临时文件 + rename）；`Cmd+Shift+S` 另存。保存后清除脏标记，`watch` 收到自身写回时忽略（见 §8.5）。

> 编辑器刻意不做：分屏、查找替换、多光标、实时 WYSIWYG（见 §2.2 非目标）。

### 6.8 `highlight`：共用高亮引擎

- 统一封装 `syntect`：语法集（SyncSet）与主题（ThemeSet）**首次使用某语言时才加载**并缓存；内置语法白名单（Markdown + Rust/Python/JS/TS/JSON/TOML/YAML/C/C++/Ruby 等常用），其余可经用户配置扩展。
- 输出**行/块 → 高亮片段（span + 颜色 token）**，`render`（预览代码块）与 `editor`（全文/代码块）都消费同一份渲染结果，保证颜色一致。
- LRU 缓存按 `(语言, 主题, 内容哈希)` 缓存行级结果，避免长文档重复计算。

---

## 7. 主题系统设计

主题即「一组色值 + 可选 egui 视觉覆盖」，以 TOML 文件描述，支持内置与用户自定义。

### 7.1 主题数据模型

```toml
# themes/github-light.toml 示例
id   = "github-light"
name = "GitHub Light"
kind = "light"          # light | dark，用于「跟随系统」时自动配对

[colors]
background    = "#ffffff"   # 页面背景
foreground    = "#24292e"   # 正文
heading       = "#24292e"   # 标题
heading_rule  = "#d0d7de"   # 标题下划线
code_fg       = "#e01e5a"   # 行内代码
code_bg       = "#f6f8fa"   # 行内/代码块背景
blockquote_fg = "#57606a"
blockquote_bg = "#f6f8fa"
blockquote_bar= "#d0d7de"
link          = "#0969da"
link_hover    = "#0550ae"
table_border  = "#d8dee4"
table_header_bg = "#f6f8fa"
hr            = "#d8dee4"
selection_bg  = "#b6d7ff"
image_bg      = "#f6f8fa"   # 图片占位背景

[egui]                      # 可选：透传覆盖 egui Visuals
panel_fill    = "#ffffff"
window_fill   = "#ffffff"

[syntax]                   # 可选：编辑器与代码块语法高亮配色（供 syntect 映射）
comment       = "#6e7781"  # 注释
keyword       = "#cf222e"  # 关键字
string        = "#0a3069"  # 字符串
number        = "#0550ae"  # 数字
function      = "#8250df"  # 函数名
type          = "#953800"  # 类型名
variable      = "#24292e"  # 变量
operator      = "#000000"  # 运算符
punctuation   = "#57606a"  # 标点
constant      = "#0550ae"  # 常量
markup_heading= "#0550ae"  # Markdown 标题
markup_link   = "#0969da"  # Markdown 链接/引用
markup_code   = "#e01e5a"  # Markdown 行内代码
```

> 高亮配色由 `Theme` 提供（`Theme.syntax.*`），缺省时由 `kind`（light/dark）自动派生一套 `base16` 风格默认调色板；用户可在自定义主题的 `[syntax]` 段覆盖。`highlight` 模块把 `syntect` 输出的语义 token（comment/keyword/string…）映射到这些色值，从而与正文、选区、弱化色保持协调。

### 7.2 内置主题（首版 ≥3）

| id | 名称 | kind |
| --- | --- | --- |
| `github-light` | GitHub Light | light |
| `github-dark` | GitHub Dark | dark |
| `sepia` | 护眼羊皮纸 | light |
| （可选）`solarized-dark` | Solarized Dark | dark |

### 7.3 加载与切换

1. **注册表 `ThemeRegistry`**：合并「内置主题 + `~/.config/mdbijou/themes/*.toml`」；用户主题同 id 可覆盖内置。
2. **切换途径**：
   - 快捷键 `Cmd+T` 循环切换；
   - CLI：`mdbijou --theme github-dark file.md`；
   - 跟随系统深色模式（`follow_system_theme = true` 时，`light`/`dark` 自动配对）。
3. **即时生效**：切换主题只重建颜色上下文并触发重绘，**不重解析文档**（文档 IR 缓存复用）。
4. **`--list-themes`**：列出可用主题 id / name / kind。

### 7.4 主题与渲染解耦

渲染层只通过 `Theme.colors.*` 取色，不直接硬编码色值；egui 的 `Visuals` 由主题 `[egui]` 段派生，保证窗口/滚动条/选中色等控件与文档配色一致。**编辑器与代码高亮同样只消费 `Theme`**：正文/字号取自 `Theme`，高亮 token 色取自 `Theme.syntax.*`，因此切换主题时阅读与编辑视图一并即时生效，且编辑区高亮与预览代码块颜色一致。

---

## 8. 关键流程

### 8.1 启动流程（冷启动路径）

```
main
 1. 解析 CLI（clap/lexopt）→ 得到文件路径 + 覆盖参数
 2. 单实例检查：若无已运行实例则继续，否则把路径转发给已运行实例并退出
 3. 加载 config.toml（缺省用默认值，失败降级默认值）
 4. 初始化主题注册表 + 选中当前主题
 5. 初始化 egui/eframe（glow 上下文、字体懒加载）
 6. 解析文档（pulldown-cmark → IR）
 7. 首帧渲染（只布局可见区）
```

> 优化点：文档解析放在首帧之前但**与字体加载并行**（字体懒加载，非首屏必需的不阻塞）。避免任何网络、插件、语法高亮加载参与启动路径——`syntect` 语法集**不随启动加载**，仅在进入编辑视图且出现对应语言时才懒加载（见 §6.8）。默认视图由 `default_view` 决定：`preview`（默认）按上图冷启动；`edit` 则额外初始化编辑器文本部件与首屏高亮。

### 8.2 打开文件流程（CLI / Finder 双击）

```
CLI:  mdbijou file.md
       └─> 单实例？否 → 直接打开；是 → IPC 转发路径 → 已运行实例 load_document(file.md)

Finder 双击 / open -a：
       NSApplication 收到 kAEOpenDocuments Apple Event
       └─> platform 层捕获路径 → channel 转发到 App 主线程 → load_document()
```

### 8.3 渲染流程（每帧）

```
egui update()
  1. 读取滚动偏移 → 计算可视块区间
  2. 对可视块：IR Block → egui 组件（RichText/Frame/Grid/Image）
  3. 应用当前 Theme 取色
  4. 输出绘制（egui 交给 glow 渲染）
```

### 8.4 主题切换流程

```
Cmd+T / --theme / 跟随系统
  └─> ThemeRegistry 取新主题 → 重建 Theme + egui Visuals
      └─> 仅触发重绘（ctx.request_repaint()），文档 IR 不重解析
```

### 8.5 编辑 / 预览切换与保存流程

```
视图切换（Cmd+E）：
  edit -> preview：用编辑缓冲重新解析（pulldown-cmark → IR）→ 更新预览 → 保留编辑滚动位置
  preview -> edit ：直接进入编辑部件（复用源文本 + 高亮可见行窗口），不重解析

保存（Cmd+S）：
  编辑缓冲变化 → dirty=true，标题栏显示「•」
  Cmd+S：写临时文件 → rename 原子替换 → dirty=false → watch 忽略自身写回事件
  Cmd+Shift+S：另存对话框（由 platform 层调用 NSSavePanel）

退出 / 打开新文件时若 dirty：
  弹「保存 / 不保存 / 取消」确认（经 platform 层）

自动保存（auto_save=true，可选）：
  编辑停顿 1s 后静默保存，不打断输入
```

---

## 9. 性能与体积优化

### 9.1 编译期（二进制体积）

`Cargo.toml` 使用自定义 release profile：

```toml
[profile.release]
opt-level = "z"        # 体积优先（若更看重运行时可用 "3"）
lto = "fat"            # 全量链接时优化
codegen-units = 1      # 单代码单元，利于内联与去重
panic = "abort"        # 去除 panic unwind 表
strip = "symbols"      # 剥离符号
```

附加手段：

- 关闭 `eframe` 的默认 features，只启用 `glow`、`persistence`（可选）等必需项，关掉 `wgpu`、`wayland`（macOS 不需要）。
- 依赖**用最小 features**：`image` 只开 png/jpeg/gif/webp；`clap` 关闭非必要 features 或用 `lexopt`。
- `syntect` 由 `feature = "highlight"` 门控。v0.2 起该 feature **默认开启**（编辑器代码高亮为核心需求），但**语法集/主题按需懒加载 + LRU 缓存**，不随启动加载（见 §6.8）；体积敏感的发行版可关闭 `highlight` 降级为 `lite-highlight`。审计 `syntect` 体积，必要时只编译常用语言相关的 `assets`，并可用 `cargo bloat` / `cargo tree -d` 持续收敛。
- 定期用 `cargo bloat` / `cargo tree -d` 审计依赖与体积，超阈值即收紧。

### 9.2 运行时（启动速度）

- 字体懒加载 + memmap + 缓存；非首屏字体不阻塞。
- 文档解析在首帧前完成（`pulldown-cmark` 极快，几十万字符通常 < 10ms 级）。
- 首屏只布局可见区，长文档避免一次性生成全部 widget。
- 无网络请求、无插件加载、无数据库初始化。
- **编辑视图**：高亮只对**可见行窗口**计算（见 §6.7），滚动用窗口滑动更新，不做整文高亮；`syntect` 语法首次用到某语言时才加载（见 §6.8）。

### 9.3 内存

- 文档 IR 缓存有上限；超大文件按需裁剪或 LRU 释放。
- 图片解码结果 LRU 缓存 + 上限，超出释放为路径占位。
- **编辑/高亮**：高亮行级结果 LRU 缓存（按语言+主题+内容哈希）；离开编辑视图释放编辑部件额外缓冲；编辑缓冲与 IR 不重复保留整文副本（编辑缓冲即源文本，IR 为派生态）。

### 9.4 体积/启动基准

- 用 `hyperfine --warmup 3 'mdbijou bench.md'` 测冷启动。
- 用 `ls -lh` / `size` 记录各 milestone 体积，纳入 CI 检查（体积/启动回归告警）。

---

## 10. macOS 平台适配

### 10.1 应用形态

- **首版交付形态**：单个可执行文件 `mdbijou` + 安装脚本（复制到 `~/bin` 或 `/usr/local/bin`，可选注册 LaunchServices）。可直接 `mdbijou file.md`。
- **后续可选**：`.app` 打包（`mdbijou.app` + `Info.plist`），用于 Finder 双击、Dock、LaunchServices 完整关联。

### 10.2 `Info.plist` 文档类型

> 延后：随 `.app` 打包阶段引入；首版以「可执行文件 + 安装脚本」交付，本节为后续实现预留。

```xml
<key>CFBundleDocumentTypes</key>
<array>
  <dict>
    <key>CFBundleTypeName</key>  <string>Markdown Document</string>
    <key>CFBundleTypeRole</key>  <string>Viewer</string>
    <key>LSHandlerRank</key>     <string>Alternate</string>
    <key>CFBundleTypeExtensions</key>
    <array><string>md</string><string>markdown</string></array>
  </dict>
</array>
```

注册后，Finder 右键「打开方式」与 `open file.md` 可指向本应用。

### 10.3 打开文件事件（关键）

eframe/winit 在 macOS 上**不直接暴露** `open file` Apple Event，需在 `platform` 层补充：

- 通过 `objc2` / `objc2-app-kit` 向 `NSAppleEventManager` 注册 `kAEOpenDocuments`（`kCoreEventClass`）处理器，捕获 Finder / `open -a` 传入的文件路径；
- 处理器将路径经 `crossbeam-channel`/`std::sync::mpsc` 转发到 App 主循环，由 `load_document()` 统一处理。

> 若未来切换到纯自绘路线（`tao`），`tao` 提供 `Event::Opened`，可移除该 shim。此层独立成 `platform/macos`，不影响核心逻辑。

### 10.4 单实例与文件转发

macOS 上连续 `open a.md; open b.md` 会触发多进程。设计：

- 单实例锁：Unix domain socket 或 `~/.config/mdbijou/` 下的锁文件 + pid；
- 后续启动检测到已有实例 → 通过 socket 把路径发给已有实例 → 新进程立即退出（返回 0）；
- 已有实例收到路径 → `load_document()`，并 `window.focus()`（通过 `winit` 激活窗口到前台）。

### 10.5 菜单与快捷键（macOS 原生体验）

| 功能 | 快捷键 |
| --- | --- |
| 打开文件 | `Cmd+O` |
| 重新加载 | `Cmd+R` |
| 切换主题 | `Cmd+T` |
| 增大/减小字号 | `Cmd+=` / `Cmd+-` |
| 滚动到顶/底 | `Cmd+↑` / `Cmd+↓` |
| 退出 | `Cmd+Q` |
| **预览/编辑 切换** | `Cmd+E` |
| **保存 / 另存** | `Cmd+S` / `Cmd+Shift+S` |
| **撤销 / 重做**（编辑视图） | `Cmd+Z` / `Cmd+Shift+Z` |
| **文本选择**（编辑视图） | `Cmd+A` / `Shift+方向键` |

### 10.6 深色模式

- 通过 `NSAppearance` 或环境检测系统外观，结合 `follow_system_theme` 自动切换 `light`/`dark` 主题。

---

## 11. 测试策略

| 层 | 手段 |
| --- | --- |
| 单元测试 | `document`（事件→IR 归约）、`theme`（加载/合并/覆盖）、`config`（默认值/容错） |
| 快照测试 | `insta` 对解析出的 IR 结构做快照，防回归 |
| 渲染回归 | `egui_kittest` 无头渲染，断言块结构/颜色注入正确 |
| 集成测试 | CLI 参数矩阵（`--theme`、`--list-themes`、`--edit`、缺参、非法路径） |
| 编辑器测试 | 光标/选区/撤销重做、**CJK 输入**、脏标记、保存（原子写 + 覆盖）、编辑→预览重解析、高亮 token 正确性（`syntect` 输出→span 映射） |
| 平台测试 | macOS 手动用例：Finder 双击、`open -a`、连续 open 单实例转发、深色模式跟随、`Cmd+E/S/Z` 快捷键、NSSavePanel 另存、未保存退出确认 |
| 基准 | `hyperfine` 启动耗时、编辑视图滚动/高亮帧耗时、`cargo bloat` 体积审计，纳入 CI 阈值 |

---

## 12. 里程碑与路线图

| 里程碑 | 内容 | 验收标准 |
| --- | --- | --- |
| **M0 脚手架** | Cargo 工程、release profile、CLI、空窗口打开纯文本 | `mdbijou x.md` 弹出窗口显示纯文本 |
| **M1 核心渲染** | pulldown-cmark 解析 + IR + 基础块渲染（标题/段落/粗斜体/链接/列表/引用/代码块/表格/图片〔本地 + 远程〕） | 标准样例 md 视觉正确，CJK 正常 |
| **M2 主题** | 主题模型、≥3 内置主题、`Cmd+T`/`--theme` 切换、`--list-themes` | 切换即时生效且不重解析 |
| **M3 macOS 集成** | `Info.plist`、`open -a`、单实例转发、菜单快捷键、深色跟随 | Finder 双击/连续 open 均正确 |
| **M4 性能** | 虚拟化滚动、字体懒加载、体积/启动优化、CI 体积与启动基准 | NFR-1/2 达标 |
| **M5 增强** | 文件 watch 热重载、预览代码块语法高亮、用户自定义主题、阅读位置记忆 | 外部改动自动刷新；预览高亮可开关 |
| **M6 简单编辑器（v0.2）** | 编辑/预览切换、基础编辑（光标/选区/撤销/保存）、编辑器 Markdown+代码高亮、脏标记 | `Cmd+E` 切换；编辑后 `Cmd+S` 保存；Markdown 全文与代码块高亮正确，CJK 输入正常 |

---

## 13. 风险与缓解

| 风险 | 影响 | 缓解措施 |
| --- | --- | --- |
| egui 的 CJK 复杂文本整形质量不足 | 中文排版错乱（FR-5 核心） | M1 前用真实中文文档做**选型验证**；不达标则改用 `cosmic-text` 排版或走自绘路线 |
| eframe 无法直接收 macOS `open` 事件 | Finder 双击失效（FR-4） | `objc2` Apple Event shim（§10.3）；或迁移 `tao` |
| OpenGL 在 macOS 被 deprecated | 未来 macOS 可能移除 | 短期可用（兼容 4.1）；预留后端抽象，必要时切 `wgpu` 或 `softbuffer` |
| `egui_commonmark` 维护停滞 / 功能缺口 | 表格/脚注等渲染不全 | IR 解耦设计（§6.1）允许快速替换为自绘块组件 |
| 超大文件首屏卡顿 | 阅读体验下降 | 可视区虚拟化（§6.2）+ 解析缓存 |
| 体积/启动超预期 | 偏离核心卖点 | `cargo bloat` 审计 + 最小 features + CI 阈值告警；必要时演进到纯自绘 |
| 编辑文本部件 CJK / 撤销栈不足（`egui_code_editor` 或 `TextEdit`） | 中文输入异常、撤销失效（FR-13/15 核心） | M6 前置**编辑器选型验证**（§4.7）；不达标回退「`TextEdit` 底层 + 高亮分片自绘」，自带撤销栈与 IME 处理 |
| `syntect` 拖大体量 / 高亮拖慢滚动 | 体积超 NFR-1、编辑卡顿 | 常用语法白名单 + 懒加载 + LRU（§6.8）；仅可视行高亮（§6.7）；超阈值关闭 `highlight` 降级 |
| 数据丢失（未保存编辑被覆盖 / watch 重置编辑器） | 用户内容损失 | 脏标记 + 退出/切换/打开确认（§8.5）；watch 自身写回忽略；原子写（临时文件 + rename） |
| 编辑器高亮与预览渲染色不一致 | 视觉割裂 | 高亮经 `Theme.syntax.*` 统一取色（§7.1），编辑与预览共用 `highlight` 引擎 |

---

## 14. 附录：依赖清单与体积估算

### 14.1 核心依赖（首版预期）

```
# 渲染与窗口
eframe = { version = "0.31", default-features = false, features = ["glow"] }
egui
# Markdown
pulldown-cmark
# CLI / 配置 / 主题
clap (或 lexopt)
serde, toml, directories
# 文件监听 / 图片 / 远程拉取
notify, image (最小 features), ureq (阻塞 + rustls, 最小 features)
# 语法高亮（feature = "highlight"，v0.2 默认开；懒加载 + LRU）
syntect        # 仅编译常用语言 assets；可选 default-features = false
# 编辑器（feature = "editor"，v0.2 默认开）
egui_code_editor   # 备选主体；若回退则用 egui::TextEdit + 高亮分片自绘
# macOS 事件（仅 target_os = "macos"）
[target.'cfg(target_os = "macos")'.dependencies]
objc2, objc2-app-kit
```

> `highlight` / `editor` 均为可裁剪 feature：体积敏感发行版可关闭，纯阅读器形态仍可用。

> 版本号以最终 `Cargo.toml` 锁定为准，此处仅为示意。

### 14.2 体积与启动估算（目标，实测为准）

| 路线 | 二进制体积（strip 后） | 冷启动 |
| --- | --- | --- |
| eframe(glow) + egui_commonmark（首版） | 约 5–12 MB | < 150 ms |
| winit + softbuffer + cosmic-text（演进） | 约 1–3 MB | < 50 ms |

---

## 附：已决议（Resolved Decisions）

| # | 问题 | 决议 |
| --- | --- | --- |
| 1 | 配置目录 | XDG：`~/.config/mdbijou/`（主题在 `themes/` 子目录） |
| 2 | 首版交付形态 | 可执行文件 + 安装脚本；`.app` 打包与 LaunchServices 完整关联延后 |
| 3 | 语法高亮 | **升级为 v0.2 核心需求（编辑器代码高亮）**：`feature = "highlight"` 默认开启；预览代码块与编辑器共用 `highlight` 模块（§6.8），懒加载 + LRU，可裁剪 |
| 4 | 远程图片 | 首版支持 http/https 远程图片，异步拉取 + 缓存，失败显示占位 |
| 5 | 简单编辑器 | v0.2 新增；**单视图「编辑/预览」切换（`Cmd+E`）**，基础编辑 + Markdown 全文 & 代码块高亮（§6.7） |
| 6 | 编辑范围边界 | 不做分屏、查找替换、多光标、实时 WYSIWYG（§2.2）；优先保证 CJK 输入、撤销/重做、脏标记与安全保存 |
