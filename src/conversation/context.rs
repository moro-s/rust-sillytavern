use crate::llm::ChatMessage;
use crate::lorebook;

/// Build the full context for an LLM request.
pub fn build(
    system_prompt: &str,
    history: &[ChatMessage],
    lore_entries: &[&lorebook::entry::LoreEntry],
    max_history: usize,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let lore_text = lorebook::matcher::build_context(&lore_entries);

    // System prompt with lorebook injected
    let mut system = system_prompt.to_string();
    if !lore_text.is_empty() {
        system.push_str("\n\n---\n【当前世界信息】\n");
        system.push_str(&lore_text);
    }
    messages.push(ChatMessage {
        role: "system".into(),
        content: system,
    });

    // Recent history
    let recent = history.iter().rev().take(max_history).rev();
    messages.extend(recent.cloned());

    messages
}
