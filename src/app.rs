use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rand::{Rng, RngCore};
use ratatui::style::{Color, Style};
use ratatui::DefaultTerminal;

use crate::audio::library::SoundLibrary;
use crate::audio::player::AudioPlayer;
use crate::combat::initiative::InitiativeTracker;
use crate::persistence::PersistenceLayer;
use crate::commands::dispatcher::{self, CommandResult, StyledLine};
use crate::commands::mapping;
use crate::game_state::loader;
use crate::game_state::GameState;
use crate::map::renderer::MapViewport;
use crate::map::world::WorldMap;
use crate::scripting::api::{ScriptContext, SoundCommand};
use crate::scripting::engine::ScriptEngine;
use crate::scripting::loader::LolScript;
use crate::ui;

/// Recursively copy a directory and all its contents.
fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Default,
    Map,
    Combat,
}

impl Mode {
    pub fn prompt(&self) -> &'static str {
        match self {
            Mode::Default => "rustory > ",
            Mode::Map => "rustory [map] > ",
            Mode::Combat => "rustory [combat] > ",
        }
    }
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
    pub mode: Mode,
    pub map_viewport: MapViewport,
    pub world_map: Option<WorldMap>,
    pub audio_player: Option<AudioPlayer>,
    pub sound_library: SoundLibrary,
    pub persistence: Option<PersistenceLayer>,
    pub redo_stack: Vec<String>,
    pub initiative_tracker: Option<InitiativeTracker>,
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
            mode: Mode::Default,
            map_viewport: MapViewport::default(),
            world_map: None,
            audio_player: AudioPlayer::new().ok(),
            sound_library: SoundLibrary::default(),
            persistence: None,
            redo_stack: Vec::new(),
            initiative_tracker: None,
        }
    }

    /// Load a campaign from a directory path into the game state.
    /// Delegates to `GameState::load()` which parses `rules/system.toml`
    /// if present, then scans `players/` and `npc/` subdirectories.
    /// Returns a list of load errors (empty if all loaded successfully).
    pub fn load_campaign(&mut self, path: &Path) -> Vec<loader::LoadError> {
        let (gs, errors) = GameState::load(path);
        self.game_state = Some(gs);

        // Load WorldMap from map/world.json if present (optional)
        let map_json = path.join("map").join("world.json");
        if map_json.exists() {
            match WorldMap::load(&map_json) {
                Ok(wm) => self.world_map = Some(wm),
                Err(_) => self.world_map = None,
            }
        } else {
            self.world_map = None;
        }
        self.mode = Mode::Default;
        self.map_viewport = MapViewport::default();

        // Load SoundLibrary from sound/ if present (optional)
        let sound_dir = path.join("sound");
        self.sound_library = SoundLibrary::scan(&sound_dir).unwrap_or_default();

        // Initialize git persistence (creates .git/ if missing, commits manual edits)
        match PersistenceLayer::init(path) {
            Ok(pl) => {
                // Auto-commit any manual edits detected
                let _ = pl.commit_manual_edits();
                self.persistence = Some(pl);
            }
            Err(_) => {
                self.persistence = None;
            }
        }

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
        if self.mode == Mode::Map {
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => self.mode = Mode::Default,
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.running = false,
                (KeyCode::Up, _) => self.map_viewport.pan(0.0, 50.0),
                (KeyCode::Down, _) => self.map_viewport.pan(0.0, -50.0),
                (KeyCode::Left, _) => self.map_viewport.pan(-50.0, 0.0),
                (KeyCode::Right, _) => self.map_viewport.pan(50.0, 0.0),
                (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
                    self.map_viewport.zoom_in()
                }
                (KeyCode::Char('-'), _) => self.map_viewport.zoom_out(),
                _ => {}
            }
            return;
        }

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

        // Handle undo/redo before clearing the stack
        if command == mapping::UNDO {
            self.handle_undo_command();
            return;
        }
        if command == mapping::REDO {
            self.handle_redo_command();
            return;
        }

        // Clear redo stack on any non-undo/redo command
        self.redo_stack.clear();

        // Handle app-level commands that need access to App state
        if command == mapping::HISTORY {
            self.handle_history_command(args);
            return;
        }
        if command == mapping::SHOW {
            self.handle_show_command(args);
            return;
        }
        if command == mapping::SET {
            self.handle_set_command(args);
            return;
        }
        if command == mapping::LIST {
            self.handle_list_command(args);
            return;
        }
        if command == mapping::HELP {
            self.handle_help_command();
            return;
        }
        if command == mapping::LOAD {
            self.handle_load_command(args);
            return;
        }
        if command == mapping::NEW {
            self.handle_new_command(args);
            return;
        }
        if command == mapping::SEARCH {
            self.handle_search_command(args);
            return;
        }
        if command == mapping::MAP {
            self.handle_map_command(args);
            return;
        }
        if command == mapping::SOUND {
            self.handle_sound_command(args);
            return;
        }
        if command == mapping::VALIDATE {
            self.handle_validate_command(args);
            return;
        }
        if command == mapping::BESTIARY {
            self.handle_bestiary_command(args);
            return;
        }
        if command == mapping::SPAWN {
            self.handle_spawn_command(args);
            return;
        }
        if command == mapping::ENCOUNTER {
            self.handle_encounter_command(args);
            return;
        }
        if command == mapping::COMBAT {
            self.handle_combat_command(args);
            return;
        }
        if command == mapping::INIT || command == mapping::NEXT || command == mapping::PREV
            || command == mapping::STATUS || command == mapping::TARGET
        {
            self.handle_combat_command(&format!("{command} {args}").trim().to_string());
            return;
        }
        // Hybrid dispatch: built-in first, then custom commands, then unknown
        let custom_commands = self
            .game_state
            .as_ref()
            .map(|gs| &gs.custom_commands);
        let result = dispatcher::dispatch(input, &mut self.rng, custom_commands);

        if let CommandResult::Custom(ref script) = result {
            let script = script.clone();
            self.execute_custom_command(&script);
            return;
        }

        self.apply_command_result(result);
    }

    fn execute_custom_command(&mut self, script: &LolScript) {
        // Temporarily take game_state and rng for ScriptContext ownership
        let game_state = match self.game_state.take() {
            Some(gs) => gs,
            None => return,
        };
        let rng = std::mem::replace(&mut self.rng, Box::new(rand::thread_rng()));

        let ctx = Rc::new(RefCell::new(ScriptContext::new(game_state, rng)));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);

        match engine.execute(&script.source) {
            Ok(stdout_output) => {
                // Show RUSTORY_DISPLAY messages
                for line in &ctx.borrow().output {
                    self.messages.push(Message {
                        text: line.clone(),
                        style: Style::default().fg(Color::Blue),
                    });
                }
                // Show VISIBLE stdout output
                let trimmed = stdout_output.trim_end();
                if !trimmed.is_empty() {
                    for line in trimmed.lines() {
                        self.messages.push(Message {
                            text: line.to_string(),
                            style: Style::default().fg(Color::Blue),
                        });
                    }
                }
            }
            Err(e) => {
                self.messages.push(Message {
                    text: format!("Script error: {e}"),
                    style: Style::default().fg(Color::Red),
                });
            }
        }

        // Recover game_state, rng, and sound commands from ScriptContext
        let ctx_inner = match Rc::try_unwrap(ctx) {
            Ok(cell) => cell.into_inner(),
            Err(_) => panic!("ScriptContext should have no other references"),
        };
        self.game_state = Some(ctx_inner.game_state);
        self.rng = ctx_inner.rng;

        // Process queued sound commands
        for cmd in ctx_inner.sound_commands {
            self.execute_sound_command(cmd);
        }
    }

    fn execute_sound_command(&mut self, cmd: SoundCommand) {
        let player = match &mut self.audio_player {
            Some(p) => p,
            None => return, // No audio device — silently skip
        };

        match cmd {
            SoundCommand::Play(filename) => {
                if let Some(full_path) = self.sound_library.resolve(&filename) {
                    if let Err(e) = player.play(&full_path) {
                        self.messages.push(Message {
                            text: format!("Sound error: {e}"),
                            style: Style::default().fg(Color::Red),
                        });
                    }
                }
            }
            SoundCommand::PlayLoop(filename) => {
                if let Some(full_path) = self.sound_library.resolve(&filename) {
                    if let Err(e) = player.play_loop(&full_path) {
                        self.messages.push(Message {
                            text: format!("Sound error: {e}"),
                            style: Style::default().fg(Color::Red),
                        });
                    }
                }
            }
            SoundCommand::Stop => {
                player.stop();
            }
        }
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
            CommandResult::Custom(_) => {
                // Custom commands are handled before apply_command_result
                unreachable!("Custom variant should be handled before apply_command_result");
            }
        }
    }

    fn handle_help_command(&mut self) {
        let mut lines = vec![
            StyledLine::plain("Available commands:"),
            StyledLine::plain("  help  — show this help"),
            StyledLine::plain("  load  — load a campaign folder (e.g. load sample)"),
            StyledLine::plain(
                "  new   — create a new campaign from a template (e.g. new my_game sample)",
            ),
            StyledLine::plain("  roll  — roll dice (e.g. roll 2d6+3)"),
            StyledLine::plain("  show     — display character sheet (e.g. show thorin)"),
            StyledLine::plain("  set      — set a character field (e.g. set thorin.hp 35)"),
            StyledLine::plain("  list     — list players or npcs (e.g. list players)"),
            StyledLine::plain("  history  — show recent state changes (e.g. history 5)"),
            StyledLine::plain("  undo     — revert the last state change"),
            StyledLine::plain("  redo     — re-apply the last undone change"),
            StyledLine::plain("  sound    — play audio (e.g. sound play ambiance/tavern.mp3)"),
            StyledLine::plain("  bestiary — list creature templates (e.g. bestiary info goblin)"),
            StyledLine::plain("  spawn     — create NPC from bestiary (e.g. spawn goblin)"),
            StyledLine::plain("  encounter — spawn an encounter group (e.g. encounter goblin patrol)"),
            StyledLine::plain("  validate — check campaign files against schemas"),
            StyledLine::plain("  quit     — exit Rustory"),
        ];

        // Campaign status
        match &self.game_state {
            Some(gs) => {
                lines.push(StyledLine::new(String::new(), Style::default()));
                lines.push(StyledLine::new(
                    format!("Campaign: {} (loaded)", gs.campaign_name),
                    Style::default().fg(Color::Green),
                ));
                if let Some(ref rules) = gs.rules {
                    lines.push(StyledLine::new(
                        format!("  System: {}", rules.system_name),
                        Style::default().fg(Color::Green),
                    ));
                }

                // List custom commands from rules/commands/*.lol if any
                let commands_dir = gs.campaign_path.join("rules/commands");
                if commands_dir.exists() {
                    let mut custom_cmds: Vec<String> = Vec::new();
                    if let Ok(entries) = std::fs::read_dir(&commands_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().and_then(|e| e.to_str()) == Some("lol") {
                                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                                    custom_cmds.push(name.to_string());
                                }
                            }
                        }
                    }
                    custom_cmds.sort();
                    if !custom_cmds.is_empty() {
                        lines.push(StyledLine::new(
                            format!("  Custom commands: {}", custom_cmds.join(", ")),
                            Style::default().fg(Color::Green),
                        ));
                    }
                }
            }
            None => {
                lines.push(StyledLine::new(String::new(), Style::default()));
                lines.push(StyledLine::new(
                    "No campaign loaded. Use \"load <path>\" to load one.".to_string(),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }

        self.apply_command_result(CommandResult::Output(lines));
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

    fn handle_new_command(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 || parts[0].is_empty() || parts[1].trim().is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: new <name> <template_path> (e.g. new my_game sample)".to_string(),
            ));
            return;
        }

        let name = parts[0];
        let template_path = PathBuf::from(parts[1].trim());

        // Validate template exists
        if !template_path.exists() || !template_path.is_dir() {
            self.apply_command_result(CommandResult::Error(format!(
                "Template not found: \"{}\"",
                template_path.display()
            )));
            return;
        }

        // Validate template has rules/
        let template_rules = template_path.join("rules");
        if !template_rules.exists() || !template_rules.is_dir() {
            self.apply_command_result(CommandResult::Error(format!(
                "Template \"{}\" is missing a rules/ directory.",
                template_path.display()
            )));
            return;
        }

        // Validate template by loading its rules
        let system_toml = template_rules.join("system.toml");
        if system_toml.exists() {
            if let Err(errors) = crate::rules::loader::load_rules(&system_toml) {
                let mut lines = vec![StyledLine::new(
                    format!(
                        "Template \"{}\" has invalid rules:",
                        template_path.display()
                    ),
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
        }

        // Check destination doesn't already exist
        let dest = PathBuf::from(name);
        if dest.exists() {
            self.apply_command_result(CommandResult::Error(format!(
                "Destination \"{name}\" already exists."
            )));
            return;
        }

        // Create the new campaign
        if let Err(e) = Self::create_campaign_from_template(&dest, &template_path) {
            self.apply_command_result(CommandResult::Error(format!(
                "Failed to create campaign: {e}"
            )));
            return;
        }

        self.apply_command_result(CommandResult::Output(vec![
            StyledLine::new(
                format!(
                    "Campaign \"{name}\" created from template \"{}\".",
                    template_path.display()
                ),
                Style::default().fg(Color::Green),
            ),
            StyledLine::new(
                format!("  Use \"load {name}\" to start playing."),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    fn create_campaign_from_template(dest: &Path, template: &Path) -> std::io::Result<()> {
        // Copy rules/ as-is
        copy_dir_recursive(&template.join("rules"), &dest.join("rules"))?;

        // Create empty data directories
        std::fs::create_dir_all(dest.join("players"))?;
        std::fs::create_dir_all(dest.join("npc"))?;
        std::fs::create_dir_all(dest.join("notes"))?;

        // Copy optional template directories if they exist
        let optional_dirs = ["map", "sound", "bestiary"];
        for dir_name in &optional_dirs {
            let src = template.join(dir_name);
            if src.exists() && src.is_dir() {
                copy_dir_recursive(&src, &dest.join(dir_name))?;
            }
        }

        Ok(())
    }

    fn handle_map_command(&mut self, args: &str) {
        if self.world_map.is_none() {
            self.apply_command_result(CommandResult::Error(
                "No map loaded. Place a world.json in the campaign's map/ directory.".to_string(),
            ));
            return;
        }

        if args.is_empty() {
            if self.mode == Mode::Map {
                self.mode = Mode::Default;
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Map mode OFF.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            } else {
                self.mode = Mode::Map;
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Map mode ON. Arrow keys: pan. +/-: zoom. Esc: exit map.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            return;
        }

        let sub_parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcmd = sub_parts[0];
        let sub_args = sub_parts.get(1).unwrap_or(&"").trim();

        match subcmd {
            "list" => self.map_list(sub_args),
            "info" => self.map_info(sub_args),
            "search" => self.map_search(sub_args),
            "near" => self.map_near(sub_args),
            "route" => self.map_route(sub_args),
            "where" => self.map_where(sub_args),
            "move" => self.map_move(sub_args),
            _ => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Unknown map subcommand \"{subcmd}\". Try: list, info, search, near, route"
                )));
            }
        }
    }

    fn map_list(&mut self, args: &str) {
        let world = self.world_map.as_ref().unwrap();
        let entity_type = if args.is_empty() { "burgs" } else { args };
        let mut lines = Vec::new();

        match entity_type {
            "burgs" => {
                lines.push(StyledLine::new(
                    "Burgs:".to_string(),
                    Style::default().fg(Color::Cyan),
                ));
                for burg in world.map.pack.burgs.iter().skip(1) {
                    if burg.name.is_empty() {
                        continue;
                    }
                    let pop = burg.population * 1000.0;
                    let kind = if burg.capital > 0 {
                        " (capital)"
                    } else {
                        ""
                    };
                    lines.push(StyledLine::plain(format!(
                        "  {} — pop: {:.0}{kind}",
                        burg.name, pop
                    )));
                }
            }
            "states" => {
                lines.push(StyledLine::new(
                    "States:".to_string(),
                    Style::default().fg(Color::Cyan),
                ));
                for state in world.map.pack.states.iter().skip(1) {
                    if state.name.is_empty() {
                        continue;
                    }
                    lines.push(StyledLine::plain(format!(
                        "  {} — {}",
                        state.name, state.form
                    )));
                }
            }
            "cultures" => {
                lines.push(StyledLine::new(
                    "Cultures:".to_string(),
                    Style::default().fg(Color::Cyan),
                ));
                for culture in world.map.pack.cultures.iter().skip(1) {
                    if culture.name.is_empty() {
                        continue;
                    }
                    lines.push(StyledLine::plain(format!(
                        "  {} — {}",
                        culture.name, culture.culture_type
                    )));
                }
            }
            other => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Unknown entity type \"{other}\". Try: burgs, states, cultures"
                )));
                return;
            }
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn map_info(&mut self, args: &str) {
        let world = self.world_map.as_ref().unwrap();
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map info <name>".to_string(),
            ));
            return;
        }

        // Try burg first, then state
        if let Some(burg) = world.get_burg(args) {
            let state_name = world
                .map
                .pack
                .states
                .get(burg.state as usize)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");
            let culture_name = world
                .map
                .pack
                .cultures
                .get(burg.culture as usize)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");

            let lines = vec![
                StyledLine::new(
                    format!("{} ({})", burg.name, burg.burg_type),
                    Style::default().fg(Color::Cyan),
                ),
                StyledLine::plain(format!("  Population: {:.0}", burg.population * 1000.0)),
                StyledLine::plain(format!("  State: {state_name}")),
                StyledLine::plain(format!("  Culture: {culture_name}")),
                StyledLine::plain(format!(
                    "  Features: {}{}{}",
                    if burg.capital > 0 { "capital " } else { "" },
                    if burg.port > 0 { "port " } else { "" },
                    if burg.citadel > 0 { "citadel " } else { "" },
                )),
            ];
            self.apply_command_result(CommandResult::Output(lines));
        } else if let Some(state) = world.get_state(args) {
            let burgs = world.burgs_in_state(args);
            let lines = vec![
                StyledLine::new(
                    format!("{} ({})", state.name, state.form),
                    Style::default().fg(Color::Cyan),
                ),
                StyledLine::plain(format!("  Area: {:.0}", state.area)),
                StyledLine::plain(format!("  Burgs: {}", burgs.len())),
            ];
            self.apply_command_result(CommandResult::Output(lines));
        } else {
            self.apply_command_result(CommandResult::Error(format!(
                "No burg or state named \"{args}\" found."
            )));
        }
    }

    fn map_search(&mut self, args: &str) {
        let world = self.world_map.as_ref().unwrap();
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map search <query>".to_string(),
            ));
            return;
        }

        let results = world.search_burgs(args);
        if results.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("No locations matching \"{args}\"."),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let mut lines = vec![StyledLine::new(
            format!("Locations matching \"{}\":", args),
            Style::default().fg(Color::Cyan),
        )];
        for burg in &results {
            lines.push(StyledLine::plain(format!(
                "  {} — pop: {:.0}",
                burg.name,
                burg.population * 1000.0
            )));
        }
        self.apply_command_result(CommandResult::Output(lines));
    }

    fn map_near(&mut self, args: &str) {
        let world = self.world_map.as_ref().unwrap();
        let parts: Vec<&str> = args.rsplitn(2, ' ').collect();
        let (name, radius) = if parts.len() == 2 {
            if let Ok(r) = parts[0].parse::<f64>() {
                (parts[1], r)
            } else {
                (args, 200.0)
            }
        } else {
            (args, 200.0)
        };

        if name.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map near <name> [radius]".to_string(),
            ));
            return;
        }

        let results = world.nearby_burgs(name, radius);
        if results.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("No burgs within {radius:.0} of \"{name}\"."),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let mut lines = vec![StyledLine::new(
            format!("Burgs near \"{}\" (within {:.0}):", name, radius),
            Style::default().fg(Color::Cyan),
        )];
        for (burg, dist) in &results {
            lines.push(StyledLine::plain(format!(
                "  {} — distance: {:.1}",
                burg.name, dist
            )));
        }
        self.apply_command_result(CommandResult::Output(lines));
    }

    fn map_route(&mut self, args: &str) {
        let world = self.world_map.as_ref().unwrap();
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map route <from> <to>".to_string(),
            ));
            return;
        }

        let from = parts[0];
        let to = parts[1];

        match world.get_route(from, to) {
            Some(route) => {
                let lines = vec![
                    StyledLine::new(
                        format!("Route from {} to {}:", from, to),
                        Style::default().fg(Color::Cyan),
                    ),
                    StyledLine::plain(format!("  Type: {}", route.group)),
                    StyledLine::plain(format!("  Length: {:.1}", route.length)),
                    StyledLine::plain(format!("  Waypoints: {}", route.points.len())),
                ];
                self.apply_command_result(CommandResult::Output(lines));
            }
            None => {
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    format!("No direct route between \"{from}\" and \"{to}\"."),
                    Style::default().fg(Color::Yellow),
                )]));
            }
        }
    }

    fn map_where(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map where <character>".to_string(),
            ));
            return;
        }

        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let name_lower = args.to_lowercase();
        let character = gs
            .players
            .iter()
            .chain(gs.npcs.iter())
            .find(|c| c.name.to_lowercase() == name_lower);

        match character {
            Some(ch) => match &ch.location {
                Some(loc) => {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::plain(
                        format!("{} is at {}.", ch.name, loc),
                    )]));
                }
                None => {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        format!("{} has no known location.", ch.name),
                        Style::default().fg(Color::Yellow),
                    )]));
                }
            },
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Character \"{args}\" not found."
                )));
            }
        }
    }

    fn map_move(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: map move <character> <location>".to_string(),
            ));
            return;
        }

        let char_name = parts[0];
        let location = parts[1];

        // Validate location exists in the map
        let world = match &self.world_map {
            Some(w) => w,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No map loaded.".to_string(),
                ));
                return;
            }
        };

        if world.get_burg(location).is_none() {
            self.apply_command_result(CommandResult::Error(format!(
                "Location \"{location}\" not found on the map."
            )));
            return;
        }

        // Find and update the character
        let gs = match &mut self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let name_lower = char_name.to_lowercase();
        let character = gs
            .players
            .iter_mut()
            .chain(gs.npcs.iter_mut())
            .find(|c| c.name.to_lowercase() == name_lower);

        let result = match character {
            Some(ch) => {
                let old_loc = ch.location.clone().unwrap_or_else(|| "nowhere".to_string());
                let ch_name = ch.name.clone();
                ch.location = Some(location.to_string());
                CommandResult::Output(vec![StyledLine::new(
                    format!("{ch_name} moved from {old_loc} to {location}."),
                    Style::default().fg(Color::Green),
                )])
            }
            None => CommandResult::Error(format!("Character \"{char_name}\" not found.")),
        };
        self.apply_command_result(result);
    }

    fn handle_sound_command(&mut self, args: &str) {
        if args.is_empty() || args == "list" {
            self.sound_list(None);
            return;
        }

        let sub_parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcmd = sub_parts[0];
        let sub_args = sub_parts.get(1).unwrap_or(&"").trim();

        match subcmd {
            "list" => self.sound_list(if sub_args.is_empty() { None } else { Some(sub_args) }),
            "play" => self.sound_play(sub_args),
            "loop" => self.sound_play_loop(sub_args),
            "stop" => self.sound_stop(),
            "pause" => self.sound_pause(),
            "resume" => self.sound_resume(),
            "volume" => self.sound_volume(sub_args),
            "status" => self.sound_status(),
            "search" => self.sound_search(sub_args),
            _ => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Unknown sound subcommand \"{subcmd}\". Try: list, play, loop, stop, pause, resume, volume, status, search"
                )));
            }
        }
    }

    fn sound_list(&mut self, subfolder: Option<&str>) {
        if self.sound_library.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                "Sound library is empty. Place audio files in the campaign's sound/ directory.".to_string(),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let entries = self.sound_library.list(subfolder);
        if entries.is_empty() {
            let msg = match subfolder {
                Some(folder) => format!("No entries in \"{folder}\"."),
                None => "Sound library is empty.".to_string(),
            };
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                msg,
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let header = match subfolder {
            Some(folder) => format!("Sound library ({folder}):"),
            None => "Sound library:".to_string(),
        };
        let mut lines = vec![StyledLine::new(header, Style::default().fg(Color::Cyan))];

        for entry in entries {
            let prefix = if entry.is_dir { "[dir] " } else { "      " };
            lines.push(StyledLine::plain(format!("  {prefix}{}", entry.name)));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn sound_play(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: sound play <path> (e.g. sound play ambiance/tavern.mp3)".to_string(),
            ));
            return;
        }

        let full_path = match self.sound_library.resolve(args) {
            Some(p) => p,
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Audio file \"{args}\" not found in sound library."
                )));
                return;
            }
        };

        let player = match &mut self.audio_player {
            Some(p) => p,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
                return;
            }
        };

        match player.play(&full_path) {
            Ok(()) => {
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    format!("Playing: {args}"),
                    Style::default().fg(Color::Green),
                )]));
            }
            Err(e) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Failed to play \"{args}\": {e}"
                )));
            }
        }
    }

    fn sound_play_loop(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: sound loop <path> (e.g. sound loop ambiance/tavern.mp3)".to_string(),
            ));
            return;
        }

        let full_path = match self.sound_library.resolve(args) {
            Some(p) => p,
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Audio file \"{args}\" not found in sound library."
                )));
                return;
            }
        };

        let player = match &mut self.audio_player {
            Some(p) => p,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
                return;
            }
        };

        match player.play_loop(&full_path) {
            Ok(()) => {
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    format!("Looping: {args}"),
                    Style::default().fg(Color::Green),
                )]));
            }
            Err(e) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Failed to loop \"{args}\": {e}"
                )));
            }
        }
    }

    fn sound_stop(&mut self) {
        match &mut self.audio_player {
            Some(player) => {
                player.stop();
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Playback stopped.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
            }
        }
    }

    fn sound_pause(&mut self) {
        match &mut self.audio_player {
            Some(player) => {
                player.pause();
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Playback paused.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
            }
        }
    }

    fn sound_resume(&mut self) {
        match &mut self.audio_player {
            Some(player) => {
                player.resume();
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Playback resumed.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
            }
        }
    }

    fn sound_volume(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: sound volume <0-100>".to_string(),
            ));
            return;
        }

        let vol: u32 = match args.parse() {
            Ok(v) => v,
            Err(_) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Invalid volume \"{args}\". Expected a number 0-100."
                )));
                return;
            }
        };

        let clamped = vol.min(100);
        let vol_f32 = clamped as f32 / 100.0;

        match &mut self.audio_player {
            Some(player) => {
                player.set_volume(vol_f32);
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    format!("Volume set to {clamped}%."),
                    Style::default().fg(Color::Green),
                )]));
            }
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
            }
        }
    }

    fn sound_status(&mut self) {
        let player = match &self.audio_player {
            Some(p) => p,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Audio device not available.".to_string(),
                ));
                return;
            }
        };

        let mut lines = vec![StyledLine::new(
            "Audio status:".to_string(),
            Style::default().fg(Color::Cyan),
        )];

        if let Some(track) = player.current_track() {
            let state = if player.is_paused() {
                "paused"
            } else if player.is_playing() {
                "playing"
            } else {
                "stopped"
            };
            let looping = if player.is_looping() { " (loop)" } else { "" };
            lines.push(StyledLine::plain(format!("  Track: {track}{looping}")));
            lines.push(StyledLine::plain(format!("  State: {state}")));
        } else {
            lines.push(StyledLine::plain("  No track loaded.".to_string()));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn sound_search(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: sound search <query>".to_string(),
            ));
            return;
        }

        let results = self.sound_library.search(args);
        if results.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("No audio files matching \"{args}\"."),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let mut lines = vec![StyledLine::new(
            format!("Sound files matching \"{}\":", args),
            Style::default().fg(Color::Cyan),
        )];

        for entry in results {
            lines.push(StyledLine::plain(format!("  {}", entry.path)));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn handle_show_command(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: show <character> [field]".to_string(),
            ));
            return;
        }

        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let char_name = parts[0];
        let field = parts.get(1).map(|s| s.trim());

        let name_lower = char_name.to_lowercase();
        let character = gs
            .players
            .iter()
            .chain(gs.npcs.iter())
            .find(|c| c.name.to_lowercase() == name_lower);

        let ch = match character {
            Some(ch) => ch,
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Character \"{char_name}\" not found."
                )));
                return;
            }
        };

        if let Some(field_name) = field {
            // Show a specific field
            if let Some(val) = ch.get_stat(field_name) {
                self.apply_command_result(CommandResult::Output(vec![StyledLine::plain(
                    format!("{}.{field_name} = {val}", ch.name),
                )]));
            } else if let Some(gauge) = ch.gauges.get(field_name) {
                self.apply_command_result(CommandResult::Output(vec![StyledLine::plain(
                    format!("{}.{field_name} = {}/{}", ch.name, gauge.current, gauge.max),
                )]));
            } else {
                self.apply_command_result(CommandResult::Error(format!(
                    "Field \"{field_name}\" not found on {}.",
                    ch.name
                )));
            }
            return;
        }

        // Show full character sheet
        let mut lines = vec![StyledLine::new(
            format!("--- {} ---", ch.name),
            Style::default().fg(Color::Cyan),
        )];

        // Stats
        if !ch.stats.is_empty() {
            lines.push(StyledLine::new(
                "Stats:".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            for stat in &ch.stats {
                let v = stat.value;
                if v.fract() == 0.0 {
                    lines.push(StyledLine::plain(format!("  {}: {}", stat.name, v as i64)));
                } else {
                    lines.push(StyledLine::plain(format!("  {}: {v}", stat.name)));
                }
            }
        }

        // Gauges
        if !ch.gauges.is_empty() {
            lines.push(StyledLine::new(
                "Gauges:".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            for (name, gauge) in &ch.gauges {
                let pct = if gauge.max > 0.0 {
                    (gauge.current / gauge.max * 100.0) as u32
                } else {
                    0
                };
                let color = if pct > 60 {
                    Color::Green
                } else if pct > 30 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                lines.push(StyledLine::new(
                    format!("  {name}: {}/{} ({pct}%)", gauge.current, gauge.max),
                    Style::default().fg(color),
                ));
            }
        }

        // Conditions
        let active: Vec<_> = ch.conditions.iter().filter(|c| c.active).collect();
        if !active.is_empty() {
            lines.push(StyledLine::new(
                "Conditions:".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            for cond in &active {
                lines.push(StyledLine::plain(format!("  {}", cond.name)));
            }
        }

        // Tags
        if !ch.tags.is_empty() {
            lines.push(StyledLine::new(
                "Tags:".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            for tag in &ch.tags {
                lines.push(StyledLine::plain(format!("  {}", tag.name)));
            }
        }

        // Inventory
        if !ch.inventory.items.is_empty() {
            lines.push(StyledLine::new(
                "Inventory:".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            for item in &ch.inventory.items {
                let props: Vec<String> = item
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                let prop_str = if props.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", props.join(", "))
                };
                lines.push(StyledLine::plain(format!("  {}{prop_str}", item.name)));
            }
        }

        // Location
        if let Some(ref loc) = ch.location {
            lines.push(StyledLine::plain(format!("Location: {loc}")));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn handle_set_command(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: set <character>.<field> <value> (e.g. set thorin.hp 35)".to_string(),
            ));
            return;
        }

        // Parse "character.field value"
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 {
            self.apply_command_result(CommandResult::Error(
                "Usage: set <character>.<field> <value>".to_string(),
            ));
            return;
        }

        let dot_parts: Vec<&str> = parts[0].splitn(2, '.').collect();
        if dot_parts.len() < 2 {
            self.apply_command_result(CommandResult::Error(
                "Use dot notation: set <character>.<field> <value>".to_string(),
            ));
            return;
        }

        let char_name = dot_parts[0];
        let field_name = dot_parts[1];
        let value_str = parts[1].trim();

        let value: f64 = match value_str.parse() {
            Ok(v) => v,
            Err(_) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Invalid value \"{value_str}\". Expected a number."
                )));
                return;
            }
        };

        let gs = match &mut self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let name_lower = char_name.to_lowercase();
        let (character, is_player) = {
            if let Some(ch) = gs.players.iter_mut().find(|c| c.name.to_lowercase() == name_lower) {
                (ch, true)
            } else if let Some(ch) = gs.npcs.iter_mut().find(|c| c.name.to_lowercase() == name_lower) {
                (ch, false)
            } else {
                self.apply_command_result(CommandResult::Error(format!(
                    "Character \"{char_name}\" not found."
                )));
                return;
            }
        };

        // Try stat first, then gauge
        if character.get_stat(field_name).is_some() {
            let old = character.get_stat(field_name).unwrap();
            character.set_stat(field_name, value);
            let ch_name = character.name.clone();

            // Persist if persistence is available
            if let (Some(ref pl), Some(ref schema)) =
                (&self.persistence, &gs.schema)
            {
                let _ = pl.persist_character(
                    character,
                    is_player,
                    schema,
                    &format!("GM set {ch_name}.{field_name} to {value} (was {old})"),
                );
            }

            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("{ch_name}.{field_name}: {old} -> {value}"),
                Style::default().fg(Color::Green),
            )]));
        } else if character.gauges.contains_key(field_name) {
            let gauge = character.gauges.get_mut(field_name).unwrap();
            let old = gauge.current;
            gauge.current = value.max(0.0).min(gauge.max);
            let ch_name = character.name.clone();
            let new_val = gauge.current;

            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("{ch_name}.{field_name}: {old} -> {new_val}"),
                Style::default().fg(Color::Green),
            )]));
        } else {
            let ch_name = character.name.clone();
            self.apply_command_result(CommandResult::Error(format!(
                "Field \"{field_name}\" not found on {ch_name}."
            )));
        }
    }

    fn handle_list_command(&mut self, args: &str) {
        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let list_type = if args.is_empty() { "players" } else { args.trim() };

        match list_type {
            "players" => {
                if gs.players.is_empty() {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        "No players loaded.".to_string(),
                        Style::default().fg(Color::Yellow),
                    )]));
                    return;
                }
                let mut lines = vec![StyledLine::new(
                    "Players:".to_string(),
                    Style::default().fg(Color::Cyan),
                )];
                for ch in &gs.players {
                    let hp_str = ch
                        .gauges
                        .get("hp")
                        .map(|g| format!(" HP: {}/{}", g.current, g.max))
                        .unwrap_or_default();
                    lines.push(StyledLine::plain(format!("  {}{hp_str}", ch.name)));
                }
                self.apply_command_result(CommandResult::Output(lines));
            }
            "npcs" => {
                if gs.npcs.is_empty() {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        "No NPCs loaded.".to_string(),
                        Style::default().fg(Color::Yellow),
                    )]));
                    return;
                }
                let mut lines = vec![StyledLine::new(
                    "NPCs:".to_string(),
                    Style::default().fg(Color::Cyan),
                )];
                for ch in &gs.npcs {
                    let hp_str = ch
                        .gauges
                        .get("hp")
                        .map(|g| format!(" HP: {}/{}", g.current, g.max))
                        .unwrap_or_default();
                    lines.push(StyledLine::plain(format!("  {}{hp_str}", ch.name)));
                }
                self.apply_command_result(CommandResult::Output(lines));
            }
            other => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Unknown list type \"{other}\". Try: players, npcs"
                )));
            }
        }
    }

    fn handle_bestiary_command(&mut self, args: &str) {
        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let sub_cmd = if args.is_empty() { "list" } else { parts[0] };
        let sub_args = parts.get(1).unwrap_or(&"").trim();

        match sub_cmd {
            "list" => {
                if gs.bestiary_entries.is_empty() {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        "No creature templates in bestiary.".to_string(),
                        Style::default().fg(Color::Yellow),
                    )]));
                    return;
                }
                let mut lines = vec![StyledLine::new(
                    "Bestiary:".to_string(),
                    Style::default().fg(Color::Cyan),
                )];
                for entry in &gs.bestiary_entries {
                    let hp_str = entry
                        .stats
                        .iter()
                        .find(|s| s.name == "hp_max")
                        .map(|s| format!(" HP: {}", s.value as i64))
                        .unwrap_or_default();
                    let ac_str = entry
                        .stats
                        .iter()
                        .find(|s| s.name == "ac")
                        .map(|s| format!(" AC: {}", s.value as i64))
                        .unwrap_or_default();
                    lines.push(StyledLine::plain(format!(
                        "  {}{hp_str}{ac_str}",
                        entry.name
                    )));
                }
                self.apply_command_result(CommandResult::Output(lines));
            }
            "info" => {
                if sub_args.is_empty() {
                    self.apply_command_result(CommandResult::Error(
                        "Usage: bestiary info <name>".to_string(),
                    ));
                    return;
                }
                match crate::bestiary::find_entry(&gs.bestiary_entries, sub_args) {
                    Some(entry) => {
                        let mut lines = vec![StyledLine::new(
                            entry.name.clone(),
                            Style::default().fg(Color::Cyan),
                        )];
                        for stat in &entry.stats {
                            lines.push(StyledLine::plain(format!(
                                "  {}: {}",
                                stat.name, stat.value
                            )));
                        }
                        self.apply_command_result(CommandResult::Output(lines));
                    }
                    None => {
                        self.apply_command_result(CommandResult::Error(format!(
                            "Creature \"{sub_args}\" not found in bestiary."
                        )));
                    }
                }
            }
            other => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Unknown bestiary subcommand \"{other}\". Try: list, info"
                )));
            }
        }
    }

    fn handle_spawn_command(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: spawn <template> [name] (e.g. spawn goblin, spawn goblin \"Goblin Guard\")".to_string(),
            ));
            return;
        }

        // Parse args: first word is template, rest is optional name
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let template_name = parts[0];
        let custom_name = parts.get(1).map(|s| s.trim()).filter(|s| !s.is_empty());

        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        // Look up the bestiary entry
        let entry = match crate::bestiary::find_entry(&gs.bestiary_entries, template_name) {
            Some(e) => e.clone(),
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Creature template \"{template_name}\" not found in bestiary."
                )));
                return;
            }
        };

        // Determine the NPC name: use custom name or auto-generate
        let npc_name = match custom_name {
            Some(name) => name.to_string(),
            None => {
                // Auto-name: "Goblin #1", "Goblin #2", etc.
                let base = &entry.name;
                let mut counter = 1u32;
                loop {
                    let candidate = format!("{base} #{counter}");
                    let gs = self.game_state.as_ref().unwrap();
                    if gs.get_npc(&candidate).is_none() {
                        break candidate;
                    }
                    counter += 1;
                }
            }
        };

        // Create character from bestiary entry stats
        let mut character = crate::game_state::Character::new(&npc_name);
        for stat in &entry.stats {
            character.stats.push(stat.clone());
        }

        // Apply resource_defs (gauges/pools) from rules
        let gs = self.game_state.as_ref().unwrap();
        if let Some(rules) = &gs.rules {
            for def in &rules.resource_defs {
                match def {
                    crate::rules::loader::ResourceDef::Gauge { name, max_stat } => {
                        if !character.gauges.contains_key(name) {
                            let max_val = character.get_stat(max_stat).unwrap_or(0.0);
                            if max_val > 0.0 {
                                character.gauges.insert(
                                    name.clone(),
                                    crate::game_state::primitives::Gauge::new(name, max_val),
                                );
                            }
                        }
                    }
                    crate::rules::loader::ResourceDef::Pool {
                        name,
                        max,
                        resets_on,
                    } => {
                        if !character.pools.contains_key(name) {
                            character.pools.insert(
                                name.clone(),
                                crate::game_state::primitives::Pool::new(name, *max, resets_on.clone()),
                            );
                        }
                    }
                }
            }
        }

        // Persist to disk if persistence + schema available
        let schema = gs.schema.clone();
        let description = format!("Spawned {} from bestiary", npc_name);
        if let (Some(ref pl), Some(ref schema)) = (&self.persistence, &schema) {
            if let Err(e) = pl.persist_character(&character, false, schema, &description) {
                self.apply_command_result(CommandResult::Error(format!(
                    "Spawned {npc_name} but failed to persist: {e}"
                )));
                // Still add to game state even if persistence fails
                self.game_state.as_mut().unwrap().add_npc(character);
                return;
            }
        }

        // Add to game state
        let npc_name_display = npc_name.clone();
        self.game_state.as_mut().unwrap().add_npc(character);

        self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
            format!("Spawned {npc_name_display} from bestiary template \"{template_name}\"."),
            Style::default().fg(Color::Green),
        )]));
    }

    fn handle_encounter_command(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: encounter <name> (e.g. encounter goblin patrol)".to_string(),
            ));
            return;
        }

        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let encounter = match crate::bestiary::find_encounter(&gs.bestiary_encounters, args) {
            Some(e) => e.clone(),
            None => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Encounter \"{args}\" not found."
                )));
                return;
            }
        };

        let mut spawned_names: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for creature_def in &encounter.creatures {
            // Look up the bestiary template
            let gs = self.game_state.as_ref().unwrap();
            let entry = match crate::bestiary::find_entry(&gs.bestiary_entries, &creature_def.template) {
                Some(e) => e.clone(),
                None => {
                    errors.push(format!(
                        "Template \"{}\" not found in bestiary.",
                        creature_def.template
                    ));
                    continue;
                }
            };

            for i in 0..creature_def.count {
                // Determine name: use name_override for count=1, otherwise auto-name
                let npc_name = if let Some(ref override_name) = creature_def.name_override {
                    if creature_def.count == 1 {
                        override_name.clone()
                    } else {
                        format!("{override_name} #{}", i + 1)
                    }
                } else {
                    // Auto-name: "Goblin #N" with incrementing counter
                    let base = &entry.name;
                    let mut counter = 1u32;
                    loop {
                        let candidate = format!("{base} #{counter}");
                        let gs = self.game_state.as_ref().unwrap();
                        if gs.get_npc(&candidate).is_none() {
                            break candidate;
                        }
                        counter += 1;
                    }
                };

                let mut character = crate::game_state::Character::new(&npc_name);
                for stat in &entry.stats {
                    character.stats.push(stat.clone());
                }

                // Apply resource_defs (gauges/pools) from rules
                let gs = self.game_state.as_ref().unwrap();
                if let Some(rules) = &gs.rules {
                    for def in &rules.resource_defs {
                        match def {
                            crate::rules::loader::ResourceDef::Gauge { name, max_stat } => {
                                if !character.gauges.contains_key(name) {
                                    let max_val = character.get_stat(max_stat).unwrap_or(0.0);
                                    if max_val > 0.0 {
                                        character.gauges.insert(
                                            name.clone(),
                                            crate::game_state::primitives::Gauge::new(name, max_val),
                                        );
                                    }
                                }
                            }
                            crate::rules::loader::ResourceDef::Pool {
                                name,
                                max,
                                resets_on,
                            } => {
                                if !character.pools.contains_key(name) {
                                    character.pools.insert(
                                        name.clone(),
                                        crate::game_state::primitives::Pool::new(
                                            name,
                                            *max,
                                            resets_on.clone(),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }

                // Persist to disk if persistence + schema available
                let schema = gs.schema.clone();
                let description = format!("Spawned {} from encounter {}", npc_name, encounter.name);
                if let (Some(ref pl), Some(ref schema)) = (&self.persistence, &schema) {
                    if let Err(e) = pl.persist_character(&character, false, schema, &description) {
                        errors.push(format!("Failed to persist {npc_name}: {e}"));
                    }
                }

                spawned_names.push(npc_name.clone());
                self.game_state.as_mut().unwrap().add_npc(character);
            }
        }

        // Build output
        let mut lines: Vec<StyledLine> = Vec::new();
        if !encounter.description.is_empty() {
            lines.push(StyledLine::new(
                encounter.description.clone(),
                Style::default().fg(Color::Yellow),
            ));
        }
        if !spawned_names.is_empty() {
            lines.push(StyledLine::new(
                format!(
                    "Encounter \"{}\" spawned {} creature(s): {}",
                    encounter.name,
                    spawned_names.len(),
                    spawned_names.join(", ")
                ),
                Style::default().fg(Color::Green),
            ));
        }
        for err in &errors {
            lines.push(StyledLine::new(
                err.clone(),
                Style::default().fg(Color::Red),
            ));
        }
        if spawned_names.is_empty() && errors.is_empty() {
            lines.push(StyledLine::new(
                format!("Encounter \"{}\" has no creatures to spawn.", encounter.name),
                Style::default().fg(Color::Yellow),
            ));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn handle_combat_command(&mut self, args: &str) {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcmd = parts[0];
        let sub_args = parts.get(1).unwrap_or(&"").trim();

        match subcmd {
            "start" => {
                if self.mode == Mode::Combat {
                    self.apply_command_result(CommandResult::Error(
                        "Already in combat mode.".to_string(),
                    ));
                    return;
                }
                self.mode = Mode::Combat;
                self.initiative_tracker = Some(InitiativeTracker::new());
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Combat started. Use 'init add <name> <value>' to add combatants.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            "end" => {
                if self.mode != Mode::Combat {
                    self.apply_command_result(CommandResult::Error(
                        "Not in combat mode.".to_string(),
                    ));
                    return;
                }
                self.mode = Mode::Default;
                self.initiative_tracker = None;
                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Combat ended.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            "init" => self.handle_init_subcommand(sub_args),
            "next" => self.handle_combat_next(),
            "prev" => self.handle_combat_prev(),
            "status" => self.handle_combat_status(),
            "target" => self.handle_combat_target(sub_args),
            _ => {
                self.apply_command_result(CommandResult::Error(
                    format!("Unknown combat subcommand: '{subcmd}'. Use 'combat start' or 'combat end'."),
                ));
            }
        }
    }

    fn handle_init_subcommand(&mut self, args: &str) {
        let tracker = match self.initiative_tracker.as_mut() {
            Some(t) => t,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Not in combat mode. Use 'combat start' first.".to_string(),
                ));
                return;
            }
        };

        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let action = parts[0];
        let action_args = parts.get(1).unwrap_or(&"").trim();

        match action {
            "add" => {
                // init add <name> <value>
                let add_parts: Vec<&str> = action_args.rsplitn(2, ' ').collect();
                if add_parts.len() < 2 {
                    self.apply_command_result(CommandResult::Error(
                        "Usage: init add <name> <value>".to_string(),
                    ));
                    return;
                }
                let value_str = add_parts[0];
                let name = add_parts[1].trim();

                match value_str.parse::<f64>() {
                    Ok(value) => {
                        tracker.add(name, value);
                        tracker.sort();
                        self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                            format!("Added {name} with initiative {value}."),
                            Style::default().fg(Color::Green),
                        )]));
                    }
                    Err(_) => {
                        self.apply_command_result(CommandResult::Error(
                            format!("Invalid initiative value: '{value_str}'. Must be a number."),
                        ));
                    }
                }
            }
            "remove" => {
                if action_args.is_empty() {
                    self.apply_command_result(CommandResult::Error(
                        "Usage: init remove <name>".to_string(),
                    ));
                    return;
                }
                if tracker.remove(action_args) {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        format!("Removed {action_args} from initiative."),
                        Style::default().fg(Color::Green),
                    )]));
                } else {
                    self.apply_command_result(CommandResult::Error(
                        format!("'{action_args}' not found in initiative order."),
                    ));
                }
            }
            "roll" => {
                self.handle_init_roll(action_args);
            }
            _ => {
                self.apply_command_result(CommandResult::Error(
                    format!("Unknown init subcommand: '{action}'. Use 'init add', 'init remove', or 'init roll'."),
                ));
            }
        }
    }

    fn handle_init_roll(&mut self, args: &str) {
        // Auto-roll initiative for all loaded characters
        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let modifier: i32 = if args.is_empty() {
            0
        } else {
            match args.parse::<i32>() {
                Ok(m) => m,
                Err(_) => {
                    self.apply_command_result(CommandResult::Error(
                        format!("Invalid modifier: '{args}'. Must be an integer."),
                    ));
                    return;
                }
            }
        };

        // Collect character names
        let names: Vec<String> = gs.players.iter().chain(gs.npcs.iter())
            .map(|c| c.name.clone())
            .collect();

        if names.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "No characters loaded to roll initiative for.".to_string(),
            ));
            return;
        }

        let tracker = match self.initiative_tracker.as_mut() {
            Some(t) => t,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Not in combat mode. Use 'combat start' first.".to_string(),
                ));
                return;
            }
        };

        let mut lines = Vec::new();
        for name in &names {
            let roll: i32 = self.rng.gen_range(1..=20);
            let total = roll + modifier;
            tracker.add(name.clone(), total as f64);
            lines.push(StyledLine::new(
                format!("  {name}: rolled {roll} + {modifier} = {total}"),
                Style::default().fg(Color::Blue),
            ));
        }
        tracker.sort();

        lines.insert(0, StyledLine::new(
            "Initiative rolled:".to_string(),
            Style::default().fg(Color::Green),
        ));
        self.apply_command_result(CommandResult::Output(lines));
    }

    fn handle_combat_next(&mut self) {
        let tracker = match self.initiative_tracker.as_mut() {
            Some(t) => t,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Not in combat mode. Use 'combat start' first.".to_string(),
                ));
                return;
            }
        };

        let result = match tracker.next() {
            Some(c) => CommandResult::Output(vec![StyledLine::new(
                format!(">> {}'s turn (initiative: {})", c.name, c.initiative),
                Style::default().fg(Color::Cyan),
            )]),
            None => CommandResult::Error(
                "No combatants in initiative order. Use 'init add <name> <value>'.".to_string(),
            ),
        };
        self.apply_command_result(result);
    }

    fn handle_combat_prev(&mut self) {
        let tracker = match self.initiative_tracker.as_mut() {
            Some(t) => t,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Not in combat mode. Use 'combat start' first.".to_string(),
                ));
                return;
            }
        };

        let result = match tracker.prev() {
            Some(c) => CommandResult::Output(vec![StyledLine::new(
                format!("<< {}'s turn (initiative: {})", c.name, c.initiative),
                Style::default().fg(Color::Cyan),
            )]),
            None => CommandResult::Error(
                "No combatants in initiative order.".to_string(),
            ),
        };
        self.apply_command_result(result);
    }

    fn handle_combat_status(&mut self) {
        let tracker = match self.initiative_tracker.as_ref() {
            Some(t) => t,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Not in combat mode. Use 'combat start' first.".to_string(),
                ));
                return;
            }
        };

        if tracker.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                "No combatants. Use 'init add <name> <value>' to add.".to_string(),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let mut lines = vec![StyledLine::new(
            "Initiative Order:".to_string(),
            Style::default().fg(Color::Cyan),
        )];

        for (i, combatant) in tracker.all().iter().enumerate() {
            let marker = if combatant.is_current { ">>" } else { "  " };
            let mut line = format!("{marker} {}. {} (init: {})", i + 1, combatant.name, combatant.initiative);

            // Show HP/conditions if character exists in game state
            if let Some(gs) = &self.game_state {
                let character = gs.players.iter().chain(gs.npcs.iter())
                    .find(|c| c.name == combatant.name);
                if let Some(ch) = character {
                    if let Some(gauge) = ch.gauges.get("hp") {
                        line.push_str(&format!("  HP: {}/{}", gauge.current, gauge.max));
                    }
                    let conditions: Vec<&str> = ch.conditions.iter()
                        .filter(|c| c.active)
                        .map(|c| c.name.as_str())
                        .collect();
                    if !conditions.is_empty() {
                        line.push_str(&format!("  [{}]", conditions.join(", ")));
                    }
                }
            }

            let style = if combatant.is_current {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Blue)
            };
            lines.push(StyledLine::new(line, style));
        }

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn handle_combat_target(&mut self, args: &str) {
        if args.is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: target <name>".to_string(),
            ));
            return;
        }

        // Verify character exists
        let exists = if let Some(gs) = &self.game_state {
            gs.players.iter().chain(gs.npcs.iter())
                .any(|c| c.name.to_lowercase() == args.to_lowercase())
        } else {
            false
        };

        if !exists {
            // Check if combatant exists in tracker
            let in_tracker = self.initiative_tracker.as_ref()
                .map(|t| t.all().iter().any(|c| c.name.to_lowercase() == args.to_lowercase()))
                .unwrap_or(false);
            if !in_tracker {
                self.apply_command_result(CommandResult::Error(
                    format!("'{args}' not found."),
                ));
                return;
            }
        }

        self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
            format!("Target set to: {args}"),
            Style::default().fg(Color::Green),
        )]));
    }

    fn handle_history_command(&mut self, args: &str) {
        let pl = match &self.persistence {
            Some(pl) => pl,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded.".to_string(),
                ));
                return;
            }
        };

        let n: usize = if args.is_empty() {
            10
        } else {
            match args.parse() {
                Ok(v) => v,
                Err(_) => {
                    self.apply_command_result(CommandResult::Error(
                        "Usage: history [n] (e.g. history 5)".to_string(),
                    ));
                    return;
                }
            }
        };

        match pl.history(n) {
            Ok(entries) => {
                if entries.is_empty() {
                    self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                        "No history available.".to_string(),
                        Style::default().fg(Color::Yellow),
                    )]));
                    return;
                }

                let mut lines = vec![StyledLine::new(
                    format!("Last {} state change(s):", entries.len()),
                    Style::default().fg(Color::Cyan),
                )];

                for entry in &entries {
                    lines.push(StyledLine::plain(format!(
                        "  [{}] {}",
                        entry.hash, entry.message
                    )));
                }

                self.apply_command_result(CommandResult::Output(lines));
            }
            Err(e) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Cannot read history: {e}"
                )));
            }
        }
    }

    fn handle_undo_command(&mut self) {
        let pl = match &self.persistence {
            Some(pl) => pl,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded (no persistence layer).".to_string(),
                ));
                return;
            }
        };

        match pl.undo() {
            Ok(hash) => {
                self.redo_stack.push(hash);

                // Reload game state from files
                if let Some(gs) = &self.game_state {
                    let path = gs.campaign_path.clone();
                    let (new_gs, _) = GameState::load(&path);
                    self.game_state = Some(new_gs);
                }

                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Undo: reverted last change.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            Err(e) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Cannot undo: {e}"
                )));
            }
        }
    }

    fn handle_redo_command(&mut self) {
        let hash = match self.redo_stack.pop() {
            Some(h) => h,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "Nothing to redo.".to_string(),
                ));
                return;
            }
        };

        let pl = match &self.persistence {
            Some(pl) => pl,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded (no persistence layer).".to_string(),
                ));
                return;
            }
        };

        match pl.redo(&hash) {
            Ok(()) => {
                // Reload game state from files
                if let Some(gs) = &self.game_state {
                    let path = gs.campaign_path.clone();
                    let (new_gs, _) = GameState::load(&path);
                    self.game_state = Some(new_gs);
                }

                self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                    "Redo: re-applied last undone change.".to_string(),
                    Style::default().fg(Color::Green),
                )]));
            }
            Err(e) => {
                self.apply_command_result(CommandResult::Error(format!(
                    "Cannot redo: {e}"
                )));
            }
        }
    }

    fn handle_validate_command(&mut self, args: &str) {
        // Determine which path to validate
        let campaign_path = if !args.is_empty() {
            PathBuf::from(args)
        } else if let Some(gs) = &self.game_state {
            gs.campaign_path.clone()
        } else {
            self.apply_command_result(CommandResult::Error(
                "Usage: validate [path]\nNo campaign loaded. Provide a path or load a campaign first.".to_string(),
            ));
            return;
        };

        if !campaign_path.exists() || !campaign_path.is_dir() {
            self.apply_command_result(CommandResult::Error(format!(
                "Path not found: \"{}\"",
                campaign_path.display()
            )));
            return;
        }

        let mut lines = vec![StyledLine::new(
            format!("Validating campaign: {}", campaign_path.display()),
            Style::default().fg(Color::Cyan),
        )];

        let mut pass_count = 0u32;
        let mut fail_count = 0u32;

        // Validate system.toml
        let system_toml = campaign_path.join("rules").join("system.toml");
        if system_toml.exists() {
            match crate::rules::loader::load_rules(&system_toml) {
                Ok((_, schema)) => {
                    lines.push(StyledLine::new(
                        format!("  \u{2713} rules/system.toml — valid"),
                        Style::default().fg(Color::Green),
                    ));
                    pass_count += 1;

                    // Validate all character CSVs against the schema
                    let char_schema = &schema.character_schema;
                    let inv_schema = &schema.inventory_schema;

                    // Scan players/
                    self.validate_character_dir(
                        &campaign_path.join("players"),
                        "players",
                        char_schema,
                        inv_schema,
                        &mut lines,
                        &mut pass_count,
                        &mut fail_count,
                    );

                    // Scan npc/
                    self.validate_character_dir(
                        &campaign_path.join("npc"),
                        "npc",
                        char_schema,
                        inv_schema,
                        &mut lines,
                        &mut pass_count,
                        &mut fail_count,
                    );
                }
                Err(errors) => {
                    lines.push(StyledLine::new(
                        "\u{2717} rules/system.toml — invalid".to_string(),
                        Style::default().fg(Color::Red),
                    ));
                    for e in errors {
                        lines.push(StyledLine::new(
                            format!("    {e}"),
                            Style::default().fg(Color::Red),
                        ));
                    }
                    fail_count += 1;
                }
            }
        } else {
            lines.push(StyledLine::new(
                "  \u{2717} rules/system.toml — missing".to_string(),
                Style::default().fg(Color::Yellow),
            ));
            fail_count += 1;
        }

        // Validate map/world.json if present
        let map_json = campaign_path.join("map").join("world.json");
        if map_json.exists() {
            match crate::map::world::WorldMap::load(&map_json) {
                Ok(wm) => {
                    let burg_count = wm.map.pack.burgs.len().saturating_sub(1);
                    let state_count = wm.map.pack.states.len().saturating_sub(1);
                    lines.push(StyledLine::new(
                        format!("  \u{2713} map/world.json — valid ({burg_count} burgs, {state_count} states)"),
                        Style::default().fg(Color::Green),
                    ));
                    pass_count += 1;
                }
                Err(e) => {
                    lines.push(StyledLine::new(
                        format!("  \u{2717} map/world.json — {e}"),
                        Style::default().fg(Color::Red),
                    ));
                    fail_count += 1;
                }
            }
        }

        // Summary
        lines.push(StyledLine::new(
            format!("{pass_count} passed, {fail_count} failed."),
            if fail_count > 0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            },
        ));

        self.apply_command_result(CommandResult::Output(lines));
    }

    fn validate_character_dir(
        &self,
        dir: &Path,
        dir_name: &str,
        char_schema: &crate::schema::csv_schema::CsvSchema,
        inv_schema: &crate::schema::csv_schema::CsvSchema,
        lines: &mut Vec<StyledLine>,
        pass_count: &mut u32,
        fail_count: &mut u32,
    ) {
        if !dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let sheet = path.join("sheet.csv");
                if sheet.exists() {
                    let col_count = char_schema.columns.len();
                    match crate::schema::csv_schema::validate_csv(&sheet, char_schema) {
                        Ok(()) => {
                            lines.push(StyledLine::new(
                                format!("  \u{2713} {dir_name}/{name}/sheet.csv — {col_count} columns, matches schema"),
                                Style::default().fg(Color::Green),
                            ));
                            *pass_count += 1;
                        }
                        Err(errors) => {
                            lines.push(StyledLine::new(
                                format!("  \u{2717} {dir_name}/{name}/sheet.csv — validation failed"),
                                Style::default().fg(Color::Red),
                            ));
                            for e in errors {
                                lines.push(StyledLine::new(
                                    format!("    {e}"),
                                    Style::default().fg(Color::Red),
                                ));
                            }
                            *fail_count += 1;
                        }
                    }
                }

                let inv = path.join("inventory.csv");
                if inv.exists() {
                    match crate::schema::csv_schema::validate_csv(&inv, inv_schema) {
                        Ok(()) => {
                            lines.push(StyledLine::new(
                                format!("  \u{2713} {dir_name}/{name}/inventory.csv — valid"),
                                Style::default().fg(Color::Green),
                            ));
                            *pass_count += 1;
                        }
                        Err(errors) => {
                            lines.push(StyledLine::new(
                                format!("  \u{2717} {dir_name}/{name}/inventory.csv — validation failed"),
                                Style::default().fg(Color::Red),
                            ));
                            for e in errors {
                                lines.push(StyledLine::new(
                                    format!("    {e}"),
                                    Style::default().fg(Color::Red),
                                ));
                            }
                            *fail_count += 1;
                        }
                    }
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("csv") {
                // Bulk CSV at root
                let name = entry.file_name().to_string_lossy().to_string();
                match crate::schema::csv_schema::validate_csv(&path, char_schema) {
                    Ok(()) => {
                        lines.push(StyledLine::new(
                            format!("  \u{2713} {dir_name}/{name} — valid"),
                            Style::default().fg(Color::Green),
                        ));
                        *pass_count += 1;
                    }
                    Err(errors) => {
                        lines.push(StyledLine::new(
                            format!("  \u{2717} {dir_name}/{name} — validation failed"),
                            Style::default().fg(Color::Red),
                        ));
                        for e in errors {
                            lines.push(StyledLine::new(
                                format!("    {e}"),
                                Style::default().fg(Color::Red),
                            ));
                        }
                        *fail_count += 1;
                    }
                }
            }
        }
    }

    fn handle_search_command(&mut self, args: &str) {
        if args.trim().is_empty() {
            self.apply_command_result(CommandResult::Error(
                "Usage: search <query> (e.g. search goblin king)".to_string(),
            ));
            return;
        }

        let gs = match &self.game_state {
            Some(gs) => gs,
            None => {
                self.apply_command_result(CommandResult::Error(
                    "No campaign loaded. Use \"load <path>\" first.".to_string(),
                ));
                return;
            }
        };

        let results = gs.search_index.search(args.trim(), 5);

        if results.is_empty() {
            self.apply_command_result(CommandResult::Output(vec![StyledLine::new(
                format!("No results found for \"{args}\"."),
                Style::default().fg(Color::Yellow),
            )]));
            return;
        }

        let mut lines = vec![StyledLine::new(
            format!("Search results for \"{}\":", args.trim()),
            Style::default().fg(Color::Blue),
        )];

        for (i, result) in results.iter().enumerate() {
            lines.push(StyledLine::new(
                format!("  {}. [{}]", i + 1, result.source),
                Style::default().fg(Color::Cyan),
            ));
            // Truncate long passages
            let passage = if result.passage.len() > 200 {
                format!("{}...", &result.passage[..200])
            } else {
                result.passage.clone()
            };
            lines.push(StyledLine::new(
                format!("     {passage}"),
                Style::default().fg(Color::White),
            ));
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

    /// Returns a sound status indicator string for the TUI header.
    /// e.g., "♪ tavern.mp3" when playing, "♪ paused" when paused, "" when stopped.
    pub fn sound_status_indicator(&self) -> String {
        let player = match &self.audio_player {
            Some(p) => p,
            None => return String::new(),
        };

        if let Some(track) = player.current_track() {
            if player.is_paused() {
                format!("♪ {track} (paused)")
            } else if player.is_playing() {
                let looping = if player.is_looping() { " ↻" } else { "" };
                format!("♪ {track}{looping}")
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    pub fn autocomplete_hint(&self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }
        let input_lower = self.input.to_lowercase();

        // Try built-in commands first
        let builtin_match = crate::commands::mapping::COMMANDS
            .iter()
            .find(|cmd| cmd.starts_with(&input_lower) && **cmd != input_lower)
            .map(|cmd| cmd[self.input.len()..].to_string());

        if builtin_match.is_some() {
            return builtin_match;
        }

        // Then try custom commands from loaded campaign
        if let Some(gs) = &self.game_state {
            let mut custom_names: Vec<&String> = gs.custom_commands.keys().collect();
            custom_names.sort();
            return custom_names
                .iter()
                .find(|cmd| cmd.starts_with(&input_lower) && ***cmd != input_lower)
                .map(|cmd| cmd[self.input.len()..].to_string());
        }

        None
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
        std::fs::write(player_dir.join("sheet.csv"), "name,strength\nBroken,10\n").unwrap();

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

    // --- new command tests ---

    #[test]
    fn test_new_command_creates_campaign() {
        let dir = TempDir::new().unwrap();

        // Create a template
        let template_dir = dir.path().join("template");
        std::fs::create_dir_all(template_dir.join("rules")).unwrap();
        std::fs::write(
            template_dir.join("rules/system.toml"),
            "[system]\nname = \"Test System\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(template_dir.join("players/hero")).unwrap();
        std::fs::write(
            template_dir.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let dest = dir.path().join("new_campaign");
        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!(
            "new {} {}",
            dest.display(),
            template_dir.display()
        ));

        // New campaign should exist with correct structure
        assert!(dest.join("rules/system.toml").exists());
        assert!(dest.join("players").exists());
        assert!(dest.join("npc").exists());
        assert!(dest.join("notes").exists());

        // rules/ should be copied as-is
        let system_toml = std::fs::read_to_string(dest.join("rules/system.toml")).unwrap();
        assert!(system_toml.contains("Test System"));

        // players/ should be empty (not copied from template)
        let player_entries: Vec<_> = std::fs::read_dir(dest.join("players")).unwrap().collect();
        assert!(
            player_entries.is_empty(),
            "players/ should be empty in new campaign"
        );

        // Output should show success
        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("created")));
    }

    #[test]
    fn test_new_command_rules_match_template() {
        let dir = TempDir::new().unwrap();

        let template_dir = dir.path().join("template");
        std::fs::create_dir_all(template_dir.join("rules/commands")).unwrap();
        std::fs::write(
            template_dir.join("rules/system.toml"),
            "[system]\nname = \"My System\"\nversion = \"2.0\"\n",
        )
        .unwrap();
        std::fs::write(
            template_dir.join("rules/commands/test.lol"),
            "HAI 1.2\nKTHXBYE\n",
        )
        .unwrap();

        let dest = dir.path().join("new_game");
        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!(
            "new {} {}",
            dest.display(),
            template_dir.display()
        ));

        // rules/ fully copied including subdirectories
        assert!(dest.join("rules/system.toml").exists());
        assert!(dest.join("rules/commands/test.lol").exists());

        let original = std::fs::read_to_string(template_dir.join("rules/system.toml")).unwrap();
        let copied = std::fs::read_to_string(dest.join("rules/system.toml")).unwrap();
        assert_eq!(original, copied);
    }

    #[test]
    fn test_new_command_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("new");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Usage")));
    }

    #[test]
    fn test_new_command_missing_template() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("new my_game /nonexistent/template/12345");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("not found")));
    }

    #[test]
    fn test_new_command_invalid_template_missing_rules() {
        let dir = TempDir::new().unwrap();
        let bad_template = dir.path().join("bad_template");
        std::fs::create_dir_all(&bad_template).unwrap();

        let dest = dir.path().join("new_campaign");
        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!(
            "new {} {}",
            dest.display(),
            bad_template.display()
        ));

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("rules/")));
        assert!(!dest.exists(), "campaign should not be created");
    }

    #[test]
    fn test_new_command_invalid_template_bad_rules() {
        let dir = TempDir::new().unwrap();
        let bad_template = dir.path().join("bad_rules_template");
        std::fs::create_dir_all(bad_template.join("rules")).unwrap();
        std::fs::write(
            bad_template.join("rules/system.toml"),
            "[system]\nname = 42\n",
        )
        .unwrap();

        let dest = dir.path().join("new_campaign");
        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!(
            "new {} {}",
            dest.display(),
            bad_template.display()
        ));

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("invalid")),
            "Should report invalid rules. Messages: {output_texts:?}"
        );
        assert!(!dest.exists(), "campaign should not be created");
    }

    #[test]
    fn test_new_command_dest_already_exists() {
        let dir = TempDir::new().unwrap();

        let template_dir = dir.path().join("template");
        std::fs::create_dir_all(template_dir.join("rules")).unwrap();
        std::fs::write(
            template_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();

        let dest = dir.path().join("existing");
        std::fs::create_dir_all(&dest).unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!(
            "new {} {}",
            dest.display(),
            template_dir.display()
        ));

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("already exists")));
    }

    // --- help command campaign status tests ---

    #[test]
    fn test_help_shows_no_campaign_when_not_loaded() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("help");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts
                .iter()
                .any(|t| t.contains("No campaign loaded")),
            "help should show 'No campaign loaded'. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_help_shows_campaign_status_when_loaded() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("test_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test System\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("help");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts
                .iter()
                .any(|t| t.contains("test_campaign") && t.contains("loaded")),
            "help should show campaign name. Messages: {output_texts:?}"
        );
        assert!(
            output_texts.iter().any(|t| t.contains("Test System")),
            "help should show system name. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_help_shows_custom_commands_when_available() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("cmd_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules/commands")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"CMD Test\"\n",
        )
        .unwrap();
        std::fs::write(
            campaign_dir.join("rules/commands/smite.lol"),
            "HAI 1.2\nKTHXBYE\n",
        )
        .unwrap();
        std::fs::write(
            campaign_dir.join("rules/commands/heal.lol"),
            "HAI 1.2\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("help");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts
                .iter()
                .any(|t| t.contains("heal") && t.contains("smite")),
            "help should list custom commands. Messages: {output_texts:?}"
        );
    }

    // --- custom command execution tests ---

    #[test]
    fn test_custom_command_executes_and_shows_output() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("lol_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules/commands")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        let player_dir = campaign_dir.join("players/hero");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(player_dir.join("sheet.csv"), "name,strength\nHero,15\n").unwrap();
        std::fs::write(
            campaign_dir.join("rules/commands/greet.lol"),
            "HAI 1.2\nI IZ RUSTORY_DISPLAY YR \"Hello from script\" MKAY\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("greet");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("Hello from script")),
            "Custom command should produce output. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_custom_command_reads_stat() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("stat_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules/commands")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        let player_dir = campaign_dir.join("players/thorin");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(
            player_dir.join("sheet.csv"),
            "name,strength\nThorin,18\n",
        )
        .unwrap();
        std::fs::write(
            campaign_dir.join("rules/commands/showstr.lol"),
            "HAI 1.2\nI IZ RUSTORY_GET_PLAYER YR \"Thorin\" MKAY\nI HAS A STR ITZ I IZ RUSTORY_GET_STAT YR \"strength\" MKAY\nVISIBLE STR\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("showstr");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("18")),
            "Custom command should show stat value. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_builtin_wins_over_custom_command() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("override_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules/commands")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        // Create a custom command named "help" — should NOT override built-in
        std::fs::write(
            campaign_dir.join("rules/commands/help.lol"),
            "HAI 1.2\nI IZ RUSTORY_DISPLAY YR \"custom help\" MKAY\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("help");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        // Built-in help should win
        assert!(
            output_texts.iter().any(|t| t.contains("Available commands")),
            "Built-in help should win over custom. Messages: {output_texts:?}"
        );
        assert!(
            !output_texts.iter().any(|t| t.contains("custom help")),
            "Custom help should NOT execute. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_unknown_command_no_campaign() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("customcmd");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("Unknown command")),
            "Unknown command without campaign should show error. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_custom_command_script_error_shows_message() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("error_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules/commands")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        std::fs::write(
            campaign_dir.join("rules/commands/broken.lol"),
            "THIS IS NOT VALID LOLCODE",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("load {}", campaign_dir.display()));
        app.messages.clear();

        app.dispatch_command("broken");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("Script error")),
            "Broken script should show error. Messages: {output_texts:?}"
        );
    }

    // --- autocomplete tests ---

    #[test]
    fn test_autocomplete_builtin_commands() {
        let app = App::new();
        // No game state loaded, should still autocomplete built-in commands
        let mut test_app = app;
        test_app.input = "he".to_string();
        assert_eq!(test_app.autocomplete_hint(), Some("lp".to_string()));

        test_app.input = "ro".to_string();
        assert_eq!(test_app.autocomplete_hint(), Some("ll".to_string()));

        test_app.input = "qu".to_string();
        assert_eq!(test_app.autocomplete_hint(), Some("it".to_string()));
    }

    #[test]
    fn test_autocomplete_empty_input_returns_none() {
        let app = App::new();
        assert_eq!(app.autocomplete_hint(), None);
    }

    #[test]
    fn test_autocomplete_exact_match_returns_none() {
        let mut app = App::new();
        app.input = "help".to_string();
        assert_eq!(app.autocomplete_hint(), None);
    }

    #[test]
    fn test_autocomplete_no_match_returns_none() {
        let mut app = App::new();
        app.input = "zzz".to_string();
        assert_eq!(app.autocomplete_hint(), None);
    }

    #[test]
    fn test_autocomplete_includes_custom_commands() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules/commands")).unwrap();
        std::fs::write(
            dir.path().join("rules/commands/smite.lol"),
            "HAI 1.2\nVISIBLE \"smite!\"\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.load_campaign(dir.path());

        // Custom command should be autocompleted
        app.input = "sm".to_string();
        assert_eq!(app.autocomplete_hint(), Some("ite".to_string()));
    }

    #[test]
    fn test_autocomplete_custom_commands_only_when_campaign_loaded() {
        let mut app = App::new();
        // No campaign loaded — "sm" should not autocomplete
        app.input = "sm".to_string();
        assert_eq!(app.autocomplete_hint(), None);

        // Now load a campaign with a custom command
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules/commands")).unwrap();
        std::fs::write(
            dir.path().join("rules/commands/smite.lol"),
            "HAI 1.2\nVISIBLE \"smite!\"\nKTHXBYE\n",
        )
        .unwrap();
        app.load_campaign(dir.path());

        // Now it should autocomplete
        app.input = "sm".to_string();
        assert_eq!(app.autocomplete_hint(), Some("ite".to_string()));
    }

    #[test]
    fn test_autocomplete_builtin_takes_priority_over_custom() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules/commands")).unwrap();
        // Create a custom command starting with "hel" (like "help")
        std::fs::write(
            dir.path().join("rules/commands/helfire.lol"),
            "HAI 1.2\nVISIBLE \"fire!\"\nKTHXBYE\n",
        )
        .unwrap();

        let mut app = App::new();
        app.load_campaign(dir.path());

        // "hel" should match built-in "help" first, not custom "helfire"
        app.input = "hel".to_string();
        assert_eq!(app.autocomplete_hint(), Some("p".to_string()));
    }

    // --- search command tests ---

    // --- map mode tests ---

    #[test]
    fn test_map_command_no_map_returns_error() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("map");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("No map loaded")),
            "Map without world.json should show error. Messages: {output_texts:?}"
        );
        assert!(app.mode != Mode::Map);
    }

    fn make_test_world_map() -> crate::map::world::WorldMap {
        let json = r##"{
            "info": {"width": 800, "height": 600},
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Silverport", "x": 100.0, "y": 100.0, "population": 28.5, "state": 1, "culture": 1, "type": "City", "capital": 1, "port": 1},
                    {"i": 2, "name": "Ironhold", "x": 200.0, "y": 100.0, "population": 12.0, "state": 1, "culture": 1, "type": "Town"},
                    {"i": 3, "name": "Silver Lake", "x": 120.0, "y": 110.0, "population": 3.0, "state": 1}
                ],
                "states": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Kingdom of Light", "form": "Monarchy"}
                ],
                "cultures": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Elven", "type": "Lake"}
                ],
                "routes": [
                    {"i": 1, "points": [{"x": 100.0, "y": 100.0}, {"x": 200.0, "y": 100.0}], "group": "roads", "length": 100.0}
                ]
            }
        }"##;
        crate::map::world::WorldMap::from_parsed(
            crate::map::azgaar::parse_azgaar_json(json).unwrap(),
        )
    }

    #[test]
    fn test_map_command_toggles_mode() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());

        app.dispatch_command("map");
        assert!(app.mode == Mode::Map, "map command should enable map mode");

        app.dispatch_command("map");
        assert!(app.mode == Mode::Default, "map command again should disable map mode");
    }

    #[test]
    fn test_map_list_burgs() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map list burgs");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Silverport")));
        assert!(output_texts.iter().any(|t| t.contains("Ironhold")));
    }

    #[test]
    fn test_map_list_states() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map list states");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Kingdom of Light")));
    }

    #[test]
    fn test_map_info_burg() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map info Silverport");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Silverport")));
        assert!(output_texts.iter().any(|t| t.contains("Population")));
        assert!(output_texts.iter().any(|t| t.contains("Kingdom of Light")));
    }

    #[test]
    fn test_map_info_not_found() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map info Nowhere");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("No burg or state")));
    }

    #[test]
    fn test_map_search() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map search silver");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Silverport")));
        assert!(output_texts.iter().any(|t| t.contains("Silver Lake")));
    }

    #[test]
    fn test_map_near() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map near Silverport 150");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Silver Lake")));
        assert!(output_texts.iter().any(|t| t.contains("Ironhold")));
    }

    #[test]
    fn test_map_where_no_location() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("where_test");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign_dir.join("players/thorin")).unwrap();
        std::fs::write(
            campaign_dir.join("players/thorin/sheet.csv"),
            "name\nThorin\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign_dir);
        app.world_map = Some(make_test_world_map());
        app.messages.clear();

        app.dispatch_command("map where Thorin");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("no known location")),
            "Should say no location. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_map_move_character() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("move_test");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign_dir.join("players/thorin")).unwrap();
        std::fs::write(
            campaign_dir.join("players/thorin/sheet.csv"),
            "name\nThorin\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign_dir);
        app.world_map = Some(make_test_world_map());
        app.messages.clear();

        app.dispatch_command("map move Thorin Silverport");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("moved") && t.contains("Silverport")),
            "Should confirm move. Messages: {output_texts:?}"
        );

        // Verify location is set
        let thorin = app.game_state.as_ref().unwrap().get_player("Thorin").unwrap();
        assert_eq!(thorin.location.as_deref(), Some("Silverport"));
    }

    #[test]
    fn test_map_move_invalid_location() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("move_invalid_test");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign_dir.join("players/thorin")).unwrap();
        std::fs::write(
            campaign_dir.join("players/thorin/sheet.csv"),
            "name\nThorin\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign_dir);
        app.world_map = Some(make_test_world_map());
        app.messages.clear();

        app.dispatch_command("map move Thorin Nowhere");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("not found on the map")),
            "Should reject invalid location. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_map_route() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());
        app.dispatch_command("map route Silverport Ironhold");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(output_texts.iter().any(|t| t.contains("Route")));
        assert!(output_texts.iter().any(|t| t.contains("roads")));
    }

    #[test]
    fn test_map_mode_arrow_keys_pan() {
        let mut app = App::new();
        app.running = true;
        app.mode = Mode::Map;
        let json = r##"{"pack": {}}"##;
        app.world_map = Some(
            crate::map::world::WorldMap::from_parsed(
                crate::map::azgaar::parse_azgaar_json(json).unwrap(),
            ),
        );

        let initial_x = app.map_viewport.offset_x;
        let initial_y = app.map_viewport.offset_y;

        app.on_key(KeyEvent::from(KeyCode::Right));
        assert!(
            app.map_viewport.offset_x > initial_x,
            "Right arrow should pan right"
        );

        app.on_key(KeyEvent::from(KeyCode::Up));
        assert!(
            app.map_viewport.offset_y > initial_y,
            "Up arrow should pan up"
        );
    }

    #[test]
    fn test_map_mode_escape_exits() {
        let mut app = App::new();
        app.running = true;
        app.mode = Mode::Map;

        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(
            app.mode == Mode::Default,
            "Esc in map mode should exit map mode, not quit"
        );
        assert!(app.running, "App should still be running after Esc in map mode");
    }

    #[test]
    fn test_map_mode_zoom() {
        let mut app = App::new();
        app.running = true;
        app.mode = Mode::Map;

        let initial_zoom = app.map_viewport.zoom;
        app.on_key(KeyEvent::from(KeyCode::Char('+')));
        assert!(app.map_viewport.zoom > initial_zoom, "Plus should zoom in");

        let zoom_after_in = app.map_viewport.zoom;
        app.on_key(KeyEvent::from(KeyCode::Char('-')));
        assert!(
            app.map_viewport.zoom < zoom_after_in,
            "Minus should zoom out"
        );
    }

    #[test]
    fn test_search_no_campaign_returns_error() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("search goblin");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("No campaign loaded")),
            "Search without campaign should show error. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_search_no_query_returns_usage() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("search");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("Usage")),
            "Search without query should show usage. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_search_returns_results_from_lore() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("search_campaign");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        // Create NPC with lore
        std::fs::create_dir_all(campaign_dir.join("npc/dragon")).unwrap();
        std::fs::write(
            campaign_dir.join("npc/dragon/lore.md"),
            "# Ancient Dragon\nThe ancient dragon sleeps beneath the frozen mountain.",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign_dir);
        app.messages.clear();

        app.dispatch_command("search ancient dragon");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("ancient dragon")
                || t.contains("Ancient Dragon")
                || t.contains("dragon")),
            "Search should find content from lore. Messages: {output_texts:?}"
        );
    }

    #[test]
    fn test_search_no_results_returns_message() {
        let dir = TempDir::new().unwrap();
        let campaign_dir = dir.path().join("empty_search");
        std::fs::create_dir_all(campaign_dir.join("rules")).unwrap();
        std::fs::write(
            campaign_dir.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign_dir);
        app.messages.clear();

        app.dispatch_command("search unicorn rainbow");

        let output_texts: Vec<&str> = app.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            output_texts.iter().any(|t| t.contains("No results")),
            "Search with no matches should say no results. Messages: {output_texts:?}"
        );
    }

    // --- Sound command tests ---

    fn create_campaign_with_sound() -> TempDir {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("sound_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"SoundTest\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("sound/ambiance")).unwrap();
        std::fs::create_dir_all(campaign.join("sound/combat")).unwrap();
        std::fs::write(campaign.join("sound/ambiance/tavern.mp3"), b"fake").unwrap();
        std::fs::write(campaign.join("sound/ambiance/forest.ogg"), b"fake").unwrap();
        std::fs::write(campaign.join("sound/combat/battle.wav"), b"fake").unwrap();
        std::fs::write(campaign.join("sound/theme.flac"), b"fake").unwrap();
        dir
    }

    #[test]
    fn test_sound_list_shows_library() {
        let dir = create_campaign_with_sound();
        let campaign = dir.path().join("sound_test");
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Sound library"), "Should show library header. Got: {output}");
        assert!(output.contains("ambiance"), "Should list ambiance dir. Got: {output}");
        assert!(output.contains("combat"), "Should list combat dir. Got: {output}");
        assert!(output.contains("theme.flac"), "Should list root file. Got: {output}");
    }

    #[test]
    fn test_sound_list_subfolder() {
        let dir = create_campaign_with_sound();
        let campaign = dir.path().join("sound_test");
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound list ambiance");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("tavern.mp3"), "Should list tavern. Got: {output}");
        assert!(output.contains("forest.ogg"), "Should list forest. Got: {output}");
    }

    #[test]
    fn test_sound_list_empty_library() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("no_sound");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"NoSound\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("empty"), "Should report empty library. Got: {output}");
    }

    #[test]
    fn test_sound_search_finds_files() {
        let dir = create_campaign_with_sound();
        let campaign = dir.path().join("sound_test");
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound search tavern");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("tavern.mp3"), "Should find tavern.mp3. Got: {output}");
    }

    #[test]
    fn test_sound_search_no_match() {
        let dir = create_campaign_with_sound();
        let campaign = dir.path().join("sound_test");
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound search nonexistent");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No audio files"), "Should report no matches. Got: {output}");
    }

    #[test]
    fn test_sound_play_missing_file() {
        let dir = create_campaign_with_sound();
        let campaign = dir.path().join("sound_test");
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("sound play nonexistent.mp3");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report file not found. Got: {output}");
    }

    #[test]
    fn test_sound_play_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound play");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_sound_volume_valid() {
        let mut app = App::new();
        app.running = true;

        // If no audio device, this should report "Audio device not available"
        // If audio device available, it should set volume
        app.dispatch_command("sound volume 50");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(
            output.contains("Volume set to 50%") || output.contains("Audio device"),
            "Should set volume or report no device. Got: {output}"
        );
    }

    #[test]
    fn test_sound_volume_invalid() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound volume abc");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Invalid volume"), "Should report invalid volume. Got: {output}");
    }

    #[test]
    fn test_sound_volume_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound volume");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_sound_status() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound status");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        // Either shows "No track loaded" or "Audio device not available"
        assert!(
            output.contains("No track loaded") || output.contains("Audio device"),
            "Should show status or report no device. Got: {output}"
        );
    }

    #[test]
    fn test_sound_stop() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound stop");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(
            output.contains("stopped") || output.contains("Audio device"),
            "Should stop or report no device. Got: {output}"
        );
    }

    #[test]
    fn test_sound_unknown_subcommand() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound foobar");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Unknown sound subcommand"), "Should report unknown subcommand. Got: {output}");
    }

    #[test]
    fn test_sound_loop_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound loop");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_sound_search_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("sound search");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_help_includes_sound() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("help");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("sound"), "Help should mention sound command. Got: {output}");
    }

    #[test]
    fn test_sound_status_indicator_no_player() {
        let mut app = App::new();
        app.audio_player = None;
        assert_eq!(app.sound_status_indicator(), "");
    }

    #[test]
    fn test_sound_status_indicator_no_track() {
        let app = App::new();
        // If audio device is available, no track should give empty string
        // If no audio device, also empty string
        assert_eq!(app.sound_status_indicator(), "");
    }

    // --- Git persistence on campaign load tests ---

    #[test]
    fn test_load_campaign_creates_git_repo() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("git_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"GitTest\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);

        // .git/ should be created
        assert!(campaign.join(".git").exists(), ".git/ should be created on load");
        assert!(app.persistence.is_some(), "Persistence layer should be initialized");
    }

    #[test]
    fn test_load_campaign_opens_existing_git_repo() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("git_existing");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"GitTest\"\n",
        )
        .unwrap();

        // First load creates .git/
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        assert!(campaign.join(".git").exists());

        // Second load should open existing
        app.load_campaign(&campaign);
        assert!(app.persistence.is_some());
    }

    // --- Undo/Redo tests ---

    #[test]
    fn test_undo_no_campaign() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("undo");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign") || output.contains("Cannot"), "Should error. Got: {output}");
    }

    #[test]
    fn test_redo_nothing_to_redo() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("redo");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Nothing to redo"), "Should report nothing to redo. Got: {output}");
    }

    #[test]
    fn test_undo_redo_with_campaign() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("undo_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"UndoTest\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);

        // Make a change via persistence
        if let Some(ref pl) = app.persistence {
            let schema = app.game_state.as_ref().unwrap().schema.as_ref().unwrap();
            let mut ch = crate::game_state::Character::new("Hero");
            ch.stats = vec![crate::game_state::primitives::Stat::new("strength", 20.0)];
            pl.persist_character(&ch, true, schema, "Strength to 20").unwrap();
        }
        app.messages.clear();

        // Undo
        app.dispatch_command("undo");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Undo") || output.contains("reverted"), "Should undo. Got: {output}");

        // Verify redo stack has an entry
        assert_eq!(app.redo_stack.len(), 1);

        // Redo
        app.messages.clear();
        app.dispatch_command("redo");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Redo") || output.contains("re-applied"), "Should redo. Got: {output}");

        // Redo stack should be empty now
        assert!(app.redo_stack.is_empty());
    }

    #[test]
    fn test_new_command_clears_redo_stack() {
        let mut app = App::new();
        app.running = true;
        app.redo_stack.push("abc123".to_string());

        // Any non-undo/redo command should clear redo stack
        app.dispatch_command("help");
        assert!(app.redo_stack.is_empty(), "Redo stack should be cleared after a regular command");
    }

    // --- Show/Set/List command tests ---

    #[test]
    fn test_show_character() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("show_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"ShowTest\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\"]\n\n[resources.hp]\ntype = \"gauge\"\nmax_stat = \"hp_max\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/thorin")).unwrap();
        std::fs::write(
            campaign.join("players/thorin/sheet.csv"),
            "name,strength,hp_max\nThorin,18,52\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("show thorin");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Thorin"), "Should show character name. Got: {output}");
        assert!(output.contains("18"), "Should show strength. Got: {output}");
    }

    #[test]
    fn test_show_specific_field() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("show_field");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("show hero strength");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("15"), "Should show strength value. Got: {output}");
    }

    #[test]
    fn test_show_unknown_character() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("show_unknown");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("show nobody");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report not found. Got: {output}");
    }

    #[test]
    fn test_set_stat() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("set_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("set hero.strength 20");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("20"), "Should show new value. Got: {output}");

        // Verify the stat was actually changed
        assert_eq!(
            app.game_state.as_ref().unwrap().get_player("Hero").unwrap().get_stat("strength"),
            Some(20.0)
        );
    }

    #[test]
    fn test_set_unknown_field() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("set_unknown");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("set hero.charisma 10");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report field not found. Got: {output}");
    }

    #[test]
    fn test_list_players() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("list_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("list players");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Hero"), "Should list Hero. Got: {output}");
    }

    #[test]
    fn test_list_npcs_empty() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("list_empty");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("list npcs");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No NPCs"), "Should report no NPCs. Got: {output}");
    }

    // --- History command tests ---

    #[test]
    fn test_history_no_campaign() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("history");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign"), "Should report no campaign. Got: {output}");
    }

    #[test]
    fn test_history_shows_commits() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("hist_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"HistTest\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("history");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Initial state"), "Should show initial commit. Got: {output}");
    }

    // --- Validate command tests ---

    #[test]
    fn test_validate_clean_campaign() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("valid_camp");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"ValidTest\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("validate {}", campaign.display()));

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("\u{2713}"), "Should have pass marks. Got: {output}");
        assert!(output.contains("0 failed"), "Should have 0 failures. Got: {output}");
    }

    #[test]
    fn test_validate_campaign_with_bad_csv() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bad_camp");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"BadTest\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\", \"dexterity\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("players/hero")).unwrap();
        // Missing "dexterity" column
        std::fs::write(
            campaign.join("players/hero/sheet.csv"),
            "name,strength\nHero,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.dispatch_command(&format!("validate {}", campaign.display()));

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("\u{2717}"), "Should have fail marks. Got: {output}");
        assert!(!output.contains("0 failed"), "Should have >0 failures. Got: {output}");
    }

    #[test]
    fn test_validate_no_campaign_no_args() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("validate");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign loaded") || output.contains("Usage"), "Should report error. Got: {output}");
    }

    #[test]
    fn test_validate_loaded_campaign() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("loaded_camp");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"LoadedTest\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        // validate without args uses loaded campaign
        app.dispatch_command("validate");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Validating"), "Should validate loaded campaign. Got: {output}");
    }

    #[test]
    fn test_load_campaign_commits_manual_edits() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("manual_edit");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"ManualTest\"\n",
        )
        .unwrap();

        // First load: initializes git
        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);

        // Simulate manual edit: modify a file outside the app
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"ManualTest Modified\"\n",
        )
        .unwrap();

        // Second load: should detect and commit the manual edit
        app.load_campaign(&campaign);

        // Verify git history has the manual edit commit
        if let Some(ref pl) = app.persistence {
            let history = pl.history(10).unwrap();
            assert!(
                history.iter().any(|h| h.message.contains("Manual edit")),
                "Should have a manual edit commit. History: {:?}",
                history.iter().map(|h| &h.message).collect::<Vec<_>>()
            );
        }
    }

    // --- Bestiary command tests ---

    #[test]
    fn test_bestiary_no_campaign() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("bestiary");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign"), "Should report no campaign. Got: {output}");
    }

    #[test]
    fn test_bestiary_list_empty() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_empty");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No creature templates"), "Should report empty bestiary. Got: {output}");
    }

    #[test]
    fn test_bestiary_list_shows_creatures() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_list");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\", \"ac\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength,hp_max,ac\nGoblin,8,7,15\n",
        )
        .unwrap();
        std::fs::write(
            campaign.join("bestiary/orc.csv"),
            "name,strength,hp_max,ac\nOrc,16,15,13\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary list");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Goblin"), "Should list Goblin. Got: {output}");
        assert!(output.contains("Orc"), "Should list Orc. Got: {output}");
        assert!(output.contains("HP: 7"), "Should show Goblin HP. Got: {output}");
        assert!(output.contains("AC: 15"), "Should show Goblin AC. Got: {output}");
    }

    #[test]
    fn test_bestiary_info_shows_stats() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_info");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\", \"ac\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength,hp_max,ac\nGoblin,8,7,15\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary info goblin");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Goblin"), "Should show creature name. Got: {output}");
        assert!(output.contains("strength"), "Should show strength stat. Got: {output}");
        assert!(output.contains("8"), "Should show strength value. Got: {output}");
        assert!(output.contains("hp_max"), "Should show hp_max stat. Got: {output}");
        assert!(output.contains("7"), "Should show hp_max value. Got: {output}");
    }

    #[test]
    fn test_bestiary_info_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_case");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength\nGoblin,8\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary info GOBLIN");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Goblin"), "Should find Goblin case-insensitively. Got: {output}");
    }

    #[test]
    fn test_bestiary_info_not_found() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_nf");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary")).unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary info dragon");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report not found. Got: {output}");
    }

    #[test]
    fn test_bestiary_info_no_args() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_noargs");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary info");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_bestiary_unknown_subcommand() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("bestiary_bad");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"T\"\n",
        )
        .unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("bestiary foo");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Unknown bestiary subcommand"), "Should report unknown subcommand. Got: {output}");
    }

    // --- Spawn command tests ---

    fn setup_bestiary_campaign(dir: &TempDir, name: &str) -> std::path::PathBuf {
        let campaign = dir.path().join(name);
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n\
             [character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\", \"ac\"]\n\n\
             [resources.hp]\ntype = \"gauge\"\nmax_stat = \"hp_max\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength,hp_max,ac\nGoblin,8,7,15\n",
        )
        .unwrap();
        campaign
    }

    #[test]
    fn test_spawn_creates_npc_with_correct_stats() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_bestiary_campaign(&dir, "spawn_stats");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("spawn goblin Guard");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Spawned Guard"), "Should confirm spawn. Got: {output}");

        let gs = app.game_state().unwrap();
        let npc = gs.get_npc("Guard").expect("Guard NPC should exist");
        assert_eq!(npc.get_stat("strength"), Some(8.0));
        assert_eq!(npc.get_stat("hp_max"), Some(7.0));
        assert_eq!(npc.get_stat("ac"), Some(15.0));
        // Resource def should create hp gauge
        assert!(npc.gauges.contains_key("hp"), "HP gauge should be created from resource_defs");
        assert_eq!(npc.gauges["hp"].max, 7.0);
        assert_eq!(npc.gauges["hp"].current, 7.0);
    }

    #[test]
    fn test_spawn_auto_names_incrementing() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_bestiary_campaign(&dir, "spawn_autoname");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("spawn goblin");
        app.dispatch_command("spawn goblin");
        app.dispatch_command("spawn goblin");

        let gs = app.game_state().unwrap();
        assert!(gs.get_npc("Goblin #1").is_some(), "Goblin #1 should exist");
        assert!(gs.get_npc("Goblin #2").is_some(), "Goblin #2 should exist");
        assert!(gs.get_npc("Goblin #3").is_some(), "Goblin #3 should exist");
    }

    #[test]
    fn test_spawn_unknown_template_error() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_bestiary_campaign(&dir, "spawn_unknown");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("spawn dragon");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report template not found. Got: {output}");
    }

    #[test]
    fn test_spawn_no_campaign_error() {
        let mut app = App::new();
        app.messages.clear();

        app.dispatch_command("spawn goblin");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign loaded"), "Should report no campaign. Got: {output}");
    }

    #[test]
    fn test_spawn_no_args_error() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_bestiary_campaign(&dir, "spawn_noargs");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("spawn");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    // --- Encounter command tests ---

    fn setup_encounter_campaign(dir: &TempDir, name: &str) -> std::path::PathBuf {
        let campaign = dir.path().join(name);
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n\
             [character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\", \"ac\"]\n\n\
             [resources.hp]\ntype = \"gauge\"\nmax_stat = \"hp_max\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary/encounters")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength,hp_max,ac\nGoblin,8,7,15\n",
        )
        .unwrap();
        std::fs::write(
            campaign.join("bestiary/orc.csv"),
            "name,strength,hp_max,ac\nOrc,16,15,13\n",
        )
        .unwrap();
        std::fs::write(
            campaign.join("bestiary/encounters/goblin_patrol.toml"),
            "[encounter]\nname = \"Goblin Patrol\"\n\
             description = \"A small group of goblins on the road\"\n\n\
             [[creatures]]\ntemplate = \"goblin\"\ncount = 3\n\n\
             [[creatures]]\ntemplate = \"orc\"\ncount = 1\n\
             name_override = \"Orc Chieftain\"\n",
        )
        .unwrap();
        campaign
    }

    #[test]
    fn test_encounter_spawns_correct_creatures() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_encounter_campaign(&dir, "enc_spawn");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("encounter goblin patrol");

        let gs = app.game_state().unwrap();
        // Should have 3 goblins + 1 orc chieftain = 4 NPCs
        assert_eq!(gs.npcs.len(), 4, "Should spawn 4 NPCs. Got: {:?}", gs.npcs.iter().map(|n| &n.name).collect::<Vec<_>>());

        assert!(gs.get_npc("Goblin #1").is_some(), "Goblin #1 should exist");
        assert!(gs.get_npc("Goblin #2").is_some(), "Goblin #2 should exist");
        assert!(gs.get_npc("Goblin #3").is_some(), "Goblin #3 should exist");
        assert!(gs.get_npc("Orc Chieftain").is_some(), "Orc Chieftain should exist");
    }

    #[test]
    fn test_encounter_creatures_have_correct_stats() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_encounter_campaign(&dir, "enc_stats");

        let mut app = App::new();
        app.load_campaign(&campaign);

        app.dispatch_command("encounter goblin patrol");

        let gs = app.game_state().unwrap();
        let goblin = gs.get_npc("Goblin #1").unwrap();
        assert_eq!(goblin.get_stat("strength"), Some(8.0));
        assert_eq!(goblin.get_stat("hp_max"), Some(7.0));
        assert_eq!(goblin.get_stat("ac"), Some(15.0));
        assert!(goblin.gauges.contains_key("hp"), "HP gauge should be created");
        assert_eq!(goblin.gauges["hp"].max, 7.0);

        let orc = gs.get_npc("Orc Chieftain").unwrap();
        assert_eq!(orc.get_stat("strength"), Some(16.0));
        assert_eq!(orc.get_stat("hp_max"), Some(15.0));
        assert_eq!(orc.get_stat("ac"), Some(13.0));
        assert!(orc.gauges.contains_key("hp"));
        assert_eq!(orc.gauges["hp"].max, 15.0);
    }

    #[test]
    fn test_encounter_output_shows_summary() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_encounter_campaign(&dir, "enc_output");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("encounter goblin patrol");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Goblin Patrol"), "Should show encounter name. Got: {output}");
        assert!(output.contains("4 creature(s)"), "Should show count. Got: {output}");
        assert!(output.contains("A small group of goblins"), "Should show description. Got: {output}");
    }

    #[test]
    fn test_encounter_not_found_error() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_encounter_campaign(&dir, "enc_notfound");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("encounter dragon horde");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should report not found. Got: {output}");
    }

    #[test]
    fn test_encounter_no_campaign_error() {
        let mut app = App::new();
        app.messages.clear();

        app.dispatch_command("encounter goblin patrol");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No campaign loaded"), "Should report no campaign. Got: {output}");
    }

    #[test]
    fn test_encounter_no_args_error() {
        let dir = TempDir::new().unwrap();
        let campaign = setup_encounter_campaign(&dir, "enc_noargs");

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("encounter");

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_encounter_unknown_template_partial_spawn() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("enc_partial");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n\
             [character.schema]\ncolumns = [\"name\", \"strength\", \"hp_max\", \"ac\"]\n\n\
             [resources.hp]\ntype = \"gauge\"\nmax_stat = \"hp_max\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(campaign.join("bestiary/encounters")).unwrap();
        std::fs::write(
            campaign.join("bestiary/goblin.csv"),
            "name,strength,hp_max,ac\nGoblin,8,7,15\n",
        )
        .unwrap();
        // Encounter references a template that doesn't exist
        std::fs::write(
            campaign.join("bestiary/encounters/mixed.toml"),
            "[encounter]\nname = \"Mixed\"\n\n\
             [[creatures]]\ntemplate = \"goblin\"\ncount = 2\n\n\
             [[creatures]]\ntemplate = \"dragon\"\ncount = 1\n",
        )
        .unwrap();

        let mut app = App::new();
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("encounter mixed");

        let gs = app.game_state().unwrap();
        // Should still spawn the 2 goblins even though dragon template is missing
        assert_eq!(gs.npcs.len(), 2, "Should spawn goblins despite missing dragon template");
        assert!(gs.get_npc("Goblin #1").is_some());
        assert!(gs.get_npc("Goblin #2").is_some());

        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("dragon"), "Should report missing template. Got: {output}");
    }

    // ---- Mode system tests ----

    #[test]
    fn test_mode_default_prompt() {
        assert_eq!(Mode::Default.prompt(), "rustory > ");
    }

    #[test]
    fn test_mode_map_prompt() {
        assert_eq!(Mode::Map.prompt(), "rustory [map] > ");
    }

    #[test]
    fn test_mode_combat_prompt() {
        assert_eq!(Mode::Combat.prompt(), "rustory [combat] > ");
    }

    #[test]
    fn test_app_starts_in_default_mode() {
        let app = App::new();
        assert_eq!(app.mode, Mode::Default);
    }

    #[test]
    fn test_map_command_switches_to_map_mode() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());

        app.dispatch_command("map");
        assert_eq!(app.mode, Mode::Map);
    }

    #[test]
    fn test_map_command_toggles_back_to_default() {
        let mut app = App::new();
        app.running = true;
        app.world_map = Some(make_test_world_map());

        app.dispatch_command("map");
        assert_eq!(app.mode, Mode::Map);

        app.dispatch_command("map");
        assert_eq!(app.mode, Mode::Default);
    }

    #[test]
    fn test_esc_in_map_mode_returns_to_default() {
        let mut app = App::new();
        app.running = true;
        app.mode = Mode::Map;

        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Default);
        assert!(app.running, "App should still be running");
    }

    #[test]
    fn test_load_campaign_resets_mode_to_default() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("mode_reset");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        ).unwrap();

        let mut app = App::new();
        app.running = true;
        app.mode = Mode::Map;

        app.load_campaign(&campaign);
        assert_eq!(app.mode, Mode::Default, "Loading campaign should reset mode to Default");
    }

    // ---- Combat start/end tests ----

    #[test]
    fn test_combat_start_enters_combat_mode() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat start");
        assert_eq!(app.mode, Mode::Combat);
        assert!(app.initiative_tracker.is_some(), "Tracker should be created");
    }

    #[test]
    fn test_combat_start_creates_empty_tracker() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat start");
        let tracker = app.initiative_tracker.as_ref().unwrap();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_combat_end_returns_to_default() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat start");
        assert_eq!(app.mode, Mode::Combat);

        app.dispatch_command("combat end");
        assert_eq!(app.mode, Mode::Default);
        assert!(app.initiative_tracker.is_none(), "Tracker should be cleared");
    }

    #[test]
    fn test_combat_start_already_in_combat_shows_error() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("combat start");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Already in combat"), "Should show error. Got: {output}");
    }

    #[test]
    fn test_combat_end_not_in_combat_shows_error() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat end");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Not in combat"), "Should show error. Got: {output}");
    }

    #[test]
    fn test_combat_start_output_message() {
        let mut app = App::new();
        app.running = true;

        app.dispatch_command("combat start");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Combat started"), "Should show start message. Got: {output}");
    }

    // ---- Combat command tests ----

    #[test]
    fn test_init_add_combatant() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("init add Thorin 18");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Added Thorin"), "Should confirm add. Got: {output}");

        let tracker = app.initiative_tracker.as_ref().unwrap();
        assert_eq!(tracker.len(), 1);
        assert_eq!(tracker.all()[0].name, "Thorin");
        assert_eq!(tracker.all()[0].initiative, 18.0);
    }

    #[test]
    fn test_init_add_multiple_sorted() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");

        app.dispatch_command("init add Goblin 12");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Elara 15");

        let tracker = app.initiative_tracker.as_ref().unwrap();
        assert_eq!(tracker.len(), 3);
        // Sorted descending by initiative
        assert_eq!(tracker.all()[0].name, "Thorin");
        assert_eq!(tracker.all()[1].name, "Elara");
        assert_eq!(tracker.all()[2].name, "Goblin");
    }

    #[test]
    fn test_init_add_no_args_error() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("init add");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Usage"), "Should show usage. Got: {output}");
    }

    #[test]
    fn test_init_add_invalid_value_error() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("init add Thorin abc");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Invalid initiative"), "Should show error. Got: {output}");
    }

    #[test]
    fn test_init_remove_combatant() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");
        app.messages.clear();

        app.dispatch_command("init remove Goblin");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Removed Goblin"), "Should confirm removal. Got: {output}");

        let tracker = app.initiative_tracker.as_ref().unwrap();
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_init_remove_not_found() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("init remove Nobody");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should show not found. Got: {output}");
    }

    #[test]
    fn test_next_advances_turn() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");
        app.messages.clear();

        app.dispatch_command("next");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Goblin"), "Should advance to next combatant. Got: {output}");
        assert!(output.contains(">>"), "Should show turn marker. Got: {output}");
    }

    #[test]
    fn test_prev_goes_back() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");
        app.dispatch_command("next"); // Move to Goblin
        app.messages.clear();

        app.dispatch_command("prev");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Thorin"), "Should go back to Thorin. Got: {output}");
        assert!(output.contains("<<"), "Should show prev marker. Got: {output}");
    }

    #[test]
    fn test_next_no_combatants_error() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("next");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No combatants"), "Should show error. Got: {output}");
    }

    #[test]
    fn test_status_shows_initiative_order() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");
        app.messages.clear();

        app.dispatch_command("status");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Initiative Order"), "Should show header. Got: {output}");
        assert!(output.contains("Thorin"), "Should show Thorin. Got: {output}");
        assert!(output.contains("Goblin"), "Should show Goblin. Got: {output}");
        assert!(output.contains(">>"), "Should show current turn marker. Got: {output}");
    }

    #[test]
    fn test_status_empty_shows_message() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("status");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("No combatants"), "Should show empty message. Got: {output}");
    }

    #[test]
    fn test_init_roll_auto_rolls_for_characters() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("init_roll");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        ).unwrap();
        std::fs::create_dir_all(campaign.join("players/thorin")).unwrap();
        std::fs::write(campaign.join("players/thorin/sheet.csv"), "name,strength\nThorin,18\n").unwrap();
        std::fs::create_dir_all(campaign.join("npc/goblin")).unwrap();
        std::fs::write(campaign.join("npc/goblin/sheet.csv"), "name,strength\nGoblin,8\n").unwrap();

        let mut app = App::with_rng(Box::new(StdRng::seed_from_u64(42)));
        app.running = true;
        app.load_campaign(&campaign);

        app.dispatch_command("combat start");
        app.messages.clear();

        app.dispatch_command("init roll");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Initiative rolled"), "Should show roll header. Got: {output}");
        assert!(output.contains("Thorin"), "Should show Thorin's roll. Got: {output}");
        assert!(output.contains("Goblin"), "Should show Goblin's roll. Got: {output}");

        let tracker = app.initiative_tracker.as_ref().unwrap();
        assert_eq!(tracker.len(), 2, "Should have 2 combatants");
    }

    #[test]
    fn test_target_sets_target() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path().join("target_test");
        std::fs::create_dir_all(campaign.join("rules")).unwrap();
        std::fs::write(
            campaign.join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        ).unwrap();
        std::fs::create_dir_all(campaign.join("npc/goblin")).unwrap();
        std::fs::write(campaign.join("npc/goblin/sheet.csv"), "name,strength\nGoblin,8\n").unwrap();

        let mut app = App::new();
        app.running = true;
        app.load_campaign(&campaign);
        app.messages.clear();

        app.dispatch_command("target Goblin");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Target set to"), "Should confirm target. Got: {output}");
    }

    #[test]
    fn test_target_not_found_error() {
        let mut app = App::new();
        app.running = true;
        app.messages.clear();

        app.dispatch_command("target Nobody");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("not found"), "Should show not found. Got: {output}");
    }

    #[test]
    fn test_combat_commands_without_combat_mode_error() {
        let mut app = App::new();
        app.running = true;
        app.messages.clear();

        app.dispatch_command("init add Thorin 18");
        let output: String = app.messages.iter().map(|m| &m.text).cloned().collect::<Vec<_>>().join("\n");
        assert!(output.contains("Not in combat mode"), "Should show error. Got: {output}");
    }

    // ---- Combat dashboard render tests ----

    fn render_app_to_buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::render(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn buffer_content(buf: &ratatui::buffer::Buffer) -> String {
        buf.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn test_combat_dashboard_renders_non_empty() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");

        let buf = render_app_to_buffer(&app, 80, 25);
        let content = buffer_content(&buf);
        assert!(content.contains("Initiative"), "Should render Initiative panel. Content: {content}");
        assert!(content.contains("Thorin"), "Should show Thorin in dashboard");
        assert!(content.contains("Goblin"), "Should show Goblin in dashboard");
    }

    #[test]
    fn test_combat_dashboard_shows_current_turn_marker() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");

        let buf = render_app_to_buffer(&app, 80, 25);
        let content = buffer_content(&buf);
        assert!(content.contains(">>"), "Should show current turn marker. Content: {content}");
    }

    #[test]
    fn test_combat_dashboard_marker_moves_after_next() {
        let mut app = App::new();
        app.running = true;
        app.dispatch_command("combat start");
        app.dispatch_command("init add Thorin 18");
        app.dispatch_command("init add Goblin 12");

        // Initially Thorin is current (highest initiative)
        let buf1 = render_app_to_buffer(&app, 80, 25);
        let content1 = buffer_content(&buf1);
        // >> should appear before Thorin
        let marker_pos = content1.find(">>").expect("Should have >> marker");
        let thorin_pos = content1.find("Thorin").expect("Should have Thorin");
        assert!(marker_pos < thorin_pos, ">> should be before Thorin (current)");

        // After next, Goblin is current
        app.dispatch_command("next");
        let buf2 = render_app_to_buffer(&app, 80, 25);
        let content2 = buffer_content(&buf2);
        let marker_pos2 = content2.find(">>").expect("Should have >> marker after next");
        let goblin_pos = content2.find("Goblin").expect("Should have Goblin");
        assert!(marker_pos2 < goblin_pos, ">> should be before Goblin (current) after next");
    }
}
