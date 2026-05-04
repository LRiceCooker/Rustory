fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    let terminal = ratatui::init();
    let result = rustory::app::App::new().run(terminal);
    ratatui::restore();
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
    result
}
