# miniclaw 项目引导

## 首要任务

开始任何工作前，**必须按顺序执行**：

1. **`git pull`** — 本项目会在多台机器上开发，动手改代码前必须先拉取远程最新代码，避免冲突。
2. 阅读 `docs/ROADMAP.md` — 项目总体规划、已实现功能清单、待办事项
3. 阅读 `docs/ARCHITECTURE.md` — 架构设计、组件详解、数据流

这三步是你理解项目的基础。不要跳过。

## 项目概述

miniclaw 是一个用 Rust 构建的终端 AI 助手，核心组件：
- **TUI**：基于 ratatui 的终端界面，header 区域使用 `HeaderWidget` 插件系统
- **Agent Loop**：LLM <-> Tool 多轮循环
- **LLM Client**：支持 Anthropic 和 OpenAI 兼容 API
- **Tool System**：通过 `Tool` trait 扩展工具

## 代码变更后的文档更新

**每次完成代码变更后，必须同步更新 `docs/ROADMAP.md`**：
- 更新对应功能的完成状态（checkbox）
- 如果新增了功能/模块，在相应阶段添加条目
- 更新「项目结构」部分（如果文件结构变化）
- 在「更新日志」表格中添加新行

如果架构发生重大变化（新增组件、trait 改动等），也要更新 `docs/ARCHITECTURE.md`。

## 编码规范

- 语言：Rust 2021 edition
- 异步运行时：Tokio
- 错误处理：anyhow（应用层） + thiserror（库层）
- 新工具：实现 `Tool` trait 并在 `create_default_router()` 中注册
- 新 UI 组件：实现 `HeaderWidget` trait，在 `RatatuiUi::new()` 中按配置注册
- 配置文件：`~/.miniclaw/config.toml`（TOML 格式）
- 始终用中文与用户交流

## 测试规范

- **编译后必须测试**：每次 `cargo build` 成功后，必须执行 `cargo test` 确认所有测试通过
- **新功能必须有测试**：每个新实现的 Tool、核心模块都必须在同文件内添加 `#[cfg(test)] mod tests` 单元测试
- **测试要求**：
  - 覆盖正常路径和错误路径（缺失参数、无效输入、不存在的文件/目录等）
  - 工具测试使用 `tempfile` crate 创建临时文件/目录，避免污染文件系统
  - 异步测试使用 `tokio::runtime::Runtime::new().unwrap()` + `block_on`
  - 测试元信息：验证 `name()`、`description()`、`parameters_schema()` 的正确性
- **验证流程**：`cargo build 2>&1 && cargo test 2>&1`，两步都成功才算完成
