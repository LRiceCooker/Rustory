use std::fmt;
use std::path::PathBuf;

/// A validation error encountered when checking a file against its schema.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Path to the file that failed validation.
    pub file: PathBuf,
    /// Line number in the file where the error was found (if applicable).
    pub line: Option<usize>,
    /// What was expected (e.g., column name, type).
    pub expected: String,
    /// What was actually found.
    pub found: String,
    /// Human-readable suggestion for how to fix the error.
    pub suggestion: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error in {}:", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, " (line {line})")?;
        }
        write!(f, "\n  Expected: {}", self.expected)?;
        write!(f, "\n  Found: {}", self.found)?;
        write!(f, "\n  \u{2192} {}", self.suggestion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display_with_line() {
        let err = ValidationError {
            file: PathBuf::from("players/thorin/sheet.csv"),
            line: Some(2),
            expected: "a number for column \"hp_max\"".to_string(),
            found: "\"lots\"".to_string(),
            suggestion: "Fix the value in the CSV file.".to_string(),
        };
        let display = format!("{err}");
        assert!(display.contains("players/thorin/sheet.csv"));
        assert!(display.contains("line 2"));
        assert!(display.contains("a number"));
        assert!(display.contains("lots"));
        assert!(display.contains("Fix the value"));
    }

    #[test]
    fn test_validation_error_display_without_line() {
        let err = ValidationError {
            file: PathBuf::from("rules/system.toml"),
            line: None,
            expected: "section [character.schema]".to_string(),
            found: "section missing".to_string(),
            suggestion: "Add [character.schema] to system.toml.".to_string(),
        };
        let display = format!("{err}");
        assert!(display.contains("rules/system.toml"));
        assert!(!display.contains("line"));
        assert!(display.contains("[character.schema]"));
    }
}
