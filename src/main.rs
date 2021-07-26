mod text;

fn main() { 
    text::display::danger("Hello, world!".to_string());
    println!("{}",text::wording::get());
}
