use ratatui::style::{Color, Style};

use super::mapping;

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Output(Vec<StyledLine>),
    Error(String),
    Quit,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledLine {
    pub text: String,
    pub style: Style,
}

impl StyledLine {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Color::Blue))
    }
}

fn help() -> CommandResult {
    CommandResult::Output(vec![
        StyledLine::plain("Available commands:"),
        StyledLine::plain("  help  — show this help"),
        StyledLine::plain("  roll  — roll dice (e.g. roll 2d6+3)"),
        StyledLine::plain("  quit  — exit Rustory"),
    ])
}

pub fn dispatch(input: &str) -> CommandResult {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let command = parts[0];
    let _args = parts.get(1).unwrap_or(&"");

    match command {
        mapping::QUIT => CommandResult::Quit,
        mapping::HELP => help(),
        _ => CommandResult::Unknown(command.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_returns_quit() {
        assert_eq!(dispatch("quit"), CommandResult::Quit);
    }

    #[test]
    fn test_help_returns_output() {
        match dispatch("help") {
            CommandResult::Output(lines) => {
                assert!(lines.len() >= 4);
                assert!(lines[0].text.contains("Available commands"));
                assert!(lines.iter().any(|l| l.text.contains("help")));
                assert!(lines.iter().any(|l| l.text.contains("roll")));
                assert!(lines.iter().any(|l| l.text.contains("quit")));
            }
            other => panic!("expected Output, got {:?}", other),
        }
    }

    #[test]
    fn test_unknown_command_returns_unknown() {
        match dispatch("foobar") {
            CommandResult::Unknown(cmd) => assert_eq!(cmd, "foobar"),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }
}
