use ratatui::layout::{Constraint, Direction, Layout, Position};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Paragraph};
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

    let main_area = Paragraph::new("")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(main_area, chunks[1]);

    let input_text = format!("{}{}", PROMPT, app.input);
    let input_bar = Paragraph::new(input_text)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(input_bar, chunks[2]);

    // Position cursor after the prompt + user's cursor position
    // +1 for the left border
    let cursor_x = chunks[2].x + 1 + PROMPT.len() as u16 + app.cursor_position as u16;
    let cursor_y = chunks[2].y + 1; // +1 for the top border
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}
