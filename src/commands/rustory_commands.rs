
#[path = "../text/mod.rs"]
mod text;

pub fn help(args: Vec<&str>) {
    text::display::primary(text::wording::lang()["command_help"]["body"].to_string()+"\n");
}