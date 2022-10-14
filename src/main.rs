mod text;
mod map;

fn main() { 
    text::display::primary(text::wording::lang()["home_screen"]["title"].to_string());
    text::display::success("█".to_string());
    //map::map_loader::load();
}