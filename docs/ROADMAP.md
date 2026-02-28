# miniclaw 项目规划与实现状态

> 本文档是项目的核心参考，记录总体规划、已实现功能和待办事项。
> **每次代码变更后，必须同步更新本文档。**

---

## 项目愿景

用 Rust 从零构建一个终端 AI 助手（类似 OpenClaw / Claude Code），核心能力：

- 通过 TUI 终端界面与用户交互
- 调用 LLM（Claude / OpenAI 兼容 API）进行推理
- LLM 自主调用工具（读写文件、执行命令等）完成任务
- 支持插件化的工具和 UI 组件扩展

---

## 项目结构（当前）

```
miniclaw/
├── Cargo.toml                # 依赖管理
├── CLAUDE.md                 # Claude Code 项目规则文件
├── docs/
│   ├── ARCHITECTURE.md       # 架构设计文档
│   ├── DEPLOYMENT.md         # 部署与安装指南
│   └── ROADMAP.md            # 本文档：规划与实现状态
├── scripts/
│   └── install.sh            # 安装脚本（构建 + 安装到 ~/.local/bin）
├── .cursor/rules/
│   └── project-guide.mdc     # AI 会话引导规则
└── src/
    ├── main.rs               # 入口，初始化并启动 TUI
    ├── config.rs             # 配置管理（TOML + 环境变量）
    ├── rules.rs              # CLAUDE.md 规则文件发现与加载
    ├── types.rs              # 核心数据类型（Message, ToolCall, TokenUsage 等）
    ├── agent.rs              # Agent Loop 核心循环 + SessionStats + Agent::create()
    ├── session.rs            # 会话持久化（保存/加载/导入/导出 JSON）
    ├── llm/
    │   ├── mod.rs            # LlmProvider trait
    │   ├── anthropic.rs      # Anthropic Claude 实现
    │   └── openai_compatible.rs  # OpenAI 兼容 API 实现
    ├── tools/
    │   ├── mod.rs            # Tool trait + ToolRouter
    │   ├── read_file.rs      # 读文件工具
    │   ├── write_file.rs     # 写文件工具（创建/覆盖）
    │   ├── edit.rs           # 编辑工具（精准文本替换）
    │   ├── bash.rs           # Bash 工具（执行 shell 命令）
    │   └── list_directory.rs # 列目录工具
    └── ui/
        ├── mod.rs            # HeaderWidget trait + WidgetContext
        ├── markdown.rs       # Markdown → ratatui 富文本转换
        └── ratatui_ui.rs     # Ratatui TUI 实现（多会话标签页, StatsWidget, PetWidget）
```

---

## 实施路线与状态

### 阶段 1：项目骨架 + 配置 + 类型 ✅ 完成

- [x] Cargo.toml 依赖配置
- [x] `AppConfig` TOML 配置管理（`~/.miniclaw/config.toml`）
- [x] 首次运行自动生成默认配置
- [x] 环境变量覆盖（`MINICLAW_PROVIDER`, `MINICLAW_MODEL`, `MINICLAW_API_BASE`）
- [x] `Message`, `ToolCall`, `ToolDefinition`, `ChatRequest`, `ChatResponse` 类型定义
- [x] `TokenUsage` token 使用量类型

### 阶段 2：LLM Client ✅ 完成

- [x] `LlmProvider` trait 抽象（`chat_completion` 方法）
- [x] Anthropic Claude Messages API 实现（含 tool calling 格式转换）
- [x] OpenAI 兼容 API 实现（支持 Qwen、DeepSeek、Moonshot、Ollama 等）
- [x] 从 API 响应中提取 token 使用量（`TokenUsage`）

### 阶段 3：Agent Loop ✅ 完成

- [x] Agent 核心循环（LLM → tool_call → 执行工具 → 反馈结果 → 重复）
- [x] 最大迭代次数限制
- [x] 对话历史管理（`Vec<Message>`）
- [x] 清空历史功能
- [x] `SessionStats` 累计统计（input/output tokens, request count）
- [x] `AgentEvent` 事件系统 + mpsc channel 实时推送工具调用进度

### 阶段 4：Tool System 框架 ✅ 完成

- [x] `Tool` trait 定义（name, description, parameters_schema, execute）
- [x] `ToolRouter` 工具注册/路由/分发
- [x] `read_file` 工具 —— 已注册，含单元测试
- [x] `write_file` 工具 —— 已注册，含单元测试
- [x] `list_directory` 工具 —— 已实现并注册（支持递归遍历、深度限制、文件大小显示），含单元测试
- [x] `ToolRouter` 单元测试（注册、路由、错误分发）
- [x] `bash` 工具 —— 执行 shell 命令，超时控制，输出截断，含单元测试
- [x] `edit` 工具 —— 精准文本替换（old_text 精确匹配），支持 replace_all，含单元测试

### 阶段 5：丰富工具集 🔶 进行中

- [x] 将 `write_file` 注册到 `create_default_router()`
- [x] 实现 `list_directory`（列出目录内容，支持递归/深度限制/大小显示）
- [x] 实现 `bash`（执行 shell 命令，超时控制，输出截断）
- [x] 实现 `edit`（精准文本替换，old_text 精确匹配，支持 replace_all）
- [ ] 实现 `web_search`（网页搜索）
- [ ] 工具权限/用户确认机制（危险操作前询问用户）
- [ ] 配置中 `tools.enabled` 列表实际生效（目前未过滤）

### 阶段 6：TUI 体验完善 🔶 进行中

- [x] Ratatui TUI 界面（分屏布局：header + 对话区 + 输入框）
- [x] 宠物动画系统（7 种状态：Idle/Typing/TypingFast/Thinking/Happy/Error/Sleeping）
- [x] 可滚动对话历史（Up/Down 键）
- [x] UTF-8/CJK 宽字符光标正确定位
- [x] 插件化 Header Widget 系统（`HeaderWidget` trait）
- [x] StatsWidget（token 计数、请求次数、使用天数）
- [x] PetWidget（宠物动画）
- [x] `/stats`、`/pet` 命令动态开关 widget
- [x] `[ui]` 配置段控制 widget 默认可见性
- [x] 使用天数持久化（`~/.miniclaw/usage.json`）
- [x] 斜杠命令自动补全（输入 `/` 即时弹出命令菜单，支持上下键选择、Enter 执行、Tab 补全、Esc 关闭）
- [x] Markdown 渲染（`pulldown-cmark` 解析，支持标题/粗体/斜体/代码/列表/分割线样式）
- [x] 工具调用实时进度显示（`⚡ 调用 xxx ...` / `✓ xxx 完成`，基于 AgentEvent + tokio::spawn 异步架构）
- [x] 流式输出（Streaming/SSE）—— `LlmProvider::chat_completion_stream` 方法 + SSE 解析
- [x] TUI 中逐 token 流式渲染（`StreamDelta` 事件 + `streaming_message_idx` 增量拼接）
- [x] 对话历史持久化（`/save`、`/load`、`/sessions` 命令，保存到 `~/.miniclaw/sessions/`）
- [x] 会话导入/导出（`/export <path>`、`/import <path>` 命令，JSON 格式）
- [x] 多会话标签页系统（`/new`、`/close`、`/rename` 命令，Ctrl+Left/Right 切换，鼠标点击切换）
- [x] 分屏同时展示多会话（左右等分列布局，活动会话青色边框，鼠标点击切换焦点）
- [x] 会话自动保存（每次用户输入/AI 输出/退出时自动持久化到 `~/.miniclaw/sessions/`）
- [x] 多行输入支持（Ctrl+J / Alt+Enter / Shift+Enter 换行，Enter 发送，输入框自动扩展）
- [x] 多行输入光标上下行移动（Up/Down 键）、鼠标点击定位光标
- [x] 待发送消息队列（处理中仍可输入，消息排队按序发送）
- [x] 每个会话独立输入框（切换会话保留各自的输入内容）
- [x] 对话滚动改进（PageUp/PageDown 快速翻页，鼠标滚轮，修复 scroll_offset 同步）
- [x] 多模型配置与会话内切换（`[[llm.models]]` 列表、`/model` 命令、方向键选择模型弹窗、`current_model_id` 持久化）
- [x] 按模型配置工具列表（`tools` 字段，空=全部；`enable_search` 支持 qwen3.5-plus 联网搜索）
- [x] 按模型配置 API Key（`api_key`、`api_key_env`），支持 Coding Plan 与按量计费混用
- [x] Provider 层级：`[llm.providers.xxx]` 统一 base_url、api_key_env、api；模型 `provider_id` 继承；id 格式 `provider_id/model_id`
- [ ] 上下文窗口管理（token 限制截断/摘要）

### 阶段 7：高级功能 🔶 进行中

- [x] CLAUDE.md 规则文件支持（多层级发现、自动注入 system prompt）
- [ ] 错误处理完善（网络超时重试、优雅降级）
- [ ] 插件系统（外部工具动态加载）
- [ ] MCP（Model Context Protocol）支持
- [x] 会话导出/导入（已在阶段 6 实现）

---

## 配置文件参考（`~/.miniclaw/config.toml`）

```toml
[llm]
provider = "openai_compatible"
model = "qwen-plus"
api_base = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = ""          # 或使用环境变量
api_key_env = "LLM_API_KEY"
max_tokens = 4096

# Provider 层级：每个 provider 有统一的 base_url、api_key_env、api 格式；模型通过 provider_id 继承
# 模型 id 格式：有 provider_id 时为 "provider_id/model_id"（如 dashscope/qwen3.5-plus）
# [llm.providers.dashscope]
# base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
# api_key_env = "LLM_API_KEY"
# api = "openai_compatible"
# [llm.providers.coding_plan]
# base_url = "https://coding.dashscope.aliyuncs.com/v1"
# api_key_env = "CODING_PLAN_API_KEY"
# api_key_env = "CODING_PLAN_API_KEY"
# api = "openai_compatible"
# [[llm.models]]
# provider_id = "dashscope"
# id = "qwen-plus"
# model = "qwen-plus"
# context_window = 131072
# max_tokens = 4096
# [[llm.models]]
# provider_id = "coding_plan"
# id = "qwen3.5-plus"
# model = "qwen3.5-plus"
# context_window = 1048576
# enable_search = true
# default_model = "dashscope/qwen3.5-plus"

[agent]
max_iterations = 20
system_prompt = "You are a helpful AI assistant..."

[tools]
enabled = ["read_file", "write_file", "list_directory", "exec_command"]

[ui]
show_stats = true
show_pet = true
```

### Provider 层级与 Coding Plan 示例

[阿里云 Coding Plan](https://help.aliyun.com/zh/model-studio/coding-plan-quickstart) 使用专属 API Key（`sk-sp-xxxxx`）和 Base URL。通过 Provider 层级，一个 provider 统一配置 base_url、api_key_env、api 格式，其下多个模型继承：

```toml
[llm]
default_model = "dashscope/qwen3.5-plus"

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key_env = "LLM_API_KEY"
api = "openai_compatible"

[llm.providers.coding_plan]
base_url = "https://coding.dashscope.aliyuncs.com/v1"
api_key_env = "CODING_PLAN_API_KEY"
api = "openai_compatible"

[[llm.models]]
provider_id = "dashscope"
id = "qwen-plus"
name = "Qwen Plus"
model = "qwen-plus"
context_window = 131072
max_tokens = 4096

[[llm.models]]
provider_id = "dashscope"
id = "qwen3.5-plus"
name = "Qwen 3.5 Plus"
model = "qwen3.5-plus"
context_window = 1048576
max_tokens = 8192
enable_search = true

[[llm.models]]
provider_id = "coding_plan"
id = "qwen3.5-plus"
name = "Qwen 3.5 Plus (Coding Plan)"
model = "qwen3.5-plus"
context_window = 1048576
max_tokens = 65536
enable_search = true

[[llm.models]]
provider_id = "coding_plan"
id = "qwen3-max-2026-01-23"
model = "qwen3-max-2026-01-23"
context_window = 262144
max_tokens = 65536

[[llm.models]]
provider_id = "coding_plan"
id = "qwen3-coder-next"
model = "qwen3-coder-next"
context_window = 262144
max_tokens = 65536

[[llm.models]]
provider_id = "coding_plan"
id = "qwen3-coder-plus"
model = "qwen3-coder-plus"
context_window = 1048576
max_tokens = 65536

[[llm.models]]
provider_id = "coding_plan"
id = "MiniMax-M2.5"
model = "MiniMax-M2.5"
context_window = 1048576
max_tokens = 65536

[[llm.models]]
provider_id = "coding_plan"
id = "glm-5"
model = "glm-5"
context_window = 202752
max_tokens = 16384

[[llm.models]]
provider_id = "coding_plan"
id = "glm-4.7"
model = "glm-4.7"
context_window = 202752
max_tokens = 16384

[[llm.models]]
provider_id = "coding_plan"
id = "kimi-k2.5"
model = "kimi-k2.5"
context_window = 262144
max_tokens = 32768

[agent]
max_iterations = 20
system_prompt = "You are a helpful AI assistant..."

[tools]
enabled = ["read_file", "write_file", "list_directory", "exec_command"]

[ui]
show_stats = true
show_pet = true
```

模型 id 格式：`provider_id/model_id`（如 `dashscope/qwen3.5-plus`、`coding_plan/kimi-k2.5`）。使用 Coding Plan 前：`export CODING_PLAN_API_KEY=sk-sp-xxxxx`

---

## 关键 trait 接口

### LlmProvider

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse>;
    fn name(&self) -> &str;
}
```

### Tool

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value) -> Result<String>;
}
```

### HeaderWidget

```rust
pub trait HeaderWidget {
    fn id(&self) -> &str;
    fn preferred_width(&self) -> Option<u16>;
    fn render(&self, f: &mut Frame, area: Rect, ctx: &WidgetContext);
}
```

---

## 更新日志

| 日期 | 变更 |
|------|------|
| 2026-02-28 | 中断功能：Ctrl+. 或 /stop 取消正在进行的 agent 任务，恢复会话状态 |
| 2026-02-28 | enable_search 强化：CRITICAL 原生搜索指令 + bash 描述禁止 curl；状态栏显示当前模型 |
| 2026-02-28 | 修复：终端过小时 set_cursor_position panic；/clear 后 Ctx 指标不更新 |
| 2026-02-28 | 文档：Git 提交规范（英文 commit message + AI 协助时 Co-authored-by） |
| 2026-02-28 | 多行输入框：Up/Down 光标上下行移动；鼠标点击定位光标 |
| 2026-02-27 | Provider 层级：ProviderConfig + RawModelEntry；[llm.providers.xxx] 统一配置；模型 provider_id 继承；id 格式 provider_id/model_id；添加 kimi-k2.5、glm-5、MiniMax-M2.5 等 Coding Plan 模型 |
| 2026-02-27 | 支持 Coding Plan：ModelEntry 新增 api_key、api_key_env；api_key_for_model 按模型解析；ROADMAP 添加 Coding Plan 配置示例 |
| 2026-02-27 | 修复 config.rs `get_model_entry` rustfmt 格式；开发规范新增「格式检查」：每次修改后执行 `cargo fmt --check` |
| 2026-02-27 | 按模型配置工具：`ModelEntry.tools`（空=全部工具）、`enable_search`（qwen3.5-plus 联网搜索）；ChatRequest 传递 enable_search；Agent 按模型过滤 tools |
| 2026-02-27 | 多模型支持：配置 `[[llm.models]]` 列表；`/model` 命令弹出方向键选择模型弹窗（与 /load 一致）；会话内切换；`current_model_id` 持久化 |
| 2026-02-27 | 新增部署支持：`scripts/install.sh` 安装脚本（自动安装 Rust、系统依赖、构建并安装到 ~/.local/bin）；`docs/DEPLOYMENT.md` 部署文档 |
| 2026-02-26 | 新增 `bash` 和 `edit` 工具：`bash` 执行 shell 命令（超时控制、输出截断）；`edit` 精准文本替换（old_text 精确匹配、支持 replace_all）；工具进度显示支持命令预览和文件路径；15 个新单元测试（共 50 个） |
| 2026-02-26 | 输入体验升级：多行输入（Ctrl+J/Alt+Enter 换行）；待发送消息队列（处理中可排队）；每个会话独立输入框；对话滚动改进（PageUp/Down、鼠标滚轮、scroll_offset 同步修复） |
| 2026-02-26 | 分屏展示 + 自动保存：多会话左右分屏同时展示（活动会话青色边框，鼠标点击切换焦点）；会话自动持久化（用户输入/AI 输出/退出时自动保存到 `~/.miniclaw/sessions/`，防止非正常退出丢失数据） |
| 2026-02-26 | 多会话标签页 + 对话持久化：新增 `src/session.rs` 模块（JSON 持久化）；重构 TUI 为 `SessionTab` 多会话架构；标签栏 UI（鼠标点击 + Ctrl+Left/Right 切换）；新增命令 `/new`、`/close`、`/rename`、`/save`、`/load`、`/sessions`、`/export`、`/import`；`Agent::create()` 工厂方法；4 个新单元测试（共 35 个） |
| 2026-02-26 | 工具调用进度优化：`AgentEvent::ToolStart/ToolEnd` 增加 `arguments` 字段；进度显示具体文件路径（如「⚡ 读取文件 src/main.rs ...」）；完成后原地覆盖替换进行中消息（非追加新行）；颜色区分：黄色=进行中、青色=完成、红色=失败 |
| 2026-02-26 | CI 修复：修正 `src/ui/ratatui_ui.rs` 格式问题使 `cargo fmt --check` 通过，GitHub Actions CI 全部步骤（fmt、clippy、build、test）执行成功 |
| 2026-02-25 | 流式输出（Streaming/SSE）：`LlmProvider` trait 新增 `chat_completion_stream` 方法（含默认非流式回退）；OpenAI 兼容 API 和 Anthropic API 分别实现 SSE 流式解析（文本 delta + 工具调用 delta 累加）；Agent 通过 `tokio::spawn` 转发 `StreamChunk` 为 `AgentEvent::StreamDelta`；TUI 新增 `streaming_message_idx` 跟踪实现逐 token 增量渲染 |
| 2026-02-25 | Markdown 渲染 + 工具调用进度显示：新增 `src/ui/markdown.rs` 模块（pulldown-cmark 解析）；引入 `AgentEvent` 枚举 + mpsc channel 实时推送工具调用事件；TUI 异步架构改造（tokio::spawn + Option&lt;Agent&gt;）；WidgetContext 解耦（stats 独立于 Agent）；新增 9 个 Markdown 单元测试 |
| 2026-02-25 | 为所有工具和 ToolRouter 添加单元测试（22 个测试用例）；添加 `tempfile` dev-dependency；在项目规则中新增「测试规范」章节 |
| 2026-02-25 | 注册 `write_file` 工具；新增 `list_directory` 工具（`src/tools/list_directory.rs`），支持递归遍历、可配置深度、文件大小显示、条目数截断 |
| 2026-02-25 | 新增斜杠命令自动补全：输入 `/` 即时弹出浮动命令菜单，支持模糊过滤、Up/Down 键导航、Enter 直接执行、Tab 补全、Esc 关闭；新增 `SlashCommand` 定义和 `SlashAutocomplete` 状态管理 |
| 2026-02-25 | 新增 CLAUDE.md 支持：添加 `src/rules.rs` 模块实现多层级规则文件发现与加载；Agent 初始化时自动将 CLAUDE.md 内容注入 system prompt；创建项目根目录 `CLAUDE.md` 文件 |
| 2026-02-25 | 删除传统 CLI，TUI-only；添加 token 统计和使用天数；引入 HeaderWidget 插件系统（StatsWidget + PetWidget）；添加 `/stats`、`/pet` 命令和 `[ui]` 配置段 |
| - | 初始版本：项目骨架、配置、Anthropic/OpenAI 兼容 LLM Client、Agent Loop、Tool System、read_file 工具、Ratatui TUI + Pet 动画 |
