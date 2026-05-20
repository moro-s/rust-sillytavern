# AGENTS.md

## 项目概述

单二进制 Rust 包（`rust-sillytavern`）—— 一个 AI 角色扮演酒馆 TUI 应用。SQLite 为数据主源；文件系统（`characters/*.md`）为辅。异步运行时为 `tokio`（full features）。

## 常用命令

```bash
# 编译检查（快，不输出二进制）
cargo check

# 启动 TUI（无参时弹出角色/世界选择器）
cargo run

# CLI 单轮对话
cargo run -- -c <slug> -m "你的消息"

# 列出角色 / 世界 / 会话
cargo run -- --cl
cargo run -- --wl
cargo run -- --ls
```

项目无 CI、无测试套件、无 lint/格式化配置（仅 Cargo 默认）。

## 关键文件

| 路径 | 用途 |
|------|------|
| `src/main.rs` | 入口 —— CLI 参数解析（clap）+ 分发 TUI 或 CLI 模式 |
| `src/tui/app.rs` | TUI 事件循环、App 状态、键盘处理 |
| `src/tui/ui.rs` | ratatui 渲染 |
| `src/config.rs` | 读取 `config.toml`（LLM 连接信息） |
| `src/llm.rs` | OpenAI 兼容 `/v1/chat/completions`，支持流式与非流式 |
| `src/db/schema.rs` | SQLite 初始化（12 表）、种子数据、`open()` |
| `src/db/store.rs` | 全部实体的 CRUD |
| `src/character/mod.rs` | 角色卡解析（Markdown + YAML 前置信息） |
| `src/conversation/context.rs` | 系统提示词 + 知识书注入 + 对话历史组装 |
| `config.toml` | **用户需自行创建**（已 gitignore），LLM 地址/密钥/模型 |

## 常见陷阱

- **`config.toml` 已被 gitignore。** 每次 clone 后必须手动创建。必填字段：`[llm]` → `base_url`、`api_key`、`model`、`stream`。
- **SQLite 数据库位于 `data/tavern.db`**，首次运行时自动创建。`data/` 目录已被 gitignore。
- **reqwest 使用 `rustls-tls`**（非 `native-tls`）。无需 OpenSSL 依赖。
- **错误处理全局使用 `anyhow::Result`** —— 无自定义错误类型。
- **流式输出使用 `tokio::sync::watch` 实现取消** —— `chat_stream()` 返回 `mpsc::UnboundedReceiver<StreamEvent>`。
- **角色卡为 Markdown+YAML 前置信息**，存放于 `characters/*.md`。YAML 位于 `---` 标记之间；正文在第二个 `---` 之后。
- **SQLite 是唯一数据源。** `character::load()` 从文件系统读取，但 `character::manager` 和 TUI 选择器查询的是数据库。`/cc` 命令会同时写入数据库和文件。
- **项目中不存在任何测试。** 测试基础设施需从零搭建。
- **ratatui 0.29 + crossterm 0.28** —— 升级时注意与 ratatui 0.28.x 的 API 差异。
- **Clap 使用 derive 宏**，通过 `#[command]` 和 `#[arg]` 属性。

## 中文本地化要求

- **所有对话内容必须使用中文**：角色设定、开场白、AI 回复、用户提示等全部为中文。
- **代码注释必须使用中文**：所有 `//` 和 `///` 注释使用中文撰写。
- **UI 文本必须使用中文**：TUI 界面的标签、提示、状态栏、帮助文本等全部中文。
- **错误消息必须使用中文**：`anyhow::bail!`、`.with_context()`、`eprintln!` 等输出的错误信息使用中文。
- **角色卡内容必须使用中文**：`characters/*.md` 中的 YAML 头信息正文和 Markdown 正文均为中文。

## 架构

```
CLI 参数 (clap) → main.rs
  ├─ CLI 模式: config → character load → llm::chat / llm::chat_stream → 打印
  └─ TUI 模式: tui::app::run()
       ├─ db::schema::open()（不存在则自动创建）
       ├─ tui::selector（角色/世界选择器）
       └─ 事件循环: 输入 → llm::chat_stream → 界面渲染
```

多角色流程通过 `conversation::context` 组装 `系统提示词 + 知识书词条 + 对话历史`，然后发送给 LLM。

## 编码约定

- 模块结构：`src/` 下平铺，子目录为 `character/`、`command/`、`conversation/`、`db/`、`lorebook/`、`tui/`
- 各子目录通过 `mod.rs` 重新导出公开类型
- 函数返回 `anyhow::Result<T>`
- UI 文本、提示、错误消息全部使用中文
- 不使用 `pub use` 重导出 —— 调用方通过模块路径访问
