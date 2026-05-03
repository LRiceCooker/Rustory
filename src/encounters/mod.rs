use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rand::Rng;
use rand::RngCore;
use serde::Deserialize;

/// A single encounter table representing a zone with weighted entries.
#[derive(Debug, Clone)]
pub struct EncounterTable {
    pub zone_name: String,
    pub description: String,
    pub entries: Vec<EncounterEntry>,
}

/// An entry in an encounter table.
#[derive(Debug, Clone)]
pub struct EncounterEntry {
    pub name: String,
    pub description: String,
    pub weight: u32,
    pub npcs: Vec<String>,
}

// --- TOML deserialization structures ---

#[derive(Deserialize)]
struct EncounterToml {
    zone: ZoneToml,
    #[serde(default)]
    entries: Vec<EntryToml>,
}

#[derive(Deserialize)]
struct ZoneToml {
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct EntryToml {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_weight")]
    weight: u32,
    #[serde(default)]
    npcs: Vec<String>,
}

fn default_weight() -> u32 {
    1
}

impl EncounterTable {
    /// Total weight of all entries.
    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    /// Roll a weighted random entry from the encounter table.
    /// Returns None if the table is empty or all weights are zero.
    pub fn roll(&self, rng: &mut dyn RngCore) -> Option<&EncounterEntry> {
        let total = self.total_weight();
        if total == 0 {
            return None;
        }
        let mut roll = rng.gen_range(0..total);
        for entry in &self.entries {
            if roll < entry.weight {
                return Some(entry);
            }
            roll -= entry.weight;
        }
        // Fallback (shouldn't happen with correct math)
        self.entries.last()
    }
}

/// Load all encounter tables from a directory (npc/encounters/*.toml).
/// Returns a map from zone filename (without .toml) to EncounterTable.
pub fn load_encounters(dir: &Path) -> HashMap<String, EncounterTable> {
    let mut tables = HashMap::new();

    if !dir.exists() || !dir.is_dir() {
        return tables;
    }

    let mut toml_files: Vec<_> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("toml"))
            .collect(),
        Err(_) => return tables,
    };
    toml_files.sort_by_key(|e| e.file_name());

    for entry in toml_files {
        let path = entry.path();
        let zone_key = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if zone_key.is_empty() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(parsed) = toml::from_str::<EncounterToml>(&content) {
                let table = EncounterTable {
                    zone_name: parsed.zone.name,
                    description: parsed.zone.description,
                    entries: parsed
                        .entries
                        .into_iter()
                        .map(|e| EncounterEntry {
                            name: e.name,
                            description: e.description,
                            weight: e.weight,
                            npcs: e.npcs,
                        })
                        .collect(),
                };
                tables.insert(zone_key, table);
            }
        }
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use tempfile::TempDir;

    fn sample_toml() -> &'static str {
        r#"[zone]
name = "Dark Forest"
description = "A dense, dangerous woodland"

[[entries]]
name = "Goblin Patrol"
description = "Three goblins on patrol"
weight = 30
npcs = ["goblin_king", "goblin_king", "goblin_king"]

[[entries]]
name = "Traveling Merchant"
description = "A friendly trader"
weight = 15
npcs = ["shopkeeper"]

[[entries]]
name = "Nothing happens"
weight = 35
npcs = []
"#
    }

    #[test]
    fn test_load_encounter_table() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();
        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();

        let tables = load_encounters(&enc_dir);
        assert_eq!(tables.len(), 1);

        let forest = tables.get("forest").expect("forest table should exist");
        assert_eq!(forest.zone_name, "Dark Forest");
        assert_eq!(forest.description, "A dense, dangerous woodland");
        assert_eq!(forest.entries.len(), 3);
        assert_eq!(forest.entries[0].name, "Goblin Patrol");
        assert_eq!(forest.entries[0].weight, 30);
        assert_eq!(forest.entries[0].npcs.len(), 3);
        assert_eq!(forest.entries[1].name, "Traveling Merchant");
        assert_eq!(forest.entries[1].npcs, vec!["shopkeeper"]);
        assert_eq!(forest.entries[2].name, "Nothing happens");
        assert!(forest.entries[2].npcs.is_empty());
    }

    #[test]
    fn test_total_weight() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();
        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();

        let tables = load_encounters(&enc_dir);
        let forest = tables.get("forest").unwrap();
        assert_eq!(forest.total_weight(), 80); // 30 + 15 + 35
    }

    #[test]
    fn test_roll_deterministic() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();
        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();

        let tables = load_encounters(&enc_dir);
        let forest = tables.get("forest").unwrap();

        // Same seed should produce the same result
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);

        let result1 = forest.roll(&mut rng1).unwrap();
        let result2 = forest.roll(&mut rng2).unwrap();

        assert_eq!(result1.name, result2.name);
    }

    #[test]
    fn test_roll_all_entries_reachable() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();
        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();

        let tables = load_encounters(&enc_dir);
        let forest = tables.get("forest").unwrap();

        let mut seen: HashMap<String, bool> = HashMap::new();
        for seed in 0..500 {
            let mut rng = StdRng::seed_from_u64(seed);
            if let Some(entry) = forest.roll(&mut rng) {
                seen.insert(entry.name.clone(), true);
            }
        }

        assert!(
            seen.contains_key("Goblin Patrol"),
            "Goblin Patrol should be reachable"
        );
        assert!(
            seen.contains_key("Traveling Merchant"),
            "Traveling Merchant should be reachable"
        );
        assert!(
            seen.contains_key("Nothing happens"),
            "Nothing happens should be reachable"
        );
    }

    #[test]
    fn test_roll_empty_table() {
        let table = EncounterTable {
            zone_name: "Empty".to_string(),
            description: String::new(),
            entries: vec![],
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(table.roll(&mut rng).is_none());
    }

    #[test]
    fn test_roll_zero_weight_table() {
        let table = EncounterTable {
            zone_name: "Zero".to_string(),
            description: String::new(),
            entries: vec![EncounterEntry {
                name: "Ghost".to_string(),
                description: String::new(),
                weight: 0,
                npcs: vec![],
            }],
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(table.roll(&mut rng).is_none());
    }

    #[test]
    fn test_load_empty_directory() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();

        let tables = load_encounters(&enc_dir);
        assert!(tables.is_empty());
    }

    #[test]
    fn test_load_nonexistent_directory() {
        let tables = load_encounters(Path::new("/nonexistent/path"));
        assert!(tables.is_empty());
    }

    #[test]
    fn test_load_multiple_tables() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();

        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();
        fs::write(
            enc_dir.join("dungeon.toml"),
            r#"[zone]
name = "Dark Dungeon"

[[entries]]
name = "Skeleton"
weight = 50
npcs = ["goblin_king"]
"#,
        )
        .unwrap();

        let tables = load_encounters(&enc_dir);
        assert_eq!(tables.len(), 2);
        assert!(tables.contains_key("forest"));
        assert!(tables.contains_key("dungeon"));
    }

    #[test]
    fn test_load_skips_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();

        fs::write(enc_dir.join("forest.toml"), sample_toml()).unwrap();
        fs::write(enc_dir.join("broken.toml"), "this is not valid toml {{{{").unwrap();

        let tables = load_encounters(&enc_dir);
        // Only the valid one should load
        assert_eq!(tables.len(), 1);
        assert!(tables.contains_key("forest"));
    }

    #[test]
    fn test_load_default_weight() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();

        fs::write(
            enc_dir.join("test.toml"),
            r#"[zone]
name = "Test Zone"

[[entries]]
name = "No weight specified"
npcs = []
"#,
        )
        .unwrap();

        let tables = load_encounters(&enc_dir);
        let test = tables.get("test").unwrap();
        assert_eq!(test.entries[0].weight, 1); // default
    }

    #[test]
    fn test_entry_description_optional() {
        let dir = TempDir::new().unwrap();
        let enc_dir = dir.path().join("encounters");
        fs::create_dir_all(&enc_dir).unwrap();

        fs::write(
            enc_dir.join("test.toml"),
            r#"[zone]
name = "Test Zone"

[[entries]]
name = "No description"
weight = 10
npcs = []
"#,
        )
        .unwrap();

        let tables = load_encounters(&enc_dir);
        let test = tables.get("test").unwrap();
        assert_eq!(test.entries[0].description, "");
    }
}
