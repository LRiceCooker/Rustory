use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::{stdout};

#[path = "./wording.rs"]
mod wording;

fn show (color : Color, message: String){
    execute!(
        stdout(),
        SetForegroundColor(color),
        Print(message.replace('"', "")),
        ResetColor
    ).expect(wording::lang()["error"]["idk"].to_string().as_str());
}

pub fn primary(message : String){
   show(Color::Blue, message);
}

pub fn danger(message : String){
    show(Color::Red, message);
}

pub fn warning(message : String){
    show(Color::Yellow, message);
}

pub fn success(message : String){
    show(Color::Green, message);
}

pub fn regular(message : String){
    show(Color::White, message);
}

pub fn dark(message : String){
    show(Color::DarkMagenta, message);
}