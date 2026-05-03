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
    registry.insert("show", complete_stub as ArgCompleter);
    registry.insert("set", complete_stub as ArgCompleter);
    registry.insert("spawn", complete_stub as ArgCompleter);
    registry.insert("damage", complete_stub as ArgCompleter);
    registry.insert("heal", complete_stub as ArgCompleter);
    registry.insert("target", complete_stub as ArgCompleter);
    registry.insert("give", complete_stub as ArgCompleter);

    // Commands with subcommands
    registry.insert("ls", complete_stub as ArgCompleter);
    registry.insert("sound", complete_stub as ArgCompleter);
    registry.insert("map", complete_stub as ArgCompleter);
    registry.insert("encounter", complete_stub as ArgCompleter);
    registry.insert("combat", complete_stub as ArgCompleter);
    registry.insert("init", complete_stub as ArgCompleter);
    registry.insert("note", complete_stub as ArgCompleter);

    registry
}

/// Stub completer — returns no completions. Will be replaced by real completers in Phase 25.2+.
fn complete_stub(_app: &App, _arg_index: usize, _partial: &str) -> Vec<String> {
    Vec::new()
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
        // These commands don't take meaningful arguments to complete
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
    fn test_stub_completer_returns_empty() {
        let app = App::new();
        let registry = build_registry();
        for completer in registry.values() {
            let results = completer(&app, 0, "foo");
            assert!(results.is_empty());
        }
    }

    #[test]
    fn test_commands_with_arg_completion_is_sorted() {
        let mut sorted = COMMANDS_WITH_ARG_COMPLETION.to_vec();
        sorted.sort();
        assert_eq!(COMMANDS_WITH_ARG_COMPLETION, sorted.as_slice());
    }
}
