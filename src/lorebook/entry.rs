use serde::Deserialize;
use std::collections::HashMap;

/// Where to insert the lorebook entry in the context
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    /// Before the system prompt (closest to the top)
    SystemTop,
    /// After system prompt, before chat history
    SystemBottom,
    /// Before the last user message
    BeforeUser,
}

impl Default for Position {
    fn default() -> Self {
        Self::SystemBottom
    }
}

/// A single lorebook entry
#[derive(Debug, Clone, Deserialize)]
pub struct LoreEntry {
    /// Unique identifier
    pub key: String,
    /// Keywords that trigger this entry
    #[serde(default)]
    pub triggers: Vec<String>,
    /// The world information text
    pub content: String,
    /// Priority (higher = more important, inserted first)
    #[serde(default)]
    pub priority: i32,
    /// Where to insert in the context
    #[serde(default)]
    #[allow(dead_code)]
    pub position: Position,
    /// Whether this entry is currently enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// If true, only the highest-priority matching entry is used
    #[serde(default)]
    pub selective: bool,
}

fn default_true() -> bool {
    true
}

/// Load all lorebook entries from a directory
pub fn load_all(dir: &str) -> Vec<LoreEntry> {
    let mut entries = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(dir) {
        for entry in dir_entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "toml") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        match toml::from_str::<LoreEntry>(&content) {
                            Ok(mut entry) => {
                                if entry.key.is_empty() {
                                    // Use filename as key
                                    entry.key = path.file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                }
                                entries.push(entry);
                            }
                            Err(e) => {
                                log::warn!("无法解析 {}: {}", path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("无法读取 {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
    entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    entries
}

/// Lorebook manager
pub struct LoreManager {
    pub entries: Vec<LoreEntry>,
    pub active_keys: Vec<String>, // currently activated entry keys
    /// File modification times for hot-reload
    mod_times: HashMap<String, std::time::SystemTime>,
}

impl LoreManager {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            active_keys: Vec::new(),
            mod_times: HashMap::new(),
        }
    }

    /// Load entries from directory
    pub fn load(&mut self, dir: &str) {
        self.entries = load_all(dir);
        self.record_mod_times(dir);
    }

    fn record_mod_times(&mut self, dir: &str) {
        self.mod_times.clear();
        if let Ok(dir_entries) = std::fs::read_dir(dir) {
            for entry in dir_entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "toml") {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(time) = meta.modified() {
                            if let Some(name) = path.to_str() {
                                self.mod_times.insert(name.to_string(), time);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if any files changed and reload
    pub fn check_hot_reload(&mut self, dir: &str) -> bool {
        if let Ok(dir_entries) = std::fs::read_dir(dir) {
            let mut changed = false;
            for entry in dir_entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "toml") {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(time) = meta.modified() {
                            let path_str = path.to_str().unwrap_or("");
                            if self.mod_times.get(path_str) != Some(&time) {
                                changed = true;
                                break;
                            }
                        }
                    }
                }
            }
            if changed {
                self.load(dir);
                return true;
            }
        }
        false
    }
}
