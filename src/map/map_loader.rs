use viuer::{print_from_file, Config};

pub fn load() {
    let termsize::Size {rows, cols} = termsize::get().unwrap();
    let conf = Config {
        x: cols / 4,
        y: 0,
        width: Some((cols / 2).into()),
        height: Some((rows / 2).into()),
        ..Default::default()
    };
    print_from_file("sample/map/test.png", &conf).expect("Image printing failed.");
}