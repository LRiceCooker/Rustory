use std::{fs};

#[path="../types/wording/mod.rs"]
mod wording;

 fn get_raw_data () -> String {
    let raw_data = fs::read_to_string("./assets/wording/eng.json")
        .expect("Something went wrong reading the file");
    return raw_data;
}

pub fn get() -> wording::Wording {
    let result: wording::Wording = serde_json::from_str(&get_raw_data()).expect("Unable to parse"); //c'est sencé marcher 
     
    return result;
}