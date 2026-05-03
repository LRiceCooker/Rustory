use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &mut App) {
    if app.mode == Mode::Map {
        // Map mode: map on top, input line at the bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(frame.area());

        if app.world_map.is_some() {
            let mut buf = frame.buffer_mut().clone();
            crate::map::renderer::render_map(
                app.world_map.as_ref().unwrap(),
                &app.map_viewport,
                chunks[0],
                &mut buf,
                app.map_image_protocol.as_mut(),
            );
            *frame.buffer_mut() = buf;
        }

        render_input_line(frame, app, chunks[1]);
    } else if app.mode == Mode::Combat {
        // Combat: dashboard on top, unified output+input below
        let combat_height = app
            .initiative_tracker
            .as_ref()
            .map(|t| t.len() as u16 + 3)
            .unwrap_or(3)
            .min(frame.area().height / 2)
            .max(3);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(combat_height), Constraint::Min(0)])
            .split(frame.area());

        render_combat_dashboard(frame, app, chunks[0]);
        render_unified(frame, app, chunks[1]);
    } else {
        // Default: single unified terminal-like view
        render_unified(frame, app, frame.area());
    }
}

/// Renders messages + input as a single continuous stream, like a real terminal.
fn render_unified(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| Line::from(Span::styled(msg.text.clone(), msg.style)))
        .collect();

    // Append the current input line as the last line in the stream
    let prompt_spans = app.prompt_spans();
    let prompt_len = app.prompt_len();
    let mut input_spans: Vec<Span> = prompt_spans;
    input_spans.push(Span::raw(app.input.clone()));
    if let Some(hint) = app.autocomplete_hint() {
        input_spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    }
    lines.push(Line::from(input_spans));

    let height = area.height as usize;
    let width = area.width as usize;
    let total_rows: usize = lines
        .iter()
        .map(|line| {
            let w = line.width();
            if w == 0 || width == 0 {
                1
            } else {
                (w + width - 1) / width
            }
        })
        .sum();
    let max_scroll = total_rows.saturating_sub(height) as u16;
    let scroll = max_scroll.saturating_sub(app.scroll_offset).min(max_scroll);

    let paragraph = Paragraph::new(lines.clone())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);

    // Cursor: sits on the input line (last line in the stream)
    let input_line_y = (total_rows as u16).saturating_sub(1).saturating_sub(scroll);
    if input_line_y < area.height {
        let cursor_x = area.x + prompt_len as u16 + app.cursor_position as u16;
        let cursor_y = area.y + input_line_y;
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

/// Renders just the input line (used in map mode where the main area is graphical).
fn render_input_line(frame: &mut Frame, app: &mut App, area: Rect) {
    let prompt_spans = app.prompt_spans();
    let prompt_len = app.prompt_len();
    let mut input_spans: Vec<Span> = prompt_spans;
    input_spans.push(Span::raw(app.input.clone()));
    if let Some(hint) = app.autocomplete_hint() {
        input_spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    }
    let input_bar = Paragraph::new(Line::from(input_spans));
    frame.render_widget(input_bar, area);

    let cursor_x = area.x + prompt_len as u16 + app.cursor_position as u16;
    let cursor_y = area.y;
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn render_combat_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let tracker = match app.initiative_tracker.as_ref() {
        Some(t) => t,
        None => {
            let empty = Paragraph::new("No initiative tracker.")
                .block(Block::default().borders(Borders::ALL).title("Combat"));
            frame.render_widget(empty, area);
            return;
        }
    };

    if tracker.is_empty() {
        let empty = Paragraph::new("No combatants. Use 'init add <name> <value>'.")
            .block(Block::default().borders(Borders::ALL).title("Combat"));
        frame.render_widget(empty, area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, combatant) in tracker.all().iter().enumerate() {
        let marker = if combatant.is_current { ">> " } else { "   " };
        let mut spans = vec![Span::raw(format!("{marker}{}. ", i + 1))];

        // Name — highlight if current
        if combatant.is_current {
            spans.push(Span::styled(
                combatant.name.clone(),
                Style::default().fg(Color::Yellow).bold(),
            ));
        } else {
            spans.push(Span::raw(combatant.name.clone()));
        }

        spans.push(Span::raw(format!("  (init: {})", combatant.initiative)));

        // HP bar from game state
        if let Some(gs) = &app.game_state {
            let character = gs
                .players
                .iter()
                .chain(gs.npcs.iter())
                .find(|c| c.name == combatant.name);
            if let Some(ch) = character {
                if let Some(gauge) = ch.gauges.get("hp") {
                    let ratio = if gauge.max > 0.0 {
                        gauge.current / gauge.max
                    } else {
                        1.0
                    };
                    let hp_color = if ratio > 0.5 {
                        Color::Green
                    } else if ratio > 0.25 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };
                    spans.push(Span::styled(
                        format!("  HP: {}/{}", gauge.current, gauge.max),
                        Style::default().fg(hp_color),
                    ));
                }

                let conditions: Vec<&str> = ch
                    .conditions
                    .iter()
                    .filter(|c| c.active)
                    .map(|c| c.name.as_str())
                    .collect();
                if !conditions.is_empty() {
                    spans.push(Span::styled(
                        format!("  [{}]", conditions.join(", ")),
                        Style::default().fg(Color::Magenta),
                    ));
                }
            }
        }

        lines.push(Line::from(spans));
    }

    let dashboard =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Initiative"));
    frame.render_widget(dashboard, area);
}
