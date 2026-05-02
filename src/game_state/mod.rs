pub mod character;
pub mod loader;
pub mod primitives;

use std::path::{Path, PathBuf};

use crate::rules::{CampaignRules, CampaignSchema};
pub use character::Character;

#[derive(Debug)]
pub struct GameState {
    pub campaign_name: String,
    pub campaign_path: PathBuf,
    pub players: Vec<Character>,
    pub npcs: Vec<Character>,
    pub rules: Option<CampaignRules>,
    pub schema: Option<CampaignSchema>,
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
        }
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
}
