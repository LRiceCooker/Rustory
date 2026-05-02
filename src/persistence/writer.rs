use std::path::Path;

use crate::game_state::character::Character;
use crate::schema::csv_schema::CsvSchema;

/// Write a character's stats to a sheet.csv file.
/// Uses the schema column order for deterministic output.
/// After writing, re-reads and validates against the schema.
/// Returns Err if the written file doesn't match the schema (writer bug).
pub fn write_character_sheet(
    character: &Character,
    path: &Path,
    schema: &CsvSchema,
) -> color_eyre::Result<()> {
    let columns = schema.column_names();

    let mut wtr = csv::Writer::from_path(path)?;

    // Write header row
    wtr.write_record(&columns)?;

    // Write data row: one value per schema column
    let mut values = Vec::with_capacity(columns.len());
    for col in &columns {
        if *col == "name" {
            values.push(character.name.clone());
        } else if let Some(v) = character.get_stat(col) {
            // Format: drop trailing ".0" for whole numbers
            if v.fract() == 0.0 && v.abs() < 1e15 {
                values.push(format!("{}", v as i64));
            } else {
                values.push(v.to_string());
            }
        } else {
            // Column exists in schema but not in character stats — empty
            values.push(String::new());
        }
    }
    wtr.write_record(&values)?;
    wtr.flush()?;

    // Post-write validation: re-read and validate against the same schema
    if let Err(errors) = crate::schema::csv_schema::validate_csv(path, schema) {
        let msg = errors
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(color_eyre::eyre::eyre!(
            "Writer bug: written file failed validation: {msg}"
        ));
    }

    Ok(())
}

/// Write a character's inventory to an inventory.csv file.
/// Uses the schema column order for deterministic output.
/// After writing, re-reads and validates against the schema.
pub fn write_inventory(
    character: &Character,
    path: &Path,
    schema: &CsvSchema,
) -> color_eyre::Result<()> {
    let columns = schema.column_names();

    let mut wtr = csv::Writer::from_path(path)?;

    // Write header row
    wtr.write_record(&columns)?;

    // Write one row per inventory item
    for item in &character.inventory.items {
        let mut values = Vec::with_capacity(columns.len());
        for col in &columns {
            if *col == "item" {
                values.push(item.name.clone());
            } else if let Some(prop) = item.properties.get(*col) {
                values.push(prop.clone());
            } else {
                values.push(String::new());
            }
        }
        wtr.write_record(&values)?;
    }
    wtr.flush()?;

    // Post-write validation
    if let Err(errors) = crate::schema::csv_schema::validate_csv(path, schema) {
        let msg = errors
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(color_eyre::eyre::eyre!(
            "Writer bug: written inventory failed validation: {msg}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::{CollectionItem, Stat};
    use crate::schema::csv_schema::{ColumnDef, ColumnType};
    use tempfile::TempDir;

    fn dnd_character_schema() -> CsvSchema {
        CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::new("class", ColumnType::String, false),
            ColumnDef::required_number("level"),
            ColumnDef::required_number("strength"),
            ColumnDef::required_number("dexterity"),
            ColumnDef::required_number("hp_max"),
        ])
    }

    fn inventory_schema() -> CsvSchema {
        CsvSchema::new(vec![
            ColumnDef::required_string("item"),
            ColumnDef::required_number("quantity"),
            ColumnDef::required_number("weight"),
            ColumnDef::new("notes", ColumnType::String, false),
        ])
    }

    fn make_character() -> Character {
        let mut ch = Character::new("Thorin");
        ch.stats = vec![
            Stat::new("class", 0.0), // String columns stored as 0.0
            Stat::new("level", 5.0),
            Stat::new("strength", 18.0),
            Stat::new("dexterity", 12.0),
            Stat::new("hp_max", 52.0),
        ];
        ch
    }

    #[test]
    fn test_write_character_sheet_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sheet.csv");
        let ch = make_character();
        let schema = dnd_character_schema();

        write_character_sheet(&ch, &path, &schema).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();

        assert_eq!(lines.len(), 2, "Should have header + 1 data row");
        assert_eq!(lines[0], "name,class,level,strength,dexterity,hp_max");
        // "class" stat has value 0.0, written as "0"
        assert!(
            lines[1].starts_with("Thorin,"),
            "Data row should start with character name"
        );
        assert!(
            lines[1].contains(",18,"),
            "Should contain strength=18: {}",
            lines[1]
        );
    }

    #[test]
    fn test_write_character_sheet_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sheet.csv");
        let ch = make_character();
        let schema = dnd_character_schema();

        // Write
        write_character_sheet(&ch, &path, &schema).unwrap();

        // Re-read using the loader's CSV parsing
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let headers: Vec<String> = reader
            .headers()
            .unwrap()
            .iter()
            .map(|h| h.to_string())
            .collect();

        let record = reader.records().next().unwrap().unwrap();
        let mut row = std::collections::HashMap::new();
        for (i, val) in record.iter().enumerate() {
            row.insert(headers[i].clone(), val.trim().to_string());
        }

        assert_eq!(row.get("name").unwrap(), "Thorin");
        assert_eq!(
            row.get("strength").unwrap().parse::<f64>().unwrap(),
            18.0
        );
        assert_eq!(
            row.get("hp_max").unwrap().parse::<f64>().unwrap(),
            52.0
        );
    }

    #[test]
    fn test_write_character_sheet_modify_and_verify() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sheet.csv");
        let mut ch = make_character();
        let schema = dnd_character_schema();

        // Write original
        write_character_sheet(&ch, &path, &schema).unwrap();

        // Modify character
        ch.set_stat("strength", 20.0);

        // Write modified
        write_character_sheet(&ch, &path, &schema).unwrap();

        // Verify
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains(",20,"),
            "Modified strength should be 20: {content}"
        );
    }

    #[test]
    fn test_write_inventory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("inventory.csv");
        let mut ch = Character::new("Thorin");
        ch.add_item(
            CollectionItem::new("Longsword")
                .with_property("quantity", "1")
                .with_property("weight", "3")
                .with_property("notes", "+1 magical"),
        );
        ch.add_item(
            CollectionItem::new("Shield")
                .with_property("quantity", "1")
                .with_property("weight", "6")
                .with_property("notes", ""),
        );

        let schema = inventory_schema();
        write_inventory(&ch, &path, &schema).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();

        assert_eq!(lines.len(), 3, "Header + 2 items");
        assert_eq!(lines[0], "item,quantity,weight,notes");
        assert!(lines[1].contains("Longsword"), "First item: {}", lines[1]);
        assert!(lines[2].contains("Shield"), "Second item: {}", lines[2]);
    }

    #[test]
    fn test_write_inventory_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("inventory.csv");
        let mut ch = Character::new("Test");
        ch.add_item(
            CollectionItem::new("Potion")
                .with_property("quantity", "3")
                .with_property("weight", "0.5")
                .with_property("notes", "heals 2d4"),
        );

        let schema = inventory_schema();
        write_inventory(&ch, &path, &schema).unwrap();

        // Re-read
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let headers: Vec<String> = reader
            .headers()
            .unwrap()
            .iter()
            .map(|h| h.to_string())
            .collect();

        let record = reader.records().next().unwrap().unwrap();
        let mut row = std::collections::HashMap::new();
        for (i, val) in record.iter().enumerate() {
            row.insert(headers[i].clone(), val.to_string());
        }

        assert_eq!(row.get("item").unwrap(), "Potion");
        assert_eq!(row.get("quantity").unwrap(), "3");
        assert_eq!(row.get("weight").unwrap(), "0.5");
        assert_eq!(row.get("notes").unwrap(), "heals 2d4");
    }

    #[test]
    fn test_write_empty_inventory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("inventory.csv");
        let ch = Character::new("Empty");

        let schema = inventory_schema();
        write_inventory(&ch, &path, &schema).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 1, "Only header, no data rows");
    }

    #[test]
    fn test_write_missing_stat_produces_empty_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sheet.csv");
        // Character with only name and strength — missing other columns
        let mut ch = Character::new("Minimal");
        ch.stats = vec![Stat::new("strength", 10.0)];

        // Schema expects more columns
        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_number("strength"),
            ColumnDef::new("charisma", ColumnType::Number, false),
        ]);

        write_character_sheet(&ch, &path, &schema).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines[0], "name,strength,charisma");
        // charisma should be empty since it's optional
        assert!(
            lines[1].ends_with(','),
            "Missing optional stat should be empty: {}",
            lines[1]
        );
    }

    #[test]
    fn test_write_fractional_values() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sheet.csv");
        let mut ch = Character::new("Half");
        ch.stats = vec![Stat::new("score", 3.5)];

        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_number("score"),
        ]);

        write_character_sheet(&ch, &path, &schema).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("3.5"),
            "Fractional value should be preserved: {content}"
        );
    }
}
