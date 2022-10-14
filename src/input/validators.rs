use inquire::{CustomUserError, validator::Validation};

#[path = "../commands/mapping.rs"]
mod mapping;

#[path = "../text/wording.rs"]
mod wording; 

pub fn should_exist (text: &str) -> Result<Validation, CustomUserError>{
    let command: &str = text.split(" ").collect::<Vec::<&str>>()[0];
    match Some(mapping::BASE.contains(&command)) {
        Some(true) => return Ok(Validation::Valid),
        _ => return Ok(Validation::Invalid(wording::lang()["error"]["unknown_command"].to_string().into()))
    }
}