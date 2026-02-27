# miniclaw

A minimal AI assistant tool inspired by [OpenClaw](https://github.com/openclaw/openclaw), built from scratch in Rust.

## What is it

miniclaw is a terminal-based AI assistant that can:

- Chat with LLMs (Anthropic Claude / OpenAI-compatible APIs including Qwen, DeepSeek, etc.)
- Autonomously call tools (read files, write files, etc.) via the Agent Loop pattern
- Be extended with new tools through a simple `Tool` trait
- Display token usage stats and a pet animation in pluggable header widgets

This is a learning project for understanding how AI agent systems work internally.

## Project Structure

```
src/
├── main.rs               # Entry point, initializes and launches TUI
├── config.rs             # Configuration management (TOML + env vars)
├── types.rs              # Core data types (Message, ToolCall, TokenUsage, etc.)
├── agent.rs              # Agent Loop (LLM <-> Tool orchestration) + SessionStats
├── llm/
│   ├── mod.rs            # LlmProvider trait
│   ├── anthropic.rs      # Anthropic Claude implementation
│   └── openai_compatible.rs  # OpenAI-compatible implementation
├── tools/
│   ├── mod.rs            # Tool trait + ToolRouter
│   ├── read_file.rs      # Read file tool
│   └── write_file.rs     # Write file tool
└── ui/
    ├── mod.rs            # HeaderWidget trait + WidgetContext
    └── ratatui_ui.rs     # Ratatui TUI (StatsWidget, PetWidget)
```

## Getting Started

### Quick Install (Recommended)

```bash
git clone https://github.com/Eonh1u/miniclaw.git && cd miniclaw
chmod +x scripts/install.sh && ./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
export LLM_API_KEY="your-key-here"
miniclaw
```

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for details.

### Manual Build

```bash
# Build
cargo build

# Set your API key (default provider: Qwen via DashScope)
export LLM_API_KEY="your-key-here"

# Run
cargo run
```

### Using other providers

```bash
# Anthropic Claude
export MINICLAW_PROVIDER="anthropic"
export MINICLAW_MODEL="claude-sonnet-4-20250514"
export LLM_API_KEY="your-anthropic-key"
cargo run

# DeepSeek
export MINICLAW_PROVIDER="openai_compatible"
export MINICLAW_MODEL="deepseek-chat"
export MINICLAW_API_BASE="https://api.deepseek.com/v1"
export LLM_API_KEY="your-deepseek-key"
cargo run
```

Or edit `~/.miniclaw/config.toml` directly (auto-generated on first run).

## TUI Commands

| Command | Description |
|---------|-------------|
| `/help`  | Show available commands |
| `/clear` | Clear conversation history |
| `/stats` | Toggle stats panel (token counts, usage days) |
| `/pet`   | Toggle pet animation panel |
| `/quit`  | Exit the program |
| `Ctrl+C` | Exit the program |

## Documentation

- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — Deployment and installation guide
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — Architecture design, component details, data flow
- [docs/ROADMAP.md](docs/ROADMAP.md) — Project roadmap, implementation status, TODO list

## License

MIT
