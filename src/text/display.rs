use std::io::Write;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

fn write(message : String, color : termcolor::Color){
    let bufwtr = BufferWriter::stderr(ColorChoice::Always);
    let mut buffer = bufwtr.buffer();
    buffer.set_color(ColorSpec::new().set_fg(Some(color)));
    writeln!(&mut buffer,"{}", message);
    bufwtr.print(&buffer);
}

pub fn primary(message : String){
    write(message, Color::Cyan);
}

pub fn danger(message : String){
    write(message, Color::Red);
}

pub fn warning(message : String){
    write(message, Color::Yellow);
}

pub fn success(message : String){
    write(message, Color::Green);
}

pub fn regular(message : String){
    write(message, Color::White);
}