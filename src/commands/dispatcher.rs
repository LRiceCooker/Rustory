#[path = "./mapping.rs"]
mod mapping; 
#[path = "./rustory_commands.rs"]
mod rustory_commands;
#[path = "../text/mod.rs"]
mod text;

pub fn eval(raw_command: String) {
    let mut splited_args: Vec<&str> = raw_command.split(" ").collect();
    let command = splited_args[0];
    splited_args.retain(|&elem| elem != command);

    let matchable_command = Some(command);

    match matchable_command {
        Some(mapping::COMMANDS::QUIT) => {}, 
        Some(mapping::COMMANDS::HELP) => rustory_commands::help(splited_args), 
        _ => text::display::danger(text::wording::lang()["error"]["unknown_command"].to_string())
    };
}