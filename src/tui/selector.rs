use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    DefaultTerminal,
};
use std::time::Duration;

pub struct Selector {
    characters: Vec<String>,
    worlds: Vec<String>,
    char_index: usize,
    world_index: usize,
}

impl Selector {
    pub fn new(db: &rusqlite::Connection) -> Self {
        let characters = if let Ok(rows) = crate::db::store::list_characters(db) {
            let mut names: Vec<_> = rows.iter().map(|r| r.slug.clone()).collect();
            names.sort();
            if names.is_empty() { names.push("(无角色)".into()); }
            names
        } else { vec!["(无角色)".into()] };

        let worlds = if let Ok(rows) = crate::db::store::list_worlds(db) {
            let mut names: Vec<_> = rows.iter().map(|r| r.slug.clone()).collect();
            names.sort();
            if names.is_empty() { names.push("(无世界)".into()); }
            names
        } else { vec!["(无世界)".into()] };

        Self { characters, worlds, char_index: 0, world_index: 0 }
    }
}

pub fn run(terminal: &mut DefaultTerminal) -> anyhow::Result<(String, Option<String>)> {
    let db = crate::db::schema::open()?;
    let mut sel = Selector::new(&db);
    let has_worlds = !sel.worlds.is_empty() && sel.worlds.first().map_or(false, |w| w != "(无世界)");

    loop {
        terminal.draw(|f| draw(f, &sel, has_worlds))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release { continue; }
                match key.code {
                    KeyCode::Enter => {
                        let char_name = sel.characters.get(sel.char_index).filter(|n| *n != "(无角色)").cloned().unwrap_or_else(|| "innkeeper".into());
                        let world_name = sel.worlds.get(sel.world_index).filter(|n| *n != "(无世界)").cloned();
                        return Ok((char_name, world_name));
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        let char_name = sel.characters.first().filter(|n| *n != "(无角色)").cloned().unwrap_or_else(|| "innkeeper".into());
                        return Ok((char_name, None));
                    }
                    KeyCode::Up => { if sel.char_index > 0 { sel.char_index -= 1; } }
                    KeyCode::Down => { if sel.char_index + 1 < sel.characters.len() { sel.char_index += 1; } }
                    KeyCode::Left if has_worlds => { if sel.world_index > 0 { sel.world_index -= 1; } }
                    KeyCode::Right if has_worlds => { if sel.world_index + 1 < sel.worlds.len() { sel.world_index += 1; } }
                    KeyCode::Tab if has_worlds => { sel.world_index = (sel.world_index + 1) % sel.worlds.len(); }
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

    draw_list(f, chunks[0], " 角色 ", &sel.characters, sel.char_index, Color::Green);
    if has_worlds { draw_list(f, chunks[1], " 世界 ", &sel.worlds, sel.world_index, Color::Magenta); }
}

fn draw_list(f: &mut ratatui::Frame, area: Rect, title: &str, items: &[String], selected: usize, accent: Color) {
    let block = Block::default().title(format!(" {} ", title)).borders(Borders::ALL).border_style(Style::default().fg(accent));
    let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, name)| {
        if i == selected {
            ListItem::new(Line::from(vec![Span::styled(" ▶ ", Style::default().fg(accent).add_modifier(Modifier::BOLD)), Span::styled(name.clone(), Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD))]))
        } else {
            ListItem::new(Line::from(vec![Span::raw("   "), Span::styled(name.clone(), Style::default().fg(Color::DarkGray))]))
        }
    }).collect();
    f.render_widget(List::new(list_items).block(block), area);
}
