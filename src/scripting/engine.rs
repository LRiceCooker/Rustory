use lci::types::Value;
use std::fmt;
use std::io;

/// Error type for script execution failures.
#[derive(Debug)]
pub enum ScriptError {
    /// LOLCODE parse or tokenize error
    ParseError(String),
    /// Runtime evaluation error
    RuntimeError(String),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::ParseError(msg) => write!(f, "Script parse error: {msg}"),
            ScriptError::RuntimeError(msg) => write!(f, "Script runtime error: {msg}"),
        }
    }
}

impl From<lci::LciError> for ScriptError {
    fn from(err: lci::LciError) -> Self {
        match err {
            lci::LciError::TokenizeError(e) => ScriptError::ParseError(e.to_string()),
            lci::LciError::ParseError(e) => ScriptError::ParseError(e.to_string()),
            lci::LciError::EvalError(e) => ScriptError::RuntimeError(e.to_string()),
        }
    }
}

type ApiCallback = Box<dyn FnMut(Vec<Value>) -> Value>;

struct CallbackEntry {
    name: String,
    arg_count: Option<usize>,
    func: ApiCallback,
}

/// Bridges LOLCODE scripts to the Rust game engine via registered callbacks.
///
/// Register API functions (e.g., RUSTORY_GET_STAT), then execute a script.
/// The interpreter delegates unknown function calls to the registered callbacks.
///
/// ```ignore
/// let mut engine = ScriptEngine::new();
/// engine.register("RUSTORY_GET_STAT", Some(1), |args| {
///     Value::Numbr(18)
/// });
/// let output = engine.execute("HAI 1.2\nVISIBLE I IZ RUSTORY_GET_STAT YR \"str\" MKAY\nKTHXBYE")?;
/// ```
pub struct ScriptEngine {
    callbacks: Vec<CallbackEntry>,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Register a callback function that LOLCODE scripts can call.
    ///
    /// - `name`: function name as called from LOLCODE (e.g., "RUSTORY_ROLL")
    /// - `arg_count`: expected argument count (`None` for variadic)
    /// - `func`: Rust closure receiving `Vec<Value>` and returning `Value`
    pub fn register<F>(
        &mut self,
        name: impl Into<String>,
        arg_count: Option<usize>,
        func: F,
    ) -> &mut Self
    where
        F: FnMut(Vec<Value>) -> Value + 'static,
    {
        self.callbacks.push(CallbackEntry {
            name: name.into(),
            arg_count,
            func: Box::new(func),
        });
        self
    }

    /// Execute a LOLCODE script with all registered callbacks available.
    ///
    /// Consumes the engine because callbacks are moved into the interpreter.
    /// Returns the script's captured stdout output on success.
    pub fn execute(self, code: &str) -> Result<String, ScriptError> {
        let callbacks = self.callbacks;
        lci::capture(code, io::empty(), |eval| {
            for entry in callbacks {
                eval.bind_func(entry.name, entry.arg_count, entry.func);
            }
        })
        .map_err(ScriptError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_simple_script() {
        let engine = ScriptEngine::new();
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE \"hello\"
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn test_register_callback_and_call() {
        let mut engine = ScriptEngine::new();
        engine.register("GET_VALUE", Some(0), |_args| Value::Numbr(42));
        let output = engine
            .execute(
                "\
HAI 1.2
I HAS A X ITZ I IZ GET_VALUE MKAY
VISIBLE X
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "42\n");
    }

    #[test]
    fn test_callback_receives_correct_args() {
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let mut engine = ScriptEngine::new();
        engine.register("CAPTURE_ARGS", Some(2), move |args| {
            let name = args[0].cast_yarn().unwrap().to_string();
            let value = args[1].cast_numbr().unwrap();
            received_clone.lock().unwrap().push((name, value));
            Value::Troof(true)
        });
        engine
            .execute(
                "\
HAI 1.2
I IZ CAPTURE_ARGS YR \"strength\" AN YR 18 MKAY
KTHXBYE",
            )
            .unwrap();

        let captured = received.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0], ("strength".to_string(), 18));
    }

    #[test]
    fn test_callback_return_value_reaches_script() {
        let mut engine = ScriptEngine::new();
        engine.register("DOUBLE", Some(1), |args| {
            let n = args[0].cast_numbr().unwrap();
            Value::Numbr(n * 2)
        });
        let output = engine
            .execute(
                "\
HAI 1.2
I HAS A X ITZ I IZ DOUBLE YR 21 MKAY
VISIBLE X
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "42\n");
    }

    #[test]
    fn test_multiple_callbacks() {
        let mut engine = ScriptEngine::new();
        engine.register("GET_NAME", Some(0), |_| Value::Yarn("Thorin".to_string()));
        engine.register("GET_HP", Some(0), |_| Value::Numbr(52));
        let output = engine
            .execute(
                "\
HAI 1.2
I HAS A NAME ITZ I IZ GET_NAME MKAY
I HAS A HP ITZ I IZ GET_HP MKAY
VISIBLE SMOOSH NAME AN \":: \" AN HP MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "Thorin: 52\n");
    }

    #[test]
    fn test_callback_with_string_return() {
        let mut engine = ScriptEngine::new();
        engine.register("GREET", Some(1), |args| {
            let name = args[0].cast_yarn().unwrap().to_string();
            Value::Yarn(format!("Hello, {name}!"))
        });
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE I IZ GREET YR \"Elara\" MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "Hello, Elara!\n");
    }

    #[test]
    fn test_callback_with_float_return() {
        let mut engine = ScriptEngine::new();
        engine.register("HALF", Some(1), |args| {
            let n = args[0].cast_numbar().unwrap();
            Value::Numbar(n / 2.0)
        });
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE I IZ HALF YR 7.0 MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "3.5\n");
    }

    #[test]
    fn test_callback_with_bool_return() {
        let mut engine = ScriptEngine::new();
        engine.register("IS_ALIVE", Some(1), |args| {
            let hp = args[0].cast_numbr().unwrap();
            Value::Troof(hp > 0)
        });
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE I IZ IS_ALIVE YR 10 MKAY
VISIBLE I IZ IS_ALIVE YR 0 MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "WIN\nFAIL\n");
    }

    #[test]
    fn test_parse_error() {
        let engine = ScriptEngine::new();
        let result = engine.execute("THIS IS NOT VALID LOLCODE");
        assert!(result.is_err());
        match result.unwrap_err() {
            ScriptError::ParseError(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Expected ParseError, got {other}"),
        }
    }

    #[test]
    fn test_runtime_error_undefined_var() {
        let engine = ScriptEngine::new();
        let result = engine.execute(
            "\
HAI 1.2
VISIBLE GHOST
KTHXBYE",
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            ScriptError::RuntimeError(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Expected RuntimeError, got {other}"),
        }
    }

    #[test]
    fn test_script_calls_callback_multiple_times() {
        let counter = std::sync::Arc::new(std::sync::Mutex::new(0));
        let counter_clone = counter.clone();

        let mut engine = ScriptEngine::new();
        engine.register("INCREMENT", Some(0), move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            Value::Numbr(*c)
        });
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE I IZ INCREMENT MKAY
VISIBLE I IZ INCREMENT MKAY
VISIBLE I IZ INCREMENT MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "1\n2\n3\n");
        assert_eq!(*counter.lock().unwrap(), 3);
    }

    #[test]
    fn test_register_chaining() {
        let mut engine = ScriptEngine::new();
        engine
            .register("A", Some(0), |_| Value::Numbr(1))
            .register("B", Some(0), |_| Value::Numbr(2));
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE SUM OF I IZ A MKAY AN I IZ B MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "3\n");
    }

    #[test]
    fn test_variadic_callback() {
        let mut engine = ScriptEngine::new();
        engine.register("SUM_ALL", None, |args| {
            let total: i64 = args.iter().filter_map(|a| a.cast_numbr()).sum();
            Value::Numbr(total)
        });
        let output = engine
            .execute(
                "\
HAI 1.2
VISIBLE I IZ SUM_ALL YR 10 AN YR 20 AN YR 30 MKAY
KTHXBYE",
            )
            .unwrap();
        assert_eq!(output, "60\n");
    }
}
