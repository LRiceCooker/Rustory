use std::collections::HashMap;
use std::path::Path;

/// A loaded LOLCODE script representing a custom command.
#[derive(Debug, Clone, PartialEq)]
pub struct LolScript {
    /// Command name (filename without extension, lowercased)
    pub name: String,
    /// Raw LOLCODE source code
    pub source: String,
}

/// Scan `rules/commands/` within a campaign directory for `.lol` files.
///
/// Returns a map from command name (lowercase, no extension) to `LolScript`.
/// Non-`.lol` files are silently ignored. Missing directory returns an empty map.
pub fn load_custom_commands(campaign_path: &Path) -> HashMap<String, LolScript> {
    let commands_dir = campaign_path.join("rules").join("commands");
    let mut commands = HashMap::new();

    let entries = match std::fs::read_dir(&commands_dir) {
        Ok(entries) => entries,
        Err(_) => return commands,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str());
        if extension != Some("lol") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_lowercase(),
            None => continue,
        };
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        commands.insert(
            name.clone(),
            LolScript {
                name,
                source,
            },
        );
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_campaign_with_commands(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let commands_dir = dir.path().join("rules").join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        for (name, content) in files {
            fs::write(commands_dir.join(name), content).unwrap();
        }
        dir
    }

    #[test]
    fn test_load_lol_files() {
        let dir = create_campaign_with_commands(&[
            ("smite.lol", "HAI 1.2\nVISIBLE \"smite!\"\nKTHXBYE"),
            ("heal.lol", "HAI 1.2\nVISIBLE \"heal!\"\nKTHXBYE"),
        ]);
        let commands = load_custom_commands(dir.path());
        assert_eq!(commands.len(), 2);
        assert!(commands.contains_key("smite"));
        assert!(commands.contains_key("heal"));
        assert!(commands["smite"].source.contains("smite!"));
        assert!(commands["heal"].source.contains("heal!"));
    }

    #[test]
    fn test_ignores_non_lol_files() {
        let dir = create_campaign_with_commands(&[
            ("smite.lol", "HAI 1.2\nKTHXBYE"),
            ("readme.txt", "not a script"),
            ("notes.md", "# notes"),
            ("data.json", "{}"),
        ]);
        let commands = load_custom_commands(dir.path());
        assert_eq!(commands.len(), 1);
        assert!(commands.contains_key("smite"));
    }

    #[test]
    fn test_empty_commands_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let commands_dir = dir.path().join("rules").join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        let commands = load_custom_commands(dir.path());
        assert!(commands.is_empty());
    }

    #[test]
    fn test_missing_commands_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let commands = load_custom_commands(dir.path());
        assert!(commands.is_empty());
    }

    #[test]
    fn test_missing_rules_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let commands = load_custom_commands(dir.path());
        assert!(commands.is_empty());
    }

    #[test]
    fn test_command_name_is_lowercased() {
        let dir = create_campaign_with_commands(&[
            ("Smite.lol", "HAI 1.2\nKTHXBYE"),
            ("HEAL.lol", "HAI 1.2\nKTHXBYE"),
        ]);
        let commands = load_custom_commands(dir.path());
        assert!(commands.contains_key("smite"));
        assert!(commands.contains_key("heal"));
    }

    #[test]
    fn test_script_source_preserved() {
        let source = "HAI 1.2\nI HAS A X ITZ 42\nVISIBLE X\nKTHXBYE";
        let dir = create_campaign_with_commands(&[("test.lol", source)]);
        let commands = load_custom_commands(dir.path());
        assert_eq!(commands["test"].source, source);
        assert_eq!(commands["test"].name, "test");
    }

    #[test]
    fn test_ignores_subdirectories() {
        let dir = tempfile::TempDir::new().unwrap();
        let commands_dir = dir.path().join("rules").join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(
            commands_dir.join("valid.lol"),
            "HAI 1.2\nKTHXBYE",
        )
        .unwrap();
        // Create a subdirectory (should be ignored, not crash)
        fs::create_dir_all(commands_dir.join("subdir")).unwrap();
        let commands = load_custom_commands(dir.path());
        assert_eq!(commands.len(), 1);
        assert!(commands.contains_key("valid"));
    }
}
