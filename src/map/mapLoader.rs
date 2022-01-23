use image2ascii;
#[path="../text/mod.rs"]
mod text; 

pub fn load(map_path: &str) {
    let termsize::Size { rows , cols} = termsize::get().unwrap();
    let parsed_map = image2ascii::image2ascii(map_path, cols.into(), Some(0.1), Some("@#/\\. ")).expect("Error loading map");
    for row in parsed_map.to_lines().iter() {
        text::display::dark(row.to_string());
    }
}