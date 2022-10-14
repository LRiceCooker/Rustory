mod commands;
// mod text;
// mod map;
// mod input;

fn main() { 
    // text::display::primary(text::wording::lang()["home_screen"]["title"].to_string());
    // text::display::success("█".to_string());
    // input::text::get();
    //map::map_loader::load();
    commands::user_input::listen();
}