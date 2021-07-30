pub struct Error {
    idk : String
}

pub fn get() -> Error{
    return Error {
        idk : "Oops ! it's break ! please report this issue :p".to_string()
    }
}
