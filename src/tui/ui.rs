use crate::tui::app::{App, Message};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    if app.show_help {
        draw_help(f, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Chat area
            Constraint::Length(3), // Input area
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_chat(f, chunks[0], app);
    draw_input(f, chunks[1], app);
    draw_status(f, chunks[2], app);
}

fn draw_chat(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(format!(" {} ", app.character_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let lines: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|msg| render_message(msg, &app.character_name))
        .collect();

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
        _ => (
            Style::default().fg(Color::Yellow),
            Style::default().fg(Color::White),
        ),
    };

    let role_name = match msg.role.as_str() {
        "user" => "你",
        "assistant" => char_name,
        other => other,
    };

    let header = Line::from(vec![
        Span::styled(format!("[{}] ", role_name), header_style),
    ]);

    let body_lines: Vec<Line> = msg
        .content
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), content_style)))
        .collect();

    let mut result = vec![header];
    result.extend(body_lines);
    result.push(Line::from(""));
    result
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let title = if app.loading {
        " 输入 (思考中...) "
    } else {
        " 输入 (Enter 发送, Ctrl+C 退出, F1 帮助) "
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

    // Show cursor
    if !app.loading {
        let cursor_x = (area.x + 2 + app.cursor_pos as u16).min(area.right().saturating_sub(1));
        f.set_cursor_position((cursor_x, area.y + 1));
    }
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let status_text = if let Some(ref err) = app.error {
        Span::styled(format!(" 错误: {} ", err), Style::default().fg(Color::Red))
    } else if app.loading {
        Span::styled(" 等待回复... ", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(
            format!(
                " {} 条消息 | ↑↓ 滚动 | Esc 回到底部 ",
                app.messages.len()
            ),
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(Line::from(status_text));
    f.render_widget(paragraph, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled("快捷键帮助", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("Enter      - 发送消息"),
        Line::from("Ctrl+C     - 退出程序"),
        Line::from("F1         - 显示/隐藏帮助"),
        Line::from("↑ / ↓      - 向上/向下滚动聊天记录"),
        Line::from("PgUp/PgDn  - 快速滚动"),
        Line::from("Esc        - 跳转到最新消息"),
        Line::from("← / →      - 移动光标"),
        Line::from("Home/End   - 光标跳到行首/行尾"),
        Line::from(""),
        Line::from(Span::styled("按任意键关闭帮助", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(Text::from(help_text))
        .block(block)
        .centered();

    let popup_area = centered_rect(40, 60, area);
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
