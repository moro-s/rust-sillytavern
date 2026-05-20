# 状态管理模式

## 何时调用 manage_state（状态类）

当角色的**生理/心理/社会状态**发生变化时，调用 `manage_state(category="status", ...)`：

| 场景类型 | 对话示例 | 应执行的操作 |
|---------|---------|------------|
| **受伤** | "被砍了一刀"、"摔断了腿" | `add` status="受伤" |
| **中毒/疾病** | "中毒了"、"感染了瘟疫" | `add` status="中毒" |
| **疲惫** | "走了三天三夜"、"累得不行了" | `add` status="疲惫" |
| **昏迷/无法行动** | "晕过去了"、"被麻痹了" | `add` status="昏迷" |
| **恢复/治愈** | "伤口愈合了"、"毒解了" | `delete` 或 `update` |
| **状态恶化** | "伤势加重了"、"毒更深了" | `update` 提高严重程度 |
| **情绪状态** | "怒不可遏"、"陷入绝望" | `add` status="愤怒" |
| **能力变化** | "觉醒了火焰之力"、"魔力耗尽了" | `add` 或 `update` |
| **关系变化** | "他成了我的仇人"、"拜了把子" | `add` status="结义兄弟" |

## key 命名规范

- 格式: `status:状态名`
- 示例: `status:受伤`、`status:中毒`、`status:愤怒`
- 状态名用简短的中文词，描述当前状态的核心特征

## data 字段建议

```json
// 受伤
{"severity": "轻伤", "body_part": "左臂", "cause": "被剑砍伤"}

// 中毒
{"severity": "中度", "type": "蛇毒", "detail": "伤口发黑，四肢麻木"}

// 情绪
{"intensity": "极度", "reason": "被背叛"}

// 能力
{"desc": "能释放火球术", "cooldown": "每天一次"}
```

## 多状态并存

一个角色可以同时有多种状态（受伤+疲惫+愤怒），每种状态独立添加/删除。

## 示例调用

```
用户: "被毒蛇咬了一口，现在全身发麻"
→ manage_state(action="add", category="status", key="status:中毒", data={"severity":"中度","type":"蛇毒","detail":"全身发麻"})

用户: "休息了一天后伤好得差不多了"
→ manage_state(action="delete", category="status", key="status:受伤")

用户: "小伤口感染了，开始化脓"
→ manage_state(action="update", category="status", key="status:受伤", data={"severity":"恶化","detail":"伤口感染化脓"})
```

## 不要调用的场景

- 角色的常态描述（如 "我很饿" 只是日常对话，不是状态异常）
- 用户描述别人的状态（除非与用户角色直接相关）
- 短暂的情绪波动（除非剧情中有持续影响）
