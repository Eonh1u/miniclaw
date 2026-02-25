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
├── docs/
│   ├── ARCHITECTURE.md       # 架构设计文档
│   └── ROADMAP.md            # 本文档：规划与实现状态
├── .cursor/rules/
│   └── project-guide.mdc     # AI 会话引导规则
└── src/
    ├── main.rs               # 入口，初始化并启动 TUI
    ├── config.rs             # 配置管理（TOML + 环境变量）
    ├── types.rs              # 核心数据类型（Message, ToolCall, TokenUsage 等）
    ├── agent.rs              # Agent Loop 核心循环 + SessionStats
    ├── llm/
    │   ├── mod.rs            # LlmProvider trait
    │   ├── anthropic.rs      # Anthropic Claude 实现
    │   └── openai_compatible.rs  # OpenAI 兼容 API 实现
    ├── tools/
    │   ├── mod.rs            # Tool trait + ToolRouter
    │   ├── read_file.rs      # 读文件工具
    │   └── write_file.rs     # 写文件工具（已实现，未注册）
    └── ui/
        ├── mod.rs            # HeaderWidget trait + WidgetContext
        └── ratatui_ui.rs     # Ratatui TUI 实现（StatsWidget, PetWidget）
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

### 阶段 4：Tool System 框架 ✅ 完成（部分工具未注册）

- [x] `Tool` trait 定义（name, description, parameters_schema, execute）
- [x] `ToolRouter` 工具注册/路由/分发
- [x] `read_file` 工具 —— **已注册**
- [x] `write_file` 工具 —— **已实现但未注册到 ToolRouter**
- [ ] `exec_command` 工具 —— 未实现
- [ ] `list_directory` 工具 —— 未实现

### 阶段 5：丰富工具集 🔲 未开始

- [ ] 将 `write_file` 注册到 `create_default_router()`
- [ ] 实现 `exec_command`（执行 shell 命令，需要安全确认机制）
- [ ] 实现 `list_directory`（列出目录内容）
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
- [ ] 流式输出（Streaming/SSE）—— `StreamChunk` 已定义但未接入
- [ ] TUI 中逐 token 流式渲染
- [ ] 对话历史持久化（退出后保留）
- [ ] 多行输入支持
- [ ] 上下文窗口管理（token 限制截断/摘要）

### 阶段 7：高级功能 🔲 未开始

- [ ] 错误处理完善（网络超时重试、优雅降级）
- [ ] 插件系统（外部工具动态加载）
- [ ] MCP（Model Context Protocol）支持
- [ ] 会话导出/导入

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

[agent]
max_iterations = 20
system_prompt = "You are a helpful AI assistant..."

[tools]
enabled = ["read_file", "write_file", "list_directory", "exec_command"]

[ui]
show_stats = true
show_pet = true
```

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
| 2026-02-25 | 删除传统 CLI，TUI-only；添加 token 统计和使用天数；引入 HeaderWidget 插件系统（StatsWidget + PetWidget）；添加 `/stats`、`/pet` 命令和 `[ui]` 配置段 |
| - | 初始版本：项目骨架、配置、Anthropic/OpenAI 兼容 LLM Client、Agent Loop、Tool System、read_file 工具、Ratatui TUI + Pet 动画 |
