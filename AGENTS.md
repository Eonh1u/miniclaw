## Cursor Cloud specific instructions

### Overview

miniclaw is a single-binary Rust TUI application (terminal AI assistant). No databases, Docker, or background services needed. See `README.md` for build/run commands and `CLAUDE.md` for coding conventions.

### System dependency

`libssl-dev` and `pkg-config` are required to compile the `openssl-sys` crate (transitive dep of `reqwest`). The update script installs them automatically.

### Lint / Test / Build

Standard commands per CI (`.github/workflows/ci.yml`):

- **Format check:** `cargo fmt --check`
- **Clippy:** `cargo clippy --all-features`
- **Build:** `cargo build`
- **Test:** `cargo test` (31 unit tests, no external services needed)

### Running the application

The app is a TUI that requires a real TTY — running via piped shell (`cargo run` in a non-interactive shell) will fail with "Failed to initialize input reader". Use a graphical terminal (e.g. Xfce Terminal in the Desktop pane) to launch it.

Requires `LLM_API_KEY` env var (or `api_key` in `~/.miniclaw/config.toml`). Without a valid key the binary compiles and launches the TUI, but chat messages to the LLM will fail. For UI-only testing, any non-empty dummy value works: `LLM_API_KEY="dummy" cargo run`.

### Config

Auto-generated at `~/.miniclaw/config.toml` on first run. Default provider is Qwen (DashScope). Provider/model can also be overridden via env vars `MINICLAW_PROVIDER`, `MINICLAW_MODEL`, `MINICLAW_API_BASE`.

### Git commit conventions

- **Commit messages in English** — Use English for all commit messages.
- **Co-authored-by for AI assistance** — When an AI agent assists with the change, append `Co-authored-by: Cursor <cursoragent@cursor.com>` (or the appropriate agent identity) at the end of the commit message. Do not add "Made-with: Cursor" or similar.
- When merging feature branch commits to `main`, squash all related changes (code + docs + tests) for the same feature into a single commit. Keep `main` history clean with one commit per feature/fix. On the feature branch itself, multiple small commits are fine.

### Hello world verification

To verify the app works end-to-end, launch it in a graphical terminal with a valid `LLM_API_KEY`, send a chat message (e.g. "你好"), and then ask it to use a tool (e.g. "请读取 Cargo.toml 文件"). You should see streaming text output and tool call progress indicators (`⚡ 调用 xxx ...` / `✓ xxx 完成`).
