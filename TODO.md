# rust-SillyTavern 开发路线图

## Phase 1：打通 LLM（当前阶段）

- [x] 项目初始化（Cargo）
- [x] 配置文件 `config.toml`（LLM 连接信息）
- [x] 角色卡格式（MD + YAML 头信息）
- [x] 角色卡解析（`character.rs`）→ 构建 system prompt
- [x] LLM 客户端（`llm.rs`）→ OpenAI 兼容 `/v1/chat/completions`
- [x] CLI 入口（`clap`）→ 命令行指定角色和消息
- [x] 示例角色卡 `characters/innkeeper.md`
- [x] cargo check 编译通过
- [x] 实际调用 LLM 测试对话（DeepSeek v4 pro）

## Phase 2：基础终端 UI（ratatui）

- [x] ratatui 项目脚手架（`tui/app.rs`）
- [x] 聊天面板（历史消息滚动显示）
- [x] 输入栏（底部状态栏 + 输入框）
- [x] 角色名 + 消息的格式化着色
- [x] 键盘快捷键（Ctrl+C 退出, Enter 发送, F1 帮助）
- [x] 流式输出（逐字打字效果）

## Phase 3：多角色支持

- [ ] 角色管理模块（`character/manager.rs`）
- [ ] 侧边栏角色列表（TUI 左侧面板）
- [ ] 角色切换（Tab 键 / 点击切换）
- [ ] 多角色对话历史隔离
- [ ] `@角色名` 语法在消息中引用其他角色
- [ ] 命令系统（`command/parser.rs`）
  - `/` 前缀：系统命令（`/help`, `/clear`, `/save`, `/load`, `/quit`）
  - `?` 前缀：查询命令（`?角色名` 查看角色信息, `?list` 列出所有角色, `?help`）
  - 命令解析器：前缀识别 → 参数提取 → 路由分发
- [ ] 多角色批量导入（扫描 `characters/` 目录）

## Phase 4：Lorebook / 世界信息

- [ ] Lorebook 数据模型（`lorebook/entry.rs`）
  - key, triggers[], content, priority, position
- [ ] 触发匹配引擎（`lorebook/matcher.rs`）
  - 用户输入 + AI 回复中的关键词扫描
  - 按 priority 排序 + 去重
- [ ] Lorebook 词条配置（`lorebooks/*.toml`）
- [ ] 上下文窗口构建（`conversation/context.rs`）
  - system prompt + 激活的 lorebook 词条 + 对话历史
- [ ] 触发词高亮提示（TUI 中显示"已激活词条：魔龙传说、帝国..."
- [ ] Lorebook 热加载（运行时修改词条文件自动生效）

## Phase 5：持久化（SQLite）

- [ ] 数据库 schema（`db/schema.rs`）
  - conversations 表、messages 表
  - characters 表（角色状态快照）
  - lorebook_entries 表
- [ ] CRUD 操作（`db/store.rs`）
  - 对话保存/加载
  - 角色状态读写
- [ ] 会话管理
  - `--new-session` 开始新对话
  - `--list-sessions` 查看历史会话
  - `--resume <id>` 恢复历史会话
- [ ] 自动保存（每 N 轮触发）

## Phase 6：角色间互动

- [ ] 剧情事件触发（用户输入 `!event 门外传来马蹄声`）
- [ ] 全局场景上下文（scene description 注入所有角色的 system prompt）
- [ ] NPC 自动对话（两个 NPC 根据设定互相搭话）
- [ ] 指令系统（`!narrate` / `!event` / `!inject`）

## Phase 7：流式输出与体验优化

- [ ] SSE 流式接收（LLM 逐 token 返回）
- [ ] 打字机效果（TUI 中逐字显示）
- [ ] 用户可中断生成（Ctrl+C 停止当前回复）
- [ ] 回复重试（`--retry` / 快捷键重新生成）
- [ ] Markdown 渲染增强（代码块、引用、分隔线）

## Phase 8：高级功能

- [ ] 多模型路由（不同角色走不同 LLM）
- [ ] Token 计数与上下文窗口管理
- [ ] 对话摘要（长对话自动压缩早期内容）
- [ ] 角色卡导入器（SillyTavern JSON → MD 转换）
- [ ] TTS 语音合成（可选）
- [ ] 图片生成（角色立绘，可选）

## 近期优先（Phase 1 剩余）

1. [x] 确认 Rust 1.95 更新完成
2. [x] `cargo check` 编译通过
3. [x] 填入真实 API Key，测试对话链路
4. [x] 验证中文角色扮演效果
5. [x] 验证错误处理（API Key 错误、网络超时等）
