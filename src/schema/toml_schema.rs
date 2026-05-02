use std::path::Path;

use super::errors::ValidationError;

/// Expected type of a TOML value.
#[derive(Debug, Clone, PartialEq)]
pub enum TomlValueType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Table,
    /// An array where every element is a string (e.g., `columns = ["a", "b"]`).
    ArrayOfStrings,
}

impl std::fmt::Display for TomlValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TomlValueType::String => write!(f, "String"),
            TomlValueType::Integer => write!(f, "Integer"),
            TomlValueType::Float => write!(f, "Float"),
            TomlValueType::Boolean => write!(f, "Boolean"),
            TomlValueType::Array => write!(f, "Array"),
            TomlValueType::Table => write!(f, "Table"),
            TomlValueType::ArrayOfStrings => write!(f, "Array of Strings"),
        }
    }
}

/// Check whether a `toml::Value` matches the expected `TomlValueType`.
fn value_matches_type(value: &toml::Value, expected: &TomlValueType) -> bool {
    match expected {
        TomlValueType::String => value.is_str(),
        TomlValueType::Integer => value.is_integer(),
        TomlValueType::Float => value.is_float(),
        TomlValueType::Boolean => matches!(value, toml::Value::Boolean(_)),
        TomlValueType::Array => value.is_array(),
        TomlValueType::Table => value.is_table(),
        TomlValueType::ArrayOfStrings => {
            if let Some(arr) = value.as_array() {
                arr.iter().all(|v| v.is_str())
            } else {
                false
            }
        }
    }
}

/// Definition of a single key expected within a TOML section.
#[derive(Debug, Clone)]
pub struct KeyDef {
    pub name: String,
    pub value_type: TomlValueType,
    pub required: bool,
}

impl KeyDef {
    pub fn required(name: &str, value_type: TomlValueType) -> Self {
        Self {
            name: name.to_string(),
            value_type,
            required: true,
        }
    }

    pub fn optional(name: &str, value_type: TomlValueType) -> Self {
        Self {
            name: name.to_string(),
            value_type,
            required: false,
        }
    }
}

/// Definition of a TOML section (table).
///
/// The `name` field supports dotted paths (e.g., `"character.schema"`) and
/// wildcard suffixes (e.g., `"check.*"`) which match any sub-table under `check`.
#[derive(Debug, Clone)]
pub struct SectionDef {
    pub name: String,
    pub required: bool,
    pub keys: Vec<KeyDef>,
}

impl SectionDef {
    pub fn required(name: &str, keys: Vec<KeyDef>) -> Self {
        Self {
            name: name.to_string(),
            required: true,
            keys,
        }
    }

    pub fn optional(name: &str, keys: Vec<KeyDef>) -> Self {
        Self {
            name: name.to_string(),
            required: false,
            keys,
        }
    }

    /// Returns true if this section definition uses a wildcard (`"check.*"`).
    fn is_wildcard(&self) -> bool {
        self.name.ends_with(".*")
    }

    /// Returns the prefix before the wildcard (e.g., `"check"` for `"check.*"`).
    fn wildcard_prefix(&self) -> &str {
        &self.name[..self.name.len() - 2]
    }
}

/// Schema for validating TOML files.
///
/// A `TomlSchema` describes expected sections (tables) and their keys.
/// It can validate a TOML file and return user-friendly `ValidationError`s.
#[derive(Debug, Clone)]
pub struct TomlSchema {
    pub sections: Vec<SectionDef>,
}

impl TomlSchema {
    pub fn new(sections: Vec<SectionDef>) -> Self {
        Self { sections }
    }
}

/// Navigate a dotted path (e.g., `"character.schema"`) through nested TOML tables.
fn resolve_path<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut current = root;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Validate a TOML file at `path` against the given `schema`.
///
/// Checks:
/// - Required sections are present
/// - Required keys within sections are present
/// - Key values match their expected types
///
/// Returns `Ok(())` if valid, or `Err(Vec<ValidationError>)` with all detected issues.
pub fn validate_toml(path: &Path, schema: &TomlSchema) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: None,
                expected: "a readable TOML file".to_string(),
                found: format!("{e}"),
                suggestion: "Check that the file exists and is valid UTF-8.".to_string(),
            });
            return Err(errors);
        }
    };

    let root: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(e) => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: None,
                expected: "valid TOML syntax".to_string(),
                found: format!("{e}"),
                suggestion: "Fix the TOML syntax error in the file.".to_string(),
            });
            return Err(errors);
        }
    };

    for section_def in &schema.sections {
        if section_def.is_wildcard() {
            validate_wildcard_section(path, &root, section_def, &mut errors);
        } else {
            validate_named_section(path, &root, section_def, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate a concrete (non-wildcard) section.
fn validate_named_section(
    path: &Path,
    root: &toml::Value,
    section_def: &SectionDef,
    errors: &mut Vec<ValidationError>,
) {
    let section_value = resolve_path(root, &section_def.name);

    match section_value {
        None if section_def.required => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: None,
                expected: format!("section [{}]", section_def.name),
                found: "section missing".to_string(),
                suggestion: format!(
                    "Add [{}] with the required keys to the TOML file.",
                    section_def.name
                ),
            });
        }
        None => {
            // Optional section not present — fine
        }
        Some(value) => {
            if !value.is_table() {
                errors.push(ValidationError {
                    file: path.to_path_buf(),
                    line: None,
                    expected: format!("[{}] to be a table/section", section_def.name),
                    found: value_type_name(value).to_string(),
                    suggestion: format!(
                        "Make [{}] a proper TOML section (table).",
                        section_def.name
                    ),
                });
                return;
            }
            validate_keys(path, value, &section_def.name, &section_def.keys, errors);
        }
    }
}

/// Validate a wildcard section (e.g., `"check.*"` matches all sub-tables under `check`).
fn validate_wildcard_section(
    path: &Path,
    root: &toml::Value,
    section_def: &SectionDef,
    errors: &mut Vec<ValidationError>,
) {
    let prefix = section_def.wildcard_prefix();
    let parent = resolve_path(root, prefix);

    match parent {
        None if section_def.required => {
            errors.push(ValidationError {
                file: path.to_path_buf(),
                line: None,
                expected: format!("at least one [{prefix}.<name>] section"),
                found: format!("[{prefix}] section missing"),
                suggestion: format!("Add at least one [{prefix}.<name>] section to the TOML file."),
            });
        }
        None => {}
        Some(value) => {
            if let Some(table) = value.as_table() {
                for (sub_name, sub_value) in table {
                    let full_name = format!("{prefix}.{sub_name}");
                    if !sub_value.is_table() {
                        errors.push(ValidationError {
                            file: path.to_path_buf(),
                            line: None,
                            expected: format!("[{full_name}] to be a table/section"),
                            found: value_type_name(sub_value).to_string(),
                            suggestion: format!("Make [{full_name}] a proper TOML section."),
                        });
                        continue;
                    }
                    validate_keys(path, sub_value, &full_name, &section_def.keys, errors);
                }
            }
        }
    }
}

/// Validate keys within a resolved section value.
fn validate_keys(
    path: &Path,
    section_value: &toml::Value,
    section_name: &str,
    keys: &[KeyDef],
    errors: &mut Vec<ValidationError>,
) {
    for key_def in keys {
        let key_value = section_value.get(&key_def.name);

        match key_value {
            None if key_def.required => {
                errors.push(ValidationError {
                    file: path.to_path_buf(),
                    line: None,
                    expected: format!(
                        "key \"{}\" ({}) in [{}]",
                        key_def.name, key_def.value_type, section_name
                    ),
                    found: "key missing".to_string(),
                    suggestion: format!(
                        "Add \"{}\" to the [{}] section.",
                        key_def.name, section_name
                    ),
                });
            }
            None => {}
            Some(value) => {
                if !value_matches_type(value, &key_def.value_type) {
                    errors.push(ValidationError {
                        file: path.to_path_buf(),
                        line: None,
                        expected: format!(
                            "{} for key \"{}\" in [{}]",
                            key_def.value_type, key_def.name, section_name
                        ),
                        found: format!("{} ({})", value_type_name(value), value),
                        suggestion: format!(
                            "Change \"{}\" in [{}] to be a valid {}.",
                            key_def.name, section_name, key_def.value_type
                        ),
                    });
                }
            }
        }
    }
}

/// Human-readable name for a `toml::Value` variant.
fn value_type_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "String",
        toml::Value::Integer(_) => "Integer",
        toml::Value::Float(_) => "Float",
        toml::Value::Boolean(_) => "Boolean",
        toml::Value::Datetime(_) => "Datetime",
        toml::Value::Array(_) => "Array",
        toml::Value::Table(_) => "Table",
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

    /// A minimal system.toml schema: requires [system] with name, and [character.schema] with columns.
    fn system_toml_schema() -> TomlSchema {
        TomlSchema::new(vec![
            SectionDef::required(
                "system",
                vec![
                    KeyDef::required("name", TomlValueType::String),
                    KeyDef::optional("version", TomlValueType::String),
                ],
            ),
            SectionDef::required(
                "character.schema",
                vec![KeyDef::required("columns", TomlValueType::ArrayOfStrings)],
            ),
            SectionDef::optional(
                "inventory.schema",
                vec![KeyDef::optional("columns", TomlValueType::ArrayOfStrings)],
            ),
        ])
    }

    #[test]
    fn test_valid_system_toml_passes() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[system]
name = "D&D 5e"
version = "1.0"

[character.schema]
columns = ["name", "class", "level", "strength"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_ok(), "Expected valid TOML to pass: {result:?}");
    }

    #[test]
    fn test_missing_required_section_detected() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // Missing [character.schema]
        fs::write(
            &path,
            r#"
[system]
name = "Test"
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("character.schema")));
        assert!(errors.iter().any(|e| e.found.contains("missing")));
    }

    #[test]
    fn test_missing_required_key_detected() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // [system] exists but missing "name" key
        fs::write(
            &path,
            r#"
[system]
version = "1.0"

[character.schema]
columns = ["name"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.expected.contains("\"name\"")
            && e.expected.contains("[system]")
            && e.found.contains("missing")));
    }

    #[test]
    fn test_wrong_value_type_detected() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // "name" should be String but is Integer
        fs::write(
            &path,
            r#"
[system]
name = 42

[character.schema]
columns = ["name"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("String") && e.found.contains("Integer")));
    }

    #[test]
    fn test_array_of_strings_rejects_mixed_array() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // columns has a non-string element
        fs::write(
            &path,
            r#"
[system]
name = "Test"

[character.schema]
columns = ["name", 42, "level"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("Array of Strings")));
    }

    #[test]
    fn test_optional_section_missing_is_ok() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // No [inventory.schema] — that's fine, it's optional
        fs::write(
            &path,
            r#"
[system]
name = "Test"

[character.schema]
columns = ["name"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_ok());
    }

    #[test]
    fn test_optional_key_missing_is_ok() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // No "version" in [system] — that's fine, it's optional
        fs::write(
            &path,
            r#"
[system]
name = "Test"

[character.schema]
columns = ["name"]
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_ok());
    }

    #[test]
    fn test_wildcard_section_validates_subtables() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[check.ability]
roll = "1d20"
success = "result >= dc"

[check.save]
roll = "1d20"
success = "result >= dc"
"#,
        )
        .unwrap();

        let schema = TomlSchema::new(vec![SectionDef::optional(
            "check.*",
            vec![
                KeyDef::required("roll", TomlValueType::String),
                KeyDef::required("success", TomlValueType::String),
            ],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(
            result.is_ok(),
            "Expected valid wildcard sections: {result:?}"
        );
    }

    #[test]
    fn test_wildcard_section_missing_key_in_subtable() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(
            &path,
            r#"
[check.ability]
roll = "1d20"

[check.save]
roll = "1d20"
success = "result >= dc"
"#,
        )
        .unwrap();

        let schema = TomlSchema::new(vec![SectionDef::optional(
            "check.*",
            vec![
                KeyDef::required("roll", TomlValueType::String),
                KeyDef::required("success", TomlValueType::String),
            ],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("\"success\"") && e.expected.contains("[check.ability]")));
    }

    #[test]
    fn test_required_wildcard_section_missing() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        fs::write(&path, "# empty\n").unwrap();

        let schema = TomlSchema::new(vec![SectionDef::required(
            "check.*",
            vec![KeyDef::required("roll", TomlValueType::String)],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.expected.contains("[check.<name>]")));
    }

    #[test]
    fn test_invalid_toml_syntax() {
        let dir = test_dir();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "[system\nname = broken").unwrap();

        let schema = TomlSchema::new(vec![]);
        let result = validate_toml(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].expected.contains("valid TOML syntax"));
    }

    #[test]
    fn test_nonexistent_file() {
        let path = Path::new("/tmp/nonexistent_toml_file_12345.toml");
        let schema = TomlSchema::new(vec![]);

        let result = validate_toml(path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].suggestion.contains("Check that the file exists"));
    }

    #[test]
    fn test_error_messages_are_human_readable() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // Missing both [system] and [character.schema]
        fs::write(&path, "# empty file\n").unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 2);

        for err in &errors {
            let display = format!("{err}");
            assert!(display.contains("system.toml"));
            assert!(display.contains("Expected:"));
            assert!(display.contains("Found:"));
            assert!(display.contains("\u{2192}")); // → arrow
        }
    }

    #[test]
    fn test_boolean_type_validation() {
        let dir = test_dir();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[settings]
debug = true
verbose = "yes"
"#,
        )
        .unwrap();

        let schema = TomlSchema::new(vec![SectionDef::required(
            "settings",
            vec![
                KeyDef::required("debug", TomlValueType::Boolean),
                KeyDef::required("verbose", TomlValueType::Boolean),
            ],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        // "verbose" is a String, not Boolean
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("Boolean") && e.found.contains("String")));
    }

    #[test]
    fn test_multiple_errors_collected() {
        let dir = test_dir();
        let path = dir.path().join("system.toml");
        // [system] has wrong type for name, AND [character.schema] is missing
        fs::write(
            &path,
            r#"
[system]
name = 123
"#,
        )
        .unwrap();

        let result = validate_toml(&path, &system_toml_schema());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        // At least: wrong type for name + missing character.schema
        assert!(errors.len() >= 2);
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("String") && e.expected.contains("\"name\"")));
        assert!(errors
            .iter()
            .any(|e| e.expected.contains("character.schema")));
    }

    #[test]
    fn test_integer_and_float_types() {
        let dir = test_dir();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[stats]
count = 6
multiplier = 1.5
"#,
        )
        .unwrap();

        let schema = TomlSchema::new(vec![SectionDef::required(
            "stats",
            vec![
                KeyDef::required("count", TomlValueType::Integer),
                KeyDef::required("multiplier", TomlValueType::Float),
            ],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(result.is_ok(), "Expected valid types: {result:?}");
    }

    #[test]
    fn test_table_type_validation() {
        let dir = test_dir();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[parent]
[parent.child]
key = "value"
"#,
        )
        .unwrap();

        let schema = TomlSchema::new(vec![SectionDef::required(
            "parent",
            vec![KeyDef::required("child", TomlValueType::Table)],
        )]);

        let result = validate_toml(&path, &schema);
        assert!(result.is_ok(), "Expected table key to pass: {result:?}");
    }
}
