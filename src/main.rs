fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = rustory::app::App::new().run(terminal);
    ratatui::restore();
    result
}
