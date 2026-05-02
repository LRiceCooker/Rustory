use std::path::Path;

use super::errors::ValidationError;

/// The type of a CSV column value.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    String,
    Number,
    Bool,
}

impl std::fmt::Display for ColumnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnType::String => write!(f, "String"),
            ColumnType::Number => write!(f, "Number"),
            ColumnType::Bool => write!(f, "Bool"),
        }
    }
}

/// Definition of a single CSV column.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
    pub required: bool,
}

impl ColumnDef {
    pub fn new(name: &str, col_type: ColumnType, required: bool) -> Self {
        Self {
            name: name.to_string(),
            col_type,
            required,
        }
    }

    /// Shorthand: required string column.
    pub fn required_string(name: &str) -> Self {
        Self::new(name, ColumnType::String, true)
    }

    /// Shorthand: required number column.
    pub fn required_number(name: &str) -> Self {
        Self::new(name, ColumnType::Number, true)
    }

    /// Shorthand: required bool column.
    pub fn required_bool(name: &str) -> Self {
        Self::new(name, ColumnType::Bool, true)
    }

    /// Shorthand: optional column.
    pub fn optional(name: &str, col_type: ColumnType) -> Self {
        Self::new(name, col_type, false)
    }
}

/// Schema describing the expected structure of a CSV file.
#[derive(Debug, Clone)]
pub struct CsvSchema {
    pub columns: Vec<ColumnDef>,
}

impl CsvSchema {
    pub fn new(columns: Vec<ColumnDef>) -> Self {
        Self { columns }
    }

    /// Get the list of expected column names.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }

    /// Get the list of required column names.
    pub fn required_column_names(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| c.required)
            .map(|c| c.name.as_str())
            .collect()
    }
}

/// Validate a CSV file at `path` against the given `schema`.
///
/// Checks:
/// - All required columns are present in the header row
/// - No unexpected/extra columns beyond those defined in the schema
/// - Value types match for each column (Number columns must parse as f64, Bool as true/false)
///
/// Returns `Ok(())` if valid, or `Err(Vec<ValidationError>)` with all detected issues.
pub fn validate_csv(path: &Path, schema: &CsvSchema) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let mut reader = match csv::Reader::from_path(path) {
        Ok(r) => r,
        Err(e) => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: None,
                expected: "a readable CSV file".to_string(),
                found: format!("{e}"),
                suggestion: "Check that the file exists and is valid CSV.".to_string(),
            });
            return Err(errors);
        }
    };

    let headers: Vec<String> = match reader.headers() {
        Ok(h) => h.iter().map(|s| s.trim().to_string()).collect(),
        Err(e) => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: Some(1),
                expected: "a valid CSV header row".to_string(),
                found: format!("{e}"),
                suggestion: "Ensure the first row contains column names.".to_string(),
            });
            return Err(errors);
        }
    };

    let expected_names = schema.column_names();
    let expected_display = expected_names.join(", ");
    let found_display = headers.join(", ");

    // Check for missing required columns
    for col_def in &schema.columns {
        if col_def.required && !headers.iter().any(|h| h == &col_def.name) {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: Some(1),
                expected: format!(
                    "column \"{}\"\n  Expected columns: {expected_display}",
                    col_def.name
                ),
                found: format!("columns: {found_display}"),
                suggestion: format!("Add the \"{}\" column to the CSV file.", col_def.name),
            });
        }
    }

    // Check for extra columns not in the schema
    for header in &headers {
        if !schema.columns.iter().any(|c| c.name == *header) {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: Some(1),
                expected: format!("only columns: {expected_display}"),
                found: format!("unexpected column \"{header}\""),
                suggestion: format!(
                    "Remove the \"{header}\" column, or add it to the schema."
                ),
            });
        }
    }

    // Build a lookup: header name -> ColumnDef for type checking
    let col_lookup: std::collections::HashMap<&str, &ColumnDef> = schema
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // Validate data rows for type correctness
    for (row_idx, result) in reader.records().enumerate() {
        let line_number = row_idx + 2; // +1 for 0-index, +1 for header row
        match result {
            Ok(record) => {
                for (col_idx, value) in record.iter().enumerate() {
                    if col_idx >= headers.len() {
                        continue;
                    }
                    let header = &headers[col_idx];
                    let value = value.trim();

                    if let Some(col_def) = col_lookup.get(header.as_str()) {
                        // Skip empty values for optional columns
                        if value.is_empty() && !col_def.required {
                            continue;
                        }

                        match col_def.col_type {
                            ColumnType::Number => {
                                if !value.is_empty() && value.parse::<f64>().is_err() {
                                    errors.push(ValidationError {
                                        file: path.to_path_buf(),
                                        line: Some(line_number),
                                        expected: format!(
                                            "a number for column \"{}\"",
                                            col_def.name
                                        ),
                                        found: format!("\"{value}\""),
                                        suggestion: format!(
                                            "Fix the value in column \"{}\" on row {} to be a valid number.",
                                            col_def.name,
                                            line_number - 1
                                        ),
                                    });
                                }
                            }
                            ColumnType::Bool => {
                                if !value.is_empty()
                                    && !matches!(
                                        value.to_lowercase().as_str(),
                                        "true" | "false" | "1" | "0" | "yes" | "no"
                                    )
                                {
                                    errors.push(ValidationError {
                                        file: path.to_path_buf(),
                                        line: Some(line_number),
                                        expected: format!(
                                            "a boolean for column \"{}\" (true/false/yes/no/1/0)",
                                            col_def.name
                                        ),
                                        found: format!("\"{value}\""),
                                        suggestion: format!(
                                            "Fix the value in column \"{}\" on row {} to be true or false.",
                                            col_def.name,
                                            line_number - 1
                                        ),
                                    });
                                }
                            }
                            ColumnType::String => {
                                // Strings always pass type validation
                            }
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(ValidationError {
                    file: path.to_path_buf(),
                    line: Some(line_number),
                    expected: "a valid CSV row".to_string(),
                    found: format!("{e}"),
                    suggestion: "Check that all rows have the correct number of columns."
                        .to_string(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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

    fn dnd_schema() -> CsvSchema {
        CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_string("class"),
            ColumnDef::required_number("level"),
            ColumnDef::required_number("strength"),
            ColumnDef::required_number("dexterity"),
            ColumnDef::required_number("constitution"),
            ColumnDef::required_number("intelligence"),
            ColumnDef::required_number("wisdom"),
            ColumnDef::required_number("charisma"),
            ColumnDef::required_number("hp_max"),
            ColumnDef::required_number("ac"),
        ])
    }

    #[test]
    fn test_valid_csv_passes() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        fs::write(
            &path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,charisma,hp_max,ac\n\
             Thorin,Fighter,5,18,12,16,10,13,8,52,18\n",
        )
        .unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_ok(), "Expected valid CSV to pass: {result:?}");
    }

    #[test]
    fn test_missing_required_column_detected() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // Missing "charisma" column
        fs::write(
            &path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,hp_max,ac\n\
             Thorin,Fighter,5,18,12,16,10,13,52,18\n",
        )
        .unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.expected.contains("charisma")));
        assert!(errors
            .iter()
            .any(|e| e.suggestion.contains("Add the \"charisma\" column")));
    }

    #[test]
    fn test_wrong_type_detected() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // "hp_max" has non-numeric value "lots"
        fs::write(
            &path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,charisma,hp_max,ac\n\
             Thorin,Fighter,5,18,12,16,10,13,8,lots,18\n",
        )
        .unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.expected.contains("a number")
            && e.expected.contains("hp_max")
            && e.found.contains("lots")));
    }

    #[test]
    fn test_extra_column_detected() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // Extra "luck" column not in schema
        fs::write(
            &path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,charisma,hp_max,ac,luck\n\
             Thorin,Fighter,5,18,12,16,10,13,8,52,18,7\n",
        )
        .unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.found.contains("luck")));
        assert!(errors
            .iter()
            .any(|e| e.suggestion.contains("Remove the \"luck\" column")));
    }

    #[test]
    fn test_error_messages_are_human_readable() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // Missing charisma AND hp_max is non-numeric
        fs::write(
            &path,
            "name,class,level,strength,dexterity,constitution,intelligence,wisdom,hp_max,ac\n\
             Thorin,Fighter,5,18,12,16,10,13,nope,18\n",
        )
        .unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();

        // Should have at least 2 errors: missing column + wrong type
        assert!(errors.len() >= 2);

        // Check that each error has a display with file path, expected, found, suggestion
        for err in &errors {
            let display = format!("{err}");
            assert!(display.contains("sheet.csv"));
            assert!(display.contains("Expected:"));
            assert!(display.contains("Found:"));
            assert!(display.contains("\u{2192}")); // → arrow
        }
    }

    #[test]
    fn test_multiple_missing_columns() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // Only name and class — missing everything else
        fs::write(&path, "name,class\nThorin,Fighter\n").unwrap();

        let result = validate_csv(&path, &dnd_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();

        // Should have errors for each missing required numeric column
        let missing_names: Vec<&str> = errors
            .iter()
            .filter(|e| e.suggestion.starts_with("Add the"))
            .map(|e| {
                // Extract column name from suggestion
                let start = e.suggestion.find('"').unwrap() + 1;
                let end = e.suggestion[start..].find('"').unwrap() + start;
                &e.suggestion[start..end]
            })
            .collect();

        assert!(missing_names.contains(&"level"));
        assert!(missing_names.contains(&"strength"));
        assert!(missing_names.contains(&"charisma"));
        assert!(missing_names.contains(&"hp_max"));
    }

    #[test]
    fn test_bool_column_validation() {
        let dir = test_dir();
        let path = dir.path().join("data.csv");
        fs::write(&path, "name,active\nAlice,true\nBob,maybe\n").unwrap();

        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_bool("active"),
        ]);

        let result = validate_csv(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.expected.contains("boolean")
            && e.found.contains("maybe")));
    }

    #[test]
    fn test_bool_accepts_valid_values() {
        let dir = test_dir();
        let path = dir.path().join("data.csv");
        fs::write(
            &path,
            "name,active\nA,true\nB,false\nC,yes\nD,no\nE,1\nF,0\n",
        )
        .unwrap();

        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_bool("active"),
        ]);

        assert!(validate_csv(&path, &schema).is_ok());
    }

    #[test]
    fn test_optional_column_missing_is_ok() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        // Schema has optional "notes" but CSV doesn't have it — that's fine
        fs::write(&path, "name,level\nHero,5\n").unwrap();

        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_number("level"),
            ColumnDef::optional("notes", ColumnType::String),
        ]);

        assert!(validate_csv(&path, &schema).is_ok());
    }

    #[test]
    fn test_nonexistent_file() {
        let path = Path::new("/tmp/nonexistent_csv_file_12345.csv");
        let schema = CsvSchema::new(vec![ColumnDef::required_string("name")]);

        let result = validate_csv(path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].suggestion.contains("Check that the file exists"));
    }

    #[test]
    fn test_empty_schema_accepts_any_csv() {
        let dir = test_dir();
        let path = dir.path().join("any.csv");
        fs::write(&path, "foo,bar,baz\n1,2,3\n").unwrap();

        // Empty schema means no required columns and no type checks,
        // but extra columns ARE flagged
        let schema = CsvSchema::new(vec![]);
        let result = validate_csv(&path, &schema);
        // Extra columns detected
        assert!(result.is_err());
    }

    #[test]
    fn test_type_error_includes_line_number() {
        let dir = test_dir();
        let path = dir.path().join("sheet.csv");
        fs::write(
            &path,
            "name,score\nAlice,100\nBob,good\nCharlie,50\n",
        )
        .unwrap();

        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_number("score"),
        ]);

        let result = validate_csv(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        // "good" is on row 2 (data row), which is line 3 in the file
        assert!(errors.iter().any(|e| e.line == Some(3)));
    }

    #[test]
    fn test_column_names_helper() {
        let schema = dnd_schema();
        let names = schema.column_names();
        assert_eq!(names[0], "name");
        assert_eq!(names[1], "class");
        assert_eq!(names.len(), 11);
    }

    #[test]
    fn test_required_column_names_helper() {
        let schema = CsvSchema::new(vec![
            ColumnDef::required_string("name"),
            ColumnDef::required_number("level"),
            ColumnDef::optional("notes", ColumnType::String),
        ]);
        let required = schema.required_column_names();
        assert_eq!(required, vec!["name", "level"]);
    }
}
