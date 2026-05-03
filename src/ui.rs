use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    // Main area: depends on mode
    if app.mode == Mode::Map {
        if let Some(ref world_map) = app.world_map {
            let mut buf = frame.buffer_mut().clone();
            crate::map::renderer::render_map(world_map, &app.map_viewport, chunks[0], &mut buf, None);
            *frame.buffer_mut() = buf;
        }
    } else if app.mode == Mode::Combat {
        // Split main area: combat dashboard + output history
        let combat_height = app
            .initiative_tracker
            .as_ref()
            .map(|t| t.len() as u16 + 3) // combatants + border + header
            .unwrap_or(3)
            .min(chunks[0].height / 2)
            .max(3);

        let combat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(combat_height), Constraint::Min(0)])
            .split(chunks[0]);

        render_combat_dashboard(frame, app, combat_chunks[0]);
        render_output_history(frame, app, combat_chunks[1]);
    } else {
        render_output_history(frame, app, chunks[0]);
    }

    // Input bar
    let prompt_spans = app.prompt_spans();
    let prompt_len = app.prompt_len();
    let mut input_spans: Vec<Span> = prompt_spans;
    input_spans.push(Span::raw(app.input.clone()));
    if let Some(hint) = app.autocomplete_hint() {
        input_spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    }
    let input_bar =
        Paragraph::new(Line::from(input_spans)).block(Block::default().borders(Borders::ALL));
    frame.render_widget(input_bar, chunks[1]);

    // Position cursor after the prompt + user's cursor position
    // +1 for the left border
    let cursor_x = chunks[1].x + 1 + prompt_len as u16 + app.cursor_position as u16;
    let cursor_y = chunks[1].y + 1; // +1 for the top border
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn render_output_history(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| Line::from(Span::styled(msg.text.clone(), msg.style)))
        .collect();

    let inner_height = area.height.saturating_sub(2) as usize;
    let max_scroll = if lines.len() > inner_height {
        (lines.len() - inner_height) as u16
    } else {
        0
    };
    let scroll = max_scroll.saturating_sub(app.scroll_offset).min(max_scroll);

    let main_area = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(main_area, area);
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
