use std::collections::HashMap;

use crate::app::App;

/// A function that returns argument completions for a command.
/// - `app`: reference to the App (provides access to GameState, WorldMap, SoundLibrary, etc.)
/// - `arg_index`: 0-based index of the argument being completed (0 = first arg after command)
/// - `partial`: what the user has typed so far for this argument
/// Returns a list of possible completions (full argument values, not suffixes).
pub type ArgCompleter = fn(app: &App, arg_index: usize, partial: &str) -> Vec<String>;

/// Build the autocomplete registry mapping command names to their argument completers.
/// This is called once at startup and the result doesn't change.
pub fn build_registry() -> HashMap<&'static str, ArgCompleter> {
    let mut registry: HashMap<&'static str, ArgCompleter> = HashMap::new();

    // Commands that take character names
    registry.insert("show", complete_character_name as ArgCompleter);
    registry.insert("set", complete_set as ArgCompleter);
    registry.insert("spawn", complete_spawn as ArgCompleter);
    registry.insert("damage", complete_character_name as ArgCompleter);
    registry.insert("heal", complete_character_name as ArgCompleter);
    registry.insert("target", complete_character_name as ArgCompleter);
    registry.insert("give", complete_character_name as ArgCompleter);

    // Commands with subcommands
    registry.insert("ls", complete_stub as ArgCompleter);
    registry.insert("sound", complete_stub as ArgCompleter);
    registry.insert("map", complete_map as ArgCompleter);
    registry.insert("encounter", complete_stub as ArgCompleter);
    registry.insert("combat", complete_stub as ArgCompleter);
    registry.insert("init", complete_stub as ArgCompleter);
    registry.insert("note", complete_stub as ArgCompleter);

    registry
}

/// Stub completer — returns no completions. Will be replaced by real completers in Phase 25.3+.
fn complete_stub(_app: &App, _arg_index: usize, _partial: &str) -> Vec<String> {
    Vec::new()
}

/// Returns all player + NPC names from GameState matching the partial prefix (case-insensitive).
/// Names are returned in lowercase for consistent matching with the autocomplete hint system.
fn get_character_names(app: &App, partial: &str) -> Vec<String> {
    let gs = match &app.game_state {
        Some(gs) => gs,
        None => return Vec::new(),
    };
    let lower_partial = partial.to_lowercase();
    gs.players
        .iter()
        .chain(gs.npcs.iter())
        .map(|c| c.name.to_lowercase())
        .filter(|name| name.starts_with(&lower_partial))
        .collect()
}

/// Completer for commands that take a character name at arg_index 0:
/// show, damage, heal, target, give.
fn complete_character_name(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        get_character_names(app, partial)
    } else {
        Vec::new()
    }
}

/// Completer for `set <character>.<field> <value>`.
/// If partial contains a dot, completes field names for the character before the dot.
/// Otherwise, completes character names (appending a dot hint).
fn complete_set(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index != 0 {
        return Vec::new();
    }

    let gs = match &app.game_state {
        Some(gs) => gs,
        None => return Vec::new(),
    };

    if let Some(dot_pos) = partial.find('.') {
        // After the dot: complete field names for the matched character
        let char_part = &partial[..dot_pos];
        let field_part = &partial[dot_pos + 1..];
        let char_lower = char_part.to_lowercase();
        let field_lower = field_part.to_lowercase();

        // Find the character
        let character = gs
            .players
            .iter()
            .chain(gs.npcs.iter())
            .find(|c| c.name.to_lowercase() == char_lower);

        if let Some(ch) = character {
            // Collect all completable field names: stats + gauges + pools
            let fields: Vec<String> = ch
                .stats
                .iter()
                .map(|s| s.name.clone())
                .chain(ch.gauges.keys().cloned())
                .chain(ch.pools.keys().cloned())
                .collect();

            fields
                .into_iter()
                .filter(|f| f.to_lowercase().starts_with(&field_lower))
                .map(|f| format!("{char_part}.{f}"))
                .collect()
        } else {
            Vec::new()
        }
    } else {
        // Before the dot: complete character names
        get_character_names(app, partial)
    }
}

/// Completer for `spawn <npc_folder> [name]`.
/// Completes NPC folder names from the campaign's npc/ directory.
fn complete_spawn(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index != 0 {
        return Vec::new();
    }

    let gs = match &app.game_state {
        Some(gs) => gs,
        None => return Vec::new(),
    };

    let npc_dir = gs.campaign_path.join("npc");
    let lower_partial = partial.to_lowercase();

    let entries = match std::fs::read_dir(&npc_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let dir_name = path.file_name()?.to_str()?.to_string();
            // Skip encounters directory
            if dir_name == "encounters" {
                return None;
            }
            // Only include folders that have a sheet.csv (valid NPC templates)
            if !path.join("sheet.csv").exists() {
                return None;
            }
            if dir_name.to_lowercase().starts_with(&lower_partial) {
                Some(dir_name)
            } else {
                None
            }
        })
        .collect()
}

/// Completer for `map` subcommands.
/// For `map where <character>` and `map move <character> <location>`, completes character names.
fn complete_map(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    // arg_index 0 = subcommand (where, move, info, etc.) — handled in Phase 25.4
    // arg_index 1+ depends on subcommand
    if arg_index == 0 {
        return Vec::new();
    }

    // We need to figure out which subcommand is being used.
    // The full input is parsed by autocomplete_hint: arg_tokens[0] is the subcommand.
    // However, we only receive arg_index and partial here.
    // We need to check the input to determine the subcommand.
    let input = &app.input;
    let parts: Vec<&str> = input.split_whitespace().collect();
    // parts[0] = "map" (or alias), parts[1] = subcommand, parts[2+] = args
    let subcmd = parts.get(1).map(|s| s.to_lowercase());

    match subcmd.as_deref() {
        Some("where") => {
            // map where <character> — arg_index 1 = character name
            if arg_index == 1 {
                get_character_names(app, partial)
            } else {
                Vec::new()
            }
        }
        Some("move") | Some("mv") => {
            // map move <character> <location> — arg_index 1 = character, arg_index 2 = burg
            if arg_index == 1 {
                get_character_names(app, partial)
            } else if arg_index == 2 {
                // Complete burg names from WorldMap
                complete_burg_names(app, partial)
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Returns burg names from WorldMap matching the partial prefix (case-insensitive).
fn complete_burg_names(app: &App, partial: &str) -> Vec<String> {
    let world_map = match &app.world_map {
        Some(wm) => wm,
        None => return Vec::new(),
    };
    let lower_partial = partial.to_lowercase();
    world_map
        .map
        .pack
        .burgs
        .iter()
        .skip(1) // skip sentinel at index 0
        .filter(|b| !b.name.is_empty())
        .map(|b| b.name.to_lowercase())
        .filter(|name| name.starts_with(&lower_partial))
        .collect()
}

/// All command names that have argument-level autocomplete registered.
/// Used by tests to verify coverage.
pub const COMMANDS_WITH_ARG_COMPLETION: &[&str] = &[
    "combat",
    "damage",
    "encounter",
    "give",
    "heal",
    "init",
    "ls",
    "map",
    "note",
    "set",
    "show",
    "sound",
    "spawn",
    "target",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::character::Character;
    use crate::game_state::GameState;
    use std::path::Path;

    /// Create an App with a GameState containing test characters.
    fn app_with_characters() -> App {
        let mut app = App::new();
        let mut gs = GameState::new(Path::new("/tmp/test_campaign"));
        gs.add_player(
            Character::new("Thorin")
                .with_stat("strength", 18.0)
                .with_stat("dexterity", 14.0)
                .with_gauge("hp", 52.0),
        );
        gs.add_player(Character::new("Elara").with_stat("intelligence", 20.0));
        gs.add_npc(
            Character::new("Goblin King")
                .with_stat("strength", 16.0)
                .with_gauge("hp", 45.0),
        );
        gs.add_npc(Character::new("Thranduil").with_stat("charisma", 22.0));
        app.game_state = Some(gs);
        app
    }

    #[test]
    fn test_registry_has_entries_for_all_expected_commands() {
        let registry = build_registry();
        for &cmd in COMMANDS_WITH_ARG_COMPLETION {
            assert!(
                registry.contains_key(cmd),
                "Registry missing entry for command: {cmd}"
            );
        }
    }

    #[test]
    fn test_registry_size_matches_expected() {
        let registry = build_registry();
        assert_eq!(registry.len(), COMMANDS_WITH_ARG_COMPLETION.len());
    }

    #[test]
    fn test_registry_does_not_contain_commands_without_args() {
        let registry = build_registry();
        assert!(!registry.contains_key("help"));
        assert!(!registry.contains_key("quit"));
        assert!(!registry.contains_key("clear"));
        assert!(!registry.contains_key("who"));
        assert!(!registry.contains_key("where"));
        assert!(!registry.contains_key("undo"));
        assert!(!registry.contains_key("redo"));
        assert!(!registry.contains_key("next"));
        assert!(!registry.contains_key("prev"));
        assert!(!registry.contains_key("status"));
    }

    #[test]
    fn test_commands_with_arg_completion_is_sorted() {
        let mut sorted = COMMANDS_WITH_ARG_COMPLETION.to_vec();
        sorted.sort();
        assert_eq!(COMMANDS_WITH_ARG_COMPLETION, sorted.as_slice());
    }

    // --- Character name completion tests ---

    #[test]
    fn test_show_completes_character_names() {
        let app = app_with_characters();
        let results = complete_character_name(&app, 0, "th");
        assert!(results.contains(&"thorin".to_string()));
        assert!(results.contains(&"thranduil".to_string()));
        assert!(!results.contains(&"elara".to_string()));
    }

    #[test]
    fn test_show_completes_case_insensitive() {
        let app = app_with_characters();
        let results = complete_character_name(&app, 0, "TH");
        assert!(results.contains(&"thorin".to_string()));
        assert!(results.contains(&"thranduil".to_string()));
    }

    #[test]
    fn test_show_completes_empty_partial_returns_all() {
        let app = app_with_characters();
        let results = complete_character_name(&app, 0, "");
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_show_no_match_returns_empty() {
        let app = app_with_characters();
        let results = complete_character_name(&app, 0, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_show_arg_index_1_returns_empty() {
        let app = app_with_characters();
        let results = complete_character_name(&app, 1, "th");
        assert!(results.is_empty());
    }

    #[test]
    fn test_show_no_game_state_returns_empty() {
        let app = App::new();
        let results = complete_character_name(&app, 0, "th");
        assert!(results.is_empty());
    }

    // --- Set command completion tests ---

    #[test]
    fn test_set_completes_character_names_without_dot() {
        let app = app_with_characters();
        let results = complete_set(&app, 0, "th");
        assert!(results.contains(&"thorin".to_string()));
        assert!(results.contains(&"thranduil".to_string()));
    }

    #[test]
    fn test_set_completes_field_names_after_dot() {
        let app = app_with_characters();
        // "thorin." should complete all fields for Thorin
        let results = complete_set(&app, 0, "thorin.");
        assert!(results.contains(&"thorin.strength".to_string()));
        assert!(results.contains(&"thorin.dexterity".to_string()));
        assert!(results.contains(&"thorin.hp".to_string()));
    }

    #[test]
    fn test_set_completes_field_names_with_partial() {
        let app = app_with_characters();
        // "thorin.s" should match "strength"
        let results = complete_set(&app, 0, "thorin.s");
        assert!(results.contains(&"thorin.strength".to_string()));
        assert!(!results.contains(&"thorin.dexterity".to_string()));
    }

    #[test]
    fn test_set_field_completion_case_insensitive() {
        let app = app_with_characters();
        let results = complete_set(&app, 0, "Thorin.S");
        assert!(results.contains(&"Thorin.strength".to_string()));
    }

    #[test]
    fn test_set_unknown_character_returns_empty() {
        let app = app_with_characters();
        let results = complete_set(&app, 0, "nobody.");
        assert!(results.is_empty());
    }

    #[test]
    fn test_set_arg_index_1_returns_empty() {
        let app = app_with_characters();
        let results = complete_set(&app, 1, "thorin");
        assert!(results.is_empty());
    }

    // --- Spawn completion tests ---

    #[test]
    fn test_spawn_completes_npc_folders() {
        let mut app = App::new();
        let gs = GameState::new(Path::new("sample"));
        app.game_state = Some(gs);
        let results = complete_spawn(&app, 0, "goblin");
        // Should find goblin_king folder (and spawned copies)
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.starts_with("goblin_king")));
    }

    #[test]
    fn test_spawn_excludes_encounters_dir() {
        let mut app = App::new();
        let gs = GameState::new(Path::new("sample"));
        app.game_state = Some(gs);
        let results = complete_spawn(&app, 0, "enc");
        assert!(results.is_empty());
    }

    #[test]
    fn test_spawn_no_game_state_returns_empty() {
        let app = App::new();
        let results = complete_spawn(&app, 0, "goblin");
        assert!(results.is_empty());
    }

    // --- Map completion tests ---

    #[test]
    fn test_map_where_completes_character_names() {
        let mut app = app_with_characters();
        app.input = "map where th".to_string();
        let results = complete_map(&app, 1, "th");
        assert!(results.contains(&"thorin".to_string()));
        assert!(results.contains(&"thranduil".to_string()));
    }

    #[test]
    fn test_map_move_completes_character_names() {
        let mut app = app_with_characters();
        app.input = "map move th".to_string();
        let results = complete_map(&app, 1, "th");
        assert!(results.contains(&"thorin".to_string()));
    }

    #[test]
    fn test_map_unknown_subcommand_returns_empty() {
        let mut app = app_with_characters();
        app.input = "map list th".to_string();
        let results = complete_map(&app, 1, "th");
        assert!(results.is_empty());
    }

    #[test]
    fn test_map_arg_index_0_returns_empty() {
        let mut app = app_with_characters();
        app.input = "map wh".to_string();
        let results = complete_map(&app, 0, "wh");
        assert!(results.is_empty());
    }
}
