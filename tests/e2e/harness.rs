use std::fs;
use std::path::Path;

use rand::rngs::StdRng;
use rand::SeedableRng;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use rustory::app::{App, Message};
use rustory::ui;
use tempfile::TempDir;

pub struct TestHarness {
    pub app: App,
}

impl TestHarness {
    /// Create a new test harness with a fresh App
    pub fn new() -> Self {
        let mut app = App::new();
        app.running = true;
        Self { app }
    }

    /// Load a fixture campaign from tests/e2e/fixtures/
    pub fn from_fixture(name: &str) -> Self {
        let path = std::path::PathBuf::from(format!("tests/e2e/fixtures/{name}"));
        assert!(path.exists(), "Fixture not found: {}", path.display());
        let mut app = App::new();
        app.running = true;
        // GameState loading will be wired in Phase 9
        Self { app }
    }

    /// Create a new test harness with a seeded RNG for deterministic tests
    pub fn with_seed(seed: u64) -> Self {
        let mut app = App::with_rng(Box::new(StdRng::seed_from_u64(seed)));
        app.running = true;
        Self { app }
    }

    /// Execute a command as if the user typed it + pressed Enter
    pub fn execute(&mut self, command: &str) {
        self.app.dispatch_command(command);
    }

    /// Get the last non-echo output text (skips the "> command" echo)
    pub fn last_output(&self) -> Option<&str> {
        self.app
            .messages
            .iter()
            .rev()
            .find(|m| !m.text.starts_with("> "))
            .map(|m| m.text.as_str())
    }

    /// Get full output history
    pub fn output_history(&self) -> &[Message] {
        &self.app.messages
    }

    /// Render to buffer for visual assertions
    pub fn render(&self, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| ui::render(frame, &self.app)).unwrap();
        terminal.backend().buffer().clone()
    }
}

/// Check if rendered buffer contains a string anywhere
pub fn buffer_contains(buf: &Buffer, text: &str) -> bool {
    let content: String = buf.content().iter().map(|cell| cell.symbol()).collect();
    content.contains(text)
}

/// Check if a specific line in the buffer contains text
pub fn buffer_line_contains(buf: &Buffer, line: u16, text: &str) -> bool {
    let width = buf.area.width;
    let start = (line * width) as usize;
    let end = start + width as usize;
    if end > buf.content().len() {
        return false;
    }
    let content: String = buf.content()[start..end]
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    content.contains(text)
}

pub struct TestCampaign {
    dir: TempDir,
}

impl TestCampaign {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        // Create required directories
        fs::create_dir_all(dir.path().join("rules")).unwrap();
        Self { dir }
    }

    pub fn with_system_toml(self, content: &str) -> Self {
        fs::write(self.dir.path().join("rules/system.toml"), content).unwrap();
        self
    }

    pub fn with_player(self, name: &str, csv: &str) -> Self {
        let player_dir = self.dir.path().join("players").join(name);
        fs::create_dir_all(&player_dir).unwrap();
        fs::write(player_dir.join("sheet.csv"), csv).unwrap();
        self
    }

    pub fn with_npc(self, name: &str, csv: &str) -> Self {
        let npc_dir = self.dir.path().join("npc").join(name);
        fs::create_dir_all(&npc_dir).unwrap();
        fs::write(npc_dir.join("sheet.csv"), csv).unwrap();
        self
    }

    pub fn with_bulk_npcs(self, filename: &str, csv: &str) -> Self {
        let npc_dir = self.dir.path().join("npc");
        fs::create_dir_all(&npc_dir).unwrap();
        fs::write(npc_dir.join(filename), csv).unwrap();
        self
    }

    pub fn with_lol_command(self, name: &str, script: &str) -> Self {
        let cmd_dir = self.dir.path().join("rules/commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join(format!("{name}.lol")), script).unwrap();
        self
    }

    pub fn with_lore(self, character: &str, markdown: &str) -> Self {
        // Determine if it's a player or NPC by checking existing dirs
        let player_dir = self.dir.path().join("players").join(character);
        let npc_dir = self.dir.path().join("npc").join(character);
        let target = if player_dir.exists() {
            player_dir
        } else if npc_dir.exists() {
            npc_dir
        } else {
            // Default to npc
            fs::create_dir_all(&npc_dir).unwrap();
            npc_dir
        };
        fs::write(target.join("lore.md"), markdown).unwrap();
        self
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_execute_help() {
        let mut harness = TestHarness::new();
        harness.execute("help");
        let output = harness.last_output().unwrap();
        assert!(output.contains("help") || output.contains("quit") || output.contains("roll"));
    }

    #[test]
    fn test_harness_execute_unknown() {
        let mut harness = TestHarness::new();
        harness.execute("foobar");
        let output = harness.last_output().unwrap();
        assert!(output.contains("Unknown command"));
    }

    #[test]
    fn test_harness_output_history() {
        let mut harness = TestHarness::new();
        harness.execute("help");
        let history = harness.output_history();
        // At least echo + output
        assert!(history.len() >= 2);
        assert!(history[0].text.starts_with("> "));
    }

    #[test]
    fn test_harness_render() {
        let harness = TestHarness::new();
        let buf = harness.render(60, 20);
        assert!(buffer_contains(&buf, "Rustory"));
        assert!(buffer_contains(&buf, "rustory >"));
    }

    #[test]
    fn test_buffer_contains() {
        let harness = TestHarness::new();
        let buf = harness.render(60, 20);
        assert!(buffer_contains(&buf, "Rustory"));
        assert!(!buffer_contains(&buf, "nonexistent_text_xyz"));
    }

    #[test]
    fn test_buffer_line_contains() {
        let harness = TestHarness::new();
        let buf = harness.render(60, 20);
        // Header is on line 0
        assert!(buffer_line_contains(&buf, 0, "Rustory"));
        // Input bar is on the last few lines
        let last_line = 20 - 2; // inside the input bar
        assert!(buffer_line_contains(&buf, last_line, "rustory >"));
    }

    #[test]
    fn test_campaign_builder_creates_structure() {
        let campaign = TestCampaign::new()
            .with_system_toml("[system]\nname = \"Test\"\n")
            .with_player("thorin", "name,level\nThorin,5\n")
            .with_npc("goblin", "name,level\nGoblin,1\n")
            .with_bulk_npcs("townspeople.csv", "name,level\nBob,1\nAlice,2\n")
            .with_lol_command("smite", "HAI 1.2\nKTHXBYE\n");

        assert!(campaign.path().join("rules/system.toml").exists());
        assert!(campaign.path().join("players/thorin/sheet.csv").exists());
        assert!(campaign.path().join("npc/goblin/sheet.csv").exists());
        assert!(campaign.path().join("npc/townspeople.csv").exists());
        assert!(campaign.path().join("rules/commands/smite.lol").exists());
    }

    #[test]
    fn test_harness_with_seed_deterministic() {
        let mut h1 = TestHarness::with_seed(42);
        let mut h2 = TestHarness::with_seed(42);
        h1.execute("roll 3d6");
        h2.execute("roll 3d6");
        assert_eq!(h1.last_output(), h2.last_output());
    }

    #[test]
    fn test_e2e_load_minimal_campaign() {
        let mut harness = TestHarness::from_fixture("minimal");
        // Verify fixture loaded (GameState verification comes in Phase 9)
        assert!(harness.app.running);

        // Verify help command works
        harness.execute("help");
        let output = harness.last_output().unwrap();
        assert!(output.contains("help") || output.contains("quit") || output.contains("roll"));
    }

    #[test]
    fn test_campaign_builder_with_lore() {
        let campaign = TestCampaign::new()
            .with_player("thorin", "name\nThorin\n")
            .with_lore("thorin", "# Thorin\nA brave dwarf.");

        assert!(campaign.path().join("players/thorin/lore.md").exists());
    }
}
