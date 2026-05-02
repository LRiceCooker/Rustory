use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rand::RngCore;
use ratatui::style::{Color, Style};
use ratatui::DefaultTerminal;

use crate::commands::dispatcher::{self, CommandResult, StyledLine};
use crate::commands::mapping;
use crate::game_state::loader;
use crate::game_state::GameState;
use crate::ui;

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub style: Style,
}

pub struct App {
    pub running: bool,
    pub input: String,
    pub cursor_position: usize,
    pub last_command: Option<String>,
    pub messages: Vec<Message>,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: u16,
    pub rng: Box<dyn RngCore>,
    pub game_state: Option<GameState>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self::with_rng(Box::new(rand::thread_rng()))
    }

    pub fn with_rng(rng: Box<dyn RngCore>) -> Self {
        Self {
            running: false,
            input: String::new(),
            cursor_position: 0,
            last_command: None,
            messages: Vec::new(),
            command_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            rng,
            game_state: None,
        }
    }

    /// Load a campaign from a directory path into the game state.
    /// Delegates to `GameState::load()` which parses `rules/system.toml`
    /// if present, then scans `players/` and `npc/` subdirectories.
    /// Returns a list of load errors (empty if all loaded successfully).
    pub fn load_campaign(&mut self, path: &Path) -> Vec<loader::LoadError> {
        let (gs, errors) = GameState::load(path);
        self.game_state = Some(gs);
        errors
    }

    pub fn game_state(&self) -> Option<&GameState> {
        self.game_state.as_ref()
    }

    pub fn game_state_mut(&mut self) -> Option<&mut GameState> {
        self.game_state.as_mut()
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

        // Parse command and args for app-level commands
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0];
        let args = parts.get(1).unwrap_or(&"").trim();

        // Handle app-level commands that need mutable self
        if command == mapping::LOAD {
            self.handle_load_command(args);
            return;
        }

        // Dispatch and handle the result via the stateless dispatcher
        let result = dispatcher::dispatch(input, &mut self.rng);
        self.apply_command_result(result);
    }

    fn apply_command_result(&mut self, result: CommandResult) {
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

    fn handle_load_command(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: load <path> (e.g. load sample)".to_string(),
            ));
            return;
        }

        let path = PathBuf::from(args);

        // Validate path exists
        if !path.exists() {
            self.apply_command_result(CommandResult::Error(format!(
                "Path not found: \"{}\"",
                path.display()
            )));
            return;
        }

        if !path.is_dir() {
            self.apply_command_result(CommandResult::Error(format!(
                "\"{}\" is not a directory.",
                path.display()
            )));
            return;
        }

        // Validate folder structure: rules/ must exist
        let rules_dir = path.join("rules");
        if !rules_dir.exists() || !rules_dir.is_dir() {
            self.apply_command_result(CommandResult::Error(format!(
                "Campaign folder \"{}\" is missing a rules/ directory.\n  \
                 A valid campaign must have a rules/ folder containing system.toml.",
                path.display()
            )));
            return;
        }

        // Load the campaign
        let (gs, errors) = GameState::load(&path);

        if !errors.is_empty() {
            // Show all errors, do NOT load the campaign
            let mut lines = vec![StyledLine::new(
                format!("Failed to load campaign from \"{}\":", path.display()),
                Style::default().fg(Color::Red),
            )];
            for error in &errors {
                lines.push(StyledLine::new(
                    format!("  {error}"),
                    Style::default().fg(Color::Red),
                ));
            }
            self.apply_command_result(CommandResult::Output(lines));
            return;
        }

        // Success — store game state
        let campaign_name = gs.campaign_name.clone();
        let player_count = gs.players.len();
        let npc_count = gs.npcs.len();
        self.game_state = Some(gs);

        let mut lines = vec![StyledLine::new(
            format!("Campaign \"{campaign_name}\" loaded successfully."),
            Style::default().fg(Color::Green),
        )];
        lines.push(StyledLine::new(
            format!("  {player_count} player(s), {npc_count} NPC(s)"),
            Style::default().fg(Color::Green),
        ));
        if let Some(ref gs) = self.game_state {
            if let Some(ref rules) = gs.rules {
                lines.push(StyledLine::new(
                    format!("  System: {}", rules.system_name),
                    Style::default().fg(Color::Green),
                ));
            }
        }
        self.apply_command_result(CommandResult::Output(lines));
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
    use tempfile::TempDir;

    #[test]
    fn test_game_state_none_by_default() {
        let app = App::new();
        assert!(app.game_state().is_none());
    }

    #[test]
    fn test_load_campaign_sets_game_state() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules")).unwrap();

        let mut app = App::new();
        let errors = app.load_campaign(dir.path());

        assert!(errors.is_empty());
        assert!(app.game_state().is_some());
        let gs = app.game_state().unwrap();
        assert!(gs.players.is_empty());
        assert!(gs.npcs.is_empty());
    }

    #[test]
    fn test_load_campaign_with_players_and_npcs() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules")).unwrap();

        let player_dir = dir.path().join("players/thorin");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(
            player_dir.join("sheet.csv"),
            "name,strength,dexterity\nThorin,18,12\n",
        )
        .unwrap();

        let npc_dir = dir.path().join("npc/goblin");
        std::fs::create_dir_all(&npc_dir).unwrap();
        std::fs::write(
            npc_dir.join("sheet.csv"),
            "name,strength,hp_max\nGoblin,8,7\n",
        )
        .unwrap();

        let mut app = App::new();
        let errors = app.load_campaign(dir.path());

        assert!(errors.is_empty());
        let gs = app.game_state().unwrap();
        assert_eq!(gs.players.len(), 1);
        assert_eq!(gs.players[0].name, "Thorin");
        assert_eq!(gs.players[0].get_stat("strength"), Some(18.0));
        assert_eq!(gs.npcs.len(), 1);
        assert_eq!(gs.npcs[0].name, "Goblin");
    }

    #[test]
    fn test_load_campaign_missing_dirs_ok() {
        let dir = TempDir::new().unwrap();
        // No players/ or npc/ directories — should still work fine

        let mut app = App::new();
        let errors = app.load_campaign(dir.path());

        assert!(errors.is_empty());
        let gs = app.game_state().unwrap();
        assert!(gs.players.is_empty());
        assert!(gs.npcs.is_empty());
    }

    #[test]
    fn test_game_state_mut_allows_modification() {
        let dir = TempDir::new().unwrap();
        let mut app = App::new();
        app.load_campaign(dir.path());

        let gs = app.game_state_mut().unwrap();
        gs.add_player(crate::game_state::Character::new("TestPlayer"));

        assert_eq!(app.game_state().unwrap().players.len(), 1);
    }

    #[test]
    fn test_campaign_name_from_path() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("my_campaign");
        std::fs::create_dir_all(&campaign_dir).unwrap();

        let mut app = App::new();
        app.load_campaign(&campaign_dir);

        assert_eq!(app.game_state().unwrap().campaign_name, "my_campaign");
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

    // --- load command tests ---

    #[test]
    fn test_load_command_valid_campaign() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("my_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();

        let player_dir = campaign_dir.join("players/hero");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(player_dir.join("sheet.csv"), "name,strength\nHero,15\n").unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));

        // Should have game state loaded
        assert!(app.game_state().is_some());
        let gs = app.game_state().unwrap();
        assert_eq!(gs.campaign_name, "my_campaign");
        assert_eq!(gs.players.len(), 1);
        assert_eq!(gs.players[0].name, "Hero");

        // Output should contain success message
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts
            .iter()
            .any(|t| t.contains("loaded successfully")));
    }

    #[test]
    fn test_load_command_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("load");

        assert!(app.game_state().is_none());
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Usage")));
    }

    #[test]
    fn test_load_command_nonexistent_path() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("load /nonexistent/path/12345");

        assert!(app.game_state().is_none());
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("not found")));
    }

    #[test]
    fn test_load_command_missing_rules_dir() {
        let dir = TempDir::new().unwrap();
        // Create a dir with no rules/ folder
        let campaign_dir = dir.path().join("bad_campaign");
        std::fs::create_dir_all(&campaign_dir).unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));

        assert!(app.game_state().is_none());
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("rules/")));
    }

    #[test]
    fn test_load_command_malformed_csv_shows_error() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("bad_csv_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\", \"dexterity\"]\n",
        )
        .unwrap();

        // Create a player with wrong columns (missing dexterity)
        let player_dir = campaign_dir.join("players/broken");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(
            player_dir.join("sheet.csv"),
            "name,strength\nBroken,10\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));

        // Should NOT load the campaign
        assert!(
            app.game_state().is_none(),
            "game_state should be None when load fails"
        );
        // Error message should mention the missing column
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("dexterity")),
            "Error should mention the missing column. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_load_command_help_includes_load() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("help");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("load")),
            "help should list the load command"
        );
    }
}
