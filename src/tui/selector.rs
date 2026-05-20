use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    DefaultTerminal,
};
use std::time::Duration;

/// 选择器操作结果
pub enum Action {
    Select { character: String, world: Option<String> },
    CreateCharacter,
    CreateWorld,
}

/// 列表项的标签常量（永远位于列表末尾）
const LABEL_CREATE_CHAR: &str = "(创建角色)";
const LABEL_CREATE_WORLD: &str = "(创建世界)";

/// 选择器当前聚焦的面板
#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Character,
    World,
}

pub struct Selector {
    /// 已有角色 slug + 末尾的 `LABEL_CREATE_CHAR`
    characters: Vec<String>,
    /// 已有世界 slug + 末尾的 `LABEL_CREATE_WORLD`
    worlds: Vec<String>,
    char_index: usize,
    world_index: usize,
    /// 当前键盘焦点：角色列表或世界列表
    focus: Focus,
}

impl Selector {
    pub fn new(db: &rusqlite::Connection) -> Self {
        let mut characters: Vec<String> = if let Ok(rows) = crate::db::store::list_characters(db) {
            let mut names: Vec<_> = rows.iter().map(|r| r.slug.clone()).collect();
            names.sort();
            names
        } else { vec![] };
        // 始终在末尾添加创建选项
        characters.push(LABEL_CREATE_CHAR.to_string());

        let mut worlds: Vec<String> = if let Ok(rows) = crate::db::store::list_worlds(db) {
            let mut names: Vec<_> = rows.iter().map(|r| r.slug.clone()).collect();
            names.sort();
            names
        } else { vec![] };
        worlds.push(LABEL_CREATE_WORLD.to_string());

        Self { characters, worlds, char_index: 0, world_index: 0, focus: Focus::Character }
    }
}

/// 判断列表项是否为创建标签
fn is_create_label(name: &str) -> bool {
    name == LABEL_CREATE_CHAR || name == LABEL_CREATE_WORLD
}

pub fn run(terminal: &mut DefaultTerminal) -> anyhow::Result<Action> {
    let db = crate::db::schema::open()?;
    let mut sel = Selector::new(&db);
    // 两个列表至少各有一个创建项，世界面板总是显示
    let has_worlds = true;

    loop {
        terminal.draw(|f| draw(f, &sel, has_worlds))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release { continue; }
                match key.code {
                    KeyCode::Enter => {
                        let chosen_char = &sel.characters[sel.char_index];
                        let chosen_world = &sel.worlds[sel.world_index];
                        if chosen_char == LABEL_CREATE_CHAR {
                            return Ok(Action::CreateCharacter);
                        }
                        if chosen_world == LABEL_CREATE_WORLD {
                            return Ok(Action::CreateWorld);
                        }
                        let world_name = if is_create_label(chosen_world) { None } else { Some(chosen_world.clone()) };
                        return Ok(Action::Select { character: chosen_char.clone(), world: world_name });
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        // 找到第一个真实角色（跳过创建标签）
                        let char_name = sel.characters.iter()
                            .find(|n| !is_create_label(n))
                            .cloned()
                            .unwrap_or_else(|| "innkeeper".into());
                        return Ok(Action::Select { character: char_name, world: None });
                    }
                    // Tab：切换角色列表 ↔ 世界列表焦点
                    KeyCode::Tab => {
                        sel.focus = match sel.focus {
                            Focus::Character => Focus::World,
                            Focus::World => Focus::Character,
                        };
                    }
                    // 上下键操作当前聚焦的列表
                    KeyCode::Up => {
                        match sel.focus {
                            Focus::Character => { if sel.char_index > 0 { sel.char_index -= 1; } }
                            Focus::World => { if sel.world_index > 0 { sel.world_index -= 1; } }
                        }
                    }
                    KeyCode::Down => {
                        match sel.focus {
                            Focus::Character => { if sel.char_index + 1 < sel.characters.len() { sel.char_index += 1; } }
                            Focus::World => { if sel.world_index + 1 < sel.worlds.len() { sel.world_index += 1; } }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, sel: &Selector, has_worlds: bool) {
    let area = f.area();
    let outer = Block::default().title(" 选择角色与世界 ").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = if has_worlds {
        Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(50), Constraint::Percentage(50)]).split(inner)
    } else {
        Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(100)]).split(inner)
    };

    let char_accent = if sel.focus == Focus::Character { Color::Green } else { Color::DarkGray };
    let world_accent = if sel.focus == Focus::World { Color::Magenta } else { Color::DarkGray };

    draw_list(f, chunks[0], " 角色 ", &sel.characters, sel.char_index, char_accent);
    if has_worlds { draw_list(f, chunks[1], " 世界 ", &sel.worlds, sel.world_index, world_accent); }
}

fn draw_list(f: &mut ratatui::Frame, area: Rect, title: &str, items: &[String], selected: usize, accent: Color) {
    let block = Block::default().title(format!(" {} ", title)).borders(Borders::ALL).border_style(Style::default().fg(accent));
    let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, name)| {
        // 创建标签用特殊样式（斜体、暗淡）
        let is_create = is_create_label(name);
        let name_style = if is_create {
            Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)
        } else if i != selected {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD)
        };
        let prefix = if i == selected {
            Span::styled(" ▶ ", Style::default().fg(accent).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("   ")
        };
        ListItem::new(Line::from(vec![prefix, Span::styled(name.clone(), name_style)]))
    }).collect();
    f.render_widget(List::new(list_items).block(block), area);
}
