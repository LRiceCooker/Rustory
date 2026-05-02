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

pub fn dispatch(input: &str) -> CommandResult {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let command = parts[0];
    let _args = parts.get(1).unwrap_or(&"");

    match command {
        mapping::QUIT => CommandResult::Quit,
        mapping::HELP => CommandResult::Output(vec![
            StyledLine::plain("Type a command and press Enter."),
        ]),
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
            CommandResult::Output(lines) => assert!(!lines.is_empty()),
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
