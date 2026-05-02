use ratatui::layout::{Constraint, Direction, Layout, Position};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

const PROMPT: &str = "rustory > ";

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new("")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Rustory")
                .title_style(Color::Cyan),
        );
    frame.render_widget(header, chunks[0]);

    // Build output history from messages
    let lines: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| Line::from(Span::styled(msg.text.clone(), msg.style)))
        .collect();

    // Calculate scroll: auto-scroll to bottom
    // Inner height = chunk height - 2 (borders)
    let inner_height = chunks[1].height.saturating_sub(2) as usize;
    let scroll = if lines.len() > inner_height {
        (lines.len() - inner_height) as u16
    } else {
        0
    };

    let main_area = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(main_area, chunks[1]);

    let mut input_spans = vec![
        Span::raw(PROMPT.to_string()),
        Span::raw(app.input.clone()),
    ];
    if let Some(hint) = app.autocomplete_hint() {
        input_spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    }
    let input_bar = Paragraph::new(Line::from(input_spans))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(input_bar, chunks[2]);

    // Position cursor after the prompt + user's cursor position
    // +1 for the left border
    let cursor_x = chunks[2].x + 1 + PROMPT.len() as u16 + app.cursor_position as u16;
    let cursor_y = chunks[2].y + 1; // +1 for the top border
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}
