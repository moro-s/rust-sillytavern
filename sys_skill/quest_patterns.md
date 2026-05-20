# 任务系统使用模式

本文档提供 LLM 判断何时调用 `manage_state` 工具处理任务（quest）和关系（relation）的场景模式与示例，供你参考借鉴——不要求逐字匹配，请根据实际情境灵活判断。

---

## 一、任务（quest）场景模式

### 1.1 创建新任务

**触发条件**：剧情中出现明确的委托、目标或待办事宜。

**调用方式**：
```
action: "add"
category: "quest"
key: "任务标识名"
data: {title: "任务标题", status: "active", description: "任务描述"}
```

**示例场景**：
- "帮我找回丢失的宝剑" → `action:add, category:quest, key:"寻回宝剑", data:{title:"寻回失落的宝剑",status:"active",description:"村长委托寻找祖传宝剑"}`
- "我们需要调查那个废弃矿洞" → add quest "调查矿洞"
- "三天后就是比武大会，我要赢" → add quest "比武大会"

### 1.2 更新任务状态

**触发条件**：任务有明显进展、完成或失败。

**调用方式**：
```
action: "update"
category: "quest"
key: "任务标识名"
data: {status: "completed", description: "更新后的描述（可选）"}
```

**status 可选值**：active（进行中）、completed（已完成）、failed（已失败）、paused（暂缓）

**示例场景**：
- "我把宝剑找回来了" → `action:update, category:quest, key:"寻回宝剑", data:{status:"completed",description:"在废弃矿洞深处找到了村长的祖传宝剑"}`
- "比武大会我输了" → update "比武大会" → status:"failed"
- "这个任务先放一放" → update quest → status:"paused"

### 1.3 查询任务

**调用方式**：
```
action: "get" 或 "search"
category: "quest"
key: "任务名或空字符串（获取全部）"
```

**示例场景**：
- 用户问"我现在有哪些任务" → `action:get, category:quest, key:""`
- "寻回宝剑的任务怎么样了" → `action:get, category:quest, key:"寻回宝剑"`

---

## 二、角色关系（relation）场景模式

### 2.1 建立/更新角色关系

**触发条件**：角色之间建立了新的关系或现有关系发生变化。

**调用方式**：
```
action: "add" 或 "update"
category: "relation"
key: "关系标识（建议用 from-to-type 格式）"
data: {to: "目标角色名", type: "关系类型", affinity: 好感度数值}
```

**type 可选值**：friend、enemy、lover、family、rival、mentor、student、neutral、ally 等
**affinity**：-100（极度仇视）到 100（极度好感），0 为中立

**示例场景**：
- "从此我们就是兄弟了" → `action:add, category:relation, key:"主角-张三-friend", data:{to:"张三", type:"friend", affinity:80}`
- "我恨死李四了" → update relation, affinity:-60, type:"enemy"
- "王五收我为徒" → add relation type:"mentor", affinity:70

### 2.2 查询关系

**调用方式**：
```
action: "get" 或 "search"
category: "relation"
key: "角色名或空字符串"
```

**示例场景**：
- "我跟谁关系比较好" → `action:get, category:relation, key:""`
- "李四对我是什么态度" → `action:get, category:relation, key:"李四"`

### 2.3 删除关系

**触发条件**：关系彻底断绝。

**调用方式**：
```
action: "delete"
category: "relation"
key: "关系标识名"
```

---

## 三、禁止事项

1. 不要在没有明确剧情触发时随意创建任务
2. 不要为了记录"角色之间存在互动"而创建关系——仅在有明确的情感定位变化时操作
3. 好感度微调（±5以内）不需要调用工具
4. 同一任务不要重复创建——先 get 查询是否已存在
