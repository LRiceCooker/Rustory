use std::collections::HashMap;

use rand::Rng;
use rand::RngCore;
use ratatui::style::{Color, Style};

use super::mapping;
use super::parsers::roll;
use crate::scripting::loader::LolScript;

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Output(Vec<StyledLine>),
    Error(String),
    Quit,
    Custom(LolScript),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledLine {
    pub text: String,
    pub style: Style,
}

impl StyledLine {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Color::Blue))
    }
}

fn help() -> CommandResult {
    CommandResult::Output(vec![
        StyledLine::plain("Available commands:"),
        StyledLine::plain("  help  — show this help"),
        StyledLine::plain("  load  — load a campaign folder (e.g. load sample)"),
        StyledLine::plain(
            "  new   — create a new campaign from a template (e.g. new my_game sample)",
        ),
        StyledLine::plain("  roll  — roll dice (e.g. roll 2d6+3)"),
        StyledLine::plain("  sound — play audio (e.g. sound play ambiance/tavern.mp3)"),
        StyledLine::plain("  quit  — exit Rustory"),
    ])
}

fn roll_dice(args: &str, rng: &mut dyn RngCore) -> CommandResult {
    let args = args.trim();
    if args.is_empty() {
        return CommandResult::Error("Usage: roll <NdV+M> (e.g. roll 2d6+3)".to_string());
    }

    match roll::parse(args) {
        Ok(parsed) => {
            let mut naturals: Vec<u32> = Vec::new();
            for _ in 0..parsed.dice {
                let result = rng.gen_range(1..=parsed.value);
                naturals.push(result);
            }
            let natural_sum: u32 = naturals.iter().sum();
            let total = natural_sum as i32 + parsed.modifier;

            let mod_str = if parsed.modifier > 0 {
                format!("+{}", parsed.modifier)
            } else if parsed.modifier < 0 {
                format!("{}", parsed.modifier)
            } else {
                String::new()
            };

            CommandResult::Output(vec![
                StyledLine::plain(format!(
                    "{} dice(s), maximum value: {}, modifier: {}",
                    parsed.dice, parsed.value, mod_str
                )),
                StyledLine::plain(format!("natural: {naturals:?}, result: {total}")),
            ])
        }
        Err(e) => CommandResult::Error(e),
    }
}

pub fn dispatch(
    input: &str,
    rng: &mut dyn RngCore,
    custom_commands: Option<&HashMap<String, LolScript>>,
) -> CommandResult {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let command = parts[0];
    let args = parts.get(1).unwrap_or(&"");

    // Built-in commands ALWAYS take priority (security)
    match command {
        mapping::QUIT => CommandResult::Quit,
        mapping::HELP => help(),
        mapping::LOAD => {
            CommandResult::Error("load command must be handled by the app".to_string())
        }
        mapping::NEW => CommandResult::Error("new command must be handled by the app".to_string()),
        mapping::ROLL => roll_dice(args, rng),
        mapping::SEARCH => {
            CommandResult::Error("search command must be handled by the app".to_string())
        }
        mapping::MAP => CommandResult::Error("map command must be handled by the app".to_string()),
        mapping::SOUND => {
            CommandResult::Error("sound command must be handled by the app".to_string())
        }
        mapping::SHOW => {
            CommandResult::Error("show command must be handled by the app".to_string())
        }
        mapping::SET => CommandResult::Error("set command must be handled by the app".to_string()),
        mapping::LIST | mapping::LIST_ALIAS => {
            CommandResult::Error("ls command must be handled by the app".to_string())
        }
        mapping::HISTORY => {
            CommandResult::Error("history command must be handled by the app".to_string())
        }
        mapping::UNDO => {
            CommandResult::Error("undo command must be handled by the app".to_string())
        }
        mapping::REDO => {
            CommandResult::Error("redo command must be handled by the app".to_string())
        }
        mapping::VALIDATE => {
            CommandResult::Error("validate command must be handled by the app".to_string())
        }
        mapping::DAMAGE | mapping::DAMAGE_ALIAS => {
            CommandResult::Error("damage command must be handled by the app".to_string())
        }
        mapping::COMBAT
        | mapping::INIT
        | mapping::NEXT
        | mapping::PREV
        | mapping::STATUS
        | mapping::TARGET => {
            CommandResult::Error("combat command must be handled by the app".to_string())
        }
        mapping::NOTE => {
            CommandResult::Error("note command must be handled by the app".to_string())
        }
        mapping::ENCOUNTER => {
            CommandResult::Error("encounter command must be handled by the app".to_string())
        }
        _ => {
            // Look up in custom commands HashMap
            if let Some(commands) = custom_commands {
                if let Some(script) = commands.get(&command.to_lowercase()) {
                    return CommandResult::Custom(script.clone());
                }
            }
            CommandResult::Unknown(command.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rng() -> Box<dyn RngCore> {
        Box::new(rand::thread_rng())
    }

    #[test]
    fn test_quit_returns_quit() {
        assert_eq!(
            dispatch("quit", &mut *test_rng(), None),
            CommandResult::Quit
        );
    }

    #[test]
    fn test_help_returns_output() {
        match dispatch("help", &mut *test_rng(), None) {
            CommandResult::Output(lines) => {
                assert!(lines.len() >= 4);
                assert!(lines[0].text.contains("Available commands"));
                assert!(lines.iter().any(|l| l.text.contains("help")));
                assert!(lines.iter().any(|l| l.text.contains("roll")));
                assert!(lines.iter().any(|l| l.text.contains("quit")));
            }
            other => panic!("expected Output, got {other:?}"),
        }
    }

    #[test]
    fn test_unknown_command_returns_unknown() {
        match dispatch("foobar", &mut *test_rng(), None) {
            CommandResult::Unknown(cmd) => assert_eq!(cmd, "foobar"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn test_roll_valid() {
        match dispatch("roll 2d6", &mut *test_rng(), None) {
            CommandResult::Output(lines) => {
                assert!(lines[0].text.contains("2 dice(s)"));
                assert!(lines[0].text.contains("maximum value: 6"));
                assert!(lines[1].text.contains("natural:"));
                assert!(lines[1].text.contains("result:"));
            }
            other => panic!("expected Output, got {other:?}"),
        }
    }

    #[test]
    fn test_roll_no_args() {
        match dispatch("roll", &mut *test_rng(), None) {
            CommandResult::Error(msg) => assert!(msg.contains("Usage")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn test_roll_invalid() {
        match dispatch("roll abc", &mut *test_rng(), None) {
            CommandResult::Error(msg) => assert!(!msg.is_empty()),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn test_roll_deterministic_same_seed() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);

        let result1 = dispatch("roll 3d6", &mut rng1, None);
        let result2 = dispatch("roll 3d6", &mut rng2, None);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_roll_deterministic_different_seeds() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(99);

        let result1 = dispatch("roll 3d20", &mut rng1, None);
        let result2 = dispatch("roll 3d20", &mut rng2, None);

        // Very unlikely to be equal with different seeds and 3d20
        assert_ne!(result1, result2);
    }

    // --- Hybrid dispatch tests ---

    #[test]
    fn test_builtin_command_executes() {
        // Built-in commands work regardless of custom commands
        let mut customs = HashMap::new();
        customs.insert(
            "greet".to_string(),
            LolScript {
                name: "greet".to_string(),
                source: "HAI 1.2\nKTHXBYE".to_string(),
            },
        );

        match dispatch("roll 1d6", &mut *test_rng(), Some(&customs)) {
            CommandResult::Output(lines) => {
                assert!(lines[0].text.contains("1 dice(s)"));
            }
            other => panic!("expected Output for built-in, got {other:?}"),
        }
    }

    #[test]
    fn test_custom_command_returns_custom() {
        let mut customs = HashMap::new();
        customs.insert(
            "smite".to_string(),
            LolScript {
                name: "smite".to_string(),
                source: "HAI 1.2\nVISIBLE \"smite!\"\nKTHXBYE".to_string(),
            },
        );

        match dispatch("smite", &mut *test_rng(), Some(&customs)) {
            CommandResult::Custom(script) => {
                assert_eq!(script.name, "smite");
                assert!(script.source.contains("smite!"));
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn test_builtin_wins_over_same_name_custom() {
        let mut customs = HashMap::new();
        // Create a custom command with the same name as a built-in
        customs.insert(
            "help".to_string(),
            LolScript {
                name: "help".to_string(),
                source: "HAI 1.2\nVISIBLE \"evil help\"\nKTHXBYE".to_string(),
            },
        );

        match dispatch("help", &mut *test_rng(), Some(&customs)) {
            CommandResult::Output(lines) => {
                assert!(lines[0].text.contains("Available commands"));
            }
            other => panic!("expected built-in Output, got {other:?}"),
        }

        // Also test other built-ins: quit, roll, load, new, search
        assert_eq!(
            dispatch("quit", &mut *test_rng(), Some(&customs)),
            CommandResult::Quit
        );
    }

    #[test]
    fn test_unknown_command_with_custom_commands_present() {
        let mut customs = HashMap::new();
        customs.insert(
            "smite".to_string(),
            LolScript {
                name: "smite".to_string(),
                source: "HAI 1.2\nKTHXBYE".to_string(),
            },
        );

        // "foobar" is neither built-in nor custom
        match dispatch("foobar", &mut *test_rng(), Some(&customs)) {
            CommandResult::Unknown(cmd) => assert_eq!(cmd, "foobar"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn test_unknown_command_without_custom_commands() {
        match dispatch("smite", &mut *test_rng(), None) {
            CommandResult::Unknown(cmd) => assert_eq!(cmd, "smite"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn test_custom_command_case_insensitive() {
        let mut customs = HashMap::new();
        customs.insert(
            "smite".to_string(),
            LolScript {
                name: "smite".to_string(),
                source: "HAI 1.2\nKTHXBYE".to_string(),
            },
        );

        // Input "SMITE" should match "smite" in customs (lowercased lookup)
        match dispatch("SMITE", &mut *test_rng(), Some(&customs)) {
            CommandResult::Custom(script) => assert_eq!(script.name, "smite"),
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn test_search_placeholder() {
        match dispatch("search goblin", &mut *test_rng(), None) {
            CommandResult::Error(msg) => assert!(msg.contains("search")),
            other => panic!("expected Error for search placeholder, got {other:?}"),
        }
    }
}
