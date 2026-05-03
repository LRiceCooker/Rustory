pub const CALC: &str = "calc";
pub const CAT: &str = "cat";
pub const CLEAR: &str = "clear";
pub const COMBAT: &str = "combat";
pub const DAMAGE: &str = "damage";
pub const ENCOUNTER: &str = "encounter";
pub const GIVE: &str = "give";
pub const HEAL: &str = "heal";
pub const HELP: &str = "help";
pub const INIT: &str = "init";
pub const LOAD: &str = "load";
pub const MAP: &str = "map";
pub const NEW: &str = "new";
pub const NEXT: &str = "next";
pub const NOTE: &str = "note";
pub const PREV: &str = "prev";
pub const QUIT: &str = "quit";
pub const ROLL: &str = "roll";
pub const SEARCH: &str = "search";
pub const STATUS: &str = "status";
pub const TARGET: &str = "target";
pub const HISTORY: &str = "history";
pub const LIST: &str = "ls";
pub const REDO: &str = "redo";
pub const SET: &str = "set";
pub const SHOW: &str = "show";
pub const SOUND: &str = "sound";
pub const SPAWN: &str = "spawn";
pub const UNDO: &str = "undo";
pub const VALIDATE: &str = "validate";
pub const WHERE: &str = "where";
pub const WHO: &str = "who";

/// Short aliases for rapid gameplay. Maps alias → canonical command.
/// Multi-word expansions (e.g. "mv" → "map move") prepend the expansion to any trailing args.
/// These are NOT shown in help but work in autocomplete.
const ALIASES: &[(&str, &str)] = &[
    ("r", "roll"),
    ("s", "show"),
    ("h", "help"),
    ("q", "quit"),
    ("n", "next"),
    ("p", "prev"),
    ("st", "status"),
    ("sp", "spawn"),
    ("enc", "encounter"),
    ("dmg", "damage"),
    ("hp", "heal"),
    ("mv", "map move"),
    ("play", "sound play"),
    ("w", "who"),
    ("wh", "where"),
    ("u", "undo"),
    ("re", "redo"),
    ("v", "validate"),
    ("hist", "history"),
    ("list", "ls"),
];

/// All strings that can start a command — canonical commands + alias keys.
/// Used for autocomplete (Tab hint matching).
pub const ALL_COMPLETABLE: &[&str] = &[
    "calc", "cat", "clear", "combat", "damage", "dmg", "enc", "encounter", "give", "h", "heal",
    "help", "hist", "history", "hp", "init", "list", "load", "ls", "map", "mv", "n", "new",
    "next", "note", "p", "play", "prev", "q", "quit", "r", "re", "redo", "roll", "s", "search",
    "set", "show", "sound", "sp", "spawn", "st", "status", "target", "u", "undo", "v", "validate",
    "w", "wh", "where", "who",
];

/// Resolve a short alias to its canonical command expansion.
/// Returns the expanded input string if an alias matched, or the original input unchanged.
pub fn resolve_alias(input: &str) -> String {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let first = parts[0];
    let rest = parts.get(1).unwrap_or(&"");

    for &(alias, expansion) in ALIASES {
        if first == alias {
            if rest.is_empty() {
                return expansion.to_string();
            } else {
                return format!("{expansion} {rest}");
            }
        }
    }

    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_resolves_single_word() {
        assert_eq!(resolve_alias("r 2d6+3"), "roll 2d6+3");
        assert_eq!(resolve_alias("h"), "help");
        assert_eq!(resolve_alias("q"), "quit");
        assert_eq!(resolve_alias("s thorin"), "show thorin");
    }

    #[test]
    fn test_alias_resolves_multi_word_expansion() {
        assert_eq!(resolve_alias("mv Thorin Silverport"), "map move Thorin Silverport");
        assert_eq!(resolve_alias("play tavern"), "sound play tavern");
        assert_eq!(resolve_alias("mv"), "map move");
    }

    #[test]
    fn test_alias_all_entries() {
        assert_eq!(resolve_alias("r"), "roll");
        assert_eq!(resolve_alias("s"), "show");
        assert_eq!(resolve_alias("h"), "help");
        assert_eq!(resolve_alias("q"), "quit");
        assert_eq!(resolve_alias("n"), "next");
        assert_eq!(resolve_alias("p"), "prev");
        assert_eq!(resolve_alias("st"), "status");
        assert_eq!(resolve_alias("sp"), "spawn");
        assert_eq!(resolve_alias("enc"), "encounter");
        assert_eq!(resolve_alias("dmg"), "damage");
        assert_eq!(resolve_alias("hp"), "heal");
        assert_eq!(resolve_alias("mv"), "map move");
        assert_eq!(resolve_alias("play"), "sound play");
        assert_eq!(resolve_alias("w"), "who");
        assert_eq!(resolve_alias("wh"), "where");
        assert_eq!(resolve_alias("u"), "undo");
        assert_eq!(resolve_alias("re"), "redo");
        assert_eq!(resolve_alias("v"), "validate");
        assert_eq!(resolve_alias("hist"), "history");
        assert_eq!(resolve_alias("list"), "ls");
    }

    #[test]
    fn test_canonical_commands_pass_through() {
        assert_eq!(resolve_alias("roll 2d6"), "roll 2d6");
        assert_eq!(resolve_alias("help"), "help");
        assert_eq!(resolve_alias("show thorin"), "show thorin");
        assert_eq!(resolve_alias("map move Thorin Silverport"), "map move Thorin Silverport");
    }

    #[test]
    fn test_unknown_input_passes_through() {
        assert_eq!(resolve_alias("foobar"), "foobar");
        assert_eq!(resolve_alias("smite goblin"), "smite goblin");
    }

    #[test]
    fn test_alias_with_args_preserved() {
        assert_eq!(resolve_alias("enc show forest"), "encounter show forest");
        assert_eq!(resolve_alias("dmg thorin 15"), "damage thorin 15");
        assert_eq!(resolve_alias("hp thorin 10"), "heal thorin 10");
        assert_eq!(resolve_alias("hist 5"), "history 5");
    }

    #[test]
    fn test_all_completable_is_sorted() {
        let mut sorted = ALL_COMPLETABLE.to_vec();
        sorted.sort();
        assert_eq!(ALL_COMPLETABLE, sorted.as_slice());
    }
}
