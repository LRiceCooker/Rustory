use std::{fs};
use toml::Value;


pub fn lang(lang: &str) -> Value {

    let en = fs::read_to_string("./assets/wording/eng.toml").expect("").parse::<Value>().unwrap();

    match lang {
        "en" => return en,
        _ => return en
    }

}