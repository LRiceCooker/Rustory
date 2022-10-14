use inquire::Text;

pub fn get() {
    let user_input = Text::new(">").prompt().unwrap();
    println!("{}", user_input);
}