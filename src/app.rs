use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use crate::ui;

#[derive(Debug, Default)]
pub struct App {
    pub running: bool,
}

impl App {
    pub fn new() -> Self {
        Self { running: false }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        self.running = true;
        while self.running {
            terminal.draw(|frame| ui::render(frame, &self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> color_eyre::Result<()> {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
            _ => {}
        }
        Ok(())
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) => self.running = false,
            (KeyCode::Esc, _) => self.running = false,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.running = false,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_on_q() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn test_quit_on_esc() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.running);
    }

    #[test]
    fn test_quit_on_ctrl_c() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.running);
    }

    #[test]
    fn test_running_stays_true_on_other_keys() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Char('a')));
        assert!(app.running);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.running);
        app.on_key(KeyEvent::from(KeyCode::Left));
        assert!(app.running);
    }
}
