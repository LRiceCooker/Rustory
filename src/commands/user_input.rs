#[path = "../input/text.rs"]
mod text; 
#[path = "./dispatcher.rs"]
mod dispatcher;
#[path = "./mapping.rs"]
mod mapping;

pub fn listen() {

    let mut user_input = "".to_string();

    while user_input != mapping::COMMANDS::QUIT {
        user_input = text::get();
        dispatcher::eval(user_input.clone());
    }

}