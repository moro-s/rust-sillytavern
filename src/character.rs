use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CharacterMeta {
    pub name: String,
    pub personality: String,
    pub speech_style: String,
    pub first_message: String,
}

#[derive(Debug, Clone)]
pub struct CharacterCard {
    pub meta: CharacterMeta,
    pub body: String,
}

pub fn load(name: &str) -> anyhow::Result<CharacterCard> {
    let path = format!("characters/{name}.md");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Character card not found: {path}"))?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 2 {
        return load_simple(name, &content);
    }

    let yaml_str = parts[1].trim();
    let body = if parts.len() > 2 { parts[2].trim().to_string() } else { String::new() };

    let meta: CharacterMeta = serde_yaml::from_str(yaml_str)
        .with_context(|| format!("Invalid YAML frontmatter in {path}"))?;

    Ok(CharacterCard { meta, body })
}

fn load_simple(name: &str, content: &str) -> anyhow::Result<CharacterCard> {
    Ok(CharacterCard {
        meta: CharacterMeta {
            name: name.to_string(),
            personality: String::new(),
            speech_style: String::new(),
            first_message: String::new(),
        },
        body: content.trim().to_string(),
    })
}

pub fn build_system_prompt(card: &CharacterCard) -> String {
    let m = &card.meta;
    let mut prompt = format!(
        "你是一个角色扮演助手。请完全沉浸入以下角色的设定中，用角色的口吻回复。\n\n\
         【角色名】{}\n\n\
         【性格】{}\n\n\
         【说话风格】{}\n\n\
         【开场白】{}\n\n\
         【重要规则】\n\
         - 始终保持角色，不要跳出角色说话\n\
         - 不要代替用户说话或替用户做决定\n\
         - 回复时只输出角色的对话和动作/场景描写\n\
         - 动作描写用括号包裹，如 (放下酒杯)\n",
        m.name, m.personality, m.speech_style, m.first_message,
    );

    if !card.body.is_empty() {
        prompt.push_str(&format!("\n【背景知识】\n{}\n", card.body));
    }

    prompt
}
