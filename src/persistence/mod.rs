pub mod git;
pub mod writer;

use std::path::{Path, PathBuf};

use git2::Repository;

use crate::game_state::character::Character;
use crate::rules::CampaignSchema;

/// Combines the CSV writer and git layer for write-through persistence.
/// Every game state mutation writes to disk and commits.
pub struct PersistenceLayer {
    repo: Repository,
    campaign_path: PathBuf,
}

impl PersistenceLayer {
    /// Initialize persistence for a campaign. Creates or opens the git repo.
    pub fn init(campaign_path: &Path) -> color_eyre::Result<Self> {
        let repo = git::init_repo(campaign_path)?;
        Ok(Self {
            repo,
            campaign_path: campaign_path.to_path_buf(),
        })
    }

    /// Open persistence for an existing campaign (repo must already exist).
    pub fn open(campaign_path: &Path) -> color_eyre::Result<Self> {
        let repo = Repository::open(campaign_path)?;
        Ok(Self {
            repo,
            campaign_path: campaign_path.to_path_buf(),
        })
    }

    /// Write a character's sheet.csv to disk and commit with a description.
    pub fn persist_character(
        &self,
        character: &Character,
        is_player: bool,
        schema: &CampaignSchema,
        description: &str,
    ) -> color_eyre::Result<()> {
        let folder = self.character_folder(character, is_player);
        std::fs::create_dir_all(&folder)?;
        let sheet_path = folder.join("sheet.csv");

        writer::write_character_sheet(character, &sheet_path, &schema.character_schema)?;
        git::commit(&self.repo, description)?;

        Ok(())
    }

    /// Write a character's inventory.csv to disk and commit.
    pub fn persist_inventory(
        &self,
        character: &Character,
        is_player: bool,
        schema: &CampaignSchema,
        description: &str,
    ) -> color_eyre::Result<()> {
        let folder = self.character_folder(character, is_player);
        std::fs::create_dir_all(&folder)?;
        let inv_path = folder.join("inventory.csv");

        writer::write_inventory(character, &inv_path, &schema.inventory_schema)?;
        git::commit(&self.repo, description)?;

        Ok(())
    }

    /// Check for uncommitted changes and commit them as manual edits.
    pub fn commit_manual_edits(&self) -> color_eyre::Result<bool> {
        git::commit_manual_edits(&self.repo)
    }

    /// Check if there are uncommitted changes.
    pub fn has_uncommitted_changes(&self) -> color_eyre::Result<bool> {
        git::has_uncommitted_changes(&self.repo)
    }

    /// Revert the last commit. Returns the reverted commit hash.
    pub fn undo(&self) -> color_eyre::Result<String> {
        git::revert_last(&self.repo)
    }

    /// Re-apply a previously reverted commit.
    pub fn redo(&self, commit_hash: &str) -> color_eyre::Result<()> {
        git::reapply(&self.repo, commit_hash)
    }

    /// Commit all current changes with the given message.
    pub fn commit(&self, message: &str) -> color_eyre::Result<()> {
        git::commit(&self.repo, message)
    }

    /// Get the last n commits from the log.
    pub fn history(&self, n: usize) -> color_eyre::Result<Vec<git::CommitInfo>> {
        git::log(&self.repo, n)
    }

    /// Get the folder path for a character based on their name and type.
    fn character_folder(&self, character: &Character, is_player: bool) -> PathBuf {
        let base = if is_player {
            self.campaign_path.join("players")
        } else {
            self.campaign_path.join("npc")
        };
        // Use lowercase name with spaces replaced by underscores
        let folder_name = character.name.to_lowercase().replace(' ', "_");
        base.join(folder_name)
    }
}

// Allow Debug for App (PersistenceLayer contains Repository which isn't Debug)
impl std::fmt::Debug for PersistenceLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistenceLayer")
            .field("campaign_path", &self.campaign_path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::Stat;
    use crate::schema::csv_schema::{ColumnDef, CsvSchema};
    use tempfile::TempDir;

    fn test_schema() -> CampaignSchema {
        CampaignSchema {
            character_schema: CsvSchema::new(vec![
                ColumnDef::required_string("name"),
                ColumnDef::required_number("strength"),
                ColumnDef::required_number("hp_max"),
            ]),
            inventory_schema: CsvSchema::new(vec![
                ColumnDef::required_string("item"),
                ColumnDef::required_number("quantity"),
            ]),
        }
    }

    fn setup_campaign() -> (TempDir, PersistenceLayer) {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("rules")).unwrap();
        std::fs::write(
            dir.path().join("rules/system.toml"),
            "[system]\nname = \"Test\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("players/thorin")).unwrap();
        std::fs::write(
            dir.path().join("players/thorin/sheet.csv"),
            "name,strength,hp_max\nThorin,18,52\n",
        )
        .unwrap();

        let pl = PersistenceLayer::init(dir.path()).unwrap();
        (dir, pl)
    }

    #[test]
    fn test_persist_character_writes_csv() {
        let (dir, pl) = setup_campaign();
        let schema = test_schema();

        let mut ch = Character::new("Thorin");
        ch.stats = vec![Stat::new("strength", 18.0), Stat::new("hp_max", 52.0)];

        pl.persist_character(&ch, true, &schema, "Updated Thorin")
            .unwrap();

        // Verify CSV file was written
        let csv = std::fs::read_to_string(dir.path().join("players/thorin/sheet.csv")).unwrap();
        assert!(csv.contains("Thorin"), "CSV should contain character name");
        assert!(csv.contains("18"), "CSV should contain strength value");
    }

    #[test]
    fn test_persist_character_creates_git_commit() {
        let (dir, pl) = setup_campaign();
        let schema = test_schema();

        let mut ch = Character::new("Thorin");
        ch.stats = vec![Stat::new("strength", 18.0), Stat::new("hp_max", 52.0)];

        pl.persist_character(&ch, true, &schema, "Updated Thorin")
            .unwrap();

        // Verify git commit was created
        let repo = Repository::open(dir.path()).unwrap();
        let entries = git::log(&repo, 10).unwrap();
        // Should have: "Updated Thorin" + "Initial state"
        assert!(entries.len() >= 2);
        assert_eq!(entries[0].message, "Updated Thorin");
    }

    #[test]
    fn test_damage_character_persists_change() {
        let (dir, pl) = setup_campaign();
        let schema = test_schema();

        // Simulate damage: modify the character, then persist
        let mut ch = Character::new("Thorin");
        ch.stats = vec![Stat::new("strength", 18.0), Stat::new("hp_max", 37.0)]; // HP reduced

        pl.persist_character(&ch, true, &schema, "Thorin takes 15 damage (HP: 52 -> 37)")
            .unwrap();

        // Verify CSV reflects the change
        let csv = std::fs::read_to_string(dir.path().join("players/thorin/sheet.csv")).unwrap();
        assert!(csv.contains("37"), "HP should be 37 after damage");

        // Verify commit message
        let repo = Repository::open(dir.path()).unwrap();
        let entries = git::log(&repo, 1).unwrap();
        assert!(entries[0].message.contains("damage"));
    }

    #[test]
    fn test_persist_npc_creates_in_npc_folder() {
        let (dir, pl) = setup_campaign();
        let schema = test_schema();

        let mut ch = Character::new("Goblin");
        ch.stats = vec![Stat::new("strength", 8.0), Stat::new("hp_max", 7.0)];

        pl.persist_character(&ch, false, &schema, "Spawned Goblin")
            .unwrap();

        // Should be in npc/ folder
        assert!(dir.path().join("npc/goblin/sheet.csv").exists());
    }

    #[test]
    fn test_character_folder_name_conversion() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("dummy"), "").unwrap();
        let pl = PersistenceLayer::init(dir.path()).unwrap();

        let ch = Character::new("Goblin King");
        let folder = pl.character_folder(&ch, false);
        assert!(folder.ends_with("npc/goblin_king"));
    }

    #[test]
    fn test_undo_reverts_change() {
        let (dir, pl) = setup_campaign();
        let schema = test_schema();

        // Write original state
        let mut ch = Character::new("Thorin");
        ch.stats = vec![Stat::new("strength", 18.0), Stat::new("hp_max", 52.0)];
        pl.persist_character(&ch, true, &schema, "Set HP to 52")
            .unwrap();

        // Write modified state
        ch.stats = vec![Stat::new("strength", 18.0), Stat::new("hp_max", 37.0)];
        pl.persist_character(&ch, true, &schema, "Damage HP to 37")
            .unwrap();

        // Undo
        let _hash = pl.undo().unwrap();

        // CSV should be back to 52
        let csv = std::fs::read_to_string(dir.path().join("players/thorin/sheet.csv")).unwrap();
        assert!(csv.contains("52"), "HP should be reverted to 52: {csv}");
    }
}
