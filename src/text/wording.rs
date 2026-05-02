use std::fs;
use std::path::Path;
use toml::Value;

#[allow(dead_code)]
pub fn load(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read wording file {}: {e}", path.display()))?;
    content
        .parse::<Value>()
        .map_err(|e| format!("Failed to parse wording file {}: {e}", path.display()))
}

#[allow(dead_code)]
pub fn get<'a>(wording: &'a Value, section: &str, key: &str) -> Result<&'a str, String> {
    wording
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing wording key: {section}.{key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from("assets/wording/eng.toml")
    }

    #[test]
    fn test_load_english() {
        let wording = load(&fixture_path()).unwrap();
        assert!(wording.is_table());
    }

    #[test]
    fn test_access_nested_key() {
        let wording = load(&fixture_path()).unwrap();
        let result = get(&wording, "command_roll", "dice").unwrap();
        assert_eq!(result, "dice(s)");
    }

    #[test]
    fn test_missing_key_returns_error() {
        let wording = load(&fixture_path()).unwrap();
        let result = get(&wording, "nonexistent", "key");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing wording key"));
    }

    #[test]
    fn test_load_missing_file_returns_error() {
        let result = load(Path::new("nonexistent/file.toml"));
        assert!(result.is_err());
    }
}
