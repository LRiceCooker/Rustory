mod text;

fn main() { 
    //text::display::success("Hello, world!".to_string());
    text::display::success(text::wording::get().homeScreen.title.to_string())
}
