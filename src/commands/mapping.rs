
pub const BASE: [&str; 3] = [
    "help", 
    "quit",
    "roll",
];
pub mod COMMANDS {
    use super::BASE;

    pub const HELP:&str = BASE[0];
    pub const QUIT:&str = BASE[1];
    pub const ROLL:&str = BASE[2];
}