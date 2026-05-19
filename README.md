# rust-SillyTavern

AI 角色扮演酒馆 —— 终端里的沉浸式角色扮演体验。受 [SillyTavern](https://github.com/SillyTavern/SillyTavern) 启发，用 Rust 从头构建。

## 特性

- **角色卡系统**：Markdown + YAML 头信息定义角色（名字、性格、说话风格、背景知识）
- **多角色支持**：在多个角色间切换对话，剧情中角色可互相交流
- **Lorebook（世界信息）**：关键词触发式记忆注入，类似 SillyTavern World Info
- **终端 UI**：ratatui 驱动的全屏交互界面
- **持久化**：SQLite 存储对话历史和角色状态
- **多后端**：支持任何 OpenAI 兼容 API（OpenAI、Ollama、Moonshot 等）

## 快速开始

### 前置条件

- Rust 1.85+
- OpenAI 兼容的 API（或本地 Ollama）

### 配置

1. 复制 `config.toml` 并填入 API Key：

```toml
[llm]
base_url = "https://api.openai.com/v1"
api_key = "sk-your-key-here"
model = "gpt-4o-mini"
```

使用 Ollama 本地模型：
```toml
[llm]
base_url = "http://localhost:11434/v1"
api_key = "ollama"
model = "qwen2.5:7b"
```

### 运行

```bash
# 与默认角色（老酒保）对话
cargo run -- -m "来杯麦酒"

# 指定角色
cargo run -- -c mage -m "教我火球术"
```

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
├── lorebooks/            # Lorebook 词条 (.toml)
├── data/                 # SQLite 数据库
└── src/
    ├── main.rs           # CLI 入口
    ├── config.rs         # 配置读取
    ├── character.rs      # 角色卡解析 + system prompt 构建
    └── llm.rs            # LLM API 客户端
```

## 技术栈

| Crate | 用途 |
|-------|------|
| `ratatui` + `crossterm` | 终端 UI |
| `reqwest` + `tokio` | 异步 HTTP 请求 |
| `serde` + `serde_json` + `serde_yaml` | 序列化 |
| `toml` | 配置文件解析 |
| `clap` | CLI 参数解析 |
| `rusqlite` | SQLite 持久化 |

## 许可证

MIT
