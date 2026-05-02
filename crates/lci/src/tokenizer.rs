use std::{char as stdchar, iter::Peekable, result::Result as StdResult};

use thiserror::Error;

use crate::types::{Interpolate, Value};

#[derive(Debug, Error)]
pub enum TokenizeError {
    #[error("invalid character in identifier: {0}")]
    InvalidIdent(char),
    #[error("invalid characters in interpolation: {0:?}")]
    InvalidInterpolation(String),
    #[error("invalid number: {0:?}")]
    InvalidNumber(String),
    #[error("invalid unicode character: {0}")]
    InvalidUnicode(String),
    #[error("unclosed comment")]
    UnclosedComment,
    #[error("unclosed interpolation in string")]
    UnclosedInterpolation,
    #[error("unclosed string")]
    UnclosedString,
    #[error("unexpected end of file")]
    UnexpectedEOF,
    #[error("unknown escape character: {0}")]
    UnknownEscape(char),
    #[error("unknown token")]
    UnknownToken,
}

type Result<T> = StdResult<T, TokenizeError>;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    It,
    Ident(String),
    Value(Value),

    Hai,
    KThxBye,
    Separator,

    IHasA,
    Itz,
    R,

    SumOf,
    DiffOf,
    ProduktOf,
    QuoshuntOf,
    ModOf,
    BiggrOf,
    SmallrOf,

    BothOf,
    EitherOf,
    WonOf,
    Not,
    AllOf,
    AnyOf,
    BothSaem,
    Diffrint,

    Smoosh,
    An,
    Mkay,

    ORly,
    YaRly,
    Mebbe,
    NoWai,
    Oic,

    Wtf,
    Omg,
    OmgWtf,
    Gtfo,

    ImInYr,
    Uppin,
    Nerfin,
    Til,
    Wile,
    ImOuttaYr,

    HowIzI,
    Yr,
    IfUSaySo,
    FoundYr,
    IIz,

    Visible,
    Exclamation,
    Gimmeh,
}

#[derive(Clone)]
pub struct Tokenizer<I: Iterator<Item = char> + Clone> {
    pub iter: Peekable<I>,
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '\t'
}

impl<I: Iterator<Item = char> + Clone> Tokenizer<I> {
    fn trim(&mut self) {
        loop {
            match self.iter.peek().cloned() {
                Some(c) if is_space(c) => {
                    self.iter.next();
                }
                _ => break,
            }
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.trim();
        self.iter.peek().cloned()
    }
    fn word(&mut self) -> String {
        let mut word = String::new();
        loop {
            match self.iter.peek().cloned() {
                Some(c) if is_space(c) => {
                    self.trim();
                    return word;
                }
                None | Some('\n') | Some(',') => return word,
                Some(c) => {
                    self.iter.next();
                    word.push(c);
                }
            }
        }
    }
    /// Read one token from the input
    pub fn next(&mut self) -> Result<Option<Token>> {
        let c = match self.peek() {
            Some(c) => c,
            None => return Ok(None),
        };
        if c == '"' {
            fn read_until<I: Iterator<Item = char>>(iter: &mut I, c: char) -> Result<String> {
                let mut string = String::new();
                loop {
                    match iter.next() {
                        Some('"') => return Err(TokenizeError::UnclosedInterpolation),
                        None => return Err(TokenizeError::UnclosedString),

                        Some(c2) if c == c2 => break,
                        Some(c) => string.push(c),
                    }
                }
                Ok(string)
            }
            self.iter.next(); // leading "
            let mut interpolated = Vec::new();
            let mut string = String::new();
            while let Some(c) = self.iter.next() {
                if c == ':' {
                    match self.iter.next() {
                        Some(')') => string.push('\n'),
                        Some('>') => string.push('\t'),
                        Some('o') => string.push('\x07'),
                        Some('"') => string.push('"'),
                        Some(':') => string.push(':'),
                        Some('(') => {
                            let hex = read_until(&mut self.iter, ')')?;
                            let num = match u32::from_str_radix(&hex, 16) {
                                Ok(num) => num,
                                Err(_) => return Err(TokenizeError::InvalidNumber(hex)),
                            };
                            match stdchar::from_u32(num) {
                                Some(c) => string.push(c),
                                None => return Err(TokenizeError::InvalidUnicode(hex)),
                            }
                        }
                        Some('{') => {
                            let var = read_until(&mut self.iter, '}')?;
                            match var.chars().next() {
                                None | Some('0'..='9') => {
                                    return Err(TokenizeError::InvalidInterpolation(var))
                                }
                                _ => (),
                            }
                            if !var
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                            {
                                return Err(TokenizeError::InvalidInterpolation(var));
                            }
                            if !string.is_empty() {
                                interpolated.push(Interpolate::Str(string));
                            }
                            interpolated.push(Interpolate::Var(var));
                            string = String::new();
                        }
                        Some('[') => {
                            // Unicode name lookup (:[NAME]) — unsupported in this fork.
                            // Use hex escape :(XXXX) instead.
                            let name = read_until(&mut self.iter, ']')?;
                            return Err(TokenizeError::InvalidUnicode(name));
                        }
                        Some(c) => return Err(TokenizeError::UnknownEscape(c)),
                        None => return Err(TokenizeError::UnclosedString),
                    };
                    continue;
                } else if c == '"' {
                    break;
                }
                string.push(c);
            }
            if interpolated.is_empty() {
                return Ok(Some(Token::Value(Value::Yarn(string))));
            } else {
                if !string.is_empty() {
                    interpolated.push(Interpolate::Str(string));
                }
                return Ok(Some(Token::Value(Value::YarnRaw(interpolated))));
            }
        } else if c == '\n' || c == ',' {
            self.iter.next();
            return Ok(Some(Token::Separator));
        }

        let word = self.word();
        match &*word {
            "HAI" => return Ok(Some(Token::Hai)),
            "KTHXBYE" => return Ok(Some(Token::KThxBye)),
            "BTW" => {
                loop {
                    match self.iter.next() {
                        Some('\n') | None => break,
                        _ => (),
                    }
                }
                return self.next();
            }
            "OBTW" => loop {
                match self.peek() {
                    None => return Err(TokenizeError::UnclosedComment),
                    Some('T') => {
                        if self.word() == "TLDR" {
                            return self.next();
                        } else {
                            self.iter.next();
                        }
                    }
                    _ => {
                        self.iter.next();
                    }
                }
            },
            "WIN" => return Ok(Some(Token::Value(Value::Troof(true)))),
            "FAIL" => return Ok(Some(Token::Value(Value::Troof(false)))),
            "IT" => return Ok(Some(Token::It)),
            "I" => {
                let mut clone = self.clone();
                match &*clone.word() {
                    "HAS" => {
                        if clone.word() == "A" {
                            *self = clone;
                            return Ok(Some(Token::IHasA));
                        }
                    }
                    "IZ" => {
                        *self = clone;
                        return Ok(Some(Token::IIz));
                    }
                    _ => (),
                }
            }
            "ITZ" => return Ok(Some(Token::Itz)),
            "R" => return Ok(Some(Token::R)),
            "SUM" | "DIFF" | "PRODUKT" | "QUOSHUNT" | "MOD" | "BIGGR" | "SMALLR" | "BOTH"
            | "EITHER" | "WON" | "ALL" | "ANY" => {
                let mut clone = self.clone();
                match &*clone.word() {
                    "OF" => {
                        *self = clone;
                        return Ok(Some(match &*word {
                            "SUM" => Token::SumOf,
                            "DIFF" => Token::DiffOf,
                            "PRODUKT" => Token::ProduktOf,
                            "QUOSHUNT" => Token::QuoshuntOf,
                            "MOD" => Token::ModOf,
                            "BIGGR" => Token::BiggrOf,
                            "SMALLR" => Token::SmallrOf,

                            "BOTH" => Token::BothOf,
                            "EITHER" => Token::EitherOf,
                            "WON" => Token::WonOf,
                            "ALL" => Token::AllOf,
                            "ANY" => Token::AnyOf,

                            _ => unreachable!(),
                        }));
                    }
                    "SAEM" if word == "BOTH" => {
                        *self = clone;
                        return Ok(Some(Token::BothSaem));
                    }
                    _ => (),
                }
            }
            "NOT" => return Ok(Some(Token::Not)),
            "DIFFRINT" => return Ok(Some(Token::Diffrint)),
            "SMOOSH" => return Ok(Some(Token::Smoosh)),
            "AN" => return Ok(Some(Token::An)),
            "MKAY" => return Ok(Some(Token::Mkay)),
            "O" => {
                let mut clone = self.clone();
                if clone.word() == "RLY?" {
                    *self = clone;
                    return Ok(Some(Token::ORly));
                }
            }
            "YA" => {
                let mut clone = self.clone();
                if clone.word() == "RLY" {
                    *self = clone;
                    return Ok(Some(Token::YaRly));
                }
            }
            "MEBBE" => return Ok(Some(Token::Mebbe)),
            "NO" => {
                let mut clone = self.clone();
                if clone.word() == "WAI" {
                    *self = clone;
                    return Ok(Some(Token::NoWai));
                }
            }
            "OIC" => return Ok(Some(Token::Oic)),
            "WTF?" => return Ok(Some(Token::Wtf)),
            "OMG" => return Ok(Some(Token::Omg)),
            "OMGWTF" => return Ok(Some(Token::OmgWtf)),
            "GTFO" => return Ok(Some(Token::Gtfo)),
            "IM" => {
                let mut clone = self.clone();
                match &*clone.word() {
                    "IN" => {
                        if clone.word() == "YR" {
                            *self = clone;
                            return Ok(Some(Token::ImInYr));
                        }
                    }
                    "OUTTA" => {
                        if clone.word() == "YR" {
                            *self = clone;
                            return Ok(Some(Token::ImOuttaYr));
                        }
                    }
                    _ => (),
                }
            }
            "UPPIN" => return Ok(Some(Token::Uppin)),
            "NERFIN" => return Ok(Some(Token::Nerfin)),
            "YR" => return Ok(Some(Token::Yr)),
            "TIL" => return Ok(Some(Token::Til)),
            "WILE" => return Ok(Some(Token::Wile)),
            "HOW" => {
                let mut clone = self.clone();
                if clone.word() == "IZ" && clone.word() == "I" {
                    *self = clone;
                    return Ok(Some(Token::HowIzI));
                }
            }
            "IF" => {
                let mut clone = self.clone();
                if clone.word() == "U" && clone.word() == "SAY" && clone.word() == "SO" {
                    *self = clone;
                    return Ok(Some(Token::IfUSaySo));
                }
            }
            "FOUND" => {
                let mut clone = self.clone();
                if clone.word() == "YR" {
                    *self = clone;
                    return Ok(Some(Token::FoundYr));
                }
            }
            "VISIBLE" => return Ok(Some(Token::Visible)),
            "!" => return Ok(Some(Token::Exclamation)),
            "GIMMEH" => return Ok(Some(Token::Gimmeh)),
            _ => (),
        }

        match c {
            'a'..='z' | 'A'..='Z' | '_' => {
                for c in word.chars() {
                    match c {
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => (),
                        c => return Err(TokenizeError::InvalidIdent(c)),
                    }
                }
                return Ok(Some(Token::Ident(word)));
            }
            '-' | '0'..='9' => {
                if let Ok(num) = word.parse::<i64>() {
                    return Ok(Some(Token::Value(Value::Numbr(num))));
                } else if let Ok(num) = word.parse::<f64>() {
                    return Ok(Some(Token::Value(Value::Numbar(num))));
                }
                return Err(TokenizeError::InvalidNumber(word));
            }
            _ => (),
        }

        Err(TokenizeError::UnknownToken)
    }
}

/// Convenience function for reading all tokens from `input`
pub fn tokenize<I: Iterator<Item = char> + Clone>(input: I) -> Result<Vec<Token>> {
    let mut tokenizer = Tokenizer {
        iter: input.peekable(),
    };
    let mut tokens = Vec::new();
    while let Some(token) = tokenizer.next()? {
        tokens.push(token);
    }
    Ok(tokens)
}

/// Convenience function for reading all tokens from a string
pub fn tokenize_str(input: &str) -> Result<Vec<Token>> {
    tokenize(input.chars())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yarns() {
        assert_eq!(
            tokenize_str(r#" "Hello World :) How are you :>? I'm:: :"fine:"" "#).unwrap(),
            &[Token::Value(Value::Yarn(
                "Hello World \n How are you \t? I'm: \"fine\"".to_string()
            ))]
        );
    }
    #[test]
    fn hex_escape() {
        // :(2603) = snowman, :(1F60A) = 😊
        assert_eq!(
            tokenize_str(r#" ":(2603) is :(1F60A)" "#).unwrap(),
            &[Token::Value(Value::Yarn("☃ is 😊".to_string()))]
        );
    }
    #[test]
    fn interpolation() {
        assert_eq!(
            tokenize_str(r#" "hello :{something}" "#).unwrap(),
            &[Token::Value(Value::YarnRaw(vec![
                Interpolate::Str("hello ".to_string()),
                Interpolate::Var("something".to_string())
            ]))]
        );
    }
    #[test]
    fn primitives() {
        assert_eq!(
            tokenize_str("1, -5, 2.3, WIN, FAIL").unwrap(),
            &[
                Token::Value(Value::Numbr(1)),
                Token::Separator,
                Token::Value(Value::Numbr(-5)),
                Token::Separator,
                Token::Value(Value::Numbar(2.3)),
                Token::Separator,
                Token::Value(Value::Troof(true)),
                Token::Separator,
                Token::Value(Value::Troof(false))
            ]
        );
    }
    #[test]
    fn assign() {
        assert_eq!(
            tokenize_str("I HAS A VAR ITZ 12           BTW this is a comment").unwrap(),
            &[
                Token::IHasA,
                Token::Ident("VAR".to_string()),
                Token::Itz,
                Token::Value(Value::Numbr(12))
            ]
        );
        assert_eq!(
            tokenize_str("VAR R 12").unwrap(),
            &[
                Token::Ident("VAR".to_string()),
                Token::R,
                Token::Value(Value::Numbr(12))
            ]
        );
    }
    #[test]
    fn sum_of() {
        assert_eq!(
            tokenize_str("SUM OF OBTW hi TLDR 2 AN 4").unwrap(),
            &[
                Token::SumOf,
                Token::Value(Value::Numbr(2)),
                Token::An,
                Token::Value(Value::Numbr(4))
            ]
        );
    }
    #[test]
    fn loops() {
        assert_eq!(
            tokenize_str(
                "\
                IM IN YR LOOP UPPIN YR VAR TIL BOTH SAEM VAR AN 5
                    VISIBLE VAR
                IM OUTTA YR LOOP\
            "
            )
            .unwrap(),
            &[
                Token::ImInYr,
                Token::Ident("LOOP".to_string()),
                Token::Uppin,
                Token::Yr,
                Token::Ident("VAR".to_string()),
                Token::Til,
                Token::BothSaem,
                Token::Ident("VAR".to_string()),
                Token::An,
                Token::Value(Value::Numbr(5)),
                Token::Separator,
                Token::Visible,
                Token::Ident("VAR".to_string()),
                Token::Separator,
                Token::ImOuttaYr,
                Token::Ident("LOOP".to_string())
            ]
        );
    }
    #[test]
    fn functions() {
        assert_eq!(
            tokenize_str(
                "\
                HOW IZ I SCREAMING YR STUFF
                    VISIBLE STUFF \"!\"
                IF U SAY SO

                I IZ SCREAMING \"STUFF\" MKAY\
            "
            )
            .unwrap(),
            &[
                Token::HowIzI,
                Token::Ident("SCREAMING".to_string()),
                Token::Yr,
                Token::Ident("STUFF".to_string()),
                Token::Separator,
                Token::Visible,
                Token::Ident("STUFF".to_string()),
                Token::Value(Value::Yarn("!".to_string())),
                Token::Separator,
                Token::IfUSaySo,
                Token::Separator,
                Token::Separator,
                Token::IIz,
                Token::Ident("SCREAMING".to_string()),
                Token::Value(Value::Yarn("STUFF".to_string())),
                Token::Mkay
            ]
        );
    }
}
