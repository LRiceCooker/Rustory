mod text;

fn main() { 
    text::display::success(text::wording::get().home_screen.title.to_string())
}
