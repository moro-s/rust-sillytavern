# rust-SillyTavern

AI 角色扮演酒馆 —— 终端里的沉浸式角色扮演体验。受 [SillyTavern](https://github.com/SillyTavern/SillyTavern) 启发，用 Rust 从头构建。

## 特性

- **角色卡系统**：Markdown + YAML 头信息定义角色（名字、性格、说话风格、背景知识）
- **终端 UI（TUI）**：ratatui 驱动的全屏交互界面，聊天面板 + 输入栏 + 着色
- **CLI 模式**：命令行快速对话，适合脚本和测试
- **流式输出**：SSE 逐 token 接收，TUI 打字机效果，CLI 即时打印
- **多后端**：支持任何 OpenAI 兼容 API（DeepSeek、OpenAI、Ollama 等）
- **多轮对话**：TUI 模式保留完整对话历史

> 📋 计划中：多角色切换 / `@角色名` 引用、`/` `?` 命令系统、Lorebook 世界信息、SQLite 持久化。详见 [TODO.md](TODO.md)。

## 快速开始

### 前置条件

- Rust 1.85+

### 配置

编辑 `config.toml`：

```toml
[llm]
# DeepSeek（推荐，中文效果好）
base_url = "https://api.deepseek.com/v1"
api_key = "sk-your-key-here"
model = "deepseek-chat"
stream = true   # 流式输出（逐字显示）

# 或 Ollama 本地模型
# base_url = "http://localhost:11434/v1"
# api_key = "ollama"
# model = "qwen2.5:7b"
# stream = true
```

### 运行

```bash
# TUI 交互模式（默认）
cargo run

# 指定角色进入 TUI
cargo run -- -c mage

# 指定世界（预留）
cargo run -- -w faerun -c innkeeper

# CLI 单次对话模式
cargo run -- -m "来杯麦酒"

# 列出可用角色 / 世界
cargo run -- --cl
cargo run -- --wl
```

### TUI 快捷键

| 键 | 功能 |
|----|------|
| `Enter` | 发送消息 |
| `Ctrl+C` | 复制输入框内容 |
| `Ctrl+V` | 粘贴剪贴板内容 |
| `/exit` `/quit` | 退出程序 |
| `F1` | 显示/隐藏帮助 |
| `↑` / `↓` | 滚动聊天记录 |
| `PgUp` / `PgDn` | 快速滚动 |
| `Esc` | 打断回复 / 跳转到最新消息 |
| `←` / `→` | 移动输入光标 |
| `Home` / `End` | 光标跳到行首/行尾 |

### 命令系统（计划）

| 前缀 | 用途 | 示例 |
|------|------|------|
| `@` | 在多角色对话中引用其他角色 | `@流浪剑客 你怎么看？` |
| `/` | 系统命令 | `/help` `/clear` `/save` `/load` `/quit` |
| `?` | 查询角色/系统信息 | `?流浪剑客` `?list` `?help` |

## 角色卡格式

角色卡放在 `characters/` 目录，使用 Markdown + YAML 头信息：

```markdown
---
name: 流浪剑客
personality: 豪爽直率，嫉恶如仇，但内心藏着一段不愿提起的往事
speech_style: 说话干脆利落，喜欢用"嘿"开头，偶尔冒出江湖黑话
first_message: "(推门进来，盔甲上还有未干的血迹) 嘿！老规矩，一杯烈的。"
---

# 背景
曾是帝国骑士团副团长，因拒绝执行屠村令被开除军籍...

# 外貌
身材魁梧，左脸有一道从眉骨延伸到下巴的刀疤...

# 我知道的事情
- 帝国骑士团的训练方式和暗号
- 北部边境有三处未被标记的古代遗迹
```

## 项目结构

```
rust-SillyTavern/
├── Cargo.toml
├── config.toml           # LLM 配置
├── characters/           # 角色卡 (.md)
└── src/
    ├── main.rs           # 入口（CLI + TUI 模式）
    ├── config.rs         # 配置读取
    ├── character.rs      # 角色卡解析 + system prompt 构建
    ├── llm.rs            # LLM API 客户端
    └── tui/
        ├── mod.rs
        ├── app.rs        # TUI 事件循环 & 状态管理
        └── ui.rs         # 界面渲染
```

## 技术栈

| Crate | 用途 |
|-------|------|
| `ratatui` + `crossterm` | 终端 UI |
| `reqwest` + `tokio` | 异步 HTTP 请求 |
| `serde` + `serde_json` + `serde_yaml` | 序列化 |
| `toml` | 配置文件解析 |
| `clap` | CLI 参数解析 |

## 许可证

MIT
