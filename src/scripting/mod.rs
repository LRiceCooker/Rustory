/// LOLCODE scripting engine — bridges custom commands to the game state.
///
/// This module wraps the vendored `lci` crate (a Rust LOLCODE interpreter)
/// and provides the callback mechanism for RUSTORY_* API functions.

#[cfg(test)]
mod tests {
    use lci::{capture, types::Value};
    use std::io;

    fn run(code: &str) -> String {
        capture(code, io::empty(), |_| ()).expect("LOLCODE execution failed")
    }

    fn run_with_callback<F>(code: &str, setup: F) -> String
    where
        F: FnOnce(&mut lci::eval::EvalParams<io::Empty, &mut Vec<u8>>),
    {
        capture(code, io::empty(), setup).expect("LOLCODE execution failed")
    }

    #[test]
    fn test_variables_and_assignment() {
        let code = "\
HAI 1.2
I HAS A X ITZ 42
I HAS A Y ITZ 10
Y R 99
VISIBLE X
VISIBLE Y
KTHXBYE";
        assert_eq!(run(code), "42\n99\n");
    }

    #[test]
    fn test_conditions_if_else() {
        let code = "\
HAI 1.2
I HAS A X ITZ 5
BOTH SAEM X AN 5
O RLY?
    YA RLY
        VISIBLE \"yes\"
    NO WAI
        VISIBLE \"no\"
OIC
KTHXBYE";
        assert_eq!(run(code), "yes\n");
    }

    #[test]
    fn test_conditions_mebbe() {
        let code = "\
HAI 1.2
I HAS A X ITZ 3
BOTH SAEM X AN 5
O RLY?
    YA RLY
        VISIBLE \"five\"
    MEBBE BOTH SAEM X AN 3
        VISIBLE \"three\"
    NO WAI
        VISIBLE \"other\"
OIC
KTHXBYE";
        assert_eq!(run(code), "three\n");
    }

    #[test]
    fn test_loop_uppin() {
        let code = "\
HAI 1.2
I HAS A RESULT ITZ 0
IM IN YR LOOP UPPIN YR I TIL BOTH SAEM I AN 5
    RESULT R SUM OF RESULT AN I
IM OUTTA YR LOOP
VISIBLE RESULT
KTHXBYE";
        // 0+1+2+3+4 = 10
        assert_eq!(run(code), "10\n");
    }

    #[test]
    fn test_loop_nerfin() {
        let code = "\
HAI 1.2
I HAS A RESULT ITZ 0
IM IN YR LOOP NERFIN YR I WILE DIFFRINT I AN -3
    RESULT R SUM OF RESULT AN I
IM OUTTA YR LOOP
VISIBLE RESULT
KTHXBYE";
        // 0 + -1 + -2 = -3
        assert_eq!(run(code), "-3\n");
    }

    #[test]
    fn test_math_operations() {
        let code = "\
HAI 1.2
VISIBLE SUM OF 10 AN 5
VISIBLE DIFF OF 10 AN 3
VISIBLE PRODUKT OF 4 AN 7
VISIBLE QUOSHUNT OF 20 AN 4
VISIBLE MOD OF 17 AN 5
VISIBLE BIGGR OF 3 AN 9
VISIBLE SMALLR OF 3 AN 9
KTHXBYE";
        assert_eq!(run(code), "15\n7\n28\n5\n2\n9\n3\n");
    }

    #[test]
    fn test_string_concatenation() {
        let code = "\
HAI 1.2
I HAS A NAME ITZ \"Thorin\"
I HAS A GREETING ITZ SMOOSH \"Hello \" AN NAME AN \"!\" MKAY
VISIBLE GREETING
KTHXBYE";
        assert_eq!(run(code), "Hello Thorin!\n");
    }

    #[test]
    fn test_string_interpolation() {
        let code = "\
HAI 1.2
I HAS A HP ITZ 35
VISIBLE \"Health:: :{HP}\"
KTHXBYE";
        assert_eq!(run(code), "Health: 35\n");
    }

    #[test]
    fn test_functions() {
        let code = "\
HAI 1.2
HOW IZ I DOUBLE YR N
    FOUND YR PRODUKT OF N AN 2
IF U SAY SO
VISIBLE I IZ DOUBLE YR 21 MKAY
KTHXBYE";
        assert_eq!(run(code), "42\n");
    }

    #[test]
    fn test_boolean_logic() {
        let code = "\
HAI 1.2
VISIBLE BOTH OF WIN AN WIN
VISIBLE EITHER OF FAIL AN WIN
VISIBLE WON OF WIN AN WIN
VISIBLE NOT WIN
KTHXBYE";
        assert_eq!(run(code), "WIN\nWIN\nFAIL\nFAIL\n");
    }

    #[test]
    fn test_rust_callback_integration() {
        let code = "\
HAI 1.2
I HAS A RESULT ITZ I IZ RUSTORY_GET_STAT YR \"strength\" MKAY
VISIBLE RESULT
KTHXBYE";
        let output = run_with_callback(code, |eval| {
            eval.bind_func("RUSTORY_GET_STAT", Some(1), |args| {
                let stat_name = args[0].cast_yarn().unwrap().to_string();
                if stat_name == "strength" {
                    Value::Numbr(18)
                } else {
                    Value::Noob
                }
            });
        });
        assert_eq!(output, "18\n");
    }

    #[test]
    fn test_callback_multiple_args() {
        let code = "\
HAI 1.2
I IZ RUSTORY_SET_STAT YR \"hp\" AN YR 42 MKAY
KTHXBYE";
        let called = std::sync::Arc::new(std::sync::Mutex::new(None));
        let called_clone = called.clone();
        let output = run_with_callback(code, |eval| {
            eval.bind_func("RUSTORY_SET_STAT", Some(2), move |args| {
                let name = args[0].cast_yarn().unwrap().to_string();
                let value = args[1].cast_numbr().unwrap();
                *called_clone.lock().unwrap() = Some((name, value));
                Value::Troof(true)
            });
        });
        assert_eq!(output, "");
        let result = called.lock().unwrap().clone().unwrap();
        assert_eq!(result, ("hp".to_string(), 42));
    }

    #[test]
    fn test_switch_statement() {
        let code = "\
HAI 1.2
I HAS A COLOR ITZ \"blue\"
COLOR
WTF?
    OMG \"red\"
        VISIBLE \"danger\"
        GTFO
    OMG \"blue\"
        VISIBLE \"water\"
        GTFO
    OMGWTF
        VISIBLE \"unknown\"
OIC
KTHXBYE";
        assert_eq!(run(code), "water\n");
    }

    #[test]
    fn test_float_math() {
        let code = "\
HAI 1.2
I HAS A X ITZ 3.14
I HAS A Y ITZ 2.0
VISIBLE PRODUKT OF X AN Y
KTHXBYE";
        assert_eq!(run(code), "6.28\n");
    }

    #[test]
    fn test_type_casting_in_comparison() {
        let code = "\
HAI 1.2
I HAS A X ITZ 5
I HAS A Y ITZ \"5\"
BOTH SAEM X AN Y
O RLY?
    YA RLY
        VISIBLE \"equal\"
    NO WAI
        VISIBLE \"not equal\"
OIC
KTHXBYE";
        assert_eq!(run(code), "equal\n");
    }

    #[test]
    fn test_error_handling_divide_by_zero() {
        let code = "\
HAI 1.2
VISIBLE QUOSHUNT OF 10 AN 0
KTHXBYE";
        let result = lci::capture(code, io::empty(), |_| ());
        assert!(result.is_err());
    }

    #[test]
    fn test_error_handling_undefined_variable() {
        let code = "\
HAI 1.2
VISIBLE GHOST_VAR
KTHXBYE";
        let result = lci::capture(code, io::empty(), |_| ());
        assert!(result.is_err());
    }
}
