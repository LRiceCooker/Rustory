use rand::Rng;
use rand::RngCore;

/// Evaluate a math expression that may contain dice notation.
///
/// Supports: +, -, *, /, parentheses, integer/float literals, and dice
/// notation (e.g. `2d6`, `1d20`). Dice are rolled using the provided RNG
/// and their sum substituted into the expression.
///
/// Returns a tuple of (result, description) where description shows rolled
/// dice values for transparency.
pub fn evaluate(input: &str, rng: &mut dyn RngCore) -> Result<(f64, Vec<String>), String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Empty expression".to_string());
    }
    let tokens = tokenize(input)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        rng,
        roll_descriptions: Vec::new(),
    };
    let result = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(format!(
            "Unexpected token: {:?}",
            parser.tokens[parser.pos]
        ));
    }
    Ok((result, parser.roll_descriptions))
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Dice(u32, u32), // (count, sides)
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                // Collect digits (possibly with one decimal point)
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                // Check for dice notation: <number>d<number>
                if i < chars.len() && chars[i] == 'd' && i > start {
                    let count_str: String = chars[start..i].iter().collect();
                    // Check that what we collected is an integer (no dots)
                    if !count_str.contains('.') {
                        let count: u32 = count_str
                            .parse()
                            .map_err(|_| format!("Invalid dice count: \"{count_str}\""))?;
                        if count == 0 {
                            return Err("Dice count must be at least 1".to_string());
                        }
                        i += 1; // skip 'd'
                        let sides_start = i;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        if i == sides_start {
                            return Err("Missing die value after 'd'".to_string());
                        }
                        let sides_str: String = chars[sides_start..i].iter().collect();
                        let sides: u32 = sides_str
                            .parse()
                            .map_err(|_| format!("Invalid die value: \"{sides_str}\""))?;
                        if sides == 0 {
                            return Err("Die value must be at least 1".to_string());
                        }
                        tokens.push(Token::Dice(count, sides));
                        continue;
                    }
                }
                let num_str: String = chars[start..i].iter().collect();
                let num: f64 = num_str
                    .parse()
                    .map_err(|_| format!("Invalid number: \"{num_str}\""))?;
                tokens.push(Token::Number(num));
            }
            c => return Err(format!("Unexpected character: '{c}'")),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    rng: &'a mut dyn RngCore,
    roll_descriptions: Vec<String>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// expr = term (('+' | '-') term)*
    fn parse_expr(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.advance();
                    left += self.parse_term()?;
                }
                Token::Minus => {
                    self.advance();
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// term = unary (('*' | '/') unary)*
    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_unary()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => {
                    self.advance();
                    left *= self.parse_unary()?;
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    if right == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    left /= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// unary = '-' unary | atom
    fn parse_unary(&mut self) -> Result<f64, String> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            let val = self.parse_unary()?;
            return Ok(-val);
        }
        self.parse_atom()
    }

    /// atom = NUMBER | DICE | '(' expr ')'
    fn parse_atom(&mut self) -> Result<f64, String> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(n),
            Some(Token::Dice(count, sides)) => {
                let mut rolls = Vec::new();
                for _ in 0..count {
                    rolls.push(self.rng.gen_range(1..=sides));
                }
                let sum: u32 = rolls.iter().sum();
                self.roll_descriptions.push(format!(
                    "{count}d{sides}: {rolls:?} = {sum}"
                ));
                Ok(sum as f64)
            }
            Some(Token::LParen) => {
                let val = self.parse_expr()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(val),
                    _ => Err("Missing closing parenthesis".to_string()),
                }
            }
            Some(tok) => Err(format!("Expected number or '(', got {tok:?}")),
            None => Err("Unexpected end of expression".to_string()),
        }
    }
}

/// Format a result: show as integer if it's a whole number, otherwise 2 decimals.
pub fn format_result(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < i64::MAX as f64 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn test_rng() -> Box<dyn RngCore> {
        Box::new(StdRng::seed_from_u64(42))
    }

    #[test]
    fn test_basic_addition() {
        let (result, _) = evaluate("15+3+4", &mut *test_rng()).unwrap();
        assert_eq!(result, 22.0);
    }

    #[test]
    fn test_basic_subtraction() {
        let (result, _) = evaluate("20-5-3", &mut *test_rng()).unwrap();
        assert_eq!(result, 12.0);
    }

    #[test]
    fn test_multiplication() {
        let (result, _) = evaluate("3*4", &mut *test_rng()).unwrap();
        assert_eq!(result, 12.0);
    }

    #[test]
    fn test_division() {
        let (result, _) = evaluate("20/4", &mut *test_rng()).unwrap();
        assert_eq!(result, 5.0);
    }

    #[test]
    fn test_operator_precedence() {
        let (result, _) = evaluate("2+3*4", &mut *test_rng()).unwrap();
        assert_eq!(result, 14.0);
    }

    #[test]
    fn test_parentheses() {
        let (result, _) = evaluate("(2+3)*4", &mut *test_rng()).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_nested_parentheses() {
        let (result, _) = evaluate("((2+3)*(4-1))", &mut *test_rng()).unwrap();
        assert_eq!(result, 15.0);
    }

    #[test]
    fn test_unary_minus() {
        let (result, _) = evaluate("-5+10", &mut *test_rng()).unwrap();
        assert_eq!(result, 5.0);
    }

    #[test]
    fn test_dice_in_expression() {
        let mut rng = test_rng();
        let (result, descriptions) = evaluate("2*1d6", &mut *rng).unwrap();
        // With seeded RNG, dice roll is deterministic
        assert!(result > 0.0);
        assert_eq!(descriptions.len(), 1);
        assert!(descriptions[0].starts_with("1d6:"));
    }

    #[test]
    fn test_dice_deterministic() {
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);
        let (r1, _) = evaluate("2d6+1d8", &mut rng1).unwrap();
        let (r2, _) = evaluate("2d6+1d8", &mut rng2).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_dice_plus_constant() {
        let mut rng = test_rng();
        let (result, descriptions) = evaluate("1d20+5", &mut *rng).unwrap();
        // Roll must be between 1 and 20, so result is 6..=25
        assert!(result >= 6.0 && result <= 25.0);
        assert_eq!(descriptions.len(), 1);
    }

    #[test]
    fn test_complex_expression_with_dice() {
        let mut rng = test_rng();
        let (result, descriptions) = evaluate("(2d6+3)*2", &mut *rng).unwrap();
        // 2d6 is 2..=12, +3 is 5..=15, *2 is 10..=30
        assert!(result >= 10.0 && result <= 30.0);
        assert_eq!(descriptions.len(), 1);
    }

    #[test]
    fn test_empty_expression() {
        assert!(evaluate("", &mut *test_rng()).is_err());
    }

    #[test]
    fn test_division_by_zero() {
        assert!(evaluate("5/0", &mut *test_rng()).is_err());
    }

    #[test]
    fn test_missing_closing_paren() {
        assert!(evaluate("(2+3", &mut *test_rng()).is_err());
    }

    #[test]
    fn test_invalid_character() {
        assert!(evaluate("2+abc", &mut *test_rng()).is_err());
    }

    #[test]
    fn test_format_result_integer() {
        assert_eq!(format_result(22.0), "22");
    }

    #[test]
    fn test_format_result_decimal() {
        assert_eq!(format_result(7.5), "7.50");
    }

    #[test]
    fn test_spaces_in_expression() {
        let (result, _) = evaluate("  10 + 5 * 2  ", &mut *test_rng()).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_zero_sided_die() {
        assert!(evaluate("1d0", &mut *test_rng()).is_err());
    }

    #[test]
    fn test_zero_dice_count() {
        assert!(evaluate("0d6", &mut *test_rng()).is_err());
    }
}
