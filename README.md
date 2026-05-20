# rust-SillyTavern

AI 角色扮演酒馆 —— 终端里的沉浸式角色扮演体验。受 [SillyTavern](https://github.com/SillyTavern/SillyTavern) 启发，用 Rust 从头构建。

## 特性

- **角色卡系统**：Markdown + YAML 头信息定义角色（名字、性格、说话风格、背景知识）
- **终端 UI（TUI）**：ratatui 驱动的全屏交互界面，聊天面板 + 输入栏 + 着色
- **CLI 模式**：命令行快速对话，适合脚本和测试
- **流式输出**：SSE 逐 token 接收，TUI 打字机效果，CLI 即时打印
- **多角色支持**：侧边栏角色列表，Tab 切换，独立历史隔离
- **命令系统**：`/` 系统命令、`?` 查询命令、`@` 角色引用
- **Lorebook 世界信息**：关键词触发式记忆注入，热加载支持
- **世界系统**：世界管理、地点系统、角色-世界关联、世界状态
- **SQLite 全数据源**：12 表完整 schema，角色/世界/词条/状态/会话全部存库
- **引导式创建**：`/cc` `/cw` 多步提示，无需手动编辑文件
- **多后端**：支持任何 OpenAI 兼容 API（DeepSeek、OpenAI、Ollama 等）
- **多轮对话**：TUI 模式保留完整对话历史

> 📋 计划中：Phase 6 角色间互动、Phase 7 体验优化、Phase 8 高级功能。详见 [TODO.md](TODO.md)。

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
# TUI 交互模式（无参数 → 启动角色/世界选择器）
cargo run

# 指定角色进入 TUI
cargo run -- -c mage

# 指定角色 + 世界
cargo run -- -c innkeeper -w faerun

# CLI 单次对话
cargo run -- -m "来杯麦酒"
cargo run -- -c mage -m "教我火球术"

# 列出可用角色 / 世界
cargo run -- --cl
cargo run -- --wl
```

### TUI 快捷键

| 键 | 功能 |
|----|------|
| `Enter` | 发送消息 |
| `Tab` / `Shift+Tab` | 切换角色 |
| `Ctrl+W` | 循环切换世界 |
| `Ctrl+C` | 复制输入框内容 |
| `Ctrl+V` | 粘贴剪贴板内容 |
| `F1` | 显示/隐藏帮助 |
| `Esc` | 打断回复 / 回到底部 |
| `↑` / `↓` | 滚动聊天记录 |
| `PgUp` / `PgDn` | 快速滚动 |

### TUI 命令

| 命令 | 说明 |
|------|------|
| `/exit` `/quit` | 保存并退出 |
| `/help` | 显示帮助 |
| `/clear` | 清除当前角色对话 |
| `/save` `/load <id>` | 保存/加载会话 |
| `/cc <slug>` | 引导式创建角色 |
| `/cw <slug>` | 引导式创建世界 |
| `/self <设定>` | 设置用户本人角色设定 |
| `/state <action> <category> <key> [data]` | 管理角色状态 |
| `/switch <slug>` | 切换到指定角色 |
| `/world <slug>` | 切换到指定世界 |
| `/link <角色> <世界>` | 关联角色到世界 |
| `/location add <世界> <地点>` | 创建地点 |
| `/location list [世界]` | 列出地点 |
| `/export` | 导出全部到 .md 文件 |
| `?<名字>` | 查看角色信息 |
| `?list` | 列出所有角色 |
| `@<名字>` | 在消息中引用其他角色 |

### CLI 参数

| 参数 | 说明 |
|------|------|
| `-c` `--char` | 选择角色 |
| `-w` `--world` | 选择世界 |
| `-m` `--message` | CLI 单次对话 |
| `--cl` | 列出所有角色 |
| `--wl` | 列出所有世界 |
| `--ls` | 列出历史会话 |
| `--resume <id>` | 恢复指定会话 |
| `--new-session` | 开始全新会话 |

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
├── config.toml              # LLM 配置
├── data/                    # SQLite 数据库
├── characters/              # 导出的角色卡 (.md)
├── worlds/                  # 导出的世界卡 (.md)
├── lorebooks/               # 导出的词条 (.md)
└── src/
    ├── main.rs              # 入口（CLI + TUI）
    ├── config.rs            # 配置读取
    ├── character/
    │   ├── mod.rs           # 角色卡数据结构
    │   └── manager.rs       # 角色管理器（SQLite）
    ├── command/
    │   ├── mod.rs
    │   └── parser.rs        # / ? @ 命令解析
    ├── conversation/
    │   ├── mod.rs
    │   └── context.rs       # 上下文构建
    ├── db/
    │   ├── mod.rs
    │   ├── schema.rs        # 12 表 schema + 初始化
    │   └── store.rs         # CRUD 操作
    ├── lorebook/
    │   ├── mod.rs
    │   ├── entry.rs         # 词条模型
    │   └── matcher.rs       # 触发匹配
    ├── llm.rs               # LLM API 客户端 + SSE 流式
    └── tui/
        ├── mod.rs
        ├── app.rs           # App 状态 + 事件循环
        ├── selector.rs      # 启动选择器
        └── ui.rs            # 界面渲染
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
