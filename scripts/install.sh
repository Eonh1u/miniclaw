#!/usr/bin/env bash
# miniclaw 安装脚本
# 用于在 Linux / macOS 上构建并安装 miniclaw 到本地

set -e

INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
BIN_DIR="$INSTALL_PREFIX/bin"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "=== miniclaw 安装脚本 ==="
echo "项目目录: $PROJECT_ROOT"
echo "安装目标: $BIN_DIR"
echo ""

# 1. 检查/安装系统依赖
install_system_deps() {
    if command -v apt-get &>/dev/null; then
        echo "[1/4] 检测到 Debian/Ubuntu，安装构建依赖..."
        sudo apt-get update -qq
        sudo apt-get install -y libssl-dev pkg-config build-essential
    elif command -v dnf &>/dev/null; then
        echo "[1/4] 检测到 Fedora/RHEL，安装构建依赖..."
        sudo dnf install -y openssl-devel pkg-config gcc
    elif command -v yum &>/dev/null; then
        echo "[1/4] 检测到 CentOS/RHEL，安装构建依赖..."
        sudo yum install -y openssl-devel pkg-config gcc
    elif command -v brew &>/dev/null; then
        echo "[1/4] 检测到 macOS (Homebrew)，安装构建依赖..."
        brew install openssl pkg-config 2>/dev/null || true
    else
        echo "[1/4] 未检测到包管理器，跳过系统依赖安装。"
        echo "      若编译失败，请手动安装 libssl-dev (或 openssl-devel) 和 pkg-config"
    fi
}

# 2. 检查/安装 Rust（支持镜像源）
# 使用方式：RUSTUP_MIRROR=rsproxy ./scripts/install.sh  或  RUSTUP_MIRROR=tuna ./scripts/install.sh
install_rust() {
    # 优先使用 rustup 安装的 cargo（~/.cargo/bin），版本较新
    if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        echo "[2/4] Rust 已安装 (rustup): $("$HOME/.cargo/bin/rustc" --version)"
        export PATH="$HOME/.cargo/bin:$PATH"
        return
    fi

    echo "[2/4] 安装 Rust (rustup)..."
    case "${RUSTUP_MIRROR:-}" in
        rsproxy)
            echo "      使用镜像: rsproxy.cn"
            export RUSTUP_DIST_SERVER="https://rsproxy.cn"
            export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
            curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y
            ;;
        tuna)
            echo "      使用镜像: 清华大学 TUNA"
            export RUSTUP_DIST_SERVER="https://mirrors.tuna.tsinghua.edu.cn/rustup"
            export RUSTUP_UPDATE_ROOT="https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            ;;
        ustc)
            echo "      使用镜像: 中科大 USTC"
            export RUSTUP_DIST_SERVER="https://mirrors.ustc.edu.cn/rust-static"
            export RUSTUP_UPDATE_ROOT="https://mirrors.ustc.edu.cn/rust-static/rustup"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            ;;
        *)
            echo "      使用官方源 (若网络异常可设置 RUSTUP_MIRROR=rsproxy 或 tuna)"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            ;;
    esac
    source "$HOME/.cargo/env" 2>/dev/null || true
    export PATH="$HOME/.cargo/bin:$PATH"
}

# 2.5 配置 crates.io 镜像（构建时拉取依赖用）
configure_cargo_mirror() {
    if [[ -z "${RUSTUP_MIRROR:-}" ]]; then
        return
    fi
    mkdir -p "$HOME/.cargo"
    local config="$HOME/.cargo/config.toml"
    if [[ -f "$config" ]] && grep -q 'rsproxy-sparse\|replace-with' "$config" 2>/dev/null; then
        return  # 已配置
    fi
    case "${RUSTUP_MIRROR}" in
        rsproxy)
            echo "      配置 crates.io 镜像: rsproxy.cn"
            cat >> "$config" << 'EOF'

[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
EOF
            ;;
        tuna)
            echo "      配置 crates.io 镜像: TUNA"
            cat >> "$config" << 'EOF'

[source.crates-io]
replace-with = 'tuna'
[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
[registries.tuna]
index = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
EOF
            ;;
    esac
}

# 3. 构建 release
build_release() {
    echo "[3/4] 构建 release 版本..."
    export PATH="${HOME}/.cargo/bin:${PATH}"
    cd "$PROJECT_ROOT"
    cargo build --release
}

# 4. 安装二进制
install_binary() {
    echo "[4/4] 安装到 $BIN_DIR ..."
    mkdir -p "$BIN_DIR"
    cp "$PROJECT_ROOT/target/release/miniclaw" "$BIN_DIR/miniclaw"
    chmod +x "$BIN_DIR/miniclaw"
    echo ""
    echo "✓ miniclaw 已安装到 $BIN_DIR/miniclaw"
}

# 5. 确保 ~/.miniclaw 存在
ensure_config_dir() {
    mkdir -p "$HOME/.miniclaw"
    if [[ ! -f "$HOME/.miniclaw/config.toml" ]]; then
        echo "首次运行时会自动生成 ~/.miniclaw/config.toml"
    fi
}

# 主流程
install_system_deps
install_rust
configure_cargo_mirror
build_release
install_binary
ensure_config_dir

echo ""
echo "=== 下一步 ==="
echo "1. 将 $BIN_DIR 加入 PATH（若尚未加入）:"
echo "   export PATH=\"$BIN_DIR:\$PATH\""
echo "   可将上述行加入 ~/.bashrc 或 ~/.zshrc"
echo ""
echo "2. 设置 LLM API Key（任选其一）:"
echo "   export LLM_API_KEY=\"your-key-here\""
echo "   或在 ~/.miniclaw/config.toml 的 [llm] 段中设置 api_key"
echo ""
echo "3. 在图形终端中运行: miniclaw"
echo "   （TUI 需要真实 TTY，不能在无头环境或管道中运行）"
echo ""
