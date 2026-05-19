use crate::character::manager::CharacterManager;
use crate::command;
use crate::config;
use crate::conversation;
use crate::db;
use crate::llm;
use crate::llm::StreamEvent;
use crate::lorebook;
use crate::state;
use crate::tui::selector;
use crate::tui::ui;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use rusqlite::Connection;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct App {
    pub manager: CharacterManager,
    pub lore_manager: lorebook::entry::LoreManager,
    pub db: Option<Connection>,
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
}

fn now_str() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple HH:MM format within the day
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600 + 8) % 24; // UTC+8
    format!("{:02}:{:02}", hours, mins)
}

impl App {
    pub fn new(
        character_name: &str,
        _world: Option<&str>,
        resume_id: Option<i64>,
        new_session: bool,
    ) -> anyhow::Result<Self> {
        let mut manager = CharacterManager::load_all(character_name)?;
        let mut lore_manager = lorebook::entry::LoreManager::new();
        lore_manager.load("lorebooks");
        if std::path::Path::new("worlds").exists() {
            lore_manager.load("worlds");
        }

        // Open database
        let (db, session_id, save_counter) = match db::schema::open() {
            Ok(conn) => {
                let sid = if let Some(id) = resume_id {
                    // Resume specific session
                    match db::store::get_session(&conn, id) {
                        Ok(Some(_)) => {
                            match db::store::load_messages(&conn, id) {
                                Ok(msgs) => {
                                    // Inject loaded messages into manager
                                    // ... handled after construction
                                    Some((id, msgs))
                                }
                                Err(_) => Some((0, vec![])),
                            }
                        }
                        _ => Some((0, vec![])),
                    }
                } else if new_session {
                    None // Start fresh
                } else {
                    // Auto-resume last session for this character
                    match db::store::list_sessions(&conn) {
                        Ok(sessions) => {
                            let last = sessions.iter()
                                .find(|s| s.character_name == character_name);
                            match last {
                                Some(s) => match db::store::load_messages(&conn, s.id) {
                                    Ok(msgs) => Some((s.id, msgs)),
                                    Err(_) => None,
                                },
                                None => None,
                            }
                        }
                        Err(_) => None,
                    }
                };

                match sid {
                    Some((id, loaded_msgs)) => {
                        if !loaded_msgs.is_empty() {
                            // Load messages into active character
                            let active = manager.active_name().to_string();
                            if let Some(state) = manager.characters.get_mut(&active) {
                                state.messages = loaded_msgs;
                            }
                        }
                        (Some(conn), id, 0)
                    }
                    None => {
                        // Create new session
                        let world = _world.unwrap_or("");
                        let name = format!("{} - {}", character_name, now_str());
                        let sid = db::store::create_session(
                            &conn,
                            &name,
                            character_name,
                            if world.is_empty() { None } else { Some(world) },
                        ).unwrap_or(0);
                        (Some(conn), sid, 0)
                    }
                }
            }
            Err(_) => (None, 0, 0),
        };

        Ok(Self {
            manager,
            lore_manager,
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
            self_persona: Self::load_self_persona(),
        })
    }

    fn send_message(&mut self) {
        let input_text = std::mem::take(&mut self.input);
        self.cursor_pos = 0;

        // Command parsing
        let (cmd, content) = command::parser::parse(&input_text);
        match cmd {
            command::parser::Command::Quit => {
                self.save_current();
                self.should_quit = true;
                return;
            }
            command::parser::Command::Help => {
                self.show_help = true;
                return;
            }
            command::parser::Command::Clear => {
                self.manager.active_mut().messages.clear();
                let first = self.manager.active().card.meta.first_message.clone();
                if !first.is_empty() {
                    self.manager.active_mut().messages.push(Message {
                        role: "assistant".into(),
                        content: first,
                    });
                }
                self.scroll_offset = 0;
                return;
            }
            command::parser::Command::Switch(name) => {
                let name = name.trim().to_string();
                if !name.is_empty() && self.manager.switch_to_name(&name) {
                    self.scroll_offset = 0;
                } else if !name.is_empty() {
                    self.error = Some(format!("角色 '{}' 不存在", name));
                }
                return;
            }
            command::parser::Command::Save => {
                self.save_current();
                return;
            }
            command::parser::Command::Load(id_str) => {
                self.load_session(&id_str);
                return;
            }
            command::parser::Command::CreateChar(name) => {
                self.handle_create_char(&name);
                return;
            }
            command::parser::Command::CreateWorld(name) => {
                self.handle_create_world(&name);
                return;
            }
            command::parser::Command::SetSelf(text) => {
                self.handle_set_self(&text);
                return;
            }
            command::parser::Command::ManageState(args) => {
                self.handle_manage_state(&args);
                return;
            }
            command::parser::Command::Info(name) => {
                let name = name.trim().to_string();
                if let Some(card) = self.manager.lookup(&name) {
                    let info = format!(
                        "角色: {}\n性格: {}\n说话风格: {}\n{}",
                        card.meta.name,
                        card.meta.personality,
                        card.meta.speech_style,
                        card.body
                    );
                    // Show as system message in current character's chat
                    self.manager.active_mut().messages.push(Message {
                        role: "system".into(),
                        content: info,
                    });
                } else {
                    self.error = Some(format!("角色 '{}' 不存在", name));
                }
                return;
            }
            command::parser::Command::List => {
                let list = self.manager.order.iter()
                    .map(|n| format!("- {}", n))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.manager.active_mut().messages.push(Message {
                    role: "system".into(),
                    content: format!("可用角色:\n{}", list),
                });
                return;
            }
            command::parser::Command::None => {}
        }

        // Expand @mentions and send
        let expanded = {
            let manager = &self.manager;
            command::parser::expand_mentions(&content, |name| {
                manager.lookup(name).cloned()
            })
        };

        if expanded.trim().is_empty() {
            return;
        }

        self.manager.active_mut().messages.push(Message {
            role: "user".into(),
            content: expanded,
        });
        self.save_counter += 1;
        self.loading = true;
        self.is_streaming = true;
        self.streaming.clear();
        self.error = None;
        self.scroll_offset = 0;
        let (cancel_tx, _) = tokio::sync::watch::channel(false);
        self.cancel_tx = Some(cancel_tx);
    }

    pub fn character_name(&self) -> &str {
        self.manager.active_name()
    }

    /// Convert char index to byte index
    fn byte_pos(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }

    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    fn insert_char(&mut self, c: char) {
        let pos = self.byte_pos();
        self.input.insert(pos, c);
        self.cursor_pos += 1;
    }

    fn remove_char_before(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let start = self.byte_pos();
            let end = self.input[start..]
                .chars()
                .next()
                .map(|ch| start + ch.len_utf8())
                .unwrap_or(start);
            self.input.drain(start..end);
        }
    }

    fn remove_char_at(&mut self) {
        if self.cursor_pos < self.char_count() {
            let pos = self.byte_pos();
            let end = self.input[pos..]
                .chars()
                .next()
                .map(|ch| pos + ch.len_utf8())
                .unwrap_or(pos);
            self.input.drain(pos..end);
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn save_current(&mut self) {
        if let Some(ref conn) = self.db {
            let msgs = &self.manager.active().messages;
            match db::store::save_messages(conn, self.session_id, msgs) {
                Ok(()) => {
                    self.save_counter = 0;
                    self.error = Some("已保存".into());
                }
                Err(e) => {
                    self.error = Some(format!("保存失败: {}", e));
                }
            }
        } else {
            self.error = Some("数据库未初始化".into());
        }
    }

    pub fn load_session(&mut self, id_str: &str) {
        let id_str = id_str.trim();
        if let Some(ref conn) = self.db {
            match id_str.parse::<i64>() {
                Ok(id) => {
                    match db::store::load_messages(conn, id) {
                        Ok(msgs) => {
                            if !msgs.is_empty() {
                                self.manager.active_mut().messages = msgs;
                                self.session_id = id;
                                self.save_counter = 0;
                                self.scroll_offset = 0;
                                self.error = Some(format!("已加载会话 {}", id));
                            } else {
                                self.error = Some(format!("会话 {} 无消息", id));
                            }
                        }
                        Err(e) => self.error = Some(format!("加载失败: {}", e)),
                    }
                }
                Err(_) => self.error = Some(format!("无效的会话 ID: {}", id_str)),
            }
        }
    }

    pub fn try_autosave(&mut self) {
        if self.save_counter >= 3 {
            if let Some(ref conn) = self.db {
                let msgs = &self.manager.active().messages;
                if db::store::save_messages(conn, self.session_id, msgs).is_ok() {
                    self.save_counter = 0;
                }
            }
        }
    }

    fn load_self_persona() -> String {
        std::fs::read_to_string("data/self.txt").unwrap_or_default()
    }

    fn save_self_persona(text: &str) {
        let _ = std::fs::create_dir_all("data");
        let _ = std::fs::write("data/self.txt", text);
    }

    pub fn handle_create_char(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.error = Some("用法: /cc <角色名>".into());
            return;
        }
        let path = format!("characters/{}.md", name);
        if std::path::Path::new(&path).exists() {
            self.error = Some(format!("角色 '{}' 已存在", name));
            return;
        }
        let template = format!(
            "---\nname: {name}\npersonality: \nspeech_style: \nfirst_message: \n---\n\n# 背景\n\n# 外貌\n\n# 我知道的事情\n- \n",
            name = name
        );
        match std::fs::write(&path, &template) {
            Ok(()) => {
                // Reload character manager
                let active = self.manager.active_name().to_string();
                if let Ok(new_manager) = CharacterManager::load_all(&active) {
                    self.manager = new_manager;
                }
                self.error = Some(format!("角色 '{}' 已创建，请编辑 {}.md", name, name));
            }
            Err(e) => {
                self.error = Some(format!("创建失败: {}", e));
            }
        }
    }

    pub fn handle_create_world(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.error = Some("用法: /cw <世界名>".into());
            return;
        }
        let path = format!("lorebooks/{}.toml", name);
        if std::path::Path::new(&path).exists() {
            self.error = Some(format!("世界 '{}' 已存在", name));
            return;
        }
        let template = format!(
            "key = \"{name}\"\ntriggers = [\"\", \"\"]\ncontent = \"\"\"\n\n\"\"\"\npriority = 5\n",
            name = name
        );
        let _ = std::fs::create_dir_all("lorebooks");
        match std::fs::write(&path, &template) {
            Ok(()) => {
                self.lore_manager.load("lorebooks");
                self.error = Some(format!("世界 '{}' 已创建，请编辑 {}.toml", name, name));
            }
            Err(e) => {
                self.error = Some(format!("创建失败: {}", e));
            }
        }
    }

    pub fn handle_set_self(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            let current = if self.self_persona.is_empty() {
                "未设置".to_string()
            } else {
                format!("当前设定: {}", self.self_persona)
            };
            self.error = Some(format!("用法: /self <你的设定>\n{}", current));
            return;
        }
        self.self_persona = text.to_string();
        Self::save_self_persona(text);
        self.error = Some(format!("用户设定已更新: {}", text));
    }

    /// Parse and execute a `/state` command
    /// Format: action category key [data...]
    pub fn handle_manage_state(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(4, ' ').collect();
        let action = parts.first().map(|s| *s).unwrap_or("get");
        let category = parts.get(1).map(|s| *s).unwrap_or("item");
        let key = parts.get(2).map(|s| *s).unwrap_or("");
        let data_str = parts.get(3).map(|s| *s).unwrap_or("");

        let data: HashMap<String, serde_json::Value> = if !data_str.is_empty() {
            serde_json::from_str(data_str).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let name = self.manager.active_name().to_string();
        let state = &mut self.manager.active_mut().state;
        let result = state::manage(state, action, category, key, &data);

        // Save state file
        let state_path = format!("characters/{}.state.md", name);
        if let Err(e) = state::save(&state, &state_path) {
            self.error = Some(format!("保存状态失败: {}", e));
        }
        self.error = Some(result);
    }
}

pub async fn run(
    character: Option<String>,
    world: Option<String>,
    resume_id: Option<i64>,
    new_session: bool,
) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();

    let (char_name, world_name) = if let Some(name) = character {
        (name, world.clone())
    } else {
        let (name, w) = selector::run(&mut terminal)?;
        (name, w)
    };

    let result = run_app(&mut terminal, &char_name, world_name.as_deref(), resume_id, new_session).await;
    ratatui::restore();
    result
}

enum AppEvent {
    Stream(StreamEvent),
    NonStream(anyhow::Result<String>),
}

async fn run_app(
    terminal: &mut DefaultTerminal,
    character_name: &str,
    world: Option<&str>,
    resume_id: Option<i64>,
    new_session: bool,
) -> anyhow::Result<()> {
    let mut app = App::new(character_name, world, resume_id, new_session)?;
    let cfg = config::load()?;
    let use_stream = cfg.llm.stream;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Check for lorebook hot-reload
        app.lore_manager.check_hot_reload("lorebooks");
        if std::path::Path::new("worlds").exists() {
            app.lore_manager.check_hot_reload("worlds");
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Paste(text) => {
                    if !app.loading {
                        for c in text.chars() {
                            app.insert_char(c);
                        }
                    }
                }
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    if app.show_help {
                        app.show_help = false;
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if !app.input.is_empty() {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(&app.input);
                                }
                            }
                        }
                        KeyCode::Char('v') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if !app.loading {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    if let Ok(text) = cb.get_text() {
                                        for c in text.chars() {
                                            app.insert_char(c);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Tab => {
                            app.manager.next();
                            app.scroll_offset = 0;
                            app.error = None;
                        }
                        KeyCode::BackTab => {
                            app.manager.prev();
                            app.scroll_offset = 0;
                            app.error = None;
                        }
                        KeyCode::F(1) => {
                            app.show_help = true;
                        }
                        KeyCode::Enter => {
                            if app.loading {
                                continue;
                            }
                            let trimmed = app.input.trim();
                            if trimmed == "/exit" || trimmed == "/quit" {
                                app.save_current();
                                break;
                            }
                            let trimmed = app.input.trim();
                            // Legacy /exit /quit (also handled by command parser)
                            if trimmed == "/exit" || trimmed == "/quit" {
                                break;
                            }
                            if trimmed.is_empty() {
                                continue;
                            }
                            app.send_message();
                            if app.should_quit {
                                break;
                            }
                        let mut system_prompt = app.manager.active().system_prompt.clone();
                        // Inject user persona
                        if !app.self_persona.is_empty() {
                            system_prompt.push_str(&format!(
                                "\n\n【当前对话对象的设定】\n{}",
                                app.self_persona
                            ));
                        }
                        // Inject character state summary
                        let state_summary = state::summary(&app.manager.active().state);
                        if !state_summary.is_empty() {
                            system_prompt.push_str(&format!(
                                "\n\n【当前角色动态状态】\n{}",
                                state_summary
                            ));
                        }
                        let llm_config = cfg.llm.clone();

                        // Scan for lorebook triggers
                        let recent_text: String = app.manager.active().messages
                            .iter()
                            .rev()
                            .take(5)
                            .map(|m| m.content.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let activated = lorebook::matcher::match_entries(
                            &app.lore_manager.entries,
                            &recent_text,
                        );
                        app.lore_manager.active_keys = activated.iter()
                            .map(|e| e.key.clone())
                            .collect();

                        let history: Vec<_> = app.manager.active().messages.iter()
                            .filter(|m| m.role != "system")
                            .map(|m| llm::ChatMessage {
                                role: m.role.clone(),
                                content: m.content.clone(),
                            })
                            .collect();

                        let all_messages = conversation::context::build(
                            &system_prompt,
                            &history,
                            &activated,
                            20,
                        );

                            if use_stream {
                                let cancel_rx = app.cancel_tx.take().map(|tx| tx.subscribe());
                                let mut stream_rx = llm::chat_stream(llm_config, all_messages, cancel_rx);
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    while let Some(event) = stream_rx.recv().await {
                                        let _ = tx.send(AppEvent::Stream(event));
                                    }
                                });
                            } else {
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    let result = llm::chat_with_messages(&llm_config, &all_messages).await;
                                    let _ = tx.send(AppEvent::NonStream(result));
                                });
                            }
                        }
                        KeyCode::Char(c) => {
                            app.insert_char(c);
                        }
                        KeyCode::Backspace => {
                            app.remove_char_before();
                        }
                        KeyCode::Delete => {
                            app.remove_char_at();
                        }
                        KeyCode::Left => {
                            if app.cursor_pos > 0 {
                                app.cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if app.cursor_pos < app.char_count() {
                                app.cursor_pos += 1;
                            }
                        }
                        KeyCode::Home => {
                            app.cursor_pos = 0;
                        }
                        KeyCode::End => {
                            app.cursor_pos = app.char_count();
                        }
                        KeyCode::Up => {
                            app.scroll_up(1);
                        }
                        KeyCode::Down => {
                            app.scroll_down(1);
                        }
                        KeyCode::PageUp => {
                            app.scroll_up(5);
                        }
                        KeyCode::PageDown => {
                            app.scroll_down(5);
                        }
                        KeyCode::Esc => {
                            if app.loading {
                                if let Some(tx) = app.cancel_tx.take() {
                                    let _ = tx.send(true);
                                }
                            } else {
                                app.scroll_to_bottom();
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Process LLM events
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Stream(StreamEvent::Token(token)) => {
                    app.streaming.push_str(&token);
                }
                AppEvent::Stream(StreamEvent::Cancelled(partial)) => {
                    if !partial.is_empty() {
                        app.manager.active_mut().messages.push(Message {
                            role: "assistant".into(),
                            content: format!("{} [已打断]", partial),
                        });
                    }
                    app.streaming.clear();
                    app.loading = false;
                    app.is_streaming = false;
                    app.cancel_tx = None;
                }
                AppEvent::Stream(StreamEvent::Done(full)) => {
                    app.manager.active_mut().messages.push(Message {
                        role: "assistant".into(),
                        content: full,
                    });
                    app.streaming.clear();
                    app.loading = false;
                    app.is_streaming = false;
                    app.cancel_tx = None;
                    app.try_autosave();
                }
                AppEvent::Stream(StreamEvent::Error(msg)) => {
                    app.error = Some(msg);
                    app.loading = false;
                    app.is_streaming = false;
                    app.streaming.clear();
                    app.cancel_tx = None;
                }
                AppEvent::NonStream(Ok(content)) => {
                    app.manager.active_mut().messages.push(Message {
                        role: "assistant".into(),
                        content,
                    });
                    app.loading = false;
                    app.is_streaming = false;
                    app.cancel_tx = None;
                    app.try_autosave();
                }
                AppEvent::NonStream(Err(e)) => {
                    app.error = Some(format!("{:#}", e));
                    app.loading = false;
                    app.is_streaming = false;
                    app.cancel_tx = None;
                }
            }
        }
    }

    Ok(())
}
