use std::collections::HashMap;

use crate::app::App;

/// A function that returns argument completions for a command.
/// - `app`: reference to the App (provides access to GameState, WorldMap, SoundLibrary, etc.)
/// - `arg_index`: 0-based index of the argument being completed (0 = first arg after command)
/// - `partial`: what the user has typed so far for this argument
///
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
    registry.insert("ls", complete_ls as ArgCompleter);
    registry.insert("sound", complete_sound as ArgCompleter);
    registry.insert("map", complete_map as ArgCompleter);
    registry.insert("encounter", complete_encounter as ArgCompleter);
    registry.insert("combat", complete_combat as ArgCompleter);
    registry.insert("init", complete_init as ArgCompleter);
    registry.insert("note", complete_note as ArgCompleter);

    registry
}

/// Complete from a static list of subcommands, matching by prefix (case-insensitive).
fn complete_static(subcommands: &[&str], partial: &str) -> Vec<String> {
    let lower = partial.to_lowercase();
    subcommands
        .iter()
        .filter(|s| s.starts_with(&lower))
        .map(|s| s.to_string())
        .collect()
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

/// Static subcommand lists for commands that take subcommands.
/// Used for arg_index 0 completion (the first argument after the command name).
const LS_SUBCOMMANDS: &[&str] = &["commands", "encounters", "npc", "players", "sound"];
const SOUND_SUBCOMMANDS: &[&str] = &[
    "list", "loop", "pause", "play", "resume", "search", "status", "stop", "volume",
];
const MAP_SUBCOMMANDS: &[&str] = &["info", "list", "move", "near", "route", "search", "where"];
const ENCOUNTER_SUBCOMMANDS: &[&str] = &["ls", "roll", "show"];
const COMBAT_SUBCOMMANDS: &[&str] = &["end", "start"];
const NOTE_SUBCOMMANDS: &[&str] = &["history", "list"];
const INIT_SUBCOMMANDS: &[&str] = &["add", "remove", "roll"];

/// Static list of `map list` subcommands.
const MAP_LIST_SUBCOMMANDS: &[&str] = &["burgs", "cultures", "states"];

/// Completer for `ls` subcommands.
fn complete_ls(_app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        complete_static(LS_SUBCOMMANDS, partial)
    } else {
        Vec::new()
    }
}

/// Completer for `combat` subcommands.
fn complete_combat(_app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        complete_static(COMBAT_SUBCOMMANDS, partial)
    } else {
        Vec::new()
    }
}

/// Completer for `note` subcommands.
fn complete_note(_app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        complete_static(NOTE_SUBCOMMANDS, partial)
    } else {
        Vec::new()
    }
}

/// Completer for `init` subcommands.
fn complete_init(_app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        complete_static(INIT_SUBCOMMANDS, partial)
    } else {
        Vec::new()
    }
}

/// Completer for `map` subcommands.
/// - arg_index 0: complete subcommand name (info, list, move, near, route, search, where)
/// - `map where <character>`: character names
/// - `map move <character> <location>`: character names then burg names
/// - `map info/search/near <burg>`: burg names
/// - `map route <from> <to>`: burg names for both args
/// - `map list [burgs/states/cultures]`: static subcommand list
fn complete_map(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        return complete_static(MAP_SUBCOMMANDS, partial);
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
                complete_burg_names(app, partial)
            } else {
                Vec::new()
            }
        }
        Some("info") | Some("search") | Some("near") => {
            // map info/search/near <burg> — arg_index 1 = burg name
            if arg_index == 1 {
                complete_burg_names(app, partial)
            } else {
                Vec::new()
            }
        }
        Some("route") => {
            // map route <from> <to> — arg_index 1 and 2 are both burg names
            if arg_index == 1 || arg_index == 2 {
                complete_burg_names(app, partial)
            } else {
                Vec::new()
            }
        }
        Some("list") => {
            // map list [burgs/states/cultures] — static subcommand list
            if arg_index == 1 {
                let lower_partial = partial.to_lowercase();
                MAP_LIST_SUBCOMMANDS
                    .iter()
                    .filter(|s| s.starts_with(&lower_partial))
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Completer for `sound` subcommands.
/// - arg_index 0: complete subcommand name (list, loop, pause, play, resume, search, status, stop, volume)
/// - `sound play <path>` / `sound loop <path>`: complete from SoundLibrary entries (relative paths).
/// - `sound list <subfolder>`: complete subfolder names.
fn complete_sound(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        return complete_static(SOUND_SUBCOMMANDS, partial);
    }

    // Determine subcommand, accounting for aliases (e.g., "play" → "sound play")
    let input = &app.input;
    let parts: Vec<&str> = input.split_whitespace().collect();
    let first_word = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();

    let resolved = crate::commands::mapping::resolve_alias(&first_word);
    let resolved_parts: Vec<&str> = resolved.split_whitespace().collect();

    let subcmd = if resolved_parts.len() > 1 {
        // Multi-word alias like "play" → "sound play"
        Some(resolved_parts[1].to_string())
    } else {
        // Direct "sound" command, subcommand is the next word from input
        parts.get(1).map(|s| s.to_lowercase())
    };

    match subcmd.as_deref() {
        Some("play") | Some("loop") => {
            if arg_index == 1 {
                app.sound_library.complete_paths(partial)
            } else {
                Vec::new()
            }
        }
        Some("list") => {
            if arg_index == 1 {
                app.sound_library.complete_subfolders(partial)
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Completer for `encounter` subcommands.
/// - arg_index 0: complete subcommand name (ls, roll, show)
/// - `encounter show <zone>` / `encounter roll <zone>`: complete zone names from loaded encounter tables.
fn complete_encounter(app: &App, arg_index: usize, partial: &str) -> Vec<String> {
    if arg_index == 0 {
        return complete_static(ENCOUNTER_SUBCOMMANDS, partial);
    }

    // Determine subcommand from input
    let input = &app.input;
    let parts: Vec<&str> = input.split_whitespace().collect();
    // parts[0] = "encounter" (or alias), parts[1] = subcommand, parts[2+] = args
    let subcmd = parts.get(1).map(|s| s.to_lowercase());

    match subcmd.as_deref() {
        Some("show") | Some("roll") => {
            if arg_index == 1 {
                complete_zone_names(app, partial)
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Returns zone names from encounter tables matching the partial prefix (case-insensitive).
fn complete_zone_names(app: &App, partial: &str) -> Vec<String> {
    let gs = match &app.game_state {
        Some(gs) => gs,
        None => return Vec::new(),
    };
    let lower_partial = partial.to_lowercase();
    gs.encounter_tables
        .keys()
        .filter(|name| name.starts_with(&lower_partial))
        .cloned()
        .collect()
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
    fn test_map_arg_index_0_completes_subcommands() {
        let app = App::new();
        let results = complete_map(&app, 0, "wh");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "where");
    }

    /// Create an App with a WorldMap containing test burgs.
    fn app_with_world_map() -> App {
        let json = r##"{
            "info": {"width": 800, "height": 600},
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Thornwall", "x": 100.0, "y": 100.0, "population": 28.5, "state": 1, "culture": 1, "type": "City", "capital": 1},
                    {"i": 2, "name": "Silverport", "x": 200.0, "y": 100.0, "population": 12.0, "state": 1, "culture": 1, "type": "Town"},
                    {"i": 3, "name": "Thornvale", "x": 120.0, "y": 110.0, "population": 3.0, "state": 1}
                ],
                "states": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Kingdom of Light", "form": "Monarchy"}
                ],
                "cultures": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Elven", "type": "Lake"}
                ],
                "routes": []
            }
        }"##;
        let world_map = crate::map::world::WorldMap::from_parsed(
            crate::map::azgaar::parse_azgaar_json(json).unwrap(),
        );
        let mut app = App::new();
        app.world_map = Some(world_map);
        app
    }

    #[test]
    fn test_map_info_completes_burg_names() {
        let mut app = app_with_world_map();
        app.input = "map info thorn".to_string();
        let results = complete_map(&app, 1, "thorn");
        assert!(results.contains(&"thornwall".to_string()));
        assert!(results.contains(&"thornvale".to_string()));
        assert!(!results.contains(&"silverport".to_string()));
    }

    #[test]
    fn test_map_search_completes_burg_names() {
        let mut app = app_with_world_map();
        app.input = "map search silver".to_string();
        let results = complete_map(&app, 1, "silver");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "silverport");
    }

    #[test]
    fn test_map_near_completes_burg_names() {
        let mut app = app_with_world_map();
        app.input = "map near thorn".to_string();
        let results = complete_map(&app, 1, "thorn");
        assert!(results.contains(&"thornwall".to_string()));
        assert!(results.contains(&"thornvale".to_string()));
    }

    #[test]
    fn test_map_route_completes_burg_names_arg1() {
        let mut app = app_with_world_map();
        app.input = "map route thorn".to_string();
        let results = complete_map(&app, 1, "thorn");
        assert!(results.contains(&"thornwall".to_string()));
    }

    #[test]
    fn test_map_route_completes_burg_names_arg2() {
        let mut app = app_with_world_map();
        app.input = "map route thornwall silver".to_string();
        let results = complete_map(&app, 2, "silver");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "silverport");
    }

    #[test]
    fn test_map_list_completes_subcommands() {
        let mut app = app_with_world_map();
        app.input = "map list b".to_string();
        let results = complete_map(&app, 1, "b");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "burgs");
    }

    #[test]
    fn test_map_list_completes_all_subcommands_on_empty() {
        let mut app = app_with_world_map();
        app.input = "map list ".to_string();
        let results = complete_map(&app, 1, "");
        assert_eq!(results.len(), 3);
        assert!(results.contains(&"burgs".to_string()));
        assert!(results.contains(&"states".to_string()));
        assert!(results.contains(&"cultures".to_string()));
    }

    #[test]
    fn test_map_list_no_match_returns_empty() {
        let mut app = app_with_world_map();
        app.input = "map list th".to_string();
        let results = complete_map(&app, 1, "th");
        assert!(results.is_empty());
    }

    #[test]
    fn test_map_info_no_world_map_returns_empty() {
        let mut app = App::new();
        app.input = "map info thorn".to_string();
        let results = complete_map(&app, 1, "thorn");
        assert!(results.is_empty());
    }

    #[test]
    fn test_map_info_case_insensitive() {
        let mut app = app_with_world_map();
        app.input = "map info THORN".to_string();
        let results = complete_map(&app, 1, "THORN");
        assert!(results.contains(&"thornwall".to_string()));
        assert!(results.contains(&"thornvale".to_string()));
    }

    // --- Sound completion tests ---

    /// Create an App with a SoundLibrary from a temp directory.
    fn app_with_sound_library() -> (App, tempfile::TempDir) {
        use std::fs;
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("ambiance")).unwrap();
        fs::create_dir_all(root.join("combat")).unwrap();
        fs::write(root.join("ambiance/tavern.mp3"), b"fake").unwrap();
        fs::write(root.join("ambiance/forest.ogg"), b"fake").unwrap();
        fs::write(root.join("combat/battle.wav"), b"fake").unwrap();
        fs::write(root.join("theme.flac"), b"fake").unwrap();

        let mut app = App::new();
        app.sound_library = crate::audio::library::SoundLibrary::scan(root).unwrap();
        (app, dir)
    }

    #[test]
    fn test_sound_play_completes_paths() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound play ambiance/".to_string();
        let results = complete_sound(&app, 1, "ambiance/");
        assert!(results.contains(&"ambiance/tavern.mp3".to_string()));
        assert!(results.contains(&"ambiance/forest.ogg".to_string()));
    }

    #[test]
    fn test_sound_play_completes_partial_path() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound play ambiance/t".to_string();
        let results = complete_sound(&app, 1, "ambiance/t");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "ambiance/tavern.mp3");
    }

    #[test]
    fn test_sound_loop_completes_paths() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound loop combat/".to_string();
        let results = complete_sound(&app, 1, "combat/");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "combat/battle.wav");
    }

    #[test]
    fn test_sound_list_completes_subfolders() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound list a".to_string();
        let results = complete_sound(&app, 1, "a");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "ambiance");
    }

    #[test]
    fn test_sound_list_empty_partial_returns_all_dirs() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound list ".to_string();
        let results = complete_sound(&app, 1, "");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"ambiance".to_string()));
        assert!(results.contains(&"combat".to_string()));
    }

    #[test]
    fn test_sound_play_via_alias_completes_paths() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "play ambiance/".to_string();
        let results = complete_sound(&app, 1, "ambiance/");
        assert!(results.contains(&"ambiance/tavern.mp3".to_string()));
    }

    #[test]
    fn test_sound_arg_index_0_completes_subcommands() {
        let (app, _dir) = app_with_sound_library();
        let results = complete_sound(&app, 0, "p");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"pause".to_string()));
        assert!(results.contains(&"play".to_string()));
    }

    #[test]
    fn test_sound_unknown_subcommand_returns_empty() {
        let (mut app, _dir) = app_with_sound_library();
        app.input = "sound stop ".to_string();
        let results = complete_sound(&app, 1, "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_sound_no_library_returns_empty() {
        let mut app = App::new();
        app.input = "sound play amb".to_string();
        let results = complete_sound(&app, 1, "amb");
        assert!(results.is_empty());
    }

    // --- Encounter completion tests ---

    /// Create an App with encounter tables containing test zones.
    fn app_with_encounters() -> App {
        use crate::encounters::EncounterTable;

        let mut app = App::new();
        let mut gs = GameState::new(Path::new("/tmp/test_campaign"));

        gs.encounter_tables.insert(
            "forest".to_string(),
            EncounterTable {
                zone_name: "forest".to_string(),
                description: "A dark forest".to_string(),
                entries: vec![],
            },
        );
        gs.encounter_tables.insert(
            "dungeon".to_string(),
            EncounterTable {
                zone_name: "dungeon".to_string(),
                description: "A deep dungeon".to_string(),
                entries: vec![],
            },
        );
        gs.encounter_tables.insert(
            "fortress".to_string(),
            EncounterTable {
                zone_name: "fortress".to_string(),
                description: "An ancient fortress".to_string(),
                entries: vec![],
            },
        );

        app.game_state = Some(gs);
        app
    }

    #[test]
    fn test_encounter_show_completes_zone_names() {
        let mut app = app_with_encounters();
        app.input = "encounter show for".to_string();
        let results = complete_encounter(&app, 1, "for");
        assert!(results.contains(&"forest".to_string()));
        assert!(results.contains(&"fortress".to_string()));
        assert!(!results.contains(&"dungeon".to_string()));
    }

    #[test]
    fn test_encounter_roll_completes_zone_names() {
        let mut app = app_with_encounters();
        app.input = "encounter roll for".to_string();
        let results = complete_encounter(&app, 1, "for");
        assert!(results.contains(&"forest".to_string()));
        assert!(results.contains(&"fortress".to_string()));
    }

    #[test]
    fn test_encounter_roll_case_insensitive() {
        let mut app = app_with_encounters();
        app.input = "encounter roll FOR".to_string();
        let results = complete_encounter(&app, 1, "FOR");
        assert!(results.contains(&"forest".to_string()));
        assert!(results.contains(&"fortress".to_string()));
    }

    #[test]
    fn test_encounter_show_empty_partial_returns_all() {
        let mut app = app_with_encounters();
        app.input = "encounter show ".to_string();
        let results = complete_encounter(&app, 1, "");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_encounter_show_no_match_returns_empty() {
        let mut app = app_with_encounters();
        app.input = "encounter show zzz".to_string();
        let results = complete_encounter(&app, 1, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_encounter_arg_index_0_completes_subcommands() {
        let app = App::new();
        let results = complete_encounter(&app, 0, "sh");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "show");
    }

    #[test]
    fn test_encounter_ls_returns_empty() {
        let mut app = app_with_encounters();
        app.input = "encounter ls ".to_string();
        let results = complete_encounter(&app, 1, "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_encounter_no_game_state_returns_empty() {
        let mut app = App::new();
        app.input = "encounter roll for".to_string();
        let results = complete_encounter(&app, 1, "for");
        assert!(results.is_empty());
    }

    #[test]
    fn test_encounter_no_tables_returns_empty() {
        let mut app = App::new();
        let gs = GameState::new(Path::new("/tmp/test_campaign"));
        app.game_state = Some(gs);
        app.input = "encounter show for".to_string();
        let results = complete_encounter(&app, 1, "for");
        assert!(results.is_empty());
    }

    // --- Subcommand completion tests (Phase 25.4) ---

    #[test]
    fn test_sound_subcommand_play_hint() {
        let app = App::new();
        let results = complete_sound(&app, 0, "p");
        assert!(results.contains(&"play".to_string()));
        assert!(results.contains(&"pause".to_string()));
        assert!(!results.contains(&"stop".to_string()));
    }

    #[test]
    fn test_sound_subcommand_all() {
        let app = App::new();
        let results = complete_sound(&app, 0, "");
        assert_eq!(results.len(), 9);
    }

    #[test]
    fn test_sound_subcommand_s_matches_two() {
        let app = App::new();
        let results = complete_sound(&app, 0, "s");
        assert_eq!(results.len(), 3);
        assert!(results.contains(&"search".to_string()));
        assert!(results.contains(&"status".to_string()));
        assert!(results.contains(&"stop".to_string()));
    }

    #[test]
    fn test_map_subcommand_list_hint() {
        let app = App::new();
        let results = complete_map(&app, 0, "l");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "list");
    }

    #[test]
    fn test_map_subcommand_all() {
        let app = App::new();
        let results = complete_map(&app, 0, "");
        assert_eq!(results.len(), 7);
    }

    #[test]
    fn test_map_subcommand_no_match() {
        let app = App::new();
        let results = complete_map(&app, 0, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_encounter_subcommand_all() {
        let app = App::new();
        let results = complete_encounter(&app, 0, "");
        assert_eq!(results.len(), 3);
        assert!(results.contains(&"ls".to_string()));
        assert!(results.contains(&"roll".to_string()));
        assert!(results.contains(&"show".to_string()));
    }

    #[test]
    fn test_encounter_subcommand_r() {
        let app = App::new();
        let results = complete_encounter(&app, 0, "r");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "roll");
    }

    #[test]
    fn test_ls_subcommand_all() {
        let app = App::new();
        let results = complete_ls(&app, 0, "");
        assert_eq!(results.len(), 5);
        assert!(results.contains(&"players".to_string()));
        assert!(results.contains(&"npc".to_string()));
        assert!(results.contains(&"sound".to_string()));
        assert!(results.contains(&"encounters".to_string()));
        assert!(results.contains(&"commands".to_string()));
    }

    #[test]
    fn test_ls_subcommand_p() {
        let app = App::new();
        let results = complete_ls(&app, 0, "p");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "players");
    }

    #[test]
    fn test_ls_subcommand_arg1_returns_empty() {
        let app = App::new();
        let results = complete_ls(&app, 1, "p");
        assert!(results.is_empty());
    }

    #[test]
    fn test_combat_subcommand_all() {
        let app = App::new();
        let results = complete_combat(&app, 0, "");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"start".to_string()));
        assert!(results.contains(&"end".to_string()));
    }

    #[test]
    fn test_combat_subcommand_s() {
        let app = App::new();
        let results = complete_combat(&app, 0, "s");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "start");
    }

    #[test]
    fn test_combat_subcommand_arg1_returns_empty() {
        let app = App::new();
        let results = complete_combat(&app, 1, "s");
        assert!(results.is_empty());
    }

    #[test]
    fn test_note_subcommand_all() {
        let app = App::new();
        let results = complete_note(&app, 0, "");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"list".to_string()));
        assert!(results.contains(&"history".to_string()));
    }

    #[test]
    fn test_note_subcommand_l() {
        let app = App::new();
        let results = complete_note(&app, 0, "l");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "list");
    }

    #[test]
    fn test_note_subcommand_arg1_returns_empty() {
        let app = App::new();
        let results = complete_note(&app, 1, "l");
        assert!(results.is_empty());
    }

    #[test]
    fn test_init_subcommand_all() {
        let app = App::new();
        let results = complete_init(&app, 0, "");
        assert_eq!(results.len(), 3);
        assert!(results.contains(&"add".to_string()));
        assert!(results.contains(&"remove".to_string()));
        assert!(results.contains(&"roll".to_string()));
    }

    #[test]
    fn test_init_subcommand_r() {
        let app = App::new();
        let results = complete_init(&app, 0, "r");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"remove".to_string()));
        assert!(results.contains(&"roll".to_string()));
    }

    #[test]
    fn test_init_subcommand_arg1_returns_empty() {
        let app = App::new();
        let results = complete_init(&app, 1, "a");
        assert!(results.is_empty());
    }

    #[test]
    fn test_subcommand_completion_case_insensitive() {
        let app = App::new();
        let results = complete_sound(&app, 0, "P");
        assert!(results.contains(&"play".to_string()));
        assert!(results.contains(&"pause".to_string()));
    }

    #[test]
    fn test_static_subcommand_lists_are_sorted() {
        // Verify all constant lists are sorted for consistent completion order
        assert!(LS_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(SOUND_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(MAP_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(ENCOUNTER_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(COMBAT_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(NOTE_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
        assert!(INIT_SUBCOMMANDS.windows(2).all(|w| w[0] <= w[1]));
    }
}
