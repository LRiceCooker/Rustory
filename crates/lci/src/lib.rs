pub mod eval;
pub mod parser;
pub mod tokenizer;
pub mod types;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LciError {
    #[error("tokenize error: {0}")]
    TokenizeError(#[from] tokenizer::TokenizeError),
    #[error("parse error: {0}")]
    ParseError(#[from] parser::ParseError),
    #[error("eval error: {0}")]
    EvalError(#[from] eval::EvalError),
}

use eval::EvalParams;
use parser::AST;
use std::io;

/// Convenience function for tokenizing and parsing code
pub fn parse(code: &str) -> Result<Vec<AST>, LciError> {
    let tokens = tokenizer::tokenize(code.chars())?;
    #[cfg(feature = "debug")]
    println!("{:#?}", tokens);
    let parsed = parser::parse(tokens)?;
    #[cfg(feature = "debug")]
    println!("{:#?}", parsed);
    Ok(parsed)
}

/// Convenience function for tokenizing, parsing, and evaluating code
pub fn eval<R, W, F>(code: &str, stdin: R, stdout: W, callback: F) -> Result<(), LciError>
where
    R: io::BufRead,
    W: io::Write,
    F: FnOnce(&mut EvalParams<R, W>),
{
    let parsed = parse(code)?;
    let mut params = eval::EvalParams::new(stdin, stdout);
    callback(&mut params);
    params.scope().eval_all(parsed)?;
    Ok(())
}

/// Convenience function for capturing the output of `eval`
pub fn capture<R, F>(code: &str, stdin: R, callback: F) -> Result<String, LciError>
where
    R: io::BufRead,
    F: FnOnce(&mut EvalParams<R, &mut Vec<u8>>),
{
    let mut output = Vec::new();
    eval(code, stdin, &mut output, callback)?;
    Ok(String::from_utf8(output).expect("Program (somehow) returned non-utf8 data"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::Value;

    fn run(code: &str) -> Result<String, LciError> {
        capture(code, io::empty(), |_| ())
    }

    #[test]
    fn run_all() {
        assert_eq!(
            run(include_str!("../tests/fac.lol")).expect("Running test failed"),
            "120\n"
        );
        assert_eq!(
            run(include_str!("../tests/implicit-return.lol")).expect("Running test failed"),
            "hi\n"
        );
        assert_eq!(
            run(include_str!("../tests/int-overflow.lol")).expect("Running test failed"),
            "WIN\n"
        );
        assert_eq!(
            run(include_str!("../tests/pow.lol")).expect("Running test failed"),
            "32\n"
        );
        assert_eq!(
            run(include_str!("../tests/function-ordering.lol")).expect("Running test failed"),
            "PING 5\nPONG 4\nPING 3\nPONG 2\nPING 1\n"
        );
        assert_eq!(
            run(include_str!("../tests/quine.lol")).expect("Running test failed"),
            include_str!("../tests/quine.lol")
        );
    }

    #[test]
    fn rust_callback() {
        assert_eq!(
            capture(include_str!("../tests/callback.lol"), io::empty(), |eval| {
                eval.bind_func("LOWERIN", Some(1), |values| {
                    Value::Yarn(values[0].clone().cast_yarn().unwrap().to_lowercase())
                });
            })
            .expect("Running test failed"),
            "test\n"
        );
    }

    #[test]
    fn run_fails() {
        match run(include_str!("../tests/fail/divide-by-zero.lol")) {
            Err(LciError::EvalError(eval::EvalError::DivideByZero)) => (),
            other => panic!("Expected DivideByZero, got {other:?}"),
        }
        match run(include_str!("../tests/fail/stack-overflow.lol")) {
            Err(LciError::EvalError(eval::EvalError::RecursionLimit(_))) => (),
            other => panic!("Expected RecursionLimit, got {other:?}"),
        }
    }
}
