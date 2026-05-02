use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui::DefaultTerminal;

use crate::commands::dispatcher::{self, CommandResult};
use crate::ui;

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub style: Style,
}

#[derive(Debug, Default)]
pub struct App {
    pub running: bool,
    pub input: String,
    pub cursor_position: usize,
    pub last_command: Option<String>,
    pub messages: Vec<Message>,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: u16,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: false,
            input: String::new(),
            cursor_position: 0,
            last_command: None,
            messages: Vec::new(),
            command_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
        }
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
            (KeyCode::Esc, _) => self.running = false,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.running = false,
            (KeyCode::Enter, _) => self.submit_input(),
            (KeyCode::Char(c), _) => self.insert_char(c),
            (KeyCode::Backspace, _) => self.delete_char_before(),
            (KeyCode::Delete, _) => self.delete_char_at(),
            (KeyCode::Up, _) => self.history_prev(),
            (KeyCode::Down, _) => self.history_next(),
            (KeyCode::PageUp, _) => self.scroll_up(10),
            (KeyCode::PageDown, _) => self.scroll_down(10),
            (KeyCode::Left, _) => self.move_cursor_left(),
            (KeyCode::Right, _) => self.move_cursor_right(),
            (KeyCode::Home, _) => self.cursor_position = 0,
            (KeyCode::End, _) => self.cursor_position = self.input.len(),
            _ => {}
        }
    }

    fn submit_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            self.input.clear();
            self.cursor_position = 0;
            return;
        }

        self.command_history.push(input.clone());
        self.history_index = None;
        self.dispatch_command(&input);

        self.input.clear();
        self.cursor_position = 0;
    }

    pub fn dispatch_command(&mut self, input: &str) {
        self.last_command = Some(input.to_string());
        self.scroll_offset = 0;

        // Echo the command in DarkGray
        self.messages.push(Message {
            text: format!("> {input}"),
            style: Style::default().fg(Color::DarkGray),
        });

        // Dispatch and handle the result
        let result = dispatcher::dispatch(input);
        match result {
            CommandResult::Output(lines) => {
                for line in lines {
                    self.messages.push(Message {
                        text: line.text,
                        style: line.style,
                    });
                }
            }
            CommandResult::Error(msg) => {
                self.messages.push(Message {
                    text: msg,
                    style: Style::default().fg(Color::Red),
                });
            }
            CommandResult::Quit => {
                self.running = false;
            }
            CommandResult::Unknown(cmd) => {
                self.messages.push(Message {
                    text: format!("Unknown command: \"{cmd}\". Type \"help\" for a list."),
                    style: Style::default().fg(Color::Red),
                });
            }
        }
    }

    fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let new_index = match self.history_index {
            Some(0) => 0,
            Some(i) => i - 1,
            None => self.command_history.len() - 1,
        };
        self.history_index = Some(new_index);
        self.input = self.command_history[new_index].clone();
        self.cursor_position = self.input.len();
    }

    fn history_next(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        match self.history_index {
            Some(i) if i + 1 < self.command_history.len() => {
                let new_index = i + 1;
                self.history_index = Some(new_index);
                self.input = self.command_history[new_index].clone();
                self.cursor_position = self.input.len();
            }
            Some(_) => {
                // Past the end of history — clear input
                self.history_index = None;
                self.input.clear();
                self.cursor_position = 0;
            }
            None => {}
        }
    }

    fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn autocomplete_hint(&self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }
        let input_lower = self.input.to_lowercase();
        crate::commands::mapping::COMMANDS
            .iter()
            .find(|cmd| cmd.starts_with(&input_lower) && **cmd != input_lower)
            .map(|cmd| cmd[self.input.len()..].to_string())
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    fn delete_char_before(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.prev_char_boundary();
            self.input.drain(prev..self.cursor_position);
            self.cursor_position = prev;
        }
    }

    fn delete_char_at(&mut self) {
        if self.cursor_position < self.input.len() {
            let next = self.next_char_boundary();
            self.input.drain(self.cursor_position..next);
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position = self.prev_char_boundary();
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.len() {
            self.cursor_position = self.next_char_boundary();
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.cursor_position - 1;
        while !self.input.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor_position + 1;
        while pos < self.input.len() && !self.input.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_type_characters() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Char('h')));
        app.on_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn test_backspace_at_middle() {
        let mut app = App::new();
        app.running = true;
        app.input = "abc".to_string();
        app.cursor_position = 2;
        app.on_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.input, "ac");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn test_backspace_at_end() {
        let mut app = App::new();
        app.running = true;
        app.input = "abc".to_string();
        app.cursor_position = 3;
        app.on_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.input, "ab");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_delete_at_cursor() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.cursor_position = 1;
        app.on_key(KeyEvent::from(KeyCode::Delete));
        assert_eq!(app.input, "ac");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn test_delete_at_end_does_nothing() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.cursor_position = 3;
        app.on_key(KeyEvent::from(KeyCode::Delete));
        assert_eq!(app.input, "abc");
    }

    #[test]
    fn test_cursor_left_right() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.cursor_position = 3;
        app.on_key(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.cursor_position, 2);
        app.on_key(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.cursor_position, 1);
        app.on_key(KeyEvent::from(KeyCode::Right));
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_cursor_left_at_start() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.cursor_position = 0;
        app.on_key(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn test_cursor_right_at_end() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.cursor_position = 3;
        app.on_key(KeyEvent::from(KeyCode::Right));
        assert_eq!(app.cursor_position, 3);
    }

    #[test]
    fn test_home_end() {
        let mut app = App::new();
        app.input = "hello".to_string();
        app.cursor_position = 3;
        app.on_key(KeyEvent::from(KeyCode::Home));
        assert_eq!(app.cursor_position, 0);
        app.on_key(KeyEvent::from(KeyCode::End));
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    fn test_insert_at_middle() {
        let mut app = App::new();
        app.input = "ac".to_string();
        app.cursor_position = 1;
        app.on_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_enter_clears_input_and_stores_command() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Char('h')));
        app.on_key(KeyEvent::from(KeyCode::Char('i')));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
        assert_eq!(app.last_command, Some("hi".to_string()));
    }

    #[test]
    fn test_enter_on_empty_input() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "");
        assert_eq!(app.last_command, None);
    }

    #[test]
    fn test_q_types_into_input() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Char('q')));
        assert!(app.running);
        assert_eq!(app.input, "q");
    }

    #[test]
    fn test_quit_command_stops_app() {
        let mut app = App::new();
        app.running = true;
        app.input = "quit".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.running);
    }

    #[test]
    fn test_unknown_command_adds_error_message() {
        let mut app = App::new();
        app.running = true;
        app.input = "foobar".to_string();
        app.cursor_position = 6;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.running);
        // Echo + error = 2 messages
        assert_eq!(app.messages.len(), 2);
        assert!(app.messages[1].text.contains("Unknown command"));
    }

    #[test]
    fn test_help_command_adds_output_messages() {
        let mut app = App::new();
        app.running = true;
        app.input = "help".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.running);
        // Echo + at least 1 output line
        assert!(app.messages.len() >= 2);
    }

    #[test]
    fn test_history_up_cycles_back() {
        let mut app = App::new();
        app.running = true;
        // Enter 3 commands
        app.input = "help".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        app.input = "roll 1d6".to_string();
        app.cursor_position = 8;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        app.input = "help".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));

        // Up arrow cycles back
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "help");
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "roll 1d6");
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "help");
        // At the beginning, stays there
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "help");
    }

    #[test]
    fn test_history_down_cycles_forward() {
        let mut app = App::new();
        app.running = true;
        app.input = "help".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        app.input = "roll 1d6".to_string();
        app.cursor_position = 8;
        app.on_key(KeyEvent::from(KeyCode::Enter));

        // Go up twice
        app.on_key(KeyEvent::from(KeyCode::Up));
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "help");

        // Down goes forward
        app.on_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.input, "roll 1d6");

        // Down past end clears input
        app.on_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn test_history_up_on_empty_does_nothing() {
        let mut app = App::new();
        app.running = true;
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "");
    }

    #[test]
    fn test_scroll_up_down() {
        let mut app = App::new();
        app.on_key(KeyEvent::from(KeyCode::PageUp));
        assert_eq!(app.scroll_offset, 10);
        app.on_key(KeyEvent::from(KeyCode::PageUp));
        assert_eq!(app.scroll_offset, 20);
        app.on_key(KeyEvent::from(KeyCode::PageDown));
        assert_eq!(app.scroll_offset, 10);
        app.on_key(KeyEvent::from(KeyCode::PageDown));
        assert_eq!(app.scroll_offset, 0);
        // Can't go below 0
        app.on_key(KeyEvent::from(KeyCode::PageDown));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_resets_on_submit() {
        let mut app = App::new();
        app.running = true;
        app.scroll_offset = 20;
        app.input = "help".to_string();
        app.cursor_position = 4;
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.scroll_offset, 0);
    }
}
