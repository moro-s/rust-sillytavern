# 任务与关系图实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 SQLite schema 上新增角色关系（好感度）、地点嵌套（parent_id）、任务系统（quests + 参与者）三大能力。

**Architecture:** 延续现有 junction 表 + fields 风格。3 张新表（character_relations、quests、quest_characters）+ 1 个 `locations.parent_id` 列。`manage_state` 工具扩展 `relation`/`quest` category 路由。5 个新 TUI 命令。`sys_skill/quest_patterns.md` 教 LLM 操作任务关系。

**Tech Stack:** rusqlite（已有）、serde_json（已有）、ratatui（已有）

---

### Task 1: Schema 变更（3 张新表 + locations 扩展）

**Files:**
- Modify: `src/db/schema.rs:14-153`

- [ ] **Step 1: 在 init() 中添加 3 张新表的 CREATE TABLE + ALTER TABLE**

在 `src/db/schema.rs` 的 `init()` 函数中的 `conn.execute_batch(...)` 调用内，在 `CREATE INDEX IF NOT EXISTS idx_timeline_world` 之前插入以下 SQL：

```sql
CREATE TABLE IF NOT EXISTS character_relations (
    from_char_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    to_char_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    rel_type     TEXT NOT NULL DEFAULT 'neutral',
    affinity     INTEGER NOT NULL DEFAULT 0,
    note         TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (from_char_id, to_char_id, rel_type)
);

CREATE TABLE IF NOT EXISTS quests (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'active',
    world_id    INTEGER REFERENCES worlds(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS quest_characters (
    quest_id     INTEGER NOT NULL REFERENCES quests(id) ON DELETE CASCADE,
    character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    role         TEXT NOT NULL DEFAULT 'member',
    task         TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (quest_id, character_id)
);
```

- [ ] **Step 2: 在 `init()` 最末尾（`conn.execute_batch` 闭合前）添加 ALTER TABLE**

`ALTER TABLE` 不能放在 `execute_batch` 里和 CREATE TABLE 混用因为 SQLite 限制，需要在 `init()` 中单独 `conn.execute()`。

在 `init()` 函数中，`conn.execute_batch(...)?;` 之后、`Ok(())` 之前添加：

```rust
// 向后兼容：为旧数据库添加 parent_id 列
let has_parent_id: bool = conn
    .prepare("SELECT parent_id FROM locations LIMIT 0")
    .is_ok();
if !has_parent_id {
    conn.execute("ALTER TABLE locations ADD COLUMN parent_id INTEGER REFERENCES locations(id) ON DELETE SET NULL", [])?;
}
```

- [ ] **Step 3: cargo check**

```bash
cargo check
```

Expected: 仅已有项目的 warning，无新 error。

- [ ] **Step 4: Commit**

```bash
git add src/db/schema.rs
git commit -m "feat: add character_relations, quests, quest_characters tables + locations.parent_id"
```

---

### Task 2: Store 层 — LocationRow 加 parent_id + 父子查询

**Files:**
- Modify: `src/db/store.rs:128-149`

- [ ] **Step 1: LocationRow 加 parent_id 字段**

```rust
#[derive(Debug, Clone)]
pub struct LocationRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub connects_to: String,
    pub parent_id: Option<i64>,
    pub world_id: i64,
}
```

- [ ] **Step 2: 更新 list_locations 查询**

```rust
pub fn list_locations(conn: &Connection, world_id: i64) -> anyhow::Result<Vec<LocationRow>> {
    let mut stmt = conn.prepare("SELECT id, slug, name, description, connects_to, COALESCE(parent_id,0), world_id FROM locations WHERE world_id=?1 ORDER BY slug")?;
    let rows = stmt.query_map(params![world_id], |row| {
        Ok(LocationRow { id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, description: row.get(3)?, connects_to: row.get(4)?, parent_id: row.get::<_, i64>(5).ok().filter(|&v| v > 0), world_id: row.get(6)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}
```

- [ ] **Step 3: 添加 get_location_children 函数**

在 `create_location` 之后添加：

```rust
/// 查询某个地点的所有子地点（直接子节点）
pub fn get_location_children(conn: &Connection, parent_id: i64) -> anyhow::Result<Vec<LocationRow>> {
    let mut stmt = conn.prepare("SELECT id, slug, name, description, connects_to, COALESCE(parent_id,0), world_id FROM locations WHERE parent_id=?1 ORDER BY slug")?;
    let rows = stmt.query_map(params![parent_id], |row| {
        Ok(LocationRow { id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, description: row.get(3)?, connects_to: row.get(4)?, parent_id: row.get::<_, i64>(5).ok().filter(|&v| v > 0), world_id: row.get(6)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}
```

- [ ] **Step 4: cargo check**

```bash
cargo check
```

Expected: 可能有 LocationRow 构造处 `parent_id` 缺失的 error（handle_location 中 line 436），将在后续 task 修复。

- [ ] **Step 5: Commit**

```bash
git add src/db/store.rs
git commit -m "feat: add parent_id to LocationRow + get_location_children"
```

---

### Task 3: Store 层 — character_relations CRUD

**Files:**
- Modify: `src/db/store.rs`（在 Location 段后、Lore 段前插入）

- [ ] **Step 1: 添加 CharacterRelation 类型**

```rust
// ──────────────────────────────────────────────
// Character Relations
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CharacterRelation {
    pub from_char_id: i64,
    pub to_char_id: i64,
    pub to_char_name: String,
    pub to_char_slug: String,
    pub rel_type: String,
    pub affinity: i32,
    pub note: String,
}
```

- [ ] **Step 2: 添加 CRUD 函数**

```rust
/// 列出某角色的所有关系（含对方名字）
pub fn list_relations(conn: &Connection, char_id: i64) -> anyhow::Result<Vec<CharacterRelation>> {
    let mut stmt = conn.prepare(
        "SELECT r.from_char_id, r.to_char_id, c.name, c.slug, r.rel_type, r.affinity, r.note
         FROM character_relations r
         JOIN characters c ON c.id = r.to_char_id
         WHERE r.from_char_id = ?1
         ORDER BY r.rel_type, c.name"
    )?;
    let rows = stmt.query_map(params![char_id], |row| {
        Ok(CharacterRelation {
            from_char_id: row.get(0)?, to_char_id: row.get(1)?,
            to_char_name: row.get(2)?, to_char_slug: row.get(3)?,
            rel_type: row.get(4)?, affinity: row.get(5)?, note: row.get(6)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

/// 设置或更新关系（INSERT OR REPLACE）
pub fn set_relation(conn: &Connection, from_char_id: i64, to_char_slug: &str, rel_type: &str, affinity: i32, note: &str) -> anyhow::Result<String> {
    let to_id: i64 = conn.query_row(
        "SELECT id FROM characters WHERE slug=?1", params![to_char_slug], |row| row.get(0)
    ).map_err(|_| anyhow::anyhow!("角色 '{}' 不存在", to_char_slug))?;

    let existing: Option<i64> = conn.query_row(
        "SELECT affinity FROM character_relations WHERE from_char_id=?1 AND to_char_id=?2 AND rel_type=?3",
        params![from_char_id, to_id, rel_type], |row| row.get(0)
    ).ok();

    if let Some(_) = existing {
        conn.execute(
            "UPDATE character_relations SET affinity=?1, note=?2 WHERE from_char_id=?3 AND to_char_id=?4 AND rel_type=?5",
            params![affinity, note, from_char_id, to_id, rel_type],
        )?;
    } else {
        conn.execute(
            "INSERT INTO character_relations (from_char_id, to_char_id, rel_type, affinity, note) VALUES (?1,?2,?3,?4,?5)",
            params![from_char_id, to_id, rel_type, affinity, note],
        )?;
    }
    Ok(format!("与 {} 的关系已更新: {} (好感度: {})", to_char_slug, rel_type, affinity))
}

/// 删除关系
pub fn delete_relation(conn: &Connection, from_char_id: i64, to_char_slug: &str, rel_type: &str) -> anyhow::Result<String> {
    let to_id: i64 = conn.query_row(
        "SELECT id FROM characters WHERE slug=?1", params![to_char_slug], |row| row.get(0)
    ).map_err(|_| anyhow::anyhow!("角色 '{}' 不存在", to_char_slug))?;

    conn.execute(
        "DELETE FROM character_relations WHERE from_char_id=?1 AND to_char_id=?2 AND rel_type=?3",
        params![from_char_id, to_id, rel_type],
    )?;
    Ok(format!("与 {} 的关系 '{}' 已删除", to_char_slug, rel_type))
}
```

- [ ] **Step 3: cargo check**

```bash
cargo check
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/db/store.rs
git commit -m "feat: character_relations CRUD (list/set/delete)"
```

---

### Task 4: Store 层 — quests + quest_characters CRUD

**Files:**
- Modify: `src/db/store.rs`（在 character_relations 段后插入）

- [ ] **Step 1: 添加 QuestRow + QuestCharacter 类型**

```rust
// ──────────────────────────────────────────────
// Quests
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QuestRow {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub world_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct QuestCharacter {
    pub quest_id: i64,
    pub character_id: i64,
    pub char_slug: String,
    pub char_name: String,
    pub role: String,
    pub task: String,
}
```

- [ ] **Step 2: 添加 CRUD 函数**

```rust
/// 创建任务，返回任务 ID
pub fn create_quest(conn: &Connection, title: &str, description: &str, world_id: Option<i64>) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO quests (title, description, world_id) VALUES (?1, ?2, ?3)",
        params![title, description, world_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 列出某世界的所有任务
pub fn list_quests(conn: &Connection, world_id: i64) -> anyhow::Result<Vec<QuestRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, status, world_id FROM quests WHERE world_id=?1 ORDER BY status, created_at"
    )?;
    let rows = stmt.query_map(params![world_id], |row| {
        Ok(QuestRow {
            id: row.get(0)?, title: row.get(1)?, description: row.get(2)?,
            status: row.get(3)?, world_id: row.get(4)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

/// 更新任务状态
pub fn update_quest_status(conn: &Connection, quest_id: i64, status: &str) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE quests SET status=?1, updated_at=datetime('now') WHERE id=?2",
        params![status, quest_id],
    )?;
    Ok(())
}

/// 为任务添加参与者
pub fn add_quest_character(conn: &Connection, quest_id: i64, char_slug: &str, role: &str, task: &str) -> anyhow::Result<String> {
    let char_id: i64 = conn.query_row(
        "SELECT id FROM characters WHERE slug=?1", params![char_slug], |row| row.get(0)
    ).map_err(|_| anyhow::anyhow!("角色 '{}' 不存在", char_slug))?;

    conn.execute(
        "INSERT OR REPLACE INTO quest_characters (quest_id, character_id, role, task) VALUES (?1,?2,?3,?4)",
        params![quest_id, char_id, role, task],
    )?;
    Ok(format!("角色 '{}' 已加入任务 (角色: {}, 分工: {})", char_slug, role, task))
}

/// 列出某任务的参与者
pub fn list_quest_characters(conn: &Connection, quest_id: i64) -> anyhow::Result<Vec<QuestCharacter>> {
    let mut stmt = conn.prepare(
        "SELECT qc.quest_id, qc.character_id, c.slug, c.name, qc.role, qc.task
         FROM quest_characters qc JOIN characters c ON c.id = qc.character_id
         WHERE qc.quest_id = ?1"
    )?;
    let rows = stmt.query_map(params![quest_id], |row| {
        Ok(QuestCharacter {
            quest_id: row.get(0)?, character_id: row.get(1)?,
            char_slug: row.get(2)?, char_name: row.get(3)?,
            role: row.get(4)?, task: row.get(5)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}
```

- [ ] **Step 3: cargo check**

```bash
cargo check
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/db/store.rs
git commit -m "feat: quests + quest_characters CRUD"
```

---

### Task 5: manage_state 工具描述扩展

**Files:**
- Modify: `src/llm.rs:186-203`

- [ ] **Step 1: 更新 manage_state_tool() 的 category enum 和 description**

在 `src/llm.rs` 中，将 `manage_state_tool()` 函数的 `category` enum 从：
```rust
"enum": ["item", "event", "skill", "status", "rule"]
```
改为：
```rust
"enum": ["item", "event", "skill", "status", "rule", "relation", "quest"]
```

同时更新 `description`（line 191）：
```rust
description: "管理角色/世界/地点状态、角色关系、任务。增删改查物品、事件、技能、状态、法则、人际关系、任务。\n\
  用于: 记录新物品、更新角色状态、查询世界信息、建立/更新人际关系、创建/管理任务等。".into(),
```

- [ ] **Step 2: cargo check**

```bash
cargo check
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/llm.rs
git commit -m "feat: extend manage_state tool with relation and quest categories"
```

---

### Task 6: LLM 工具执行器 — 路由 relation/quest category

**Files:**
- Modify: `src/tui/app.rs:820-835`

- [ ] **Step 1: 在 tool executor 中增加 relation/quest 路由**

替换 `chat_with_tools` 回调中的 `if tool_name == "manage_state"` 代码块（line 821-835）。在当前 `manage_state` 处理逻辑中，根据 `category` 分流：

```
当前代码 (line 821-835):
    if tool_name == "manage_state" {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_json) {
            let action = ...
            let category = ...
            // 总是调 db::store::manage_state(&db, "character_states", ...)
        }
    }
```

改为：

```rust
if tool_name == "manage_state" {
    if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_json) {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("get");
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("item");
        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let data = args.get("data").map(|v| v.to_string()).unwrap_or_default();
        let tl_id = db::store::current_timeline(&db, 1).ok().flatten().map(|t| t.id);

        match category {
            "relation" => {
                // data 中应包含: to_char_slug, rel_type, affinity, note
                if let Ok(d) = serde_json::from_str::<serde_json::Value>(&data) {
                    let to_slug = d.get("to_char_slug").and_then(|v| v.as_str()).unwrap_or("");
                    let rel = d.get("rel_type").and_then(|v| v.as_str()).unwrap_or("neutral");
                    let aff: i32 = d.get("affinity").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let note = d.get("note").and_then(|v| v.as_str()).unwrap_or("");
                    if !to_slug.is_empty() {
                        match action {
                            "add" | "update" => match db::store::set_relation(&db, char_id, to_slug, rel, aff, note) {
                                Ok(r) => r,
                                Err(e) => format!("Error: {}", e),
                            },
                            "delete" => match db::store::delete_relation(&db, char_id, to_slug, rel) {
                                Ok(r) => r,
                                Err(e) => format!("Error: {}", e),
                            },
                            _ => match db::store::list_relations(&db, char_id) {
                                Ok(rels) => {
                                    if rels.is_empty() { "暂无关系记录".to_string() }
                                    else { rels.iter().map(|r| format!("- {} ({} / {} 好感: {})", r.to_char_name, r.rel_type, if r.affinity > 0 {format!("+{}",r.affinity)} else {format!("{}",r.affinity)}, if r.note.is_empty() {""} else {&r.note})).collect::<Vec<_>>().join("\n") }
                                },
                                Err(e) => format!("Error: {}", e),
                            },
                        }
                    } else { "缺少 to_char_slug 参数".to_string() }
                } else { "数据格式错误".to_string() }
            }
            "quest" => {
                if let Ok(d) = serde_json::from_str::<serde_json::Value>(&data) {
                    let title = d.get("title").and_then(|v| v.as_str()).unwrap_or(key);
                    let desc = d.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let status = d.get("status").and_then(|v| v.as_str()).unwrap_or("active");
                    let wid: Option<i64> = d.get("world_id").and_then(|v| v.as_i64());
                    match action {
                        "add" | "create" => {
                            match db::store::create_quest(&db, title, desc, wid.or(Some(1))) {
                                Ok(qid) => {
                                    // 如果有 characters 列表，添加参与者
                                    let mut results = vec![format!("任务 '{}' 已创建 (id={})", title, qid)];
                                    if let Some(chars) = d.get("characters").and_then(|v| v.as_array()) {
                                        for c in chars {
                                            let slug = c.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                                            let role = c.get("role").and_then(|v| v.as_str()).unwrap_or("member");
                                            let task = c.get("task").and_then(|v| v.as_str()).unwrap_or("");
                                            if !slug.is_empty() {
                                                match db::store::add_quest_character(&db, qid, slug, role, task) {
                                                    Ok(r) => results.push(r),
                                                    Err(e) => results.push(format!("添加参与者失败: {}", e)),
                                                }
                                            }
                                        }
                                    }
                                    results.join("\n")
                                },
                                Err(e) => format!("创建任务失败: {}", e),
                            }
                        }
                        "update" => {
                            // 通过 key 找 quest id 并更新状态
                            let qid: i64 = if let Ok(id) = key.parse() { id } else { 0 };
                            if qid > 0 {
                                match db::store::update_quest_status(&db, qid, status) {
                                    Ok(_) => format!("任务 '{}' 状态已更新为: {}", title, status),
                                    Err(e) => format!("Error: {}", e),
                                }
                            } else { "需要有效的任务 ID".to_string() }
                        }
                        _ => {
                            match db::store::list_quests(&db, wid.unwrap_or(1)) {
                                Ok(quests) => {
                                    if quests.is_empty() { "暂无任务".to_string() }
                                    else { quests.iter().map(|q| format!("- [{}] {}: {}", q.status, q.title, q.description)).collect::<Vec<_>>().join("\n") }
                                },
                                Err(e) => format!("Error: {}", e),
                            }
                        }
                    }
                } else { "数据格式错误".to_string() }
            }
            _ => {
                match db::store::manage_state(&db, "character_states", char_id, action, category, key, &data, tl_id) {
                    Ok(result) => result,
                    Err(e) => format!("Error: {}", e),
                }
            }
        }
    } else { "Invalid arguments".to_string() }
}
```

- [ ] **Step 2: cargo check**

```bash
cargo check
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat: route manage_state relation/quest categories in tool executor"
```

---

### Task 7: Command 解析器 — 5 个新命令

**Files:**
- Modify: `src/command/parser.rs:5-98`

- [ ] **Step 1: 在 Command enum 中添加新变体**

```rust
/// `/relations [char]` — 查看角色关系
Relations(Option<String>),
/// `/affinity <char> <值>` — 设置好感度
Affinity(String, String),
/// `/quests` — 列出任务
Quests,
/// `/quest <标题>` — 创建任务
Quest(String),
/// `/task <任务ID> <操作> <参数>` — 管理任务参与者/状态
Task(String, String, String),
```

- [ ] **Step 2: 在 parse() 函数中添加解析分支**

在 `/timeline` 别名后面（line 80 `"timeline" | "tl"` 行之后）添加：

```rust
"relations" | "rel" => (Command::Relations(if _args.is_empty() { None } else { Some(_args.trim().to_string()) }), String::new()),
"affinity" | "aff" => {
    let (char_name, val) = _args.split_once(' ').unwrap_or((_args, "0"));
    (Command::Affinity(char_name.trim().to_string(), val.trim().to_string()), String::new())
},
"quests" | "ql" => (Command::Quests, String::new()),
"quest" => (Command::Quest(_args.trim().to_string()), String::new()),
"task" => {
    let parts: Vec<&str> = _args.splitn(3, ' ').collect();
    let id = parts.first().map(|s| *s).unwrap_or("");
    let act = parts.get(1).map(|s| *s).unwrap_or("");
    let rest = parts.get(2).map(|s| *s).unwrap_or("");
    (Command::Task(id.to_string(), act.to_string(), rest.to_string()), String::new())
},
```

- [ ] **Step 3: cargo check — 会有未处理的 match arm 警告，后续 task 修复**

```bash
cargo check
```

Expected: `Command` enum new variants not handled in app.rs 匹配处（会显示 `non-exhaustive patterns` error）。

- [ ] **Step 4: Commit**

```bash
git add src/command/parser.rs
git commit -m "feat: parse /relations /affinity /quests /quest /task commands"
```

---

### Task 8: TUI 命令处理 — 5 个新 handler + dispatch

**Files:**
- Modify: `src/tui/app.rs:179-223`（dispatch）+ 新 handler 函数

- [ ] **Step 1: 在事件循环 dispatch 中添加新命令路由**

在 line 208 `Command::Timeline` 之后添加：

```rust
Command::Relations(char_opt) => { self.handle_relations(char_opt.as_deref().unwrap_or("")); return; }
Command::Affinity(char_name, val) => { self.handle_affinity(&char_name, &val); return; }
Command::Quests => { self.handle_quests(); return; }
Command::Quest(title) => { self.handle_quest(&title); return; }
Command::Task(id, action, rest) => { self.handle_task(&id, &action, &rest); return; }
```

- [ ] **Step 2: 添加 handle_relations 函数**

在 `handle_timeline` 函数之后（`pub fn handle_timeline(&mut self)` 结尾后）添加：

```rust
/// 查看当前角色的人际关系
pub fn handle_relations(&mut self, char_name: &str) {
    let char_id = if char_name.is_empty() {
        self.manager.active().id
    } else {
        match db::store::get_character(&self.db, char_name).ok().flatten() {
            Some(c) => c.id,
            None => { self.error = Some(format!("角色 '{}' 不存在", char_name)); return; }
        }
    };
    match db::store::list_relations(&self.db, char_id) {
        Ok(rels) => {
            if rels.is_empty() {
                self.manager.active_mut().messages.push(Message { role: "system".into(), content: "暂无关系记录".into() });
            } else {
                let list = rels.iter().map(|r| {
                    let aff = if r.affinity > 0 { format!("好感 +{}", r.affinity) }
                        else if r.affinity < 0 { format!("反感 {}", r.affinity) }
                        else { "中立".into() };
                    let note = if r.note.is_empty() { String::new() } else { format!(" ({})", r.note) };
                    format!("- {} | {} | {}{}", r.to_char_name, r.rel_type, aff, note)
                }).collect::<Vec<_>>().join("\n");
                let title = if char_name.is_empty() { "你的人际关系".into() } else { format!("{} 的人际关系", char_name) };
                self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("{}\n{}", title, list) });
            }
            self.scroll_offset = 0;
        }
        Err(e) => self.error = Some(format!("查询关系失败: {}", e)),
    }
}
```

- [ ] **Step 3: 添加 handle_affinity 函数**

```rust
/// 设置与某角色的好感度
pub fn handle_affinity(&mut self, char_name: &str, val_str: &str) {
    if char_name.is_empty() {
        self.error = Some("用法: /affinity <角色slug> <好感度>".into());
        return;
    }
    let val: i32 = val_str.parse().unwrap_or(0);
    let rel_type = if val >= 50 { "friend" } else if val <= -50 { "enemy" } else { "neutral" };
    let char_id = self.manager.active().id;
    match db::store::set_relation(&self.db, char_id, char_name, rel_type, val, "") {
        Ok(msg) => self.error = Some(msg),
        Err(e) => self.error = Some(format!("设置好感度失败: {}", e)),
    }
}
```

- [ ] **Step 4: 添加 handle_quests 函数**

```rust
/// 列出当前世界的所有任务
pub fn handle_quests(&mut self) {
    let world_id = self.manager.active_world.map(|i| self.manager.worlds[i].id).unwrap_or(1);
    match db::store::list_quests(&self.db, world_id) {
        Ok(quests) => {
            if quests.is_empty() {
                self.manager.active_mut().messages.push(Message { role: "system".into(), content: "当前世界暂无任务".into() });
            } else {
                let list = quests.iter().map(|q| {
                    format!("[{}] #{} {} - {}", q.status, q.id, q.title, if q.description.len() > 40 { &q.description[..40] } else { &q.description })
                }).collect::<Vec<_>>().join("\n");
                self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("当前世界任务:\n{}", list) });
            }
            self.scroll_offset = 0;
        }
        Err(e) => self.error = Some(format!("查询任务失败: {}", e)),
    }
}
```

- [ ] **Step 5: 添加 handle_quest 函数**

```rust
/// 创建新任务
pub fn handle_quest(&mut self, title: &str) {
    if title.is_empty() {
        self.error = Some("用法: /quest <任务标题>".into());
        return;
    }
    let world_id = self.manager.active_world.map(|i| self.manager.worlds[i].id);
    match db::store::create_quest(&self.db, title, "", world_id) {
        Ok(id) => self.error = Some(format!("任务 '{}' 已创建 (id={})", title, id)),
        Err(e) => self.error = Some(format!("创建任务失败: {}", e)),
    }
}
```

- [ ] **Step 6: 添加 handle_task 函数**

```rust
/// 管理任务（添加参与者/更新状态）
pub fn handle_task(&mut self, id_str: &str, action: &str, rest: &str) {
    let qid: i64 = match id_str.parse() {
        Ok(id) => id,
        Err(_) => { self.error = Some("任务 ID 必须是数字".into()); return; }
    };
    match action {
        "add" | "a" => {
            // /task <id> add <char_slug> <role> <task_desc>
            let parts: Vec<&str> = rest.splitn(3, ' ').collect();
            let slug = parts.first().map(|s| *s).unwrap_or("");
            let role = parts.get(1).map(|s| *s).unwrap_or("member");
            let task = parts.get(2).map(|s| *s).unwrap_or("");
            if slug.is_empty() { self.error = Some("用法: /task <id> add <角色slug> <role> <分工>".into()); return; }
            match db::store::add_quest_character(&self.db, qid, slug, role, task) {
                Ok(msg) => self.error = Some(msg),
                Err(e) => self.error = Some(format!("添加失败: {}", e)),
            }
        }
        "status" | "st" => {
            // /task <id> status <completed|failed|active|abandoned>
            if rest.is_empty() { self.error = Some("用法: /task <id> status <completed|failed|active|abandoned>".into()); return; }
            match db::store::update_quest_status(&self.db, qid, rest) {
                Ok(_) => self.error = Some(format!("任务 #{} 状态已更新为: {}", qid, rest)),
                Err(e) => self.error = Some(format!("更新失败: {}", e)),
            }
        }
        "info" | "i" => {
            match db::store::list_quest_characters(&self.db, qid) {
                Ok(chars) => {
                    if chars.is_empty() {
                        self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("任务 #{} 暂无参与者", qid) });
                    } else {
                        let list = chars.iter().map(|c| format!("- {} (角色: {}, 分工: {})", c.char_name, c.role, if c.task.is_empty() { "未分配" } else { &c.task })).collect::<Vec<_>>().join("\n");
                        self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("任务 #{} 参与者:\n{}", qid, list) });
                    }
                    self.scroll_offset = 0;
                }
                Err(e) => self.error = Some(format!("查询失败: {}", e)),
            }
        }
        _ => { self.error = Some("用法: /task <id> add|status|info ...".into()); }
    }
}
```

- [ ] **Step 7: 修复 LocationRow 构造**

在 `handle_location` 函数中（line 436）修复 LocationRow 缺少 `parent_id` 字段：

将：
```rust
let row = db::store::LocationRow { id: 0, slug: rest.to_string(), name: rest.to_string(), description: String::new(), connects_to: String::new(), world_id: world.id };
```
改为：
```rust
let row = db::store::LocationRow { id: 0, slug: rest.to_string(), name: rest.to_string(), description: String::new(), connects_to: String::new(), parent_id: None, world_id: world.id };
```

- [ ] **Step 8: 在 system_prompt 组装中注入角色关系上下文**

在 system_prompt 组装代码中（line ~772 之后，skill_text 注入之后），添加角色关系注入：

```rust
// 注入当前角色的人际关系
if let Ok(rels) = db::store::list_relations(&app.db, app.manager.active().id) {
    if !rels.is_empty() {
        let rel_summary: Vec<_> = rels.iter().map(|r| {
            format!("{} ({} / 好感: {}{})", r.to_char_name, r.rel_type, r.affinity, if r.note.is_empty() { String::new() } else { format!(": {}", r.note) })
        }).collect();
        system_prompt.push_str(&format!("\n\n【当前角色人际关系】\n{}", rel_summary.join("\n")));
    }
}
```

插入位置：在 `skill_text` 注入之后、`let llm_config = cfg.llm.clone();` 之前。

- [ ] **Step 9: cargo check**

```bash
cargo check
```

Expected: clean。

- [ ] **Step 10: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat: implement /relations /affinity /quests /quest /task handlers"
```

---

### Task 9: sys_skill/quest_patterns.md

**Files:**
- Create: `sys_skill/quest_patterns.md`

- [ ] **Step 1: 创建文件**

```markdown
# 任务与关系管理模式

## 何时管理角色关系

当对话中出现以下场景时，调用 `manage_state(category="relation", ...)`：

| 场景 | 示例 | action | 参数 |
|------|------|--------|------|
| 结识新角色 | "我是镇上的铁匠" | `add` | rel_type="neutral" |
| 结盟/成为朋友 | "从今天起我们就是兄弟了" | `add` 或 `update` | rel_type="friend", affinity=+60 |
| 产生敌意 | "你这叛徒！" | `add` 或 `update` | rel_type="enemy", affinity=-70 |
| 好感上升 | "谢谢你救了我" | `update` | affinity 提升 |
| 关系恶化 | "我再也无法信任你了" | `update` | affinity 降低 |
| 断绝关系 | "我们的交情到此为止" | `delete` | — |

**参数约定：**
- `key`: 关系标识，如 `relation:friend`
- `data` 必须包含 `to_char_slug`（对方的 slug）
- `data` 选填 `rel_type`、`affinity`（-100~100）、`note`

**affinity 参考值：**
- +80~100: 挚友/至亲
- +40~79: 朋友/盟友
- +1~39: 友好
- 0: 中立/初识
- -1~-39: 冷淡
- -40~-79: 敌意
- -80~-100: 死敌

## 何时管理任务

| 场景 | 操作 | 示例 |
|------|------|------|
| NPC 提出委托 | `add` quest | "帮我去北山采药" |
| 剧情形成目标 | `add` quest | "我们必须找到失落的圣剑" |
| 任务完成 | `update` status="completed" | 目标达成后 |
| 任务失败 | `update` status="failed" | 关键节点失败后 |
| 新角色加入任务 | 在 quest data 中添 characters | 有人加入队伍 |

**任务 data 参数：**
```json
{
  "title": "任务名称",
  "description": "任务描述",
  "world_id": 1,
  "characters": [
    {"slug": "innkeeper", "role": "client", "task": "提供情报"},
    {"slug": "knight", "role": "member", "task": "保护队伍"}
  ]
}
```

**更新任务状态：**
```
manage_state(action="update", category="quest", key="1", data={"status":"completed"})
key 为任务 ID 数字
```

## 示例调用

```
用户: "老酒保让我去找镇外的魔法师帮忙"
→ manage_state(add, category="relation", key="relation:contact",
    data={"to_char_slug":"mage", "rel_type":"neutral", "affinity":0, "note":"通过老酒保介绍认识"})
→ manage_state(add, category="quest", key="quest:find_mage",
    data={"title":"寻找魔法师", "description":"老酒保委托寻找镇外的魔法师", "characters":[{"slug":"innkeeper","role":"client","task":"委托并提供情报"}]})

用户: "和魔法师并肩作战后，我们成了生死之交"
→ manage_state(update, category="relation", key="relation:friend",
    data={"to_char_slug":"mage", "affinity":80, "rel_type":"friend", "note":"生死之交"})
→ advance_time(...)  （如果战斗跨越了时间）
```

## 不要做的事

- 不要为角色的预设背景关系调用工具（这些是角色卡静态设定）
- 不要为 NPC 之间的非玩家相关关系调用（除非直接影响剧情）
- 不要创建没有明确目标的模糊任务
```

- [ ] **Step 2: cargo check**

```bash
cargo check
```

Expected: clean（.md 文件不影响编译）。

- [ ] **Step 3: Commit**

```bash
git add sys_skill/quest_patterns.md
git commit -m "feat: add sys_skill/quest_patterns.md for LLM quest/relation guidance"
```

---

### Task 10: 集成验证

**Files:** 无修改

- [ ] **Step 1: cargo check 全量编译**

```bash
cargo check
```

Expected: clean（仅已有项目的 25 warnings）。

- [ ] **Step 2: 运行应用验证 Schema 迁移**

```bash
cargo run -- --cl
```

Expected: 列出角色（第一次运行会触发 schema init，包括新表和 locations ALTER）。

- [ ] **Step 3: 手动测试 TUI 命令**

```bash
cargo run
```

在 TUI 中输入：
- `/relations` → 应显示"暂无关系记录"
- `/affinity innkeeper 60` → 应显示关系已更新
- `/relations` → 应显示与老酒保的关系
- `/quests` → 应显示"暂无任务"
- `/quest 寻找失落的圣剑` → 应显示任务已创建
- `/quests` → 应显示新任务
- `/task 1 add innkeeper client 提供情报` → 应显示参与者已添加
- `/task 1 info` → 应显示参与者列表
- `/task 1 status completed` → 应显示状态已更新

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: integration verification passed"
```

---

## Self-Review

1. **Spec coverage:** 每项 spec 要求有对应 task：新表 (Task 1)、parent_id (Tasks 1+2)、store CRUD (Tasks 2-4)、manage_state 扩展 (Tasks 5-6)、TUI 命令 (Tasks 7-8)、sys_skill (Task 9)。
2. **Placeholder scan:** 无 TBD/TODO，所有代码完整。
3. **Type consistency:** `CharacterRelation` 的字段贯穿 Task 3 的 CRUD 和 Task 8 的 handler，一致。`QuestRow` 同样。`LocationRow.parent_id` 在 Tasks 2 和 8 中一致使用。
