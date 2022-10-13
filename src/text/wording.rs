use std::{fs};
use toml::Value;
use sys_locale::get_locale;


pub fn lang() -> Value {
    
    let lang: &str = &get_locale().unwrap().to_owned();
    let en :Value = fs::read_to_string("./assets/wording/eng.toml").expect("").parse::<Value>().unwrap();

    match lang {
        "en-EN" => return en,
        _ => return en
    }

}