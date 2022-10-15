#[path = "./parsers/mod.rs"]
mod parsers;
#[path = "../text/mod.rs"]
mod text;

use std::convert::TryInto;

use rand::Rng;

pub fn roll (args: Vec<&str>){
    let mut rng = rand::thread_rng();
    let mut result: i16 = 0;
    match parsers::roll::parse(args) {
        Ok(dice) => {
            let mut naturals: Vec<i16> = Vec::new();
            for _ in 0..dice.dice {
                let res:i16 = rng.gen_range(1,dice.value.try_into().unwrap());
                naturals.push(res);
                result += res; 
            }
            result += dice.add; 
            text::display::primary(
                format!(
                    "{} {1}, {2}: {3}, {4}: {5}, {6}: {7:?}, {8}: {9}\n", 
                    dice.dice,
                    text::wording::lang()["command_roll"]["dice"],
                    text::wording::lang()["command_roll"]["max_value"],
                    dice.value,
                    text::wording::lang()["command_roll"]["modifier"],
                    dice.add,
                    text::wording::lang()["command_roll"]["natural"],
                    naturals,
                    text::wording::lang()["command_roll"]["result"], 
                    result
                )
            )

        }, 
        Err(_) => {
            print!("{}[2J", 27 as char);
            text::display::danger(format!("{}\n", text::wording::lang()["command_roll"]["error"]));
        }
    }
}