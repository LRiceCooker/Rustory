use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, _app: &App) {
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

    let input_bar = Paragraph::new("rustory > ")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(input_bar, chunks[2]);
}
