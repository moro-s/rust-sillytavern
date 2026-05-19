use crate::character::{build_system_prompt, load as load_card, CharacterCard};
use crate::tui::app::Message;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CharacterState {
    pub card: CharacterCard,
    pub system_prompt: String,
    pub messages: Vec<Message>,
}

pub struct CharacterManager {
    pub characters: HashMap<String, CharacterState>,
    pub order: Vec<String>,   // display order
    pub active_index: usize,
}

impl CharacterManager {
    /// Load all characters from `characters/` directory.
    /// If `active` is given, set that character as the active one.
    pub fn load_all(active: &str) -> anyhow::Result<Self> {
        let mut characters = HashMap::new();
        let mut order = Vec::new();

        if let Ok(entries) = std::fs::read_dir("characters") {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        match load_card(stem) {
                            Ok(card) => {
                                let system_prompt = build_system_prompt(&card);
                                let mut messages = Vec::new();
                                if !card.meta.first_message.is_empty() {
                                    messages.push(Message {
                                        role: "assistant".into(),
                                        content: card.meta.first_message.clone(),
                                    });
                                }
                                characters.insert(stem.to_string(), CharacterState {
                                    card,
                                    system_prompt,
                                    messages,
                                });
                                order.push(stem.to_string());
                            }
                            Err(e) => {
                                eprintln!("Warning: failed to load character '{}': {}", stem, e);
                            }
                        }
                    }
                }
            }
        }

        order.sort();

        if characters.is_empty() {
            anyhow::bail!("No characters found in characters/ directory");
        }

        let active_index = order.iter().position(|n| n == active).unwrap_or(0);

        Ok(Self {
            characters,
            order,
            active_index,
        })
    }

    pub fn active_name(&self) -> &str {
        &self.order[self.active_index]
    }

    pub fn active(&self) -> &CharacterState {
        &self.characters[self.active_name()]
    }

    pub fn active_mut(&mut self) -> &mut CharacterState {
        let name = self.active_name().to_string();
        self.characters.get_mut(&name).unwrap()
    }

    pub fn switch_to_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self.order.iter().position(|n| n == name) {
            self.active_index = pos;
            true
        } else {
            false
        }
    }

    pub fn next(&mut self) {
        self.active_index = (self.active_index + 1) % self.order.len();
    }

    pub fn prev(&mut self) {
        if self.active_index == 0 {
            self.active_index = self.order.len() - 1;
        } else {
            self.active_index -= 1;
        }
    }

    /// Look up character info for @mention
    pub fn lookup(&self, name: &str) -> Option<&CharacterCard> {
        self.characters.get(name).map(|s| &s.card)
    }
}
