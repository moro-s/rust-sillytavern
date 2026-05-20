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

- [x] 角色管理模块（`character/manager.rs`）
- [x] 侧边栏角色列表（TUI 左侧面板）
- [x] 角色切换（Tab 键 / Shift+Tab）
- [x] 多角色对话历史隔离
- [x] `@角色名` 语法在消息中引用其他角色
- [x] 命令系统（`command/parser.rs`）
  - `/` 前缀：`/exit`, `/help`, `/clear`, `/switch <name>`
  - `?` 前缀：`?角色名` 查看角色信息, `?list` 列出所有角色
  - 命令解析器：前缀识别 → 参数提取 → 路由分发
- [x] 多角色批量导入（扫描 `characters/` 目录）

## Phase 4：Lorebook / 世界信息

- [x] Lorebook 数据模型（`lorebook/entry.rs`）
  - key, triggers[], content, priority, position
- [x] 触发匹配引擎（`lorebook/matcher.rs`）
  - 用户输入 + AI 回复中的关键词扫描
  - 按 priority 排序 + 去重
- [x] Lorebook 词条配置（`lorebooks/*.toml`）
- [x] 上下文窗口构建（`conversation/context.rs`）
  - system prompt + 激活的 lorebook 词条 + 对话历史
- [x] 触发词高亮提示（TUI 状态栏显示"已激活词条：魔龙传说、帝国骑士团"）
- [x] Lorebook 热加载（运行时修改词条文件自动生效）

## Phase 5：持久化（SQLite）

- [x] 数据库 schema（`db/schema.rs`）
  - sessions 表、messages 表
- [x] CRUD 操作（`db/store.rs`）
  - 对话保存/加载
  - 会话查询
- [x] 会话管理
  - `--new-session` 开始新对话
  - `--ls` 查看历史会话
  - `--resume <id>` 恢复历史会话
- [x] 自动保存（每 N 轮触发）

## Phase 5.5：全部迁移 SQLite（主数据源）

- [x] 12 表完整 schema（worlds, characters, locations, lore_entries, 3 中间表, sessions, messages, 3 状态表, user_persona）
- [x] full CRUD（`db/store.rs`）
- [x] character/manager.rs 改走 SQLite
- [x] lorebook 词条扫描改走 SQLite（LoreRow）
- [x] state 管理改走 SQLite（manage_state → character_states 表）
- [x] selector 选角界面改走 SQLite
- [x] CLI `--cl` `--wl` 改走 SQLite
- [x] 用户设定 `self_persona` 入 SQLite
- [x] 清理废弃文件（.state.md, lorebooks/*.toml .md, state.rs 旧代码）
- [x] `/export` 命令（SQLite → .md 导出）
- [x] 热加载迁移（SQLite 为主后简化）

## Phase 5.6：世界/地点/引导式创建

- [x] world support: 侧边栏世界列表, Ctrl+W 切换, 角色过滤
- [x] `/world <name>` 切换世界, `/link <角色> <世界>` 关联
- [x] `/location add <世界> <地点>` 创建地点
- [x] `/location list [世界]` 列出地点
- [x] `/cc` `/cw` 改为引导式多步创建
- [x] `manage_state` 通用接口 (character/world/location 三表路由)
- [x] `-w <world>` 启动参数生效
- [ ] 世界卡细节完善（world.md 导出, 世界级事件/法则）
- [ ] 地点状态交互式管理

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
