# miniclaw

A minimal AI assistant tool inspired by [OpenClaw](https://github.com/openclaw/openclaw), built from scratch in Rust.

## What is it

miniclaw is a terminal-based AI assistant that can:

- Chat with LLMs (Anthropic Claude / OpenAI)
- Autonomously call tools (read files, execute commands, etc.) via the Agent Loop pattern
- Be extended with new tools through a simple trait interface

This is a learning project for understanding how AI agent systems work internally.

## Project Structure

```
src/
├── main.rs           # Entry point
├── cli.rs            # Terminal chat interface (REPL)
├── config.rs         # Configuration management (TOML + env vars)
├── types.rs          # Core data types (Message, ToolCall, etc.)
├── agent.rs          # Agent Loop (LLM <-> Tool orchestration)
├── llm/
│   ├── mod.rs        # LlmProvider trait
│   └── anthropic.rs  # Anthropic Claude implementation
└── tools/
    ├── mod.rs        # Tool trait + ToolRouter
    └── read_file.rs  # Read file tool
```

## Getting Started

```bash
# Build
cargo build

# Set your API key
export ANTHROPIC_API_KEY="your-key-here"

# Run
cargo run
```

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full architecture documentation, component details, and implementation roadmap.

## License

MIT
