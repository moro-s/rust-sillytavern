use crate::character::manager::CharacterManager;
use crate::command;
use crate::config;
use crate::conversation;
use crate::db;
use crate::llm;
use crate::llm::StreamEvent;
use crate::tui::selector;
use crate::tui::ui;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use rusqlite::Connection;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct App {
    pub manager: CharacterManager,
    pub db: Connection,
    pub session_id: i64,
    pub save_counter: usize,
    pub input: String,
    pub cursor_pos: usize,
    pub loading: bool,
    pub scroll_offset: usize,
    pub error: Option<String>,
    pub show_help: bool,
    pub streaming: String,
    pub is_streaming: bool,
    pub cancel_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub should_quit: bool,
    pub self_persona: String,
    pub world_id: Option<i64>,
    pub active_lore_keys: Vec<String>,
    pub wizard: Option<Wizard>,
}

#[derive(Debug, Clone)]
pub enum Wizard {
    CreateWorld { slug: String, name: String, step: u8, description: String },
    CreateChar  { slug: String, step: u8, name: String, personality: String, speech_style: String },
}

fn build_lore_text(entries: &[&db::store::LoreRow]) -> String {
    if entries.is_empty() { return String::new(); }
    let selective: Vec<_> = entries.iter().filter(|e| e.selective).collect();
    if !selective.is_empty() {
        return format!("[世界信息: {}]\n{}\n\n", selective[0].key, selective[0].content);
    }
    let mut ctx = String::new();
    for entry in entries.iter().filter(|e| !e.selective) {
        ctx.push_str(&format!("[世界信息: {}]\n{}\n\n", entry.key, entry.content));
    }
    ctx
}

impl App {
    pub fn new(
        character_name: &str,
        world: Option<&str>,
        resume_id: Option<i64>,
        new_session: bool,
    ) -> anyhow::Result<Self> {
        let db = db::schema::open()?;
        let mut manager = CharacterManager::load_all(&db, character_name, world)?;
        let self_persona = db::store::get_persona(&db)?;

        // Session handling
        let (session_id, save_counter) = if let Some(id) = resume_id {
            match db::store::get_session(&db, id) {
                Ok(Some(_)) => {
                    match db::store::load_messages(&db, id) {
                        Ok(msgs) => {
                            let active = manager.active_name().to_string();
                            if let Some(state) = manager.characters.get_mut(&active) {
                                state.messages = msgs;
                            }
                        }
                        Err(_) => {}
                    }
                    (id, 0)
                }
                _ => (create_new_session(&db, &manager, world)?, 0),
            }
        } else if new_session {
            (create_new_session(&db, &manager, world)?, 0)
        } else {
            let id = match db::store::list_sessions(&db) {
                Ok(sessions) => {
                    let last = sessions.iter().find(|s| s.character_name == manager.active().name);
                    match last {
                        Some(s) => match db::store::load_messages(&db, s.id) {
                            Ok(msgs) => {
                                let active = manager.active_name().to_string();
                                if let Some(state) = manager.characters.get_mut(&active) {
                                    state.messages = msgs;
                                }
                                s.id
                            }
                            Err(_) => create_new_session(&db, &manager, world)?,
                        },
                        None => create_new_session(&db, &manager, world)?,
                    }
                }
                Err(_) => create_new_session(&db, &manager, world)?,
            };
            (id, 0)
        };

        Ok(Self {
            manager,
            db,
            session_id,
            save_counter,
            input: String::new(),
            cursor_pos: 0,
            loading: false,
            scroll_offset: 0,
            error: None,
            show_help: false,
            streaming: String::new(),
            is_streaming: false,
            cancel_tx: None,
            should_quit: false,
            self_persona,
            world_id: None,
            active_lore_keys: Vec::new(),
            wizard: None,
        })
    }

    // ── Input helpers ──
    fn byte_pos(&self) -> usize {
        self.input.char_indices().nth(self.cursor_pos).map(|(i,_)| i).unwrap_or(self.input.len())
    }
    fn char_count(&self) -> usize { self.input.chars().count() }
    fn insert_char(&mut self, c: char) {
        let pos = self.byte_pos(); self.input.insert(pos, c); self.cursor_pos += 1;
    }
    fn remove_char_before(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let start = self.byte_pos();
            let end = self.input[start..].chars().next().map(|ch| start + ch.len_utf8()).unwrap_or(start);
            self.input.drain(start..end);
        }
    }
    fn remove_char_at(&mut self) {
        if self.cursor_pos < self.char_count() {
            let pos = self.byte_pos();
            let end = self.input[pos..].chars().next().map(|ch| pos + ch.len_utf8()).unwrap_or(pos);
            self.input.drain(pos..end);
        }
    }
    pub fn scroll_up(&mut self, amount: usize) { self.scroll_offset = self.scroll_offset.saturating_add(amount); }
    pub fn scroll_down(&mut self, amount: usize) { self.scroll_offset = self.scroll_offset.saturating_sub(amount); }
    pub fn scroll_to_bottom(&mut self) { self.scroll_offset = 0; }

    // ── Send / Command ──
    fn send_message(&mut self) {
        let input_text = std::mem::take(&mut self.input);
        self.cursor_pos = 0;

        // Handle wizard input first
        if self.wizard.is_some() {
            self.handle_wizard_input(&input_text);
            return;
        }

        let (cmd, content) = command::parser::parse(&input_text);
        match cmd {
            command::parser::Command::Quit => { self.save_current(); self.should_quit = true; return; }
            command::parser::Command::Help => { self.show_help = true; return; }
            command::parser::Command::Clear => {
                let first_msg = self.manager.active().first_message.clone();
                self.manager.active_mut().messages.clear();
                if !first_msg.is_empty() {
                    self.manager.active_mut().messages.push(Message { role: "assistant".into(), content: first_msg });
                }
                self.scroll_offset = 0;
                return;
            }
            command::parser::Command::Switch(name) => {
                let name = name.trim().to_string();
                if !name.is_empty() && self.manager.switch_to_name(&name) { self.scroll_offset = 0; }
                else if !name.is_empty() { self.error = Some(format!("角色 '{}' 不存在", name)); }
                return;
            }
            command::parser::Command::Save => { self.save_current(); return; }
            command::parser::Command::Load(id_str) => { self.load_session(&id_str); return; }
            command::parser::Command::CreateChar(name) => { self.handle_create_char(&name); return; }
            command::parser::Command::CreateWorld(name) => { self.handle_create_world(&name); return; }
            command::parser::Command::SetSelf(text) => { self.handle_set_self(&text); return; }
            command::parser::Command::ManageState(args) => { self.handle_manage_state(&args); return; }
            command::parser::Command::Export => { self.handle_export(); return; }
            command::parser::Command::Import(args) => { self.handle_import(&args); return; }
            command::parser::Command::World(name) => { self.handle_switch_world(&name); return; }
            command::parser::Command::Link(char_name, world_name) => { self.handle_link(&char_name, &world_name); return; }
            command::parser::Command::Location(args) => { self.handle_location(&args); return; }
            command::parser::Command::Time(args) => { self.handle_time(&args); return; }
            command::parser::Command::Timeline => { self.handle_timeline(); return; }
            command::parser::Command::Info(name) => {
                if let Some(c) = db::store::get_character(&self.db, name.trim()).ok().flatten() {
                    self.manager.active_mut().messages.push(Message {
                        role: "system".into(),
                        content: format!("角色: {}\n性格: {}\n说话风格: {}", c.name, c.personality, c.speech_style),
                    });
                } else { self.error = Some(format!("角色 '{}' 不存在", name)); }
                return;
            }
            command::parser::Command::List => {
                let list = self.manager.order.iter().map(|n| format!("- {}", n)).collect::<Vec<_>>().join("\n");
                self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("可用角色:\n{}", list) });
                return;
            }
            command::parser::Command::None => {}
        }

        // Expand @mentions (simplified: lookup by slug)
        let expanded = command::parser::expand_mentions(&content, |name| {
            db::store::get_character(&self.db, name).ok().flatten().map(|c| crate::character::CharacterCard {
                meta: crate::character::CharacterMeta {
                    name: c.name, personality: c.personality, speech_style: c.speech_style, first_message: c.first_message,
                },
                body: c.background,
            })
        });

        if expanded.trim().is_empty() { return; }
        self.manager.active_mut().messages.push(Message { role: "user".into(), content: expanded });
        self.save_counter += 1;
        self.loading = true; self.is_streaming = true; self.streaming.clear();
        self.error = None; self.scroll_offset = 0;
        let (cancel_tx, _) = tokio::sync::watch::channel(false);
        self.cancel_tx = Some(cancel_tx);
    }

    // ── Save / Load ──
    pub fn save_current(&mut self) {
        let msgs = &self.manager.active().messages;
        match db::store::save_messages(&self.db, self.session_id, msgs) {
            Ok(()) => { self.save_counter = 0; self.error = Some("已保存".into()); }
            Err(e) => { self.error = Some(format!("保存失败: {}", e)); }
        }
    }

    pub fn load_session(&mut self, id_str: &str) {
        match id_str.trim().parse::<i64>() {
            Ok(id) => match db::store::load_messages(&self.db, id) {
                Ok(msgs) => {
                    if !msgs.is_empty() {
                        self.manager.active_mut().messages = msgs;
                        self.session_id = id; self.save_counter = 0; self.scroll_offset = 0;
                        self.error = Some(format!("已加载会话 {}", id));
                    } else { self.error = Some(format!("会话 {} 无消息", id)); }
                }
                Err(e) => self.error = Some(format!("加载失败: {}", e)),
            },
            Err(_) => self.error = Some(format!("无效ID: {}", id_str)),
        }
    }

    pub fn try_autosave(&mut self) {
        if self.save_counter >= 3 {
            let msgs = &self.manager.active().messages;
            if db::store::save_messages(&self.db, self.session_id, msgs).is_ok() { self.save_counter = 0; }
        }
    }

    // ── Create ──
    pub fn handle_create_char(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() { self.error = Some("用法: /cc <角色名>".into()); return; }
        let slug = name.to_string();
        if db::store::get_character(&self.db, &slug).ok().flatten().is_some() {
            self.error = Some(format!("角色 '{}' 已存在", name)); return;
        }
        self.wizard = Some(Wizard::CreateChar { slug, step: 0, name: name.to_string(), personality: String::new(), speech_style: String::new() });
    }

    pub fn handle_create_world(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() { self.error = Some("用法: /cw <世界名>".into()); return; }
        let slug = name.to_string();
        if db::store::get_world(&self.db, &slug).ok().flatten().is_some() {
            self.error = Some(format!("世界 '{}' 已存在", name)); return;
        }
        self.wizard = Some(Wizard::CreateWorld { slug, name: name.to_string(), step: 0, description: String::new() });
    }

    fn handle_wizard_input(&mut self, input: &str) {
        let wizard = self.wizard.take();
        match wizard {
            Some(Wizard::CreateWorld { slug, name, step, description }) => {
                let input = input.trim().to_string();
                match step {
                    0 => {
                        self.error = Some(format!("世界名: {}\n\n请输入一句话描述（可选，回车跳过）:", name));
                        self.wizard = Some(Wizard::CreateWorld { slug, name, step: 1, description: input });
                    }
                    1 => {
                        let desc = if description.is_empty() { "未填写".into() } else { description };
                        let row = db::store::WorldRow { id: 0, slug: slug.clone(), name: name.clone(), description: desc, overview: input };
                        match db::store::create_world(&self.db, &row) {
                            Ok(_) => self.error = Some(format!("世界 '{}' 已创建", name)),
                            Err(e) => self.error = Some(format!("创建失败: {}", e)),
                        }
                    }
                    _ => {}
                }
            }
            Some(Wizard::CreateChar { slug, step, name, personality, speech_style }) => {
                let input = input.trim().to_string();
                match step {
                    0 => {
                        self.error = Some(format!("角色名: {}\n\n请输入性格描述:", name));
                        self.wizard = Some(Wizard::CreateChar { slug, step: 1, name: input, personality: String::new(), speech_style: String::new() });
                    }
                    1 => {
                        self.error = Some(format!("角色名: {}\n性格: {}\n\n请输入说话风格:", name, input));
                        self.wizard = Some(Wizard::CreateChar { slug, step: 2, name, personality: input, speech_style: String::new() });
                    }
                    2 => {
                        self.error = Some(format!("角色名: {}\n性格: {}\n说话风格: {}\n\n请输入开场白:", name, personality, input));
                        self.wizard = Some(Wizard::CreateChar { slug, step: 3, name, personality, speech_style: input });
                    }
                    3 => {
                        let row = db::store::CharacterRow {
                            id: 0, slug: slug.clone(), name: name.clone(),
                            personality, speech_style,
                            first_message: input, background: String::new(),
                        };
                        match db::store::create_character(&self.db, &row) {
                            Ok(_) => {
                                let active = self.manager.active_name().to_string();
                                if let Ok(m) = CharacterManager::load_all(&self.db, &active, self.manager.active_world_name()) { self.manager = m; }
                                self.error = Some(format!("角色 '{}' 已创建，可用 /self 设定背景", name));
                            }
                            Err(e) => self.error = Some(format!("创建失败: {}", e)),
                        }
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }

    // ── Self ──
    pub fn handle_set_self(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            let current = if self.self_persona.is_empty() { "未设置".to_string() } else { format!("当前: {}", self.self_persona) };
            self.error = Some(format!("用法: /self <设定>\n{}", current));
            return;
        }
        self.self_persona = text.to_string();
        let _ = db::store::set_persona(&self.db, text);
        self.error = Some(format!("已更新: {}", text));
    }

    // ── State ──
    pub fn handle_manage_state(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(4, ' ').collect();
        let action = parts.first().map(|s| *s).unwrap_or("get");
        let category = parts.get(1).map(|s| *s).unwrap_or("item");
        let key = parts.get(2).map(|s| *s).unwrap_or("");
        let data_str = parts.get(3).map(|s| *s).unwrap_or("");
        let char_id = self.manager.active().id;
        match db::store::manage_state(&self.db, "character_states", char_id, action, category, key, data_str) {
            Ok(result) => self.error = Some(result),
            Err(e) => self.error = Some(format!("状态操作失败: {}", e)),
        }
    }

    /// Switch to a world by name (or clear world filter)
    pub fn handle_switch_world(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() || name == "none" {
            self.manager.switch_world(&self.db, None);
            self.error = Some("已切换到全部角色".into());
            return;
        }
        if let Some(idx) = self.manager.worlds.iter().position(|w| w.slug == name) {
            self.manager.switch_world(&self.db, Some(idx));
            self.error = Some(format!("已切换到世界: {}", name));
        } else {
            self.error = Some(format!("世界 '{}' 不存在", name));
        }
    }

    /// Link a character to a world
    pub fn handle_link(&mut self, char_name: &str, world_name: &str) {
        let (char_name, world_name) = (char_name.trim(), world_name.trim());
        if char_name.is_empty() || world_name.is_empty() {
            self.error = Some("用法: /link <角色slug> <世界slug>".into());
            return;
        }
        let char = match db::store::get_character(&self.db, char_name).ok().flatten() {
            Some(c) => c,
            None => { self.error = Some(format!("角色 '{}' 不存在", char_name)); return; }
        };
        let world = match db::store::get_world(&self.db, world_name).ok().flatten() {
            Some(w) => w,
            None => { self.error = Some(format!("世界 '{}' 不存在", world_name)); return; }
        };
        match db::store::link_character_world(&self.db, char.id, world.id, "") {
            Ok(_) => {
                // Reload manager
                let active = self.manager.active_name().to_string();
                if let Ok(m) = CharacterManager::load_all(&self.db, &active, self.manager.active_world_name()) { self.manager = m; }
                self.error = Some(format!("已将 '{}' 关联到世界 '{}'", char_name, world_name));
            }
            Err(e) => self.error = Some(format!("关联失败: {}", e)),
        }
    }

    /// Advance world timeline: /time <label> [description]
    pub fn handle_location(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(3, ' ').collect();
        let action = parts.first().map(|s| *s).unwrap_or("list");
        let world_slug = parts.get(1).map(|s| *s).unwrap_or("");
        let rest = parts.get(2).map(|s| *s).unwrap_or("");
        match action {
            "add" | "create" => {
                if world_slug.is_empty() || rest.is_empty() { self.error = Some("用法: /location add <世界> <地点>".into()); return; }
                if let Ok(Some(world)) = db::store::get_world(&self.db, world_slug) {
                    let row = db::store::LocationRow { id: 0, slug: rest.to_string(), name: rest.to_string(), description: String::new(), connects_to: String::new(), world_id: world.id };
                    match db::store::create_location(&self.db, &row) {
                        Ok(_) => self.error = Some(format!("地点 '{}' 已创建", rest)),
                        Err(e) => self.error = Some(format!("创建失败: {}", e)),
                    }
                } else { self.error = Some(format!("世界 '{}' 不存在", world_slug)); }
            }
            "list" | "ls" => {
                let ws = if world_slug.is_empty() { self.manager.active_world_name().map(|s| s.to_string()) } else { Some(world_slug.to_string()) };
                if let Some(ws) = ws {
                    if let Ok(Some(world)) = db::store::get_world(&self.db, &ws) {
                        if let Ok(locs) = db::store::list_locations(&self.db, world.id) {
                            if locs.is_empty() { self.error = Some(format!("世界 '{}' 暂无地点", ws)); }
                            else {
                                let list = locs.iter().map(|l| format!("- {}", l.name)).collect::<Vec<_>>().join("\n");
                                self.manager.active_mut().messages.push(Message { role: "system".into(), content: format!("世界 '{}':\n{}", ws, list) });
                                self.scroll_offset = 0;
                            }
                        }
                    }
                } else { self.error = Some("请激活世界或 /location list <世界>".into()); }
            }
            _ => { self.error = Some("用法: /location add <世界> <地点> 或 /location list".into()); }
        }
    }

    /// Advance world timeline: /time <label> [description]
    pub fn handle_time(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let label = parts.first().map(|s| *s).unwrap_or("");
        let desc = parts.get(1).map(|s| *s).unwrap_or("");

        if label.is_empty() {
            self.error = Some("用法: /time <标签> [描述]  如: /time 第2天早晨".into());
            return;
        }

        let world_id = match self.manager.active_world {
            Some(i) => self.manager.worlds[i].id,
            None => { self.error = Some("请先激活一个世界".into()); return; }
        };

        match db::store::advance_timeline(&self.db, world_id, label, desc) {
            Ok(_) => self.error = Some(format!("时间推进到: {}", label)),
            Err(e) => self.error = Some(format!("时间推进失败: {}", e)),
        }
    }

    /// Show current world timeline
    pub fn handle_timeline(&mut self) {
        let world_id = match self.manager.active_world {
            Some(i) => self.manager.worlds[i].id,
            None => { self.error = Some("请先激活一个世界".into()); return; }
        };

        match db::store::list_timeline(&self.db, world_id) {
            Ok(entries) => {
                if entries.is_empty() {
                    self.error = Some("时间线为空，使用 /time 添加时刻".into());
                } else {
                    let text = entries.iter()
                        .map(|e| format!("  {} - {}", e.time_label, e.description))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let current = entries.last().unwrap();
                    self.manager.active_mut().messages.push(Message {
                        role: "system".into(),
                        content: format!("世界时间线 (当前: {}):\n{}", current.time_label, text),
                    });
                    self.scroll_offset = 0;
                }
            }
            Err(e) => self.error = Some(format!("读取时间线失败: {}", e)),
        }
    }

    /// Import from .md files: /import <type> <slug> <path>
    /// Types: char, world, lore
    pub fn handle_import(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(3, ' ').collect();
        let import_type = parts.first().map(|s| *s).unwrap_or("");
        let slug = parts.get(1).map(|s| *s).unwrap_or("");
        let path = parts.get(2).map(|s| *s).unwrap_or("");

        if import_type.is_empty() || slug.is_empty() {
            self.error = Some("用法: /import char <slug> <path.md> 或 /import world <slug> <path.md> 或 /import lore <key> <path.md>".into());
            return;
        }

        let file_path = if path.is_empty() {
            match import_type {
                "char" | "character" => format!("characters/{}.md", slug),
                "world" => format!("worlds/{}/world.md", slug),
                "lore" => format!("lorebooks/{}.md", slug),
                _ => { self.error = Some(format!("未知类型: {}", import_type)); return; }
            }
        } else { path.to_string() };

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => { self.error = Some(format!("无法读取 {}: {}", file_path, e)); return; }
        };

        match import_type {
            "char" | "character" => {
                let (meta, body) = parse_md_frontmatter(&content);
                let row = db::store::CharacterRow {
                    id: 0, slug: slug.to_string(),
                    name: meta.get("name").unwrap_or(&slug.to_string()).clone(),
                    personality: meta.get("personality").unwrap_or(&String::new()).clone(),
                    speech_style: meta.get("speech_style").unwrap_or(&String::new()).clone(),
                    first_message: meta.get("first_message").unwrap_or(&String::new()).clone(),
                    background: body,
                };
                match db::store::create_character(&self.db, &row) {
                    Ok(_) => {
                        let active = self.manager.active_name().to_string();
                        if let Ok(m) = CharacterManager::load_all(&self.db, &active, self.manager.active_world_name()) { self.manager = m; }
                        self.error = Some(format!("角色 '{}' 已导入", slug));
                    }
                    Err(e) => self.error = Some(format!("导入失败: {}", e)),
                }
            }
            "world" => {
                let (meta, body) = parse_md_frontmatter(&content);
                let row = db::store::WorldRow {
                    id: 0, slug: slug.to_string(),
                    name: meta.get("name").unwrap_or(&slug.to_string()).clone(),
                    description: meta.get("description").unwrap_or(&String::new()).clone(),
                    overview: body,
                };
                match db::store::create_world(&self.db, &row) {
                    Ok(_) => self.error = Some(format!("世界 '{}' 已导入", slug)),
                    Err(e) => self.error = Some(format!("导入失败: {}", e)),
                }
            }
            "lore" => {
                // Parse markdown table for lore
                let key = slug.to_string();
                let mut triggers = Vec::new();
                let mut lore_content = String::new();
                let mut priority = 5i32;
                for line in content.lines() {
                    let t = line.trim();
                    if t.starts_with("| triggers") { if let Some(v) = t.split('|').nth(2) { triggers = v.trim().split(',').map(|s| s.trim().to_string()).collect(); } }
                    if t.starts_with("| priority") { if let Some(v) = t.split('|').nth(2) { priority = v.trim().parse().unwrap_or(5); } }
                    if !t.starts_with('|') && !t.starts_with('#') && !t.is_empty() { lore_content.push_str(t); lore_content.push('\n'); }
                }
                match db::store::create_lore(&self.db, &key, &triggers, &lore_content, priority, false) {
                    Ok(_) => self.error = Some(format!("词条 '{}' 已导入", key)),
                    Err(e) => self.error = Some(format!("导入失败: {}", e)),
                }
            }
            _ => {}
        }
    }

    pub fn handle_export(&mut self) {
        let mut count = 0;
        if let Ok(chars) = db::store::list_characters(&self.db) {
            let _ = std::fs::create_dir_all("characters");
            for c in &chars {
                let md = format!("---\nname: {name}\npersonality: {pers}\nspeech_style: {style}\nfirst_message: {first}\n---\n\n{body}\n", name=c.name, pers=c.personality, style=c.speech_style, first=c.first_message, body=c.background);
                let _ = std::fs::write(format!("characters/{}.md", c.slug), &md); count += 1;
            }
        }
        if let Ok(worlds) = db::store::list_worlds(&self.db) {
            for w in &worlds {
                let dir = format!("worlds/{}", w.slug); let _ = std::fs::create_dir_all(&dir);
                let md = format!("---\nname: {name}\ndescription: {desc}\n---\n\n{overview}\n", name=w.name, desc=w.description, overview=w.overview);
                let _ = std::fs::write(format!("{}/world.md", dir), &md); count += 1;
            }
        }
        if let Ok(lores) = db::store::list_lore(&self.db) {
            let _ = std::fs::create_dir_all("lorebooks");
            for l in &lores {
                let t = l.triggers.join(", ");
                let md = format!("# {key}\n\n| 属性 | 值 |\n|------|----|\n| triggers | {t} |\n| priority | {p} |\n\n{content}\n", key=l.key, t=t, p=l.priority, content=l.content);
                let _ = std::fs::write(format!("lorebooks/{}.md", l.key), &md); count += 1;
            }
        }
        self.error = Some(format!("已导出 {} 个文件", count));
    }

    pub fn character_name(&self) -> &str { self.manager.active_name() }

    pub fn lore_entries(&self) -> Vec<db::store::LoreRow> {
        db::store::list_lore(&self.db).unwrap_or_default()
    }
}

/// Parse YAML frontmatter from .md content. Returns (meta map, body).
fn parse_md_frontmatter(content: &str) -> (std::collections::HashMap<String, String>, String) {
    let mut meta = std::collections::HashMap::new();
    let body;
    if let Some(rest) = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n")) {
        if let Some(end) = rest.find("\n---") {
            let yaml_str = &rest[..end];
            body = rest[end + 4..].trim().to_string();
            for line in yaml_str.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    meta.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        } else {
            body = content.to_string();
        }
    } else {
        body = content.to_string();
    }
    (meta, body)
}

fn create_new_session(db: &Connection, manager: &CharacterManager, world: Option<&str>) -> anyhow::Result<i64> {
    let name = format!("{} - {}", manager.active().name, now_str());
    let char_id = manager.active().id;
    let world_id = world.and_then(|w| db::store::get_world(db, w).ok().flatten().map(|r| r.id));
    db::store::create_session(db, &name, char_id, world_id)
}

fn now_str() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600 + 8) % 24;
    format!("{:02}:{:02}", hours, mins)
}

pub async fn run(character: Option<String>, world: Option<String>, resume_id: Option<i64>, new_session: bool) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let (char_name, world_name) = if let Some(n) = character { (n, world) }
    else { let (n,w) = selector::run(&mut terminal)?; (n,w) };
    let result = run_app(&mut terminal, &char_name, world_name.as_deref(), resume_id, new_session).await;
    ratatui::restore();
    result
}

enum AppEvent { Stream(StreamEvent), NonStream(anyhow::Result<String>) }

async fn run_app(terminal: &mut DefaultTerminal, character_name: &str, world: Option<&str>, resume_id: Option<i64>, new_session: bool) -> anyhow::Result<()> {
    let mut app = App::new(character_name, world, resume_id, new_session)?;
    let cfg = config::load()?;
    let use_stream = cfg.llm.stream;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Check for lorebook hot-reload (using file-based, will migrate)
        // Hot reload skipped in DB mode

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Paste(text) => {
                    if !app.loading { for c in text.chars() { app.insert_char(c); } }
                }
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release { continue; }
                    if app.show_help { app.show_help = false; continue; }
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if !app.input.is_empty() {
                                if let Ok(mut cb) = arboard::Clipboard::new() { let _ = cb.set_text(&app.input); }
                            }
                        }
                        KeyCode::Char('v') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if !app.loading {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    if let Ok(t) = cb.get_text() { for c in t.chars() { app.insert_char(c); } }
                                }
                            }
                        }
                        KeyCode::Tab => { app.manager.next(); app.scroll_offset = 0; app.error = None; }
                        KeyCode::BackTab => { app.manager.prev(); app.scroll_offset = 0; app.error = None; }
                        KeyCode::Char('w') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if !app.manager.worlds.is_empty() {
                                let next = match app.manager.active_world {
                                    Some(i) if i + 1 < app.manager.worlds.len() => Some(i + 1),
                                    Some(_) => None,
                                    None => Some(0),
                                };
                                app.manager.switch_world(&app.db, next);
                                app.scroll_offset = 0;
                            }
                        }
                        KeyCode::F(1) => { app.show_help = true; }
                        KeyCode::Enter => {
                            if app.loading { continue; }
                            let trimmed = app.input.trim();
                            if trimmed == "/exit" || trimmed == "/quit" { app.save_current(); break; }
                            if trimmed.is_empty() { continue; }
                            app.send_message();
                            if app.should_quit { break; }

                            // Build LLM context
                            let mut system_prompt = app.manager.active().system_prompt.clone();
                            if !app.self_persona.is_empty() {
                                system_prompt.push_str(&format!("\n\n【当前对话对象的设定】\n{}", app.self_persona));
                            }
                            // Inject world-level context (events, rules, timeline)
                            if let Some(widx) = app.manager.active_world {
                                if let Some(world) = app.manager.worlds.get(widx) {
                                    // Timeline current time
                                    if let Ok(Some(tl)) = db::store::current_timeline(&app.db, world.id) {
                                        system_prompt.push_str(&format!("\n\n【世界当前时间】\n{}", tl.time_label));
                                    }
                                    if let Ok(states) = db::store::list_states(&app.db, "world_states", world.id) {
                                        let events: Vec<_> = states.iter().filter(|(c,_,_)| c=="event").map(|(_,k,d)| format!("{}:{}", k, d)).collect();
                                        let rules: Vec<_> = states.iter().filter(|(c,_,_)| c=="rule").map(|(_,k,d)| format!("{}:{}", k, d)).collect();
                                        if !events.is_empty() || !rules.is_empty() {
                                            system_prompt.push_str("\n\n【世界全局信息】\n");
                                            if !events.is_empty() { system_prompt.push_str(&format!("当前事件: {}\n", events.join("; "))); }
                                            if !rules.is_empty() { system_prompt.push_str(&format!("世界法则: {}\n", rules.join("; "))); }
                                        }
                                    }
                                }
                            }

                            // Inject character state summary
                            if let Ok(states) = db::store::list_states(&app.db, "character_states", app.manager.active().id) {
                                let items: Vec<_> = states.iter().filter(|(c,_,_)| c=="item").map(|(_,k,d)| format!("{}:{}", k, d)).collect();
                                let sts: Vec<_> = states.iter().filter(|(c,_,_)| c=="status").map(|(_,k,d)| format!("{}:{}", k, d)).collect();
                                let mut summary = String::new();
                                if !items.is_empty() { summary.push_str(&format!("物品: {}", items.join(", "))); }
                                if !sts.is_empty() {
                                    if !summary.is_empty() { summary.push_str(" | "); }
                                    summary.push_str(&format!("状态: {}", sts.join(", ")));
                                }
                                if !summary.is_empty() { system_prompt.push_str(&format!("\n\n【当前角色动态状态】\n{}", summary)); }
                            }

                            let llm_config = cfg.llm.clone();
                            let recent_text: String = app.manager.active().messages.iter().rev().take(5).map(|m| m.content.as_str()).collect::<Vec<_>>().join(" ");
                            // Scan lore (from DB now)
                            let lore_rows = app.lore_entries();
                            let activated = lore_rows.iter().filter(|r| {
                                r.triggers.iter().any(|t| recent_text.to_lowercase().contains(&t.to_lowercase()))
                            }).collect::<Vec<_>>();
                            app.active_lore_keys = activated.iter().map(|r| r.key.clone()).collect();

                            let history: Vec<_> = app.manager.active().messages.iter()
                                .filter(|m| m.role != "system")
                                .map(|m| llm::ChatMessage { role: m.role.clone(), content: Some(m.content.clone()), tool_calls: None, tool_call_id: None })
                                .collect();

                            let all_messages = conversation::context::build(&system_prompt, &history, &[], 20);
                            // Build lore text inline from LoreRow
                            let lore_text = build_lore_text(&activated);
                            let all_messages = if lore_text.is_empty() { all_messages } else {
                                let mut msgs = all_messages;
                                if let Some(sys) = msgs.first_mut() {
                                if let Some(ref mut content) = sys.content {
                                    content.push_str(&format!("\n\n---\n【当前世界信息】\n{}", lore_text));
                                }
                                }
                                msgs
                            };

                            if use_stream {
                            let cancel_rx = app.cancel_tx.take().map(|tx| tx.subscribe());
                            let mut stream_rx = llm::chat_stream(llm_config, all_messages, cancel_rx);
                            let tx = tx.clone();
                            tokio::spawn(async move { while let Some(e) = stream_rx.recv().await { let _ = tx.send(AppEvent::Stream(e)); } });
                        } else {
                            // Use tools-enabled chat (open fresh connection for spawned task)
                            let char_id = app.manager.active().id;
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let db = db::schema::open().expect("Failed to open DB for tools");
                                let mut messages = all_messages;
                                let result = llm::chat_with_tools(&llm_config, &mut messages, move |tool_name, args_json| {
                                    if tool_name == "manage_state" {
                                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_json) {
                                            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("get");
                                            let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("item");
                                            let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
                                            let data = args.get("data").map(|v| v.to_string()).unwrap_or_default();
                                            match db::store::manage_state(
                                                &db, "character_states", char_id,
                                                action, category, key, &data
                                            ) {
                                                Ok(result) => result,
                                                Err(e) => format!("Error: {}", e),
                                            }
                                        } else { "Invalid arguments".to_string() }
                                    } else { format!("Unknown tool: {}", tool_name) }
                                }).await;
                                let _ = tx.send(AppEvent::NonStream(result));
                            });
                        }
                        }
                        KeyCode::Char(c) => { app.insert_char(c); }
                        KeyCode::Backspace => { app.remove_char_before(); }
                        KeyCode::Delete => { app.remove_char_at(); }
                        KeyCode::Left => { if app.cursor_pos > 0 { app.cursor_pos -= 1; } }
                        KeyCode::Right => { if app.cursor_pos < app.char_count() { app.cursor_pos += 1; } }
                        KeyCode::Home => { app.cursor_pos = 0; }
                        KeyCode::End => { app.cursor_pos = app.char_count(); }
                        KeyCode::Up => { app.scroll_up(1); }
                        KeyCode::Down => { app.scroll_down(1); }
                        KeyCode::PageUp => { app.scroll_up(5); }
                        KeyCode::PageDown => { app.scroll_down(5); }
                        KeyCode::Esc => {
                            if app.loading {
                                if let Some(tx) = app.cancel_tx.take() { let _ = tx.send(true); }
                            } else { app.scroll_to_bottom(); }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Stream(StreamEvent::Token(t)) => { app.streaming.push_str(&t); }
                AppEvent::Stream(StreamEvent::Cancelled(p)) => {
                    if !p.is_empty() { app.manager.active_mut().messages.push(Message { role: "assistant".into(), content: format!("{} [已打断]", p) }); }
                    app.streaming.clear(); app.loading = false; app.is_streaming = false; app.cancel_tx = None;
                }
                AppEvent::Stream(StreamEvent::Done(full)) => {
                    app.manager.active_mut().messages.push(Message { role: "assistant".into(), content: full });
                    app.streaming.clear(); app.loading = false; app.is_streaming = false; app.cancel_tx = None;
                    app.try_autosave();
                }
                AppEvent::Stream(StreamEvent::Error(msg)) => { app.error = Some(msg); app.loading = false; app.is_streaming = false; app.streaming.clear(); app.cancel_tx = None; }
                AppEvent::NonStream(Ok(content)) => {
                    app.manager.active_mut().messages.push(Message { role: "assistant".into(), content });
                    app.loading = false; app.is_streaming = false; app.cancel_tx = None; app.try_autosave();
                }
                AppEvent::NonStream(Err(e)) => { app.error = Some(format!("{:#}", e)); app.loading = false; app.is_streaming = false; app.cancel_tx = None; }
            }
        }
    }
    Ok(())
}
