use std::path::{Path, PathBuf};

use crate::game_state::primitives::{
    Check, Derived, ModifierEffect, ResetTrigger, ResolutionMode, Threshold,
};
use crate::schema::csv_schema::{ColumnDef, ColumnType, CsvSchema};
use crate::schema::toml_schema::{validate_toml, KeyDef, SectionDef, TomlSchema, TomlValueType};

/// Error returned when loading rules fails.
#[derive(Debug, Clone)]
pub struct LoadRulesError {
    pub file: PathBuf,
    pub message: String,
    pub suggestion: String,
}

impl std::fmt::Display for LoadRulesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error loading {}:\n  {}\n  \u{2192} {}",
            self.file.display(),
            self.message,
            self.suggestion
        )
    }
}

/// Parsed rules from system.toml — the game system definition.
#[derive(Debug, Clone)]
pub struct CampaignRules {
    pub system_name: String,
    pub system_version: Option<String>,
    pub stat_names: Vec<String>,
    pub derived: Vec<Derived>,
    pub checks: Vec<Check>,
    pub modifier_defs: Vec<ModifierDef>,
    pub resource_defs: Vec<ResourceDef>,
}

/// A modifier definition from [modifier.*] in system.toml.
#[derive(Debug, Clone)]
pub struct ModifierDef {
    pub name: String,
    pub target: String,
    pub effect: ModifierEffect,
}

/// A resource definition from [resources.*] — either a Gauge or a Pool.
#[derive(Debug, Clone)]
pub enum ResourceDef {
    Gauge {
        name: String,
        max_stat: String,
    },
    Pool {
        name: String,
        max: f64,
        resets_on: ResetTrigger,
    },
}

/// Campaign-wide schemas for file validation.
#[derive(Debug, Clone)]
pub struct CampaignSchema {
    pub character_schema: CsvSchema,
    pub inventory_schema: CsvSchema,
}

/// Build the TomlSchema used to validate system.toml structure.
fn system_toml_schema() -> TomlSchema {
    TomlSchema::new(vec![
        SectionDef::required(
            "system",
            vec![
                KeyDef::required("name", TomlValueType::String),
                KeyDef::optional("version", TomlValueType::String),
            ],
        ),
        SectionDef::optional(
            "stats.definition",
            vec![
                KeyDef::optional("abilities", TomlValueType::ArrayOfStrings),
                KeyDef::optional("derived", TomlValueType::Table),
            ],
        ),
        SectionDef::optional(
            "character.schema",
            vec![KeyDef::required("columns", TomlValueType::ArrayOfStrings)],
        ),
        SectionDef::optional(
            "inventory.schema",
            vec![KeyDef::optional("columns", TomlValueType::ArrayOfStrings)],
        ),
        SectionDef::optional(
            "check.*",
            vec![
                KeyDef::required("roll", TomlValueType::String),
                KeyDef::optional("success", TomlValueType::String),
                KeyDef::optional("thresholds", TomlValueType::Array),
            ],
        ),
        SectionDef::optional(
            "modifier.*",
            vec![
                KeyDef::optional("type", TomlValueType::String),
                KeyDef::optional("effect", TomlValueType::String),
                KeyDef::optional("target", TomlValueType::String),
                KeyDef::optional("value", TomlValueType::Float),
            ],
        ),
        SectionDef::optional(
            "resources.*",
            vec![
                KeyDef::optional("type", TomlValueType::String),
                KeyDef::optional("max_stat", TomlValueType::String),
                KeyDef::optional("max", TomlValueType::Float),
                KeyDef::optional("resets_on", TomlValueType::String),
            ],
        ),
    ])
}

/// Default inventory schema columns when [inventory.schema] is absent.
fn default_inventory_schema() -> CsvSchema {
    CsvSchema::new(vec![
        ColumnDef::required_string("item"),
        ColumnDef::required_number("quantity"),
        ColumnDef::required_number("weight"),
        ColumnDef::new("notes", ColumnType::String, false),
    ])
}

/// Load and parse rules from a system.toml file.
///
/// Validates the TOML structure first, then parses each section into
/// the corresponding rule primitives. Returns both the rules and the
/// campaign schemas derived from the file.
pub fn load_rules(path: &Path) -> Result<(CampaignRules, CampaignSchema), Vec<LoadRulesError>> {
    let mut errors = Vec::new();

    // Step 1: Validate structure using TomlSchema
    if let Err(validation_errors) = validate_toml(path, &system_toml_schema()) {
        for ve in validation_errors {
            errors.push(LoadRulesError {
                file: ve.file,
                message: format!("Expected: {}\n  Found: {}", ve.expected, ve.found),
                suggestion: ve.suggestion,
            });
        }
        return Err(errors);
    }

    // Step 2: Read and parse the TOML
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            errors.push(LoadRulesError {
                file: path.to_path_buf(),
                message: format!("Cannot read file: {e}"),
                suggestion: "Check that the file exists and is readable.".to_string(),
            });
            return Err(errors);
        }
    };

    let root: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(e) => {
            errors.push(LoadRulesError {
                file: path.to_path_buf(),
                message: format!("Invalid TOML: {e}"),
                suggestion: "Fix the TOML syntax error.".to_string(),
            });
            return Err(errors);
        }
    };

    // Step 3: Parse [system]
    let system_name = root
        .get("system")
        .and_then(|s| s.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unnamed")
        .to_string();

    let system_version = root
        .get("system")
        .and_then(|s| s.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Step 4: Parse [stats.definition]
    let stat_names = parse_stat_names(&root);
    let derived = parse_derived(&root);

    // Step 5: Parse [character.schema]
    let character_schema = parse_character_schema(&root, &stat_names);

    // Step 6: Parse [inventory.schema]
    let inventory_schema = parse_inventory_schema(&root);

    // Step 7: Parse [check.*]
    let checks = parse_checks(&root);

    // Step 8: Parse [modifier.*]
    let modifier_defs = parse_modifiers(&root);

    // Step 9: Parse [resources.*]
    let resource_defs = parse_resources(&root);

    let rules = CampaignRules {
        system_name,
        system_version,
        stat_names,
        derived,
        checks,
        modifier_defs,
        resource_defs,
    };

    let schema = CampaignSchema {
        character_schema,
        inventory_schema,
    };

    Ok((rules, schema))
}

/// Parse stat names from [stats.definition].abilities
fn parse_stat_names(root: &toml::Value) -> Vec<String> {
    root.get("stats")
        .and_then(|s| s.get("definition"))
        .and_then(|d| d.get("abilities"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse derived values from [stats.definition].derived (table of name = formula)
fn parse_derived(root: &toml::Value) -> Vec<Derived> {
    root.get("stats")
        .and_then(|s| s.get("definition"))
        .and_then(|d| d.get("derived"))
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, val)| val.as_str().map(|formula| Derived::new(name, formula)))
                .collect()
        })
        .unwrap_or_default()
}

/// Build CsvSchema from [character.schema].columns.
/// If absent, builds a minimal schema from stat_names.
fn parse_character_schema(root: &toml::Value, stat_names: &[String]) -> CsvSchema {
    if let Some(columns) = root
        .get("character")
        .and_then(|c| c.get("schema"))
        .and_then(|s| s.get("columns"))
        .and_then(|c| c.as_array())
    {
        let col_defs: Vec<ColumnDef> = columns
            .iter()
            .filter_map(|v| v.as_str())
            .map(|name| {
                // "name" and "class" are strings, everything else is a number
                if name == "name" || name == "class" || name == "race" || name == "occupation" {
                    ColumnDef::required_string(name)
                } else {
                    ColumnDef::required_number(name)
                }
            })
            .collect();
        CsvSchema::new(col_defs)
    } else if !stat_names.is_empty() {
        // Fallback: build schema from stat_names with a "name" column first
        let mut col_defs = vec![ColumnDef::required_string("name")];
        for stat in stat_names {
            col_defs.push(ColumnDef::required_number(stat));
        }
        CsvSchema::new(col_defs)
    } else {
        // Minimal: just a name column
        CsvSchema::new(vec![ColumnDef::required_string("name")])
    }
}

/// Parse [inventory.schema].columns, or use defaults.
fn parse_inventory_schema(root: &toml::Value) -> CsvSchema {
    if let Some(columns) = root
        .get("inventory")
        .and_then(|i| i.get("schema"))
        .and_then(|s| s.get("columns"))
        .and_then(|c| c.as_array())
    {
        let col_defs: Vec<ColumnDef> = columns
            .iter()
            .filter_map(|v| v.as_str())
            .map(|name| {
                if name == "item" || name == "notes" || name == "name" {
                    ColumnDef::required_string(name)
                } else if name == "quantity" || name == "weight" || name == "durability" {
                    ColumnDef::required_number(name)
                } else {
                    ColumnDef::required_string(name)
                }
            })
            .collect();
        CsvSchema::new(col_defs)
    } else {
        default_inventory_schema()
    }
}

/// Parse [check.*] sections into Check primitives.
fn parse_checks(root: &toml::Value) -> Vec<Check> {
    let check_table = match root.get("check").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut checks = Vec::new();

    for (name, value) in check_table {
        let table = match value.as_table() {
            Some(t) => t,
            None => continue,
        };

        let roll = table
            .get("roll")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Determine resolution mode
        let has_thresholds = table.contains_key("thresholds");
        let success_str = table.get("success").and_then(|v| v.as_str()).unwrap_or("");

        let resolution_mode = if has_thresholds {
            ResolutionMode::Tiered
        } else if success_str.contains("<=") {
            ResolutionMode::RollUnder
        } else {
            ResolutionMode::RollOver
        };

        let mut check = Check::new(name, &roll, resolution_mode);

        // Parse thresholds for tiered resolution
        if let Some(thresholds_arr) = table.get("thresholds").and_then(|v| v.as_array()) {
            for threshold_val in thresholds_arr {
                if let Some(t_table) = threshold_val.as_table() {
                    let range = t_table.get("range").and_then(|v| v.as_str()).unwrap_or("");
                    let result = t_table.get("result").and_then(|v| v.as_str()).unwrap_or("");

                    let (min, max) = parse_range(range);
                    check = check.with_threshold(Threshold::new(min, max, result));
                }
            }
        }

        checks.push(check);
    }

    checks
}

/// Parse a range string like "..6", "7..9", "10.." into (Option<min>, Option<max>).
fn parse_range(range: &str) -> (Option<i32>, Option<i32>) {
    if let Some(rest) = range.strip_prefix("..") {
        // "..6" → (None, Some(6))
        let max = rest.parse::<i32>().ok();
        (None, max)
    } else if let Some(rest) = range.strip_suffix("..") {
        // "10.." → (Some(10), None)
        let min = rest.parse::<i32>().ok();
        (min, None)
    } else if range.contains("..") {
        // "7..9"
        let parts: Vec<&str> = range.split("..").collect();
        let min = parts.first().and_then(|s| s.parse::<i32>().ok());
        let max = parts.get(1).and_then(|s| s.parse::<i32>().ok());
        (min, max)
    } else {
        // Single value — treat as both min and max
        let val = range.parse::<i32>().ok();
        (val, val)
    }
}

/// Parse [modifier.*] sections into ModifierDef structs.
fn parse_modifiers(root: &toml::Value) -> Vec<ModifierDef> {
    let modifier_table = match root.get("modifier").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut defs = Vec::new();

    for (name, value) in modifier_table {
        let table = match value.as_table() {
            Some(t) => t,
            None => continue,
        };

        let target = table
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let effect_str = table.get("effect").and_then(|v| v.as_str()).unwrap_or("");
        let value_num = table
            .get("value")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(0.0);

        let effect = match effect_str {
            "add" => ModifierEffect::Add(value_num),
            "multiply" => ModifierEffect::Multiply(value_num),
            "advantage" | "take_best" => ModifierEffect::Advantage,
            "disadvantage" | "take_worst" => ModifierEffect::Disadvantage,
            other => ModifierEffect::Custom(other.to_string()),
        };

        defs.push(ModifierDef {
            name: name.to_string(),
            target,
            effect,
        });
    }

    defs
}

/// Parse [resources.*] sections into ResourceDef structs.
fn parse_resources(root: &toml::Value) -> Vec<ResourceDef> {
    let resources_table = match root.get("resources").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut defs = Vec::new();

    for (name, value) in resources_table {
        let table = match value.as_table() {
            Some(t) => t,
            None => continue,
        };

        let res_type = table
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("gauge");

        match res_type {
            "pool" => {
                let max = table
                    .get("max")
                    .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
                    .unwrap_or(1.0);

                let resets_on = table
                    .get("resets_on")
                    .and_then(|v| v.as_str())
                    .map(parse_reset_trigger)
                    .unwrap_or(ResetTrigger::Manual);

                defs.push(ResourceDef::Pool {
                    name: name.to_string(),
                    max,
                    resets_on,
                });
            }
            _ => {
                // Default to gauge
                let max_stat = table
                    .get("max_stat")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                defs.push(ResourceDef::Gauge {
                    name: name.to_string(),
                    max_stat,
                });
            }
        }
    }

    defs
}

/// Parse a reset trigger string into a ResetTrigger enum variant.
fn parse_reset_trigger(s: &str) -> ResetTrigger {
    match s.to_lowercase().as_str() {
        "short_rest" | "short rest" => ResetTrigger::ShortRest,
        "long_rest" | "long rest" => ResetTrigger::LongRest,
        "dawn" => ResetTrigger::Dawn,
        "manual" => ResetTrigger::Manual,
        other => ResetTrigger::Custom(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_parse_minimal_system_toml() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "Minimal"
"#,
        )
        .unwrap();

        let (rules, schema) = load_rules(&path).unwrap();
        assert_eq!(rules.system_name, "Minimal");
        assert!(rules.system_version.is_none());
        assert!(rules.stat_names.is_empty());
        assert!(rules.checks.is_empty());
        // Minimal schema: just a "name" column
        assert_eq!(schema.character_schema.columns.len(), 1);
        assert_eq!(schema.character_schema.columns[0].name, "name");
        // Default inventory schema
        assert_eq!(schema.inventory_schema.columns.len(), 4);
    }

    #[test]
    fn test_parse_dnd_like_system() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "D&D 5e"
version = "1.0"

[stats.definition]
abilities = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"]
derived = { ac = "10 + modifier(dexterity)", initiative = "modifier(dexterity)" }

[character.schema]
columns = ["name", "class", "level", "strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma", "hp_max", "ac"]

[inventory.schema]
columns = ["item", "quantity", "weight", "notes"]

[check.ability_check]
roll = "1d20 + modifier({ability})"
success = "result >= dc"

[check.saving_throw]
roll = "1d20 + modifier({ability})"
success = "result >= dc"

[modifier.advantage]
type = "roll_modifier"
effect = "take_best"
target = "attack_roll"

[modifier.bless]
type = "roll_modifier"
effect = "add"
target = "attack_roll"
value = 1.0

[resources.hp]
type = "gauge"
max_stat = "hp_max"

[resources.spell_slots]
type = "pool"
max = 4.0
resets_on = "long_rest"
"#,
        )
        .unwrap();

        let (rules, schema) = load_rules(&path).unwrap();

        // System info
        assert_eq!(rules.system_name, "D&D 5e");
        assert_eq!(rules.system_version, Some("1.0".to_string()));

        // Stats
        assert_eq!(rules.stat_names.len(), 6);
        assert_eq!(rules.stat_names[0], "strength");
        assert_eq!(rules.stat_names[5], "charisma");

        // Derived
        assert_eq!(rules.derived.len(), 2);
        assert!(rules
            .derived
            .iter()
            .any(|d| d.name == "ac" && d.formula == "10 + modifier(dexterity)"));
        assert!(rules.derived.iter().any(|d| d.name == "initiative"));

        // Character schema
        assert_eq!(schema.character_schema.columns.len(), 11);
        assert_eq!(schema.character_schema.columns[0].name, "name");
        assert_eq!(
            schema.character_schema.columns[0].col_type,
            ColumnType::String
        );
        assert_eq!(schema.character_schema.columns[1].name, "class");
        assert_eq!(
            schema.character_schema.columns[1].col_type,
            ColumnType::String
        );
        assert_eq!(schema.character_schema.columns[2].name, "level");
        assert_eq!(
            schema.character_schema.columns[2].col_type,
            ColumnType::Number
        );

        // Inventory schema
        assert_eq!(schema.inventory_schema.columns.len(), 4);

        // Checks
        assert_eq!(rules.checks.len(), 2);
        let ability_check = rules
            .checks
            .iter()
            .find(|c| c.name == "ability_check")
            .unwrap();
        assert_eq!(ability_check.roll, "1d20 + modifier({ability})");
        assert_eq!(ability_check.resolution_mode, ResolutionMode::RollOver);

        // Modifiers
        assert_eq!(rules.modifier_defs.len(), 2);
        let advantage = rules
            .modifier_defs
            .iter()
            .find(|m| m.name == "advantage")
            .unwrap();
        assert_eq!(advantage.effect, ModifierEffect::Advantage);
        let bless = rules
            .modifier_defs
            .iter()
            .find(|m| m.name == "bless")
            .unwrap();
        assert_eq!(bless.effect, ModifierEffect::Add(1.0));

        // Resources
        assert_eq!(rules.resource_defs.len(), 2);
        assert!(rules.resource_defs.iter().any(|r| matches!(r, ResourceDef::Gauge { name, max_stat } if name == "hp" && max_stat == "hp_max")));
        assert!(rules.resource_defs.iter().any(|r| matches!(r, ResourceDef::Pool { name, max, resets_on } if name == "spell_slots" && *max == 4.0 && *resets_on == ResetTrigger::LongRest)));
    }

    #[test]
    fn test_parse_pbta_like_system() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "PbtA"
version = "2.0"

[stats.definition]
abilities = ["cool", "hard", "hot", "sharp", "weird"]

[character.schema]
columns = ["name", "class", "cool", "hard", "hot", "sharp", "weird"]

[check.move]
roll = "2d6 + {stat}"
thresholds = [
  { range = "..6", result = "miss" },
  { range = "7..9", result = "partial" },
  { range = "10..", result = "success" },
]
"#,
        )
        .unwrap();

        let (rules, schema) = load_rules(&path).unwrap();

        assert_eq!(rules.system_name, "PbtA");
        assert_eq!(rules.stat_names.len(), 5);
        assert_eq!(rules.stat_names[0], "cool");

        // Character schema from explicit columns
        assert_eq!(schema.character_schema.columns.len(), 7);
        assert_eq!(schema.character_schema.columns[0].name, "name");

        // Check with tiered resolution
        assert_eq!(rules.checks.len(), 1);
        let check = &rules.checks[0];
        assert_eq!(check.name, "move");
        assert_eq!(check.roll, "2d6 + {stat}");
        assert_eq!(check.resolution_mode, ResolutionMode::Tiered);
        assert_eq!(check.thresholds.len(), 3);

        // Verify thresholds
        assert!(check.thresholds[0].matches(5));
        assert!(check.thresholds[0].matches(6));
        assert!(!check.thresholds[0].matches(7));
        assert_eq!(check.thresholds[0].result, "miss");

        assert!(check.thresholds[1].matches(7));
        assert!(check.thresholds[1].matches(9));
        assert!(!check.thresholds[1].matches(10));
        assert_eq!(check.thresholds[1].result, "partial");

        assert!(check.thresholds[2].matches(10));
        assert!(check.thresholds[2].matches(12));
        assert!(!check.thresholds[2].matches(9));
        assert_eq!(check.thresholds[2].result, "success");
    }

    #[test]
    fn test_missing_system_section_returns_error() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(&path, "# empty file\n").unwrap();

        let result = load_rules(&path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("system")));
    }

    #[test]
    fn test_invalid_toml_syntax_returns_error() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(&path, "[system\nname = broken").unwrap();

        let result = load_rules(&path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_system_toml_structure() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // [system] exists but name is wrong type
        fs::write(
            &path,
            r#"
[system]
name = 42
"#,
        )
        .unwrap();

        let result = load_rules(&path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("String")));
    }

    #[test]
    fn test_default_inventory_schema_when_absent() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "Test"

[character.schema]
columns = ["name", "level"]
"#,
        )
        .unwrap();

        let (_, schema) = load_rules(&path).unwrap();
        // Should use default inventory schema
        let inv_names: Vec<&str> = schema.inventory_schema.column_names();
        assert_eq!(inv_names, vec!["item", "quantity", "weight", "notes"]);
    }

    #[test]
    fn test_custom_inventory_schema() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "Test"

[character.schema]
columns = ["name", "level"]

[inventory.schema]
columns = ["item", "quantity", "durability"]
"#,
        )
        .unwrap();

        let (_, schema) = load_rules(&path).unwrap();
        let inv_names: Vec<&str> = schema.inventory_schema.column_names();
        assert_eq!(inv_names, vec!["item", "quantity", "durability"]);
    }

    #[test]
    fn test_character_schema_fallback_to_stat_names() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // Has stats but no [character.schema]
        fs::write(
            &path,
            r#"
[system]
name = "Fallback"

[stats.definition]
abilities = ["strength", "dexterity", "constitution"]
"#,
        )
        .unwrap();

        let (rules, schema) = load_rules(&path).unwrap();
        assert_eq!(rules.stat_names.len(), 3);
        // Schema should be: name + the 3 stat names
        assert_eq!(schema.character_schema.columns.len(), 4);
        assert_eq!(schema.character_schema.columns[0].name, "name");
        assert_eq!(schema.character_schema.columns[1].name, "strength");
        assert_eq!(
            schema.character_schema.columns[1].col_type,
            ColumnType::Number
        );
    }

    #[test]
    fn test_parse_range_variants() {
        assert_eq!(parse_range("..6"), (None, Some(6)));
        assert_eq!(parse_range("10.."), (Some(10), None));
        assert_eq!(parse_range("7..9"), (Some(7), Some(9)));
        assert_eq!(parse_range("5"), (Some(5), Some(5)));
    }

    #[test]
    fn test_nonexistent_file_returns_error() {
        let path = Path::new("/tmp/nonexistent_rules_12345.toml");
        let result = load_rules(path);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_roll_under_check_detection() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "CoC"

[check.skill]
roll = "1d100"
success = "result <= target"
"#,
        )
        .unwrap();

        let (rules, _) = load_rules(&path).unwrap();
        assert_eq!(rules.checks.len(), 1);
        assert_eq!(rules.checks[0].resolution_mode, ResolutionMode::RollUnder);
    }

    #[test]
    fn test_error_display_format() {
        let err = LoadRulesError {
            file: PathBuf::from("rules/system.toml"),
            message: "Section [system] is missing".to_string(),
            suggestion: "Add [system] with a name key.".to_string(),
        };
        let display = format!("{err}");
        assert!(display.contains("rules/system.toml"));
        assert!(display.contains("Section [system] is missing"));
        assert!(display.contains("\u{2192}"));
        assert!(display.contains("Add [system]"));
    }
}
