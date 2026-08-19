# mdbijou — command runner (just)
# 用法: just [recipe]  ； 输入 `just --list` 查看全部命令

# ---------- 默认命令: 一键构建 + 运行 ----------
# 输入 `just` 直接编译调试版并运行（默认打开 sample.md，可跟文件参数）
default *args="sample.md":
    @just build
    @echo "→ 运行中: {{args}}"
    @cargo run -- {{args}}

# 调试构建（快，含调试信息）
build:
    cargo build

# 快速类型检查（比 build 更快）
check:
    cargo check

# 正式发布构建（体积小、启动快，产物 ~5.3 MB）
release:
    cargo build --release

# lint
clippy:
    cargo clippy --all-targets -- -D warnings

# 格式化
fmt:
    cargo fmt

# ---------- 运行 ----------
# 运行示例（默认打开 sample.md）
run *args="sample.md":
    cargo run -- {{args}}

# 以阅读/预览视图运行
preview *args="sample.md":
    cargo run -- {{args}}

# 直接进入编辑器视图（带代码高亮）
edit *args="sample.md":
    cargo run -- --edit {{args}}

# 运行已构建的 release 产物（免重编译）
run-release *args="sample.md":
    ./target/release/mdbijou {{args}}

# 列出内置主题
themes:
    cargo run -- --list-themes
    @echo ""

# ---------- 安装与产物 ----------
# 安装为 `mdb` 命令到 PATH（默认 ~/.local/bin）
install target="target/release/mdbijou":
    ./scripts/install.sh {{target}}

# 查看 release 二进制体积
size:
    @ls -lh target/release/mdbijou 2>/dev/null || echo "run 'just release' first"

# 记录体积/启动基线
baseline:
    ./scripts/bench.sh

# ---------- 打包与发布 ----------
# 从 logo.png 生成 assets/mdbijou.icns 应用图标（sips + iconutil）
icon:
    ./scripts/make-icon.sh

# 打包 dist/mdbijou.app（release 构建 + 图标 + Info.plist + ad-hoc 签名）
bundle:
    ./scripts/bundle.sh

# 打包 .app 并生成 dist/mdbijou-<version>.dmg 发布镜像
dmg:
    ./scripts/bundle.sh --dmg

# 清理构建产物
clean:
    cargo clean

# ---------- 极简体积构建（去掉语法高亮） ----------
# 体积敏感时关闭 highlight（用轻量高亮）构建
release-minus:
    cargo build --release --no-default-features --features "editor,remote-images"
    @echo "built WITHOUT syntect highlight (smaller)"

release-lite:
    cargo build --release --no-default-features --features "editor,remote-images,lite-highlight"
