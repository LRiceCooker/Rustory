use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::{stdout};

fn show (color : Color, message: String){
    execute!(
        stdout(),
        SetForegroundColor(color),
        Print(message),
        ResetColor
    ).expect("Oops ! it's break ! please report this issue :p");
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