use std::collections::HashMap;

/// Parsed state from a .state.md file
#[derive(Debug, Clone, Default)]
pub struct CharacterState {
    pub items: Vec<StateItem>,
    pub events: Vec<StateEvent>,
    pub skills: Vec<StateSkill>,
    pub statuses: Vec<StateStatus>,
}

#[derive(Debug, Clone)]
pub struct StateItem {
    pub item: String,
    pub qty: i32,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct StateEvent {
    pub desc: String,
    pub importance: String,
}

#[derive(Debug, Clone)]
pub struct StateSkill {
    pub skill: String,
    pub desc: String,
    pub skill_type: String,
}

#[derive(Debug, Clone)]
pub struct StateStatus {
    pub status: String,
    pub detail: String,
}

/// Load state from .state.md
pub fn load(path: &str) -> CharacterState {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return CharacterState::default(),
    };
    parse_markdown_tables(&content)
}

/// Save state to .state.md
pub fn save(state: &CharacterState, path: &str) -> anyhow::Result<()> {
    let mut md = String::new();

    md.push_str("# 物品\n| item | qty | note |\n|------|-----|------|\n");
    for item in &state.items {
        md.push_str(&format!("| {} | {} | {} |\n", item.item, item.qty, item.note));
    }

    md.push_str("\n# 重要事件\n| 事件 | importance |\n|------|------------|\n");
    for event in &state.events {
        md.push_str(&format!("| {} | {} |\n", event.desc, event.importance));
    }

    md.push_str("\n# 技能\n| skill | desc | type |\n|-------|------|------|\n");
    for skill in &state.skills {
        md.push_str(&format!("| {} | {} | {} |\n", skill.skill, skill.desc, skill.skill_type));
    }

    md.push_str("\n# 当前状态\n| status | detail |\n|--------|--------|\n");
    for status in &state.statuses {
        md.push_str(&format!("| {} | {} |\n", status.status, status.detail));
    }

    std::fs::write(path, &md)?;
    Ok(())
}

/// Parse markdown tables from state content
fn parse_markdown_tables(content: &str) -> CharacterState {
    let mut state = CharacterState::default();
    let mut section: &str = "";

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('|') && trimmed.contains("---") {
            continue;
        }

        if trimmed.starts_with("# ") {
            if trimmed.contains("物品") { section = "items"; }
            else if trimmed.contains("事件") { section = "events"; }
            else if trimmed.contains("技能") { section = "skills"; }
            else if trimmed.contains("状态") { section = "statuses"; }
            else { section = ""; }
            continue;
        }

        if !trimmed.starts_with('|') || section.is_empty() {
            continue;
        }

        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .collect();

        match section {
            "items" => {
                if cells.len() >= 3 {
                    state.items.push(StateItem {
                        item: cells[0].to_string(),
                        qty: cells[1].parse().unwrap_or(0),
                        note: cells.get(2).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            "events" => {
                if cells.len() >= 2 {
                    state.events.push(StateEvent {
                        desc: cells[0].to_string(),
                        importance: cells.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            "skills" => {
                if cells.len() >= 3 {
                    state.skills.push(StateSkill {
                        skill: cells[0].to_string(),
                        desc: cells.get(1).map(|s| s.to_string()).unwrap_or_default(),
                        skill_type: cells.get(2).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            "statuses" => {
                if cells.len() >= 2 {
                    state.statuses.push(StateStatus {
                        status: cells[0].to_string(),
                        detail: cells.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            _ => {}
        }
    }

    state
}

/// Search state by keyword
pub fn search(state: &CharacterState, category: &str, keyword: &str) -> String {
    let kw = keyword.to_lowercase();
    let mut results = Vec::new();

    match category {
        "item" => {
            for item in &state.items {
                if kw.is_empty() || item.item.to_lowercase().contains(&kw) {
                    results.push(format!("- {} x{} ({})", item.item, item.qty, item.note));
                }
            }
        }
        "event" => {
            for event in &state.events {
                if kw.is_empty() || event.desc.to_lowercase().contains(&kw) {
                    results.push(format!("- [{}] {}", event.importance, event.desc));
                }
            }
        }
        "skill" => {
            for skill in &state.skills {
                if kw.is_empty() || skill.skill.to_lowercase().contains(&kw) {
                    results.push(format!("- {} ({}): {}", skill.skill, skill.skill_type, skill.desc));
                }
            }
        }
        "status" => {
            for status in &state.statuses {
                if kw.is_empty() || status.status.to_lowercase().contains(&kw) {
                    results.push(format!("- {}: {}", status.status, status.detail));
                }
            }
        }
        _ => {}
    }

    if results.is_empty() {
        format!("未找到 {} 中匹配 '{}' 的记录", category, keyword)
    } else {
        results.join("\n")
    }
}

/// Build a compact summary for system prompt injection
pub fn summary(state: &CharacterState) -> String {
    let mut parts = Vec::new();

    if !state.items.is_empty() {
        let names: Vec<_> = state.items.iter().map(|i| i.item.as_str()).collect();
        parts.push(format!("物品: {}", names.join(", ")));
    }
    if !state.statuses.is_empty() {
        let names: Vec<_> = state.statuses.iter().map(|s| format!("{}:{}", s.status, s.detail)).collect();
        parts.push(format!("状态: {}", names.join(", ")));
    }

    if parts.is_empty() {
        String::new()
    } else {
        parts.join(" | ")
    }
}

/// Manage state: add/update/delete operations
pub fn manage(
    state: &mut CharacterState,
    action: &str,
    category: &str,
    key: &str,
    data: &HashMap<String, serde_json::Value>,
) -> String {
    match (action, category) {
        ("add", "item") => {
            let qty = data.get("qty").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            let note = data.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
            state.items.push(StateItem { item: key.to_string(), qty, note });
            format!("已添加物品: {}", key)
        }
        ("add", "event") => {
            let desc = key.to_string();
            let importance = data.get("importance").and_then(|v| v.as_str()).unwrap_or("medium").to_string();
            state.events.push(StateEvent { desc, importance });
            format!("已记录事件: {}", key)
        }
        ("add", "skill") => {
            let desc = data.get("desc").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let stype = data.get("type").and_then(|v| v.as_str()).unwrap_or("passive").to_string();
            state.skills.push(StateSkill { skill: key.to_string(), desc, skill_type: stype });
            format!("已添加技能: {}", key)
        }
        ("add", "status") => {
            let detail = data.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string();
            state.statuses.push(StateStatus { status: key.to_string(), detail });
            format!("已更新状态: {}", key)
        }
        ("update", "item") => {
            if let Some(item) = state.items.iter_mut().find(|i| i.item == key) {
                if let Some(q) = data.get("qty").and_then(|v| v.as_i64()) { item.qty = q as i32; }
                if let Some(n) = data.get("note").and_then(|v| v.as_str()) { item.note = n.to_string(); }
                format!("已更新物品: {}", key)
            } else {
                format!("未找到物品: {}", key)
            }
        }
        ("update", "status") => {
            if let Some(s) = state.statuses.iter_mut().find(|s| s.status == key) {
                if let Some(d) = data.get("detail").and_then(|v| v.as_str()) { s.detail = d.to_string(); }
                format!("已更新状态: {}", key)
            } else {
                state.statuses.push(StateStatus { status: key.to_string(), detail: data.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string() });
                format!("已添加状态: {}", key)
            }
        }
        ("delete", category) => {
            match category {
                "item" => state.items.retain(|i| i.item != key),
                "event" => state.events.retain(|e| e.desc != key),
                "skill" => state.skills.retain(|s| s.skill != key),
                "status" => state.statuses.retain(|s| s.status != key),
                _ => {}
            }
            format!("已删除 {}: {}", category, key)
        }
        ("get", _) | ("search", _) => search(state, category, key),
        _ => format!("不支持的操作: {} {}", action, category),
    }
}
