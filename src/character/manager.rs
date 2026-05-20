use crate::db;
use crate::tui::app::Message;
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CharacterState {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub personality: String,
    pub speech_style: String,
    pub first_message: String,
    pub background: String,
    pub system_prompt: String,
    pub messages: Vec<Message>,
}

pub struct CharacterManager {
    pub all_characters: HashMap<String, CharacterState>,
    pub all_order: Vec<String>,
    pub characters: HashMap<String, CharacterState>,
    pub order: Vec<String>,
    pub active_index: usize,
    pub worlds: Vec<WorldInfo>,
    pub active_world: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct WorldInfo {
    pub id: i64,
    pub slug: String,
    pub name: String,
}

impl CharacterManager {
    pub fn load_all(conn: &Connection, active_slug: &str, world_slug: Option<&str>) -> anyhow::Result<Self> {
        let rows = db::store::list_characters(conn)?;
        if rows.is_empty() {
            anyhow::bail!("No characters in database");
        }

        let mut all_characters = HashMap::new();
        let mut all_order = Vec::new();

        for row in &rows {
            let sp = build_system_prompt(row);
            let mut msgs = Vec::new();
            if !row.first_message.is_empty() {
                msgs.push(Message { role: "assistant".into(), content: row.first_message.clone() });
            }
            all_characters.insert(row.slug.clone(), CharacterState {
                id: row.id, slug: row.slug.clone(), name: row.name.clone(),
                personality: row.personality.clone(), speech_style: row.speech_style.clone(),
                first_message: row.first_message.clone(), background: row.background.clone(),
                system_prompt: sp, messages: msgs,
            });
            all_order.push(row.slug.clone());
        }
        all_order.sort();

        // Load worlds
        let worlds: Vec<WorldInfo> = db::store::list_worlds(conn).unwrap_or_default().into_iter().map(|w| WorldInfo { id: w.id, slug: w.slug, name: w.name }).collect();

        // Determine active world
        let active_world = world_slug.and_then(|ws| worlds.iter().position(|w| w.slug == ws));

        // Filter characters by world
        let (order, characters) = Self::filter_characters(conn, &all_order, &all_characters, active_world.map(|i| worlds[i].id))?;

        let active_index = order.iter().position(|n| n == active_slug).unwrap_or(0);

        Ok(Self { all_characters, all_order, characters, order, active_index, worlds, active_world })
    }

    fn filter_characters(
        conn: &Connection,
        all_order: &[String],
        all_chars: &HashMap<String, CharacterState>,
        world_id: Option<i64>,
    ) -> anyhow::Result<(Vec<String>, HashMap<String, CharacterState>)> {
        match world_id {
            Some(wid) => {
                let linked: Vec<i64> = db::store::get_world_characters(conn, wid)?;
                let mut order = Vec::new();
                let mut chars = HashMap::new();
                for slug in all_order {
                    if let Some(c) = all_chars.get(slug) {
                        if linked.contains(&c.id) {
                            order.push(slug.clone());
                            chars.insert(slug.clone(), c.clone());
                        }
                    }
                }
                if order.is_empty() {
                    return Ok((all_order.to_vec(), all_chars.clone()));
                }
                Ok((order, chars))
            }
            None => Ok((all_order.to_vec(), all_chars.clone())),
        }
    }

    pub fn switch_world(&mut self, conn: &Connection, index: Option<usize>) {
        self.active_world = index;
        let world_id = index.map(|i| self.worlds[i].id);
        if let Ok((order, chars)) = Self::filter_characters(conn, &self.all_order, &self.all_characters, world_id) {
            self.order = order;
            self.characters = chars;
            if self.active_index >= self.order.len() { self.active_index = 0; }
        }
    }

    pub fn active_world_name(&self) -> Option<&str> {
        self.active_world.map(|i| self.worlds[i].slug.as_str())
    }

    pub fn active_name(&self) -> &str { &self.order[self.active_index] }
    pub fn active(&self) -> &CharacterState { &self.characters[self.active_name()] }
    pub fn active_mut(&mut self) -> &mut CharacterState {
        let name = self.active_name().to_string();
        self.characters.get_mut(&name).unwrap()
    }
    pub fn switch_to_name(&mut self, slug: &str) -> bool {
        if let Some(pos) = self.order.iter().position(|n| n == slug) { self.active_index = pos; true } else { false }
    }
    pub fn next(&mut self) { self.active_index = (self.active_index + 1) % self.order.len(); }
    pub fn prev(&mut self) {
        if self.active_index == 0 { self.active_index = self.order.len() - 1; }
        else { self.active_index -= 1; }
    }
}

fn build_system_prompt(row: &db::store::CharacterRow) -> String {
    let mut prompt = format!(
        "你是一个角色扮演助手。请完全沉浸入以下角色的设定中，用角色的口吻回复。\n\n\
         【角色名】{}\n\n【性格】{}\n\n【说话风格】{}\n\n【开场白】{}\n\n\
         【重要规则】\n- 始终保持角色，不要跳出角色说话\n\
         - 不要代替用户说话或替用户做决定\n\
         - 回复时只输出角色的对话和动作/场景描写\n\
         - 动作描写用括号包裹，如 (放下酒杯)\n",
        row.name, row.personality, row.speech_style, row.first_message,
    );
    if !row.background.is_empty() {
        prompt.push_str(&format!("\n【背景知识】\n{}\n", row.background));
    }
    prompt
}
