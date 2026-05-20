//! sys_skill/ 目录加载器
//!
//! 读取 sys_skill/ 下的所有 .md 文件，
//! 拼接为 LLM system prompt 的补充内容。
//! 这些 prompt 教 LLM 何时、如何调用 manage_state / advance_time 工具。

use std::fs;
use std::path::Path;

/// 加载 sys_skill/ 目录下所有 .md 文件，拼接为技能引导文本
pub fn load() -> String {
    let dir = Path::new("sys_skill");
    if !dir.is_dir() {
        return String::new();
    }

    // 优先加载 tool_guide.md，因为它提供核心原则
    let mut combined = String::new();

    let files: Vec<_> = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return String::new(),
    };

    // 排序：tool_guide 排第一，其余按字母序
    let mut guide_content = String::new();
    let mut others: Vec<String> = Vec::new();

    for entry in &files {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            let filename = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if filename == "tool_guide" {
                guide_content = content;
            } else {
                let title = filename_to_title(filename);
                others.push(format!("## {}\n\n{}", title, content));
            }
        }
    }

    if guide_content.is_empty() && others.is_empty() {
        return String::new();
    }

    // 组装：tool_guide 在前，其他在后
    if !guide_content.is_empty() {
        combined.push_str(&guide_content);
        combined.push_str("\n\n---\n\n");
    }
    combined.push_str(&others.join("\n\n---\n\n"));

    combined
}

/// 将文件名转换为中文标题（用于 section header）
fn filename_to_title(filename: &str) -> &str {
    match filename {
        "tool_guide" => "工具调用总纲",
        "item_patterns" => "物品管理模式",
        "state_patterns" => "状态管理模式",
        "time_patterns" => "时间推进模式",
        "location_patterns" => "地点追踪模式",
        "constraints" => "工具调用约束",
        _ => filename,
    }
}
