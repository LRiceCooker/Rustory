extern crate rustc_serialize;
use rustc_serialize::json::Json;
use std::fs::File;
use std::io::Read;

pub fn get() -> Json{
    let mut file = File::open("text/wording/wording.json").unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();
    let result = Json::from_str(&buffer).unwrap();
    return result;
}