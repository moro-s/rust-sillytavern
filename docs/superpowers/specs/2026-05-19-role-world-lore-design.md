# Role-World-Lore 关联设计

> 2026-05-19 | brainstorm 结论 | 待实施

## 目录结构

```
characters/                     # 全局角色池
├── index.md                    # 角色索引
├── innkeeper.md                # 角色卡（静态设定）
├── innkeeper.state.md          # 角色状态（动态，对话中更新）
└── ...

worlds/                         # 世界目录
├── index.md                    # 世界索引
└── faerun/                     # 一个世界
    ├── world.md                # 世界元信息
    ├── characters/             # 世界内专属角色
    │   ├── mage.md
    │   ├── mage.state.md
    │   └── ...
    └── lore/                   # 世界内词条
        ├── dragon.md
        └── knights.md
```

## 文件格式

### 角色卡 (`*.md`)

```markdown
---
name: 老酒保
personality: 见多识广，说话慢条斯理
speech_style: 喜欢用"哼"开头，偶尔冒出老派口吻
first_message: (慢慢擦着酒杯) 哼，这年头刀口舔血的人不少...
world: faerun                  # 可选：所属世界
---

# 背景
曾是冒险者，三十年前被龙伤了腿...

# 外貌
花白头发，左腿是木制假肢...

# 我知道的事情
- 镇上来往冒险者的传闻
- 北方山脉的古老传说
```

### 角色状态 (`*.state.md`)

```markdown
# 物品
| item | qty | note |
|------|-----|------|
| 银制酒杯 | 1 | 吧台上的老物件 |

# 重要事件
| 时间 | 事件 | importance |
|------|------|------------|
| 2026-01 | 遇到了自称屠龙者的冒险者 | medium |

# 技能
| skill | desc | type |
|-------|------|------|
| 调酒大师 | 精通各类酒饮调制 | passive |

# 当前状态
| status | detail |
|--------|--------|
| 疲惫 | 连续接待了三波客人 |
```

### 世界卡 (`world.md`)

```markdown
---
name: faerun
desc: 一个剑与魔法的中世纪世界
associated_characters:         # 关联的角色 key
  - innkeeper
  - mage
---

# 世界观概述
大陆上人类、精灵、矮人三大种族...

# 当前大事件
北方山脉的魔龙即将苏醒...
```

### 世界词条 (`lore/*.md`)

```markdown
# 魔龙传说

| 属性 | 值 |
|------|-----|
| key | 魔龙传说 |
| triggers | 龙, 魔龙, 喷火, 巨龙 |
| priority | 10 |
| selective | false |

在北方山脉深处，沉睡着一头名为"暗翼"的远古魔龙...
```

### 角色索引 (`characters/index.md`)

```markdown
| id | name | location | world | updated |
|----|------|----------|-------|---------|
| c001 | innkeeper | characters/innkeeper.md | faerun | 2026-05-19 |
| c002 | mage | worlds/faerun/characters/mage.md | faerun | 2026-05-19 |
```

### 世界索引 (`worlds/index.md`)

```markdown
| id | name | location |
|----|------|----------|
| w001 | faerun | worlds/faerun/ |
```

## Function Call 接口

### 通用接口 `manage_state`

```json
{
  "name": "manage_state",
  "description": "管理角色/世界状态。增删改查物品、事件、技能、状态、角色信息、世界词条。",
  "parameters": {
    "action": "get | search | add | update | delete",
    "category": "item | event | skill | status | character | lore",
    "key": "标识名",
    "data": {}
  }
}
```

**示例：**
```
manage_state(action="search", category="item", key="钥匙")
manage_state(action="add", category="item", key="地窖钥匙", data={qty:1,note:"冒险者送的"})
manage_state(action="add", category="event", key="", data={desc:"北境要塞被龙烧毁",importance:"high"})
manage_state(action="update", category="status", key="精力", data={detail:"疲惫"})
manage_state(action="get", category="character", key="mage")
manage_state(action="get", category="lore", key="魔龙传说")
```

## Token 优化策略

| 层级 | 内容 | 注入条件 | 预估 token |
|------|------|---------|-----------|
| 固定 | 角色名 + 性格摘要（2-3句）| 总是 | ~50 |
| 按需 | 匹配关键词的 state 项 | 用户/AI 提到时 | 0-200 |
| 按需 | 匹配关键词的 lore 词条 | 用户/AI 提到时 | 0-300 |
| 滚动 | 最近 20 条对话 | 总是 | 已有逻辑 |
| AI查 | manage_state 查询结果 | AI 主动调用 | 按需 |

- 事件积累 10 条 → 后台压缩为摘要
- state 文件内容不在 system prompt 里全量发送
- AI 通过 `manage_state` 主动查所需信息

## 创建命令（模板引导）

| 命令 | 说明 |
|------|------|
| `/cc <name> [--world <w>]` | 创建角色卡模板 |
| `/cw <name>` | 创建世界目录 + world.md 模板 |
| `/cl <name>` | 在世界内创建词条模板 |

创建时输出 YAML 头 + 表格框架，引导用户填写。

## 实施计划

1. 重构目录和文件格式（lorebooks/*.toml → lore/*.md）
2. 实现 `index.md` 读写（角色/世界索引）
3. 实现 `.state.md` 读写 + `manage_state` 后端
4. 实现 function call 集成（DeepSeek tools API）
5. token 优化（摘要 + 按需注入）
6. 更新 `/cc` `/cw` 模板
7. 热加载适配新格式
