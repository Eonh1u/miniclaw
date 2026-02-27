# miniclaw 部署指南

miniclaw 是单二进制 TUI 应用，无需数据库、Docker 或后台服务。本文档说明如何在各类环境中部署和运行。

---

## 快速安装（推荐）

```bash
# 克隆项目
git clone https://github.com/Eonh1u/miniclaw.git
cd miniclaw

# 执行安装脚本（会自动安装 Rust、系统依赖、构建并安装）
chmod +x scripts/install.sh
./scripts/install.sh
```

安装完成后，二进制位于 `~/.local/bin/miniclaw`。确保该目录在 PATH 中：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

---

## 手动安装

### 1. 系统依赖

| 系统 | 依赖 | 安装命令 |
|------|------|----------|
| Debian/Ubuntu | libssl-dev, pkg-config | `sudo apt-get install libssl-dev pkg-config` |
| Fedora/RHEL | openssl-devel, pkg-config | `sudo dnf install openssl-devel pkg-config` |
| macOS | openssl, pkg-config | `brew install openssl pkg-config` |

### 2. 安装 Rust

若未安装 Rust：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**网络受限时**，可用系统包管理器安装（版本可能较旧）：

```bash
# Fedora/RHEL/AlmaLinux
sudo dnf install rust cargo

# Debian/Ubuntu (需启用 rust 源或使用 rustup)
sudo apt install rustc cargo
```

### 3. 构建与安装

```bash
cd miniclaw
cargo build --release
# 二进制位于 target/release/miniclaw

# 安装到系统（可选）
sudo cp target/release/miniclaw /usr/local/bin/
# 或安装到用户目录
mkdir -p ~/.local/bin
cp target/release/miniclaw ~/.local/bin/
```

---

## 配置

### 首次运行

首次运行时会自动创建 `~/.miniclaw/config.toml`，默认使用 Qwen（阿里云 DashScope）。

### API Key

任选其一：

1. **环境变量**（推荐）：
   ```bash
   export LLM_API_KEY="your-api-key"
   ```

2. **配置文件**：编辑 `~/.miniclaw/config.toml` 的 `[llm]` 段，设置 `api_key`。

### 切换 Provider

通过环境变量或配置文件切换模型提供商：

```bash
# Anthropic Claude
export MINICLAW_PROVIDER="anthropic"
export MINICLAW_MODEL="claude-sonnet-4-20250514"
export LLM_API_KEY="your-anthropic-key"

# DeepSeek
export MINICLAW_PROVIDER="openai_compatible"
export MINICLAW_MODEL="deepseek-chat"
export MINICLAW_API_BASE="https://api.deepseek.com/v1"
export LLM_API_KEY="your-deepseek-key"
```

---

## 运行要求

- **真实 TTY**：miniclaw 是 TUI 应用，必须在图形终端（如 Xfce Terminal、iTerm2）中运行，不能在无头环境或管道中运行。
- **网络**：调用 LLM API 需要网络连接。

---

## 验证部署

在图形终端中执行：

```bash
export LLM_API_KEY="your-key"
miniclaw
```

发送一条消息（如「你好」），若能看到流式回复，说明部署成功。可进一步测试工具调用：「请读取 Cargo.toml 文件」。

---

## 数据目录

| 路径 | 用途 |
|------|------|
| `~/.miniclaw/config.toml` | 主配置 |
| `~/.miniclaw/sessions/` | 会话持久化 |
| `~/.miniclaw/usage.json` | 使用天数统计 |

---

## 故障排查

### 编译失败：找不到 openssl

安装系统依赖后重试。Linux 需 `libssl-dev` 或 `openssl-devel`，macOS 需 `brew install openssl`。

### 运行失败：Failed to initialize input reader

说明在非交互式环境（如 SSH 无 TTY、CI、管道）中运行。请在图形终端中启动。

### 聊天无响应

检查 `LLM_API_KEY` 是否已设置且有效，以及网络是否可达对应 API 端点。
