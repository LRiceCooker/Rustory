pub mod character;
pub mod loader;
pub mod primitives;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::rules::{self, CampaignRules, CampaignSchema};
use crate::scripting::loader::LolScript;
pub use character::Character;

#[derive(Debug)]
pub struct GameState {
    pub campaign_name: String,
    pub campaign_path: PathBuf,
    pub players: Vec<Character>,
    pub npcs: Vec<Character>,
    pub rules: Option<CampaignRules>,
    pub schema: Option<CampaignSchema>,
    pub custom_commands: HashMap<String, LolScript>,
}

impl GameState {
    pub fn new(campaign_path: &Path) -> Self {
        let campaign_name = campaign_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unnamed".to_string());

        Self {
            campaign_name,
            campaign_path: campaign_path.to_path_buf(),
            players: Vec::new(),
            npcs: Vec::new(),
            rules: None,
            schema: None,
            custom_commands: HashMap::new(),
        }
    }

    /// Load a campaign from a directory path.
    /// Reads `rules/system.toml` if present, then loads players and NPCs.
    /// Returns a list of load errors (empty if everything loaded successfully).
    pub fn load(path: &Path) -> (Self, Vec<loader::LoadError>) {
        let mut gs = Self::new(path);
        let mut all_errors = Vec::new();

        // Load rules from system.toml
        let system_toml = path.join("rules").join("system.toml");
        let expected_columns: Vec<String>;

        if system_toml.exists() {
            match rules::loader::load_rules(&system_toml) {
                Ok((campaign_rules, campaign_schema)) => {
                    expected_columns = campaign_schema
                        .character_schema
                        .column_names()
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    gs.rules = Some(campaign_rules);
                    gs.schema = Some(campaign_schema);
                }
                Err(rule_errors) => {
                    for re in rule_errors {
                        all_errors.push(loader::LoadError {
                            file: re.file,
                            message: re.message,
                            suggestion: re.suggestion,
                        });
                    }
                    expected_columns = Vec::new();
                }
            }
        } else {
            expected_columns = Vec::new();
        }

        let expected_refs: Vec<&str> = expected_columns.iter().map(|s| s.as_str()).collect();

        // Load players
        let players_dir = path.join("players");
        let player_result = loader::load_characters_from_dir(&players_dir, &expected_refs);
        all_errors.extend(player_result.errors);
        for player in player_result.characters {
            gs.add_player(player);
        }

        // Load NPCs
        let npc_dir = path.join("npc");
        let npc_result = loader::load_characters_from_dir(&npc_dir, &expected_refs);
        all_errors.extend(npc_result.errors);
        for npc in npc_result.characters {
            gs.add_npc(npc);
        }

        // Load custom commands from rules/commands/*.lol
        gs.custom_commands = crate::scripting::loader::load_custom_commands(path);

        (gs, all_errors)
    }

    pub fn add_player(&mut self, character: Character) {
        self.players.push(character);
    }

    pub fn add_npc(&mut self, character: Character) {
        self.npcs.push(character);
    }

    pub fn get_player(&self, name: &str) -> Option<&Character> {
        self.players.iter().find(|c| c.name == name)
    }

    pub fn get_npc(&self, name: &str) -> Option<&Character> {
        self.npcs.iter().find(|c| c.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::ResolutionMode;

    #[test]
    fn test_new_game_state_empty() {
        let gs = GameState::new(Path::new("/tmp/my_campaign"));
        assert_eq!(gs.campaign_name, "my_campaign");
        assert_eq!(gs.campaign_path, Path::new("/tmp/my_campaign"));
        assert!(gs.players.is_empty());
        assert!(gs.npcs.is_empty());
    }

    #[test]
    fn test_campaign_name_from_path() {
        let gs = GameState::new(Path::new("/home/user/campaigns/dragon_quest"));
        assert_eq!(gs.campaign_name, "dragon_quest");
    }

    #[test]
    fn test_campaign_name_root_path() {
        let gs = GameState::new(Path::new("/"));
        assert_eq!(gs.campaign_name, "Unnamed");
    }

    #[test]
    fn test_add_player() {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        let thorin = Character::new("Thorin")
            .with_stat("strength", 18.0)
            .with_stat("dexterity", 12.0);
        gs.add_player(thorin);

        assert_eq!(gs.players.len(), 1);
        assert_eq!(gs.players[0].name, "Thorin");
        assert_eq!(gs.players[0].get_stat("strength"), Some(18.0));
        assert_eq!(gs.players[0].get_stat("dexterity"), Some(12.0));
    }

    #[test]
    fn test_add_npc() {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        let goblin = Character::new("Goblin King")
            .with_stat("ac", 15.0)
            .with_gauge("hp", 45.0);
        gs.add_npc(goblin);

        assert_eq!(gs.npcs.len(), 1);
        assert_eq!(gs.npcs[0].name, "Goblin King");
        assert_eq!(gs.npcs[0].get_gauge("hp").unwrap().current, 45.0);
    }

    #[test]
    fn test_add_multiple_players_and_npcs() {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        gs.add_player(Character::new("Thorin"));
        gs.add_player(Character::new("Elara"));
        gs.add_npc(Character::new("Goblin"));
        gs.add_npc(Character::new("Orc"));
        gs.add_npc(Character::new("Dragon"));

        assert_eq!(gs.players.len(), 2);
        assert_eq!(gs.npcs.len(), 3);
    }

    #[test]
    fn test_get_player() {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        gs.add_player(Character::new("Thorin").with_stat("strength", 18.0));
        gs.add_player(Character::new("Elara").with_stat("intelligence", 20.0));

        let thorin = gs.get_player("Thorin");
        assert!(thorin.is_some());
        assert_eq!(thorin.unwrap().get_stat("strength"), Some(18.0));

        let elara = gs.get_player("Elara");
        assert!(elara.is_some());
        assert_eq!(elara.unwrap().get_stat("intelligence"), Some(20.0));

        assert!(gs.get_player("Nobody").is_none());
    }

    #[test]
    fn test_get_npc() {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        gs.add_npc(Character::new("Goblin King").with_gauge("hp", 45.0));

        assert!(gs.get_npc("Goblin King").is_some());
        assert!(gs.get_npc("Ghost").is_none());
    }

    #[test]
    fn test_character_new() {
        let c = Character::new("Test");
        assert_eq!(c.name, "Test");
        assert!(c.stats.is_empty());
    }

    #[test]
    fn test_character_with_stat_chaining() {
        let c = Character::new("Hero")
            .with_stat("str", 10.0)
            .with_stat("dex", 14.0)
            .with_stat("con", 12.0);
        assert_eq!(c.stats.len(), 3);
        assert_eq!(c.get_stat("str"), Some(10.0));
        assert_eq!(c.get_stat("dex"), Some(14.0));
        assert_eq!(c.get_stat("con"), Some(12.0));
    }

    // --- GameState::load integration tests ---

    #[test]
    fn test_load_sample_campaign_rules_accessible() {
        let (gs, errors) = GameState::load(Path::new("sample"));
        assert!(errors.is_empty(), "Load errors: {errors:?}");

        // Rules should be loaded
        let rules = gs.rules.as_ref().expect("rules should be loaded");
        assert_eq!(rules.system_name, "D&D 5e");
        assert_eq!(rules.system_version.as_deref(), Some("1.0"));

        // Stat names
        assert_eq!(rules.stat_names.len(), 6);
        assert!(rules.stat_names.contains(&"strength".to_string()));
        assert!(rules.stat_names.contains(&"charisma".to_string()));

        // Derived values
        assert_eq!(rules.derived.len(), 2);
        assert!(rules.derived.iter().any(|d| d.name == "ac"));
        assert!(rules.derived.iter().any(|d| d.name == "initiative"));

        // Checks
        assert_eq!(rules.checks.len(), 2);
        let ability_check = rules
            .checks
            .iter()
            .find(|c| c.name == "ability_check")
            .expect("ability_check should exist");
        assert_eq!(ability_check.resolution_mode, ResolutionMode::RollOver);
        assert!(rules.checks.iter().any(|c| c.name == "saving_throw"));

        // Resources
        assert_eq!(rules.resource_defs.len(), 2);
    }

    #[test]
    fn test_load_sample_campaign_schema_accessible() {
        let (gs, errors) = GameState::load(Path::new("sample"));
        assert!(errors.is_empty(), "Load errors: {errors:?}");

        let schema = gs.schema.as_ref().expect("schema should be loaded");

        // Character schema columns
        let col_names: Vec<&str> = schema.character_schema.column_names();
        assert_eq!(col_names.len(), 11);
        assert_eq!(col_names[0], "name");
        assert!(col_names.contains(&"strength"));
        assert!(col_names.contains(&"ac"));

        // Inventory schema columns
        let inv_names: Vec<&str> = schema.inventory_schema.column_names();
        assert_eq!(inv_names, vec!["item", "quantity", "weight", "notes"]);
    }

    #[test]
    fn test_load_sample_campaign_characters_loaded() {
        let (gs, errors) = GameState::load(Path::new("sample"));
        assert!(errors.is_empty(), "Load errors: {errors:?}");

        // Player: Thorin
        assert_eq!(gs.players.len(), 1);
        let thorin = gs.get_player("Thorin").expect("Thorin should be loaded");
        assert_eq!(thorin.get_stat("strength"), Some(18.0));
        assert_eq!(thorin.get_stat("hp_max"), Some(52.0));
        assert_eq!(thorin.get_stat("ac"), Some(18.0));

        // NPC: Goblin King
        assert_eq!(gs.npcs.len(), 1);
        let gk = gs
            .get_npc("Goblin King")
            .expect("Goblin King should be loaded");
        assert_eq!(gk.get_stat("strength"), Some(16.0));
        assert_eq!(gk.get_stat("hp_max"), Some(45.0));
        assert!(gk.lore.is_some());
    }

    #[test]
    fn test_load_empty_dir_no_rules() {
        let dir = tempfile::TempDir::new().unwrap();
        let (gs, errors) = GameState::load(dir.path());

        assert!(errors.is_empty());
        assert!(gs.rules.is_none());
        assert!(gs.schema.is_none());
        assert!(gs.players.is_empty());
        assert!(gs.npcs.is_empty());
    }

    #[test]
    fn test_load_with_rules_validates_characters() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules")).unwrap();
        std::fs::write(
            dir.path().join("rules/system.toml"),
            "[system]\nname = \"Test\"\n\n[character.schema]\ncolumns = [\"name\", \"strength\"]\n",
        )
        .unwrap();

        let player_dir = dir.path().join("players/hero");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(player_dir.join("sheet.csv"), "name,strength\nHero,15\n").unwrap();

        let (gs, errors) = GameState::load(dir.path());
        assert!(errors.is_empty());
        assert!(gs.rules.is_some());
        assert_eq!(gs.rules.as_ref().unwrap().system_name, "Test");
        assert_eq!(gs.players.len(), 1);
        assert_eq!(gs.players[0].get_stat("strength"), Some(15.0));
    }
}
