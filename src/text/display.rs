use ansi_term::Colour;

pub fn primary(message : String){
    println!("{}", Colour::Blue.bold().paint(message));
}

pub fn danger(message : String){
    println!("{}", Colour::Red.bold().paint(message));
}

pub fn warning(message : String){
    println!("{}", Colour::Yellow.bold().paint(message));
}

pub fn success(message : String){
    println!("{}", Colour::Green.bold().paint(message));
}

pub fn regular(message : String){
    println!("{}", Colour::White.bold().paint(message));
}