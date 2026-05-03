use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Format current time as "HH:MM".
fn current_time_str() -> String {
    // Get seconds since UNIX epoch, convert to local time components
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Get local timezone offset from the environment
    // We just format UTC for simplicity (notes are session-relative, not absolute)
    let secs_in_day = now % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

/// Format current date as "YYYY-MM-DD".
fn current_date_str() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simple date calculation from epoch seconds (UTC)
    let days = (now / 86400) as i64;
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert days since epoch to (year, month, day).
fn days_to_date(days_since_epoch: i64) -> (i32, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + (era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// Append a timestamped note to the campaign's notes directory.
/// Creates `notes/YYYY-MM-DD.md` if it doesn't exist.
/// Returns the date filename (without path).
pub fn append(campaign_path: &Path, text: &str) -> std::io::Result<String> {
    let notes_dir = campaign_path.join("notes");
    fs::create_dir_all(&notes_dir)?;

    let date = current_date_str();
    let time = current_time_str();
    let filepath = notes_dir.join(format!("{date}.md"));

    let entry = format!("## {time}\n{text}\n\n");

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filepath)?;
    file.write_all(entry.as_bytes())?;

    Ok(date)
}

/// Read today's notes file content.
pub fn list_today(campaign_path: &Path) -> Option<String> {
    let date = current_date_str();
    let filepath = campaign_path.join("notes").join(format!("{date}.md"));
    fs::read_to_string(filepath).ok()
}

/// List all note files in the campaign's notes directory.
pub fn list_files(campaign_path: &Path) -> Vec<String> {
    let notes_dir = campaign_path.join("notes");
    if !notes_dir.exists() {
        return Vec::new();
    }

    let mut files: Vec<String> = fs::read_dir(&notes_dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_append_creates_file() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path();

        let date = append(campaign, "Test note").unwrap();
        assert!(!date.is_empty());

        let notes_dir = campaign.join("notes");
        assert!(notes_dir.exists());

        let content = fs::read_to_string(notes_dir.join(format!("{date}.md"))).unwrap();
        assert!(content.contains("Test note"));
        assert!(content.contains("##")); // timestamp header
    }

    #[test]
    fn test_multiple_appends_concatenate() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path();

        append(campaign, "First note").unwrap();
        append(campaign, "Second note").unwrap();

        let content = list_today(campaign).unwrap();
        assert!(content.contains("First note"), "Should contain first note. Got: {content}");
        assert!(content.contains("Second note"), "Should contain second note. Got: {content}");
    }

    #[test]
    fn test_list_today_returns_content() {
        let dir = TempDir::new().unwrap();
        let campaign = dir.path();

        append(campaign, "Today's note").unwrap();
        let content = list_today(campaign).unwrap();
        assert!(content.contains("Today's note"));
    }

    #[test]
    fn test_list_today_no_notes_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(list_today(dir.path()).is_none());
    }

    #[test]
    fn test_list_files_returns_sorted() {
        let dir = TempDir::new().unwrap();
        let notes_dir = dir.path().join("notes");
        fs::create_dir_all(&notes_dir).unwrap();

        fs::write(notes_dir.join("2026-05-01.md"), "day 1").unwrap();
        fs::write(notes_dir.join("2026-05-03.md"), "day 3").unwrap();
        fs::write(notes_dir.join("2026-05-02.md"), "day 2").unwrap();

        let files = list_files(dir.path());
        assert_eq!(files, vec!["2026-05-01.md", "2026-05-02.md", "2026-05-03.md"]);
    }

    #[test]
    fn test_list_files_no_notes_dir() {
        let dir = TempDir::new().unwrap();
        let files = list_files(dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_list_files_ignores_non_md() {
        let dir = TempDir::new().unwrap();
        let notes_dir = dir.path().join("notes");
        fs::create_dir_all(&notes_dir).unwrap();

        fs::write(notes_dir.join("2026-05-01.md"), "note").unwrap();
        fs::write(notes_dir.join("temp.txt"), "not a note").unwrap();

        let files = list_files(dir.path());
        assert_eq!(files, vec!["2026-05-01.md"]);
    }

    #[test]
    fn test_days_to_date_epoch() {
        let (y, m, d) = days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_date_known_date() {
        // 2026-05-03 = day 20576 from epoch
        let (y, m, d) = days_to_date(20576);
        assert_eq!((y, m, d), (2026, 5, 3));
    }
}
