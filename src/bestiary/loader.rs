use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::game_state::loader::LoadError;
use crate::game_state::primitives::Stat;

/// A creature template loaded from a bestiary CSV file.
#[derive(Debug, Clone)]
pub struct BestiaryEntry {
    pub name: String,
    pub stats: Vec<Stat>,
}

/// A creature reference within an encounter definition.
#[derive(Debug, Clone)]
pub struct EncounterCreature {
    pub template: String,
    pub count: usize,
    pub name_override: Option<String>,
}

/// An encounter loaded from a TOML file in bestiary/encounters/.
#[derive(Debug, Clone)]
pub struct Encounter {
    pub name: String,
    pub description: String,
    pub creatures: Vec<EncounterCreature>,
}

/// Result of loading the bestiary directory.
pub struct BestiaryLoadResult {
    pub entries: Vec<BestiaryEntry>,
    pub encounters: Vec<Encounter>,
    pub errors: Vec<LoadError>,
}

/// Load all bestiary entries and encounters from a campaign's `bestiary/` directory.
///
/// Creature CSVs are validated against `expected_columns` (from the character schema).
/// Encounter TOMLs are parsed from `bestiary/encounters/`.
/// If the directory does not exist, returns an empty result (no error).
pub fn load_bestiary(bestiary_dir: &Path, expected_columns: &[&str]) -> BestiaryLoadResult {
    let mut entries = Vec::new();
    let mut encounters = Vec::new();
    let mut errors = Vec::new();

    if !bestiary_dir.exists() {
        return BestiaryLoadResult {
            entries,
            encounters,
            errors,
        };
    }

    // Scan for creature CSV files in the bestiary root
    match fs::read_dir(bestiary_dir) {
        Ok(dir_entries) => {
            let mut csv_files: Vec<PathBuf> = dir_entries
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("csv") {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect();
            csv_files.sort();

            for csv_path in csv_files {
                match load_creature_csv(&csv_path, expected_columns) {
                    Ok(entry) => entries.push(entry),
                    Err(errs) => errors.extend(errs),
                }
            }
        }
        Err(e) => {
            errors.push(LoadError {
                file: bestiary_dir.to_path_buf(),
                message: format!("Cannot read bestiary directory: {e}"),
                suggestion: "Check that the bestiary/ directory is readable.".to_string(),
            });
        }
    }

    // Scan for encounter TOML files in bestiary/encounters/
    let encounters_dir = bestiary_dir.join("encounters");
    if encounters_dir.exists() {
        match fs::read_dir(&encounters_dir) {
            Ok(dir_entries) => {
                let mut toml_files: Vec<PathBuf> = dir_entries
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("toml") {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect();
                toml_files.sort();

                for toml_path in toml_files {
                    match load_encounter_toml(&toml_path) {
                        Ok(enc) => encounters.push(enc),
                        Err(errs) => errors.extend(errs),
                    }
                }
            }
            Err(e) => {
                errors.push(LoadError {
                    file: encounters_dir,
                    message: format!("Cannot read encounters directory: {e}"),
                    suggestion: "Check that the bestiary/encounters/ directory is readable."
                        .to_string(),
                });
            }
        }
    }

    BestiaryLoadResult {
        entries,
        encounters,
        errors,
    }
}

/// Load a single creature template from a CSV file.
/// The CSV must have a header row and exactly one data row.
/// Columns are validated against expected_columns if provided.
fn load_creature_csv(
    path: &Path,
    expected_columns: &[&str],
) -> Result<BestiaryEntry, Vec<LoadError>> {
    let mut reader = csv::Reader::from_path(path).map_err(|e| {
        vec![LoadError {
            file: path.to_path_buf(),
            message: format!("Cannot read CSV file: {e}"),
            suggestion: "Check that the file exists and is valid CSV.".to_string(),
        }]
    })?;

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| {
            vec![LoadError {
                file: path.to_path_buf(),
                message: format!("Cannot read CSV headers: {e}"),
                suggestion: "Ensure the first row contains column names.".to_string(),
            }]
        })?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();

    // Validate columns
    if !expected_columns.is_empty() {
        let mut col_errors = Vec::new();
        for &col in expected_columns {
            if !headers.iter().any(|h| h == col) {
                col_errors.push(LoadError {
                    file: path.to_path_buf(),
                    message: format!(
                        "Column \"{}\" is missing.\n  Expected columns: {}\n  Found columns: {}",
                        col,
                        expected_columns.join(", "),
                        headers.join(", ")
                    ),
                    suggestion: format!("Add the \"{col}\" column to the creature CSV file."),
                });
            }
        }
        if !col_errors.is_empty() {
            return Err(col_errors);
        }
    }

    // Read data row(s) — expect exactly one
    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    for (i, result) in reader.records().enumerate() {
        match result {
            Ok(record) => {
                let mut row = HashMap::new();
                for (j, value) in record.iter().enumerate() {
                    if j < headers.len() {
                        row.insert(headers[j].clone(), value.trim().to_string());
                    }
                }
                rows.push(row);
            }
            Err(e) => {
                return Err(vec![LoadError {
                    file: path.to_path_buf(),
                    message: format!("Error reading row {}: {e}", i + 2),
                    suggestion: "Check that all rows have the correct number of columns."
                        .to_string(),
                }]);
            }
        }
    }

    if rows.is_empty() {
        return Err(vec![LoadError {
            file: path.to_path_buf(),
            message: "Creature CSV has no data rows (only headers).".to_string(),
            suggestion: "Add a data row with the creature's stats below the header row."
                .to_string(),
        }]);
    }

    let row = &rows[0];
    let name = row.get("name").cloned().unwrap_or_else(|| {
        // Fall back to filename without extension
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    });

    let mut stats = Vec::new();
    for header in &headers {
        if header == "name" {
            continue;
        }
        if let Some(value) = row.get(header) {
            if let Ok(num) = value.parse::<f64>() {
                stats.push(Stat::new(header, num));
            } else {
                stats.push(Stat::new(header, 0.0));
            }
        }
    }

    Ok(BestiaryEntry { name, stats })
}

/// Load an encounter definition from a TOML file.
fn load_encounter_toml(path: &Path) -> Result<Encounter, Vec<LoadError>> {
    let content = fs::read_to_string(path).map_err(|e| {
        vec![LoadError {
            file: path.to_path_buf(),
            message: format!("Cannot read file: {e}"),
            suggestion: "Check that the file exists and is readable.".to_string(),
        }]
    })?;

    let root: toml::Value = content.parse().map_err(|e: toml::de::Error| {
        vec![LoadError {
            file: path.to_path_buf(),
            message: format!("Invalid TOML: {e}"),
            suggestion: "Fix the TOML syntax error.".to_string(),
        }]
    })?;

    // Parse [encounter] section
    let encounter_section = root.get("encounter").ok_or_else(|| {
        vec![LoadError {
            file: path.to_path_buf(),
            message: "Missing [encounter] section.".to_string(),
            suggestion: "Add an [encounter] section with name and description.".to_string(),
        }]
    })?;

    let name = encounter_section
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            vec![LoadError {
                file: path.to_path_buf(),
                message: "Missing \"name\" key in [encounter] section.".to_string(),
                suggestion: "Add a \"name\" key to the [encounter] section.".to_string(),
            }]
        })?
        .to_string();

    let description = encounter_section
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Parse [[creatures]] array
    let creatures_arr = root
        .get("creatures")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            vec![LoadError {
                file: path.to_path_buf(),
                message: "Missing [[creatures]] array.".to_string(),
                suggestion: "Add at least one [[creatures]] entry with template and count."
                    .to_string(),
            }]
        })?;

    let mut creatures = Vec::new();
    for (i, creature_val) in creatures_arr.iter().enumerate() {
        let template = creature_val
            .get("template")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                vec![LoadError {
                    file: path.to_path_buf(),
                    message: format!("Missing \"template\" key in [[creatures]] entry #{}", i + 1),
                    suggestion: "Add a \"template\" key referencing a bestiary creature name."
                        .to_string(),
                }]
            })?
            .to_string();

        let count = creature_val
            .get("count")
            .and_then(|v| v.as_integer())
            .unwrap_or(1) as usize;

        let name_override = creature_val
            .get("name_override")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        creatures.push(EncounterCreature {
            template,
            count,
            name_override,
        });
    }

    Ok(Encounter {
        name,
        description,
        creatures,
    })
}

/// Look up a bestiary entry by name (case-insensitive).
pub fn find_entry<'a>(entries: &'a [BestiaryEntry], name: &str) -> Option<&'a BestiaryEntry> {
    let lower = name.to_lowercase();
    entries.iter().find(|e| e.name.to_lowercase() == lower)
}

/// Look up an encounter by name (case-insensitive).
pub fn find_encounter<'a>(encounters: &'a [Encounter], name: &str) -> Option<&'a Encounter> {
    let lower = name.to_lowercase();
    encounters.iter().find(|e| e.name.to_lowercase() == lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // --- load_creature_csv ---

    #[test]
    fn test_load_creature_csv_valid() {
        let dir = test_dir();
        let csv_path = dir.path().join("goblin.csv");
        fs::write(
            &csv_path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,charisma,hp_max,ac\n\
             Goblin,Monster,1,8,14,10,10,8,8,7,15\n",
        )
        .unwrap();

        let expected = &[
            "name",
            "class",
            "level",
            "strength",
            "dexterity",
            "constitution",
            "intelligence",
            "wisdom",
            "charisma",
            "hp_max",
            "ac",
        ];
        let entry = load_creature_csv(&csv_path, expected).unwrap();
        assert_eq!(entry.name, "Goblin");
        // class is non-numeric -> 0.0, level=1, strength=8, etc.
        assert!(entry
            .stats
            .iter()
            .any(|s| s.name == "strength" && s.value == 8.0));
        assert!(entry
            .stats
            .iter()
            .any(|s| s.name == "hp_max" && s.value == 7.0));
        assert!(entry
            .stats
            .iter()
            .any(|s| s.name == "ac" && s.value == 15.0));
    }

    #[test]
    fn test_load_creature_csv_missing_column() {
        let dir = test_dir();
        let csv_path = dir.path().join("bad_creature.csv");
        fs::write(&csv_path, "name,strength\nGoblin,8\n").unwrap();

        let expected = &["name", "strength", "hp_max"];
        let result = load_creature_csv(&csv_path, expected);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("hp_max"));
        assert!(errors[0].message.contains("missing"));
    }

    #[test]
    fn test_load_creature_csv_empty_data() {
        let dir = test_dir();
        let csv_path = dir.path().join("empty.csv");
        fs::write(&csv_path, "name,strength\n").unwrap();

        let result = load_creature_csv(&csv_path, &["name", "strength"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("no data rows"));
    }

    #[test]
    fn test_load_creature_csv_no_column_validation() {
        let dir = test_dir();
        let csv_path = dir.path().join("any.csv");
        fs::write(&csv_path, "name,power\nDragon,99\n").unwrap();

        let entry = load_creature_csv(&csv_path, &[]).unwrap();
        assert_eq!(entry.name, "Dragon");
        assert!(entry
            .stats
            .iter()
            .any(|s| s.name == "power" && s.value == 99.0));
    }

    #[test]
    fn test_load_creature_csv_name_fallback_to_filename() {
        let dir = test_dir();
        let csv_path = dir.path().join("skeleton.csv");
        // CSV without a "name" column
        fs::write(&csv_path, "strength,hp_max\n12,20\n").unwrap();

        let entry = load_creature_csv(&csv_path, &[]).unwrap();
        assert_eq!(entry.name, "skeleton");
    }

    // --- load_encounter_toml ---

    #[test]
    fn test_load_encounter_toml_valid() {
        let dir = test_dir();
        let toml_path = dir.path().join("goblin_patrol.toml");
        fs::write(
            &toml_path,
            r#"
[encounter]
name = "Goblin Patrol"
description = "A small group of goblins on the road"

[[creatures]]
template = "goblin"
count = 3

[[creatures]]
template = "orc"
count = 1
name_override = "Orc Chieftain"
"#,
        )
        .unwrap();

        let enc = load_encounter_toml(&toml_path).unwrap();
        assert_eq!(enc.name, "Goblin Patrol");
        assert_eq!(enc.description, "A small group of goblins on the road");
        assert_eq!(enc.creatures.len(), 2);
        assert_eq!(enc.creatures[0].template, "goblin");
        assert_eq!(enc.creatures[0].count, 3);
        assert!(enc.creatures[0].name_override.is_none());
        assert_eq!(enc.creatures[1].template, "orc");
        assert_eq!(enc.creatures[1].count, 1);
        assert_eq!(
            enc.creatures[1].name_override.as_deref(),
            Some("Orc Chieftain")
        );
    }

    #[test]
    fn test_load_encounter_toml_missing_encounter_section() {
        let dir = test_dir();
        let toml_path = dir.path().join("bad.toml");
        fs::write(
            &toml_path,
            r#"
[[creatures]]
template = "goblin"
count = 2
"#,
        )
        .unwrap();

        let result = load_encounter_toml(&toml_path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("[encounter]"));
    }

    #[test]
    fn test_load_encounter_toml_missing_name() {
        let dir = test_dir();
        let toml_path = dir.path().join("no_name.toml");
        fs::write(
            &toml_path,
            r#"
[encounter]
description = "No name"

[[creatures]]
template = "goblin"
count = 1
"#,
        )
        .unwrap();

        let result = load_encounter_toml(&toml_path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("name"));
    }

    #[test]
    fn test_load_encounter_toml_missing_creatures() {
        let dir = test_dir();
        let toml_path = dir.path().join("no_creatures.toml");
        fs::write(
            &toml_path,
            r#"
[encounter]
name = "Empty fight"
"#,
        )
        .unwrap();

        let result = load_encounter_toml(&toml_path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("[[creatures]]"));
    }

    #[test]
    fn test_load_encounter_toml_missing_template() {
        let dir = test_dir();
        let toml_path = dir.path().join("no_template.toml");
        fs::write(
            &toml_path,
            r#"
[encounter]
name = "Bad"

[[creatures]]
count = 2
"#,
        )
        .unwrap();

        let result = load_encounter_toml(&toml_path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("template"));
    }

    #[test]
    fn test_load_encounter_toml_default_count() {
        let dir = test_dir();
        let toml_path = dir.path().join("default_count.toml");
        fs::write(
            &toml_path,
            r#"
[encounter]
name = "Solo"

[[creatures]]
template = "dragon"
"#,
        )
        .unwrap();

        let enc = load_encounter_toml(&toml_path).unwrap();
        assert_eq!(enc.creatures[0].count, 1);
    }

    #[test]
    fn test_load_encounter_toml_invalid_toml() {
        let dir = test_dir();
        let toml_path = dir.path().join("broken.toml");
        fs::write(&toml_path, "[encounter\nbroken").unwrap();

        let result = load_encounter_toml(&toml_path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("Invalid TOML"));
    }

    // --- load_bestiary (integration) ---

    #[test]
    fn test_load_bestiary_full() {
        let dir = test_dir();
        let bestiary_dir = dir.path().join("bestiary");
        fs::create_dir_all(bestiary_dir.join("encounters")).unwrap();

        fs::write(
            bestiary_dir.join("goblin.csv"),
            "name,strength,hp_max\nGoblin,8,7\n",
        )
        .unwrap();
        fs::write(
            bestiary_dir.join("orc.csv"),
            "name,strength,hp_max\nOrc,16,15\n",
        )
        .unwrap();

        fs::write(
            bestiary_dir.join("encounters/ambush.toml"),
            r#"
[encounter]
name = "Ambush"
description = "Surprise attack"

[[creatures]]
template = "goblin"
count = 2

[[creatures]]
template = "orc"
count = 1
"#,
        )
        .unwrap();

        let expected = &["name", "strength", "hp_max"];
        let result = load_bestiary(&bestiary_dir, expected);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].name, "Goblin");
        assert_eq!(result.entries[1].name, "Orc");
        assert_eq!(result.encounters.len(), 1);
        assert_eq!(result.encounters[0].name, "Ambush");
        assert_eq!(result.encounters[0].creatures.len(), 2);
    }

    #[test]
    fn test_load_bestiary_missing_dir() {
        let dir = test_dir();
        let bestiary_dir = dir.path().join("bestiary");

        let result = load_bestiary(&bestiary_dir, &["name"]);
        assert!(result.errors.is_empty());
        assert!(result.entries.is_empty());
        assert!(result.encounters.is_empty());
    }

    #[test]
    fn test_load_bestiary_no_encounters_dir() {
        let dir = test_dir();
        let bestiary_dir = dir.path().join("bestiary");
        fs::create_dir_all(&bestiary_dir).unwrap();

        fs::write(bestiary_dir.join("goblin.csv"), "name,strength\nGoblin,8\n").unwrap();

        let result = load_bestiary(&bestiary_dir, &[]);
        assert!(result.errors.is_empty());
        assert_eq!(result.entries.len(), 1);
        assert!(result.encounters.is_empty());
    }

    #[test]
    fn test_load_bestiary_invalid_creature_csv() {
        let dir = test_dir();
        let bestiary_dir = dir.path().join("bestiary");
        fs::create_dir_all(&bestiary_dir).unwrap();

        fs::write(bestiary_dir.join("bad.csv"), "name,strength\nBad,8\n").unwrap();

        let expected = &["name", "strength", "hp_max"];
        let result = load_bestiary(&bestiary_dir, expected);
        assert_eq!(result.entries.len(), 0);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("hp_max"));
    }

    #[test]
    fn test_load_bestiary_ignores_non_csv_files() {
        let dir = test_dir();
        let bestiary_dir = dir.path().join("bestiary");
        fs::create_dir_all(&bestiary_dir).unwrap();

        fs::write(bestiary_dir.join("readme.md"), "# Bestiary notes").unwrap();
        fs::write(bestiary_dir.join("goblin.csv"), "name,strength\nGoblin,8\n").unwrap();

        let result = load_bestiary(&bestiary_dir, &[]);
        assert!(result.errors.is_empty());
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "Goblin");
    }

    // --- find helpers ---

    #[test]
    fn test_find_entry_case_insensitive() {
        let entries = vec![
            BestiaryEntry {
                name: "Goblin".to_string(),
                stats: vec![],
            },
            BestiaryEntry {
                name: "Orc".to_string(),
                stats: vec![],
            },
        ];

        assert!(find_entry(&entries, "goblin").is_some());
        assert!(find_entry(&entries, "GOBLIN").is_some());
        assert!(find_entry(&entries, "Orc").is_some());
        assert!(find_entry(&entries, "dragon").is_none());
    }

    #[test]
    fn test_find_encounter_case_insensitive() {
        let encounters = vec![Encounter {
            name: "Goblin Patrol".to_string(),
            description: String::new(),
            creatures: vec![],
        }];

        assert!(find_encounter(&encounters, "goblin patrol").is_some());
        assert!(find_encounter(&encounters, "GOBLIN PATROL").is_some());
        assert!(find_encounter(&encounters, "dragon fight").is_none());
    }
}
