# 任务与关系图设计

> 2026-05-20 | brainstorm 结论 | 待实施

## 概述

在现有 SQLite schema 基础上，新增角色关系、地点嵌套、任务系统三大能力。不引入新 crate，完全延续现有 junction 表 + fields 风格。

## Schema 变更

### 1. `character_relations`（角色关系）

新增 junction 表，记录角色间的定性关系 + 量化好感度。

```sql
CREATE TABLE IF NOT EXISTS character_relations (
    from_char_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    to_char_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    rel_type     TEXT NOT NULL DEFAULT 'neutral',  -- friend|enemy|family|master|lover|neutral|rival
    affinity     INTEGER NOT NULL DEFAULT 0,       -- -100 ~ +100
    note         TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (from_char_id, to_char_id, rel_type)
);
```

**ABI 约定**：
- 单向关系从 `from_char_id` → `to_char_id`
- 双向关系需两条记录（如互相为友）
- `affinity` 为对称值时不强制双向，由业务层决定是否同步
- `note` 为自然语言备注，供 LLM 阅读和人回看

**典型查询**：
```sql
-- 老酒保认识的所有人及好感度
SELECT c.name, r.rel_type, r.affinity, r.note
FROM character_relations r JOIN characters c ON c.id = r.to_char_id
WHERE r.from_char_id = ?;

-- 所有敌意关系
SELECT a.name, b.name FROM character_relations r
JOIN characters a ON a.id=r.from_char_id JOIN characters b ON b.id=r.to_char_id
WHERE r.rel_type = 'enemy';
```

### 2. `locations` 扩展 `parent_id`

修改现有 `locations` 表，新增一列，表达地点树形嵌套（酒馆⊂镇子⊂王国）。

```sql
ALTER TABLE locations ADD COLUMN parent_id INTEGER REFERENCES locations(id) ON DELETE SET NULL;
```

- `parent_id = NULL` 表示顶层地点（如世界本身、独立大陆）
- `connects_to` 字段保留不动，用于表达连通关系（镇子↔森林）而非嵌套
- `location_links` junction 表不创建，树形结构用 `parent_id` 足够

**典型查询**：
```sql
-- 酒馆的父级地点
SELECT * FROM locations WHERE id = (SELECT parent_id FROM locations WHERE slug='inn');

-- 王国下的所有子地点（含多级）
WITH RECURSIVE subtree(id) AS (
    SELECT id FROM locations WHERE slug='kingdom'
    UNION ALL
    SELECT l.id FROM locations l JOIN subtree s ON l.parent_id = s.id
) SELECT * FROM locations WHERE id IN subtree;
```

### 3. `quests`（任务主表）

```sql
CREATE TABLE IF NOT EXISTS quests (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'active',  -- active|completed|failed|abandoned
    world_id    INTEGER REFERENCES worlds(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- `status` 由 `manage_state(action="update")` 驱动变更
- `world_id` 可空：世界级任务绑定 world，角色间任务可为 NULL
- 任务进度不设 phases 列 —— 通过 `character_states` 的 `category="quest"` 条目按角色记录具体进度

### 4. `quest_characters`（任务参与者）

```sql
CREATE TABLE IF NOT EXISTS quest_characters (
    quest_id     INTEGER NOT NULL REFERENCES quests(id) ON DELETE CASCADE,
    character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    role         TEXT NOT NULL DEFAULT 'member',  -- leader|member|target|client|witness
    task         TEXT NOT NULL DEFAULT '',         -- 该角色的分工作描述
    PRIMARY KEY (quest_id, character_id)
);
```

- 一个任务可有多个角色参与，每个角色有分工（`task`）
- `role='target'` 表示该角色是任务目标（被救/被杀/被找的对象）
- `role='client'` 表示该角色是任务委托人
- 分工（`task`）可为空 —— LLM 在对话中动态补充

## 与 `manage_state` 工具集成

现有 `manage_state` 工具扩展两个新 category：

| category | 说明 | 示例 key | 存储位置 |
|----------|------|---------|---------|
| `relation` | 角色关系增删改 | `relation:friend` | `character_relations` |
| `quest` | 任务相关 | `quest:kill_dragon` | `quests` + `character_states` |

`manage_state` 新增 action → category 映射：
- `manage_state(add, "quest", key, data)` → 创建 quests 行 + 可选 quest_characters
- `manage_state(add, "relation", key, data)` → 创建 character_relations 行
- `manage_state(get, "relation", key)` → 查询某角色的关系列表
- `manage_state(update, "quest", key, data)` → 更新任务状态/阶段
- `manage_state(delete, "relation", key)` → 删除关系

`data` 字段约定：
```json
// 添加关系
{"to_char_slug": "mage", "rel_type": "friend", "affinity": 60, "note": "一起屠过龙"}

// 创建任务
{"world_id": 1, "description": "找到并杀死北山的恶龙", "characters": [
    {"slug": "innkeeper", "role": "client", "task": "委托并提供情报"},
    {"slug": "mage", "role": "member", "task": "提供魔法支援"}
]}

// 更新任务状态
{"status": "completed"}
```

## `sys_skill/` 扩展

新增 `sys_skill/quest_patterns.md`，教 LLM 何时及如何创建/更新任务和关系：

```markdown
# 任务与关系管理模式

## 何时创建任务
- NPC 明确提出委托/请求
- 对话中自然形成目标（"找到丢失的戒指"）
- 剧情揭示新的使命

## 何时更新关系
- 建立新的人脉（认识、结盟、收徒）
- 关系恶化（背叛、结仇）
- 好感度变化（帮忙+、得罪-）

## 示例
用户: "老酒保说让我去找魔法师帮忙"
→ manage_state(add, category="relation", key="relation:friend",
    data={"to_char_slug":"mage", "rel_type":"contact", "affinity":0, "note":"老酒保介绍的认识"})
→ manage_state(add, category="quest", key="quest:find_mage",
    data={"description":"找到镇外的魔法师", "characters":[{"slug":"innkeeper","role":"client"}]})
```

## TUI 命令扩展

| 命令 | 说明 |
|------|------|
| `/relations [char]` | 查看当前/指定角色的人际关系表 |
| `/affinity <char> <值>` | 设置与某角色的好感度 |
| `/quests` | 列出当前世界所有任务 |
| `/quest <标题>` | 创建新任务 |
| `/task <任务ID> <操作>` | 更新任务状态/参与者 |

## 迁移路径

1. `ALTER TABLE locations ADD COLUMN parent_id`（NULL 安全，不改已有数据）
2. 新建 `character_relations`、`quests`、`quest_characters` 三表
3. `manage_state` 扩展 category 路由，非破坏
4. 新增 `sys_skill/quest_patterns.md`
5. new TUI 命令

所有变更为增量，不删除或修改任何现有数据行。
