use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use rustory::app::{App, Message};
use rustory::ui;

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
        terminal
            .draw(|frame| ui::render(frame, &self.app))
            .unwrap();
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
}
