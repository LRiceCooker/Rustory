use inquire::{Text, CustomUserError};

#[path = "../text/wording.rs"]
mod wording;

#[path = "../commands/mapping.rs"]
mod mapping;

fn suggester(val: &str) -> Result<Vec<String>, CustomUserError> {
    let val_lower = val.to_lowercase();
    Ok(
        mapping::BASE
        .iter()
        .filter(|s| s.to_lowercase().contains(&val_lower))
        .map(|s| String::from(*s))
        .collect()
    )
}

pub fn get() {
    let user_input = Text::new("rustory >")
        .with_autocomplete(&suggester)
        .prompt()
        .unwrap();
    println!("{}", user_input);
}