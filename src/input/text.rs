use inquire::{Text, CustomUserError};

#[path = "../text/wording.rs"]
mod wording;
#[path = "../commands/mapping.rs"]
mod mapping;
#[path = "./validators.rs"]
mod validators;

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

pub fn get() -> String {
    let user_input = Text::new("rustory >")
        .with_autocomplete(&suggester)
        .with_validator(&validators::should_exist)
        .prompt()
        .unwrap();
    return user_input
}