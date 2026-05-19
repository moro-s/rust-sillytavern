use super::entry::LoreEntry;

/// Scan text for trigger keywords and return activated entries.
/// Returns unique entries sorted by priority (highest first).
pub fn match_entries<'a>(entries: &'a [LoreEntry], text: &str) -> Vec<&'a LoreEntry> {
    let text_lower = text.to_lowercase();
    let mut matched: Vec<&LoreEntry> = Vec::new();

    for entry in entries {
        if !entry.enabled || entry.triggers.is_empty() {
            continue;
        }

        let triggered = entry.triggers.iter().any(|trigger| {
            text_lower.contains(&trigger.to_lowercase())
        });

        if triggered {
            matched.push(entry);
        }
    }

    // Deduplicate by key
    let mut seen = std::collections::HashSet::new();
    matched.retain(|e| seen.insert(&e.key));

    // Sort by priority descending
    matched.sort_by(|a, b| b.priority.cmp(&a.priority));

    matched
}

/// Build lorebook context text from activated entries.
/// Respects `selective` flag: if any selective entry matches, only the
/// highest-priority selective entry is used.
pub fn build_context(entries: &[&LoreEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let selective: Vec<&&LoreEntry> = entries.iter().filter(|e| e.selective).collect();

    if !selective.is_empty() {
        // Only use the highest priority selective entry
        let entry = selective[0];
        return format!("[世界信息: {}]\n{}\n\n", entry.key, entry.content);
    }

    // Use all non-selective entries
    let mut ctx = String::new();
    for entry in entries.iter().filter(|e| !e.selective) {
        ctx.push_str(&format!("[世界信息: {}]\n{}\n\n", entry.key, entry.content));
    }
    ctx
}
