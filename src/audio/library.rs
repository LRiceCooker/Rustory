use std::path::{Path, PathBuf};

/// Result of a fuzzy sound file resolution.
#[derive(Debug)]
pub enum FuzzyResult {
    /// Exactly one match found: (full filesystem path, relative display path).
    Found(PathBuf, String),
    /// Multiple matches — user must be more specific.
    Ambiguous(Vec<String>),
    /// No match found.
    NotFound,
}

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

    /// Try to resolve a sound file by exact path first, then by fuzzy filename match.
    /// Returns `Ok(path)` if exactly one match, `Err(matches)` if ambiguous (multiple matches),
    /// or `Ok(None)` wrapped as empty vec if no match.
    pub fn resolve_fuzzy(&self, query: &str) -> FuzzyResult {
        // 1. Try exact path resolution first
        if let Some(full_path) = self.resolve(query) {
            return FuzzyResult::Found(full_path, query.to_string());
        }

        // 2. Fall back to case-insensitive substring search on filenames
        let matches: Vec<&SoundEntry> = self.search(query);

        match matches.len() {
            0 => FuzzyResult::NotFound,
            1 => {
                let entry = matches[0];
                // resolve using the entry's relative path
                match self.resolve(&entry.path) {
                    Some(full_path) => FuzzyResult::Found(full_path, entry.path.clone()),
                    None => FuzzyResult::NotFound,
                }
            }
            _ => {
                let paths: Vec<String> = matches.iter().map(|e| e.path.clone()).collect();
                FuzzyResult::Ambiguous(paths)
            }
        }
    }

    /// Return audio file paths matching a prefix (case-insensitive).
    /// Used for `sound play` / `sound loop` autocomplete.
    pub fn complete_paths(&self, partial: &str) -> Vec<String> {
        let lower_partial = partial.to_lowercase();
        self.entries
            .iter()
            .filter(|e| !e.is_dir && e.path.to_lowercase().starts_with(&lower_partial))
            .map(|e| e.path.clone())
            .collect()
    }

    /// Return subfolder paths matching a prefix (case-insensitive).
    /// Used for `sound list` autocomplete.
    pub fn complete_subfolders(&self, partial: &str) -> Vec<String> {
        let lower_partial = partial.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.is_dir && e.path.to_lowercase().starts_with(&lower_partial))
            .map(|e| e.path.clone())
            .collect()
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

    #[test]
    fn test_resolve_fuzzy_exact_path() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        match lib.resolve_fuzzy("ambiance/tavern.mp3") {
            FuzzyResult::Found(full, rel) => {
                assert!(full.ends_with("ambiance/tavern.mp3"));
                assert_eq!(rel, "ambiance/tavern.mp3");
            }
            other => panic!("Expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_fuzzy_filename_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        // "tavern" should find "ambiance/tavern.mp3"
        match lib.resolve_fuzzy("tavern") {
            FuzzyResult::Found(full, rel) => {
                assert!(full.ends_with("ambiance/tavern.mp3"));
                assert_eq!(rel, "ambiance/tavern.mp3");
            }
            other => panic!("Expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_fuzzy_case_insensitive() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        match lib.resolve_fuzzy("BATTLE") {
            FuzzyResult::Found(_, rel) => {
                assert_eq!(rel, "combat/battle.wav");
            }
            other => panic!("Expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_fuzzy_ambiguous() {
        let dir = create_test_sound_dir();
        // Add a second file that matches "t" — both tavern.mp3 and theme.flac match
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        match lib.resolve_fuzzy("t") {
            FuzzyResult::Ambiguous(matches) => {
                assert!(
                    matches.len() >= 2,
                    "Expected at least 2 matches, got {matches:?}"
                );
            }
            other => panic!("Expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_fuzzy_not_found() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        match lib.resolve_fuzzy("nonexistent") {
            FuzzyResult::NotFound => {}
            other => panic!("Expected NotFound, got {other:?}"),
        }
    }

    // --- complete_paths tests ---

    #[test]
    fn test_complete_paths_prefix_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("ambiance/");
        assert!(results.contains(&"ambiance/tavern.mp3".to_string()));
        assert!(results.contains(&"ambiance/forest.ogg".to_string()));
        assert!(!results.iter().any(|r| r.starts_with("combat/")));
    }

    #[test]
    fn test_complete_paths_partial_filename() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("ambiance/t");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "ambiance/tavern.mp3");
    }

    #[test]
    fn test_complete_paths_case_insensitive() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("AMBIANCE/");
        assert!(results.contains(&"ambiance/tavern.mp3".to_string()));
    }

    #[test]
    fn test_complete_paths_empty_partial_returns_all_files() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("");
        assert_eq!(results.len(), 4); // 4 audio files total
    }

    #[test]
    fn test_complete_paths_excludes_directories() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("ambiance");
        // Should match files like "ambiance/tavern.mp3", not the directory "ambiance" itself
        assert!(results.iter().all(|r| r.contains('/')));
    }

    #[test]
    fn test_complete_paths_no_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_paths("nonexistent/");
        assert!(results.is_empty());
    }

    // --- complete_subfolders tests ---

    #[test]
    fn test_complete_subfolders_prefix_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_subfolders("a");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "ambiance");
    }

    #[test]
    fn test_complete_subfolders_empty_returns_all_dirs() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_subfolders("");
        assert_eq!(results.len(), 2); // ambiance, combat
        assert!(results.contains(&"ambiance".to_string()));
        assert!(results.contains(&"combat".to_string()));
    }

    #[test]
    fn test_complete_subfolders_no_match() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_subfolders("zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_complete_subfolders_excludes_files() {
        let dir = create_test_sound_dir();
        let lib = SoundLibrary::scan(dir.path()).unwrap();

        let results = lib.complete_subfolders("t");
        // "theme.flac" is a file, should not appear
        assert!(results.is_empty());
    }
}
