use std::path::{Path, PathBuf};

/// Supported audio file extensions.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac"];

/// A single entry in the sound library (file or directory).
#[derive(Debug, Clone)]
pub struct SoundEntry {
    /// Display name (filename or directory name)
    pub name: String,
    /// Path relative to the sound/ root
    pub path: String,
    /// Whether this entry is a directory
    pub is_dir: bool,
}

/// Sound library that indexes audio files from a campaign's `sound/` directory.
#[derive(Debug, Default)]
pub struct SoundLibrary {
    /// Root directory of the sound library
    root: PathBuf,
    /// All entries (files and directories), stored flat
    entries: Vec<SoundEntry>,
}

impl SoundLibrary {
    /// Scan a `sound/` directory recursively and build the library.
    /// Returns an empty library if the directory doesn't exist.
    pub fn scan(sound_dir: &Path) -> color_eyre::Result<Self> {
        let mut library = SoundLibrary {
            root: sound_dir.to_path_buf(),
            entries: Vec::new(),
        };

        if !sound_dir.is_dir() {
            return Ok(library);
        }

        library.scan_dir(sound_dir, "")?;
        library.entries.sort_by(|a, b| {
            // Directories first, then alphabetical
            b.is_dir.cmp(&a.is_dir).then(a.path.cmp(&b.path))
        });

        Ok(library)
    }

    /// Recursively scan a directory, building relative paths from the sound root.
    fn scan_dir(&mut self, dir: &Path, prefix: &str) -> color_eyre::Result<()> {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();

            // Skip hidden files
            if file_name.starts_with('.') {
                continue;
            }

            let rel_path = if prefix.is_empty() {
                file_name.clone()
            } else {
                format!("{prefix}/{file_name}")
            };

            if path.is_dir() {
                self.entries.push(SoundEntry {
                    name: file_name.clone(),
                    path: rel_path.clone(),
                    is_dir: true,
                });
                self.scan_dir(&path, &rel_path)?;
            } else if is_audio_file(&path) {
                self.entries.push(SoundEntry {
                    name: file_name,
                    path: rel_path,
                    is_dir: false,
                });
            }
        }

        Ok(())
    }

    /// List entries in a subfolder (or root if None).
    /// Returns only direct children of the specified folder.
    pub fn list(&self, subfolder: Option<&str>) -> Vec<&SoundEntry> {
        let prefix = subfolder.unwrap_or("");
        self.entries
            .iter()
            .filter(|e| {
                let parent = parent_path(&e.path);
                parent == prefix
            })
            .collect()
    }

    /// Fuzzy search for entries by filename (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&SoundEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| !e.is_dir && e.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Resolve a relative path to its full filesystem path.
    /// Returns None if the file doesn't exist or isn't an audio file.
    pub fn resolve(&self, rel_path: &str) -> Option<PathBuf> {
        let full = self.root.join(rel_path);
        if full.is_file() && is_audio_file(&full) {
            Some(full)
        } else {
            None
        }
    }

    /// Returns true if the library has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total number of audio files (excluding directories).
    pub fn file_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_dir).count()
    }
}

/// Check if a file has a supported audio extension.
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Extract the parent path portion from a relative path string.
/// e.g., "combat/battle.mp3" -> "combat", "tavern.mp3" -> ""
fn parent_path(rel_path: &str) -> &str {
    match rel_path.rfind('/') {
        Some(idx) => &rel_path[..idx],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a test sound directory structure:
    /// sound/
    ///   ambiance/
    ///     tavern.mp3
    ///     forest.ogg
    ///   combat/
    ///     battle.wav
    ///   theme.flac
    ///   .hidden_file
    fn create_test_sound_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("ambiance")).unwrap();
        fs::create_dir_all(root.join("combat")).unwrap();

        fs::write(root.join("ambiance/tavern.mp3"), b"fake").unwrap();
        fs::write(root.join("ambiance/forest.ogg"), b"fake").unwrap();
        fs::write(root.join("combat/battle.wav"), b"fake").unwrap();
        fs::write(root.join("theme.flac"), b"fake").unwrap();
        fs::write(root.join(".hidden_file"), b"fake").unwrap();
        // Non-audio file should be ignored
        fs::write(root.join("readme.txt"), b"not audio").unwrap();

        dir
    }

    #[test]
    fn test_scan_nested_folders() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        // 2 directories + 4 audio files
        assert_eq!(lib.entries.len(), 6);
        assert_eq!(lib.file_count(), 4);
        assert!(!lib.is_empty());
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let lib = SoundLibrary::scan(Path::new("/nonexistent/sound")).unwrap();
        assert!(lib.is_empty());
        assert_eq!(lib.file_count(), 0);
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = TempDir::new().unwrap();
        let lib = SoundLibrary::scan(dir.path()).unwrap();
        assert!(lib.is_empty());
    }

    #[test]
    fn test_list_root_shows_folders_and_root_files() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let root_entries = lib.list(None);
        let names: Vec<&str> = root_entries.iter().map(|e| e.name.as_str()).collect();

        // Should have directories (ambiance, combat) and root file (theme.flac)
        assert!(names.contains(&"ambiance"));
        assert!(names.contains(&"combat"));
        assert!(names.contains(&"theme.flac"));
        // Hidden and non-audio files excluded
        assert!(!names.contains(&".hidden_file"));
        assert!(!names.contains(&"readme.txt"));
    }

    #[test]
    fn test_list_subfolder_shows_files() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let ambiance = lib.list(Some("ambiance"));
        let names: Vec<&str> = ambiance.iter().map(|e| e.name.as_str()).collect();

        assert_eq!(ambiance.len(), 2);
        assert!(names.contains(&"tavern.mp3"));
        assert!(names.contains(&"forest.ogg"));
    }

    #[test]
    fn test_list_nonexistent_subfolder_returns_empty() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let result = lib.list(Some("nonexistent"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_search_finds_partial_matches() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.search("tav");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "tavern.mp3");
    }

    #[test]
    fn test_search_case_insensitive() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.search("BATTLE");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "battle.wav");
    }

    #[test]
    fn test_search_no_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_excludes_directories() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        // "ambiance" is a directory name, search should not return it
        let results = lib.search("ambiance");
        assert!(results.is_empty());
    }

    #[test]
    fn test_resolve_existing_audio_file() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let resolved = lib.resolve("ambiance/tavern.mp3");
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("ambiance/tavern.mp3"));
    }

    #[test]
    fn test_resolve_missing_file_returns_none() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        assert!(lib.resolve("missing.mp3").is_none());
    }

    #[test]
    fn test_resolve_non_audio_file_returns_none() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        assert!(lib.resolve("readme.txt").is_none());
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file(Path::new("test.mp3")));
        assert!(is_audio_file(Path::new("test.wav")));
        assert!(is_audio_file(Path::new("test.ogg")));
        assert!(is_audio_file(Path::new("test.flac")));
        assert!(is_audio_file(Path::new("test.MP3"))); // case-insensitive
        assert!(!is_audio_file(Path::new("test.txt")));
        assert!(!is_audio_file(Path::new("test")));
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("combat/battle.mp3"), "combat");
        assert_eq!(parent_path("a/b/c.wav"), "a/b");
        assert_eq!(parent_path("theme.flac"), "");
    }

    #[test]
    fn test_directories_sorted_first() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let root_entries = lib.list(None);
        // Directories should come before files
        let dir_indices: Vec<usize> = root_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_dir)
            .map(|(i, _)| i)
            .collect();
        let file_indices: Vec<usize> = root_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_dir)
            .map(|(i, _)| i)
            .collect();

        if let (Some(&last_dir), Some(&first_file)) = (dir_indices.last(), file_indices.first()) {
            assert!(
                last_dir < first_file,
                "Directories should sort before files"
            );
        }
    }
}
