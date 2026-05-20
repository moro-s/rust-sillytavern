# 日志系统设计方案

**日期**: 2026-05-20
**状态**: 待审核

---

## 动机

当前项目零可观测性，全部使用 `println!` / `eprintln!` 输出。TUI 模式下 `eprintln!` 会直接破坏 ratatui 终端渲染，且 LLM 调用失败、流中断、DB 错误等关键事件均无记录。

## 选型

经过 3 种方案的比较，选择 **`log` + `flexi_logger`**：

| 方案 | 依赖数 | 按天轮转 | 结论 |
|------|--------|----------|------|
| `log` + `simplelog` | 2 | 不支持 | 需自建轮转 |
| `log` + `flexi_logger` | 2 | 原生支持 | **采用** |
| `tracing` + `tracing-appender` | 5+ | 支持 | 过度工程 |

## 依赖

```toml
log = "0.4"
flexi_logger = "0.29"
```

## 架构

```
main.rs (入口)
  └─ logging::init()          ← 应用启动第一行
       ├─ 创建 data/logs/ 目录
       ├─ 配置 flexi_logger（按天轮转、保留 7 天）
       └─ 注册全局 logger

源代码各处
  └─ log::trace! / debug! / info! / warn! / error!
       └─ 写入 data/logs/tavern_<timestamp>.log
```

## 日志输出格式

```
[2026-05-20 10:30:45.123 INFO] [rust_sillytavern::llm] 正在连接 LLM: https://api.deepseek.com/v1
[2026-05-20 10:30:46.789 DEBUG] [rust_sillytavern::llm] 收到 token: "你好"
[2026-05-20 10:30:50.001 WARN] [rust_sillytavern::lorebook::entry] 无法解析 characters/broken.md: invalid YAML
```

格式模板：`[时间戳 级别] [模块路径] 消息`

## 日志级别

- **默认**: `info,rust_sillytavern=debug`（项目自身 debug 级别，第三方库 info 级别）
- **可通过环境变量覆盖**: `RUST_LOG=trace` / `RUST_LOG=warn` / `RUST_LOG=off`

## 文件管理

| 项目 | 设定 |
|------|------|
| 输出目录 | `data/logs/` |
| 文件名 | `tavern_r<YYYY-MM-DD>_<HH-MM-SS>.log` |
| 轮转策略 | 每天轮转一次 |
| 保留策略 | 最近 7 个文件（过期自动删除） |

## 现有输出迁移

| 原有方式 | 目标 | 示例 |
|---------|------|------|
| `eprintln!("Warning: ...")` | `log::warn!("...")` | Lorebook 解析警告 |
| `eprintln!("Failed to ...")` | `log::error!("...")` | DB 打开失败 |
| `println!`（CLI 列表输出） | **保留不变** | `--cl` `--wl` `--ls` 的用户可见输出 |
| `println!("[{}]", name)` | **保留不变** | CLI 模式角色扮演对话输出 |
| 无（新增） | `log::debug!("...")` | LLM 请求参数、token 计数 |
| 无（新增） | `log::trace!("...")` | 每个流式 token 内容、DB 查询耗时 |
| 无（新增） | `log::info!("...")` | LLM 回复完成、会话保存成功 |

原则：**用户可见的对话输出、列表结果保持 `println!`；诊断信息迁移到 `log::*!`。**

## 新增文件

```
src/
  logging.rs      ← 新增，仅导出 pub fn init()
```

## 修改文件

| 文件 | 改动 |
|------|------|
| `Cargo.toml` | 新增 `log` + `flexi_logger` 依赖 |
| `src/main.rs` | `mod logging;` + 开头调用 `logging::init()?;` |
| `src/db/schema.rs` | 无改动（错误通过 `rusqlite::Result` 传播，调用方处理） |
| `src/lorebook/entry.rs` | `eprintln!` → `log::warn!`（2 处） |

## 不变更

- CLI 模式对话输出（`println!`）保持不变，这些是用户期望看到的角色扮演内容
- `--cl` / `--wl` / `--ls` 的列表输出保持不变
- DB `open()` 内部错误通过 `Result` 传播，调用方决定如何处理

## 风险

- 无。`flexi_logger` 即使初始化失败也不会 panic（使用 `try_with_env_or_str`），日志目录创建失败仅静默跳过（`.ok()`）
