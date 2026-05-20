use crate::tui::app::{App, Message, Wizard};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    if app.show_help {
        draw_help(f, area);
        return;
    }

    // Main layout: sidebar | chat+input+status
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20), // sidebar
            Constraint::Min(1),     // chat area
        ])
        .split(area);

    draw_sidebar(f, main_chunks[0], app);

    let right = main_chunks[1];
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // chat
            Constraint::Length(3), // input
            Constraint::Length(1), // status
        ])
        .split(right);

    draw_chat(f, right_chunks[0], app);
    draw_input(f, right_chunks[1], app);
    draw_status(f, right_chunks[2], app);
}

fn draw_sidebar(f: &mut Frame, area: Rect, app: &App) {
    let has_worlds = !app.manager.worlds.is_empty();
    let chunks = if has_worlds {
        Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3 + app.manager.worlds.len().min(5) as u16)])
            .split(area)
    } else {
        Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1)]).split(area)
    };

    // Character list
    let block = Block::default().title(" 角色 ").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan));
    let items: Vec<ListItem> = app.manager.order.iter().enumerate().map(|(i, name)| {
        if i == app.manager.active_index {
            ListItem::new(Line::from(vec![Span::styled(" ▶ ", Style::default().fg(Color::Green)), Span::styled(name.clone(), Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))]))
        } else {
            ListItem::new(Line::from(vec![Span::raw("   "), Span::styled(name.clone(), Style::default().fg(Color::DarkGray))]))
        }
    }).collect();
    f.render_widget(List::new(items).block(block), chunks[0]);

    // World list
    if has_worlds {
        let world_block = Block::default().title(" 世界 ").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta));
        let world_items: Vec<ListItem> = app.manager.worlds.iter().enumerate().map(|(i, w)| {
            let is_active = app.manager.active_world == Some(i);
            if is_active {
                ListItem::new(Line::from(vec![Span::styled(" ◆ ", Style::default().fg(Color::Magenta)), Span::styled(w.slug.clone(), Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD))]))
            } else {
                ListItem::new(Line::from(vec![Span::raw("   "), Span::styled(w.slug.clone(), Style::default().fg(Color::DarkGray))]))
            }
        }).chain(std::iter::once({
            let is_none = app.manager.active_world.is_none();
            if is_none {
                ListItem::new(Line::from(vec![Span::styled(" ◆ ", Style::default().fg(Color::Yellow)), Span::styled("全部", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))]))
            } else {
                ListItem::new(Line::from(vec![Span::raw("   "), Span::styled("全部", Style::default().fg(Color::DarkGray))]))
            }
        })).collect();
        f.render_widget(List::new(world_items).block(world_block), chunks[1]);
    }
}

fn draw_chat(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(format!(" {} ", app.character_name()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines: Vec<Line> = app.manager.active().messages
        .iter()
        .flat_map(|msg| render_message(msg, app.character_name()))
        .collect();

    if app.is_streaming && !app.streaming.is_empty() {
        let header = Line::from(vec![
            Span::styled(
                format!("[{}] ", app.character_name()),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]);
        lines.push(header);
        let content_with_cursor = format!("{}▌", app.streaming);
        for content_line in content_with_cursor.lines() {
            lines.push(Line::from(Span::styled(
                content_line.to_string(),
                Style::default().fg(Color::Gray),
            )));
        }
        lines.push(Line::from(""));
    } else if app.is_streaming {
        lines.push(Line::from(Span::styled("▌", Style::default().fg(Color::Magenta))));
        lines.push(Line::from(""));
    }

    let visible_height = area.height.saturating_sub(2) as usize;
    let total_lines = lines.len().max(1);
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = app.scroll_offset.min(max_scroll);
    let start = total_lines.saturating_sub(visible_height).saturating_sub(scroll);

    let visible: Vec<Line> = if total_lines <= visible_height {
        lines
    } else {
        lines.into_iter().skip(start).take(visible_height).collect()
    };

    let paragraph = Paragraph::new(Text::from(visible))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn render_message<'a>(msg: &'a Message, char_name: &'a str) -> Vec<Line<'a>> {
    let (header_style, content_style) = match msg.role.as_str() {
        "user" => (
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::White),
        ),
        "assistant" => (
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
        "system" => (
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::DarkGray),
        ),
        _ => (
            Style::default().fg(Color::Yellow),
            Style::default().fg(Color::White),
        ),
    };

    let role_name = match msg.role.as_str() {
        "user" => "你",
        "assistant" => char_name,
        "system" => "系统",
        other => other,
    };

    let header = Line::from(vec![
        Span::styled(format!("[{}] ", role_name), header_style),
    ]);

    let body_lines: Vec<Line> = msg.content
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), content_style)))
        .collect();

    let mut result = vec![header];
    result.extend(body_lines);
    result.push(Line::from(""));
    result
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let title = if let Some(ref wiz) = app.wizard {
        match wiz {
            Wizard::CreateWorld { name, step, .. } => {
                match step {
                    0 => format!(" 创建世界: 「{}」 - 输入一句话描述 ", name),
                    1 => format!(" 创建世界: 「{}」 - 输入世界观概述 ", name),
                    _ => " 输入 ".into(),
                }
            }
            Wizard::CreateChar { name, step, .. } => {
                match step {
                    0 => format!(" 创建角色: 「{}」 - 输入显示名 ", name),
                    1 => format!(" 创建角色: 「{}」 - 输入性格描述 ", name),
                    2 => format!(" 创建角色: 「{}」 - 输入说话风格 ", name),
                    3 => format!(" 创建角色: 「{}」 - 输入开场白 ", name),
                    _ => " 输入 ".into(),
                }
            }
        }
    } else if app.loading {
        " 输入 (思考中...) ".into()
    } else {
        " 输入 (Enter/Tab, Esc打断, Ctrl+C/V, F1) ".into()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let display_text = if app.input.is_empty() {
        "".to_string()
    } else {
        format!("> {}", app.input)
    };

    let input_paragraph = Paragraph::new(display_text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(input_paragraph, area);

    if !app.loading {
        let prefix_width = 2u16;
        let visual_pos: u16 = app.input.chars()
            .take(app.cursor_pos)
            .map(|c| if is_wide(c) { 2u16 } else { 1u16 })
            .sum();
        let cursor_x = (area.x + prefix_width + visual_pos).min(area.right().saturating_sub(1));
        f.set_cursor_position((cursor_x, area.y + 1));
    }
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let status_text = if let Some(ref err) = app.error {
        Span::styled(format!(" 错误: {} ", err), Style::default().fg(Color::Red))
    } else if app.is_streaming {
        Span::styled(" 流式输出中... ", Style::default().fg(Color::Yellow))
    } else if app.loading {
        Span::styled(" 等待回复... ", Style::default().fg(Color::Yellow))
    } else {
        let active_info = if app.active_lore_keys.is_empty() {
            String::new()
        } else {
            format!(" [世界: {}] ", app.active_lore_keys.join(", "))
        };
        Span::styled(
            format!("{} 条消息 | ↑↓滚动 | Tab切换角色{}",
                app.manager.active().messages.len(),
                active_info),
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(Line::from(status_text));
    f.render_widget(paragraph, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled("快捷键 & 命令帮助", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("Enter      - 发送消息"),
        Line::from("Tab        - 切换到下一个角色"),
        Line::from("Shift+Tab  - 切换到上一个角色"),
        Line::from("Ctrl+C     - 复制输入框内容"),
        Line::from("Ctrl+V     - 粘贴剪贴板内容"),
        Line::from("F1         - 显示/隐藏帮助"),
        Line::from("Esc        - 打断回复 / 跳转到最新消息"),
        Line::from("↑ / ↓      - 滚动聊天记录"),
        Line::from("PgUp/PgDn  - 快速滚动"),
        Line::from(""),
        Line::from(Span::styled("命令:", Style::default().fg(Color::Cyan))),
        Line::from("/exit      - 保存并退出"),
        Line::from("/save      - 手动保存会话"),
        Line::from("/load <id> - 加载会话"),
        Line::from("/clear     - 清除当前角色对话"),
        Line::from("/cc <name> - 创建角色卡"),
        Line::from("/cw <name> - 创建世界词条"),
        Line::from("/self <text> - 设置用户本人的设定"),
        Line::from("/state <cmd> - 管理角色状态 (add/get item/event/skill/status)"),
        Line::from("/switch X  - 切换到角色 X"),
        Line::from("/help      - 显示帮助"),
        Line::from("?X         - 查看角色 X 的信息"),
        Line::from("?list      - 列出所有角色"),
        Line::from("@X         - 在消息中引用角色 X"),
        Line::from(""),
        Line::from(Span::styled("按任意键关闭帮助", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(Text::from(help_text))
        .block(block)
        .centered();

    let popup_area = centered_rect(50, 70, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_width = (r.width * percent_x) / 100;
    let popup_height = (r.height * percent_y) / 100;
    let x = r.x + (r.width.saturating_sub(popup_width)) / 2;
    let y = r.y + (r.height.saturating_sub(popup_height)) / 2;
    Rect::new(x, y, popup_width.min(r.width), popup_height.min(r.height))
}

fn is_wide(c: char) -> bool {
    matches!(
        c,
        '\u{1100}'..='\u{115F}'
        | '\u{2329}'..='\u{232A}'
        | '\u{2E80}'..='\u{A4CF}'
        | '\u{AC00}'..='\u{D7A3}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FE10}'..='\u{FE19}'
        | '\u{FE30}'..='\u{FE6F}'
        | '\u{FF01}'..='\u{FF60}'
        | '\u{FFE0}'..='\u{FFE6}'
        | '\u{1F300}'..='\u{1F64F}'
        | '\u{1F900}'..='\u{1F9FF}'
        | '\u{20000}'..='\u{2FFFD}'
        | '\u{30000}'..='\u{3FFFD}'
    )
}
