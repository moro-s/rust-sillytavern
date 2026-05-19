use crate::character;
use crate::config;
use crate::llm;
use crate::llm::StreamEvent;
use crate::tui::ui;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct App {
    pub character_name: String,
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub input: String,
    pub cursor_pos: usize,
    pub loading: bool,
    pub scroll_offset: usize,
    pub error: Option<String>,
    pub show_help: bool,
    /// Accumulated streaming content (not yet committed to messages)
    pub streaming: String,
    /// Whether we're in streaming mode
    pub is_streaming: bool,
}

impl App {
    pub fn new(character_name: &str) -> anyhow::Result<Self> {
        let card = character::load(character_name)?;
        let system_prompt = character::build_system_prompt(&card);

        let mut messages = Vec::new();
        if !card.meta.first_message.is_empty() {
            messages.push(Message {
                role: "assistant".into(),
                content: card.meta.first_message.clone(),
            });
        }

        Ok(Self {
            character_name: card.meta.name,
            system_prompt,
            messages,
            input: String::new(),
            cursor_pos: 0,
            loading: false,
            scroll_offset: 0,
            error: None,
            show_help: false,
            streaming: String::new(),
            is_streaming: false,
        })
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

    fn send_message(&mut self) {
        let input = std::mem::take(&mut self.input);
        self.cursor_pos = 0;
        if input.trim().is_empty() {
            return;
        }

        self.messages.push(Message {
            role: "user".into(),
            content: input,
        });
        self.loading = true;
        self.is_streaming = true;
        self.streaming.clear();
        self.error = None;
        self.scroll_offset = 0; // auto-scroll to bottom
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
}

pub async fn run(character_name: &str) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, character_name).await;
    ratatui::restore();
    result
}

enum AppEvent {
    Stream(StreamEvent),
    NonStream(anyhow::Result<String>),
}

async fn run_app(terminal: &mut DefaultTerminal, character_name: &str) -> anyhow::Result<()> {
    let mut app = App::new(character_name)?;
    let cfg = config::load()?;
    let use_stream = cfg.llm.stream;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Poll for keyboard events
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
                        // Ctrl+C: copy input to clipboard
                        if !app.input.is_empty() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(&app.input);
                            }
                        }
                    }
                    KeyCode::Char('v') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        // Ctrl+V: paste from clipboard
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
                    KeyCode::Char('q') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::F(1) => {
                        app.show_help = true;
                    }
                    KeyCode::Enter => {
                        if !app.loading && !app.input.is_empty() {
                            app.send_message();

                            let system_prompt = app.system_prompt.clone();
                            let llm_config = cfg.llm.clone();
                            let history: Vec<_> = app.messages.iter()
                                .filter(|m| m.role != "system")
                                .map(|m| llm::ChatMessage {
                                    role: m.role.clone(),
                                    content: m.content.clone(),
                                })
                                .collect();

                            let all_messages = {
                                let mut msgs = vec![
                                    llm::ChatMessage {
                                        role: "system".into(),
                                        content: system_prompt,
                                    },
                                ];
                                let recent = history.iter().rev().take(20).rev();
                                msgs.extend(recent.cloned());
                                msgs
                            };

                            if use_stream {
                                let mut stream_rx = llm::chat_stream(llm_config, all_messages);
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
                        app.scroll_to_bottom();
                    }
                    _ => {}
                }
                } // Event::Key
                _ => {} // Ignore other events
            } // match event::read()
        }

        // Process LLM events
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Stream(StreamEvent::Token(token)) => {
                    app.streaming.push_str(&token);
                }
                AppEvent::Stream(StreamEvent::Done(full)) => {
                    app.messages.push(Message {
                        role: "assistant".into(),
                        content: full,
                    });
                    app.streaming.clear();
                    app.loading = false;
                    app.is_streaming = false;
                }
                AppEvent::Stream(StreamEvent::Error(msg)) => {
                    app.error = Some(msg);
                    app.loading = false;
                    app.is_streaming = false;
                    app.streaming.clear();
                }
                AppEvent::NonStream(Ok(content)) => {
                    app.messages.push(Message {
                        role: "assistant".into(),
                        content,
                    });
                    app.loading = false;
                    app.is_streaming = false;
                }
                AppEvent::NonStream(Err(e)) => {
                    app.error = Some(format!("{:#}", e));
                    app.loading = false;
                    app.is_streaming = false;
                }
            }
        }
    }

    Ok(())
}
