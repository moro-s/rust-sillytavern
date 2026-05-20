# 地点追踪模式

## 何时调用 manage_state（地点相关）

当角色**物理位置发生变化**时，通过状态记录当前位置：

| 场景 | 对话示例 | 操作 |
|------|---------|------|
| **到达新地点** | "走进了酒馆"、"来到了广场" | `add` status="位于酒馆" |
| **离开地点** | "离开了城堡"、"走出了森林" | `delete` 旧地点 + `add` 新地点 |
| **穿越** | "穿过密林来到了湖边" | `update` 当前所在地 |
| **被移动** | "被传送到了王座厅" | `add` 新地点 |

## key 和 data 规范

```
key: "status:位于{地点名}"
data: {"detail": "场景描述", "arrived_at": "第2天早晨"}
```

## 示例调用

```
用户: "我走进了昏暗的酒馆"
→ manage_state(action="add", category="status", key="status:位于酒馆", data={"detail":"昏暗的小酒馆，只有几盏油灯"})

用户: "离开酒馆，冒着雨往城门口走"
→ manage_state(action="delete", category="status", key="status:位于酒馆")
→ manage_state(action="add", category="status", key="status:前往城门", data={"detail":"冒着大雨赶路"})
```

## 与 advance_time 配合

地点变化通常伴随时间推进，两者可以同时调用：

```
用户: "骑马赶了三天路，终于到了王城"
→ advance_time(label="三天后", description="赶往王城")
→ manage_state(add, category="status", key="status:位于王城", ...)
```

## 不要调用的场景

- 地点仅作为背景描述（如 "窗外的雪山很美"——角色没移动）
- 梦境、幻觉中的地点
- 微小的室内移动（如 "走到窗边"）
