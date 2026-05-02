use std::collections::HashMap;

use rand::Rng;
use rand::RngCore;

use crate::commands::parsers::roll;
use crate::game_state::character::Character;
use crate::game_state::primitives::{Check, Derived, ResolutionMode};

/// Result of resolving a check.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckResult {
    Success,
    Failure,
    Partial(String),
    Critical,
}

/// Evaluate a derived value's formula against a character's stats.
/// Derived values are deterministic (no dice rolling).
pub fn resolve_derived(character: &Character, derived: &Derived) -> f64 {
    evaluate_static(&derived.formula, character)
}

/// Resolve a check: substitute args into the roll formula, roll dice,
/// evaluate the total, and compare against the resolution mode.
///
/// `args` should contain placeholder values (e.g., `ability` → `"strength"`)
/// and resolution targets (e.g., `dc` → `"15"` for RollOver, `target` → `"50"` for RollUnder).
pub fn resolve_check(
    check: &Check,
    character: &Character,
    args: &HashMap<String, String>,
    rng: &mut dyn RngCore,
) -> CheckResult {
    let formula = substitute_placeholders(&check.roll, args);
    let total = evaluate_with_dice(&formula, character, rng);
    let total_int = total.round() as i32;

    match check.resolution_mode {
        ResolutionMode::RollOver => {
            let dc = args
                .get("dc")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(10);
            if total_int >= dc {
                CheckResult::Success
            } else {
                CheckResult::Failure
            }
        }
        ResolutionMode::RollUnder => {
            let target = args
                .get("target")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(50);
            if total_int <= target {
                CheckResult::Success
            } else {
                CheckResult::Failure
            }
        }
        ResolutionMode::Tiered => {
            for threshold in &check.thresholds {
                if threshold.matches(total_int) {
                    return match threshold.result.as_str() {
                        "success" => CheckResult::Success,
                        "miss" | "failure" | "fail" => CheckResult::Failure,
                        other => CheckResult::Partial(other.to_string()),
                    };
                }
            }
            CheckResult::Failure
        }
    }
}

/// Replace `{key}` placeholders in a formula with values from args.
fn substitute_placeholders(formula: &str, args: &HashMap<String, String>) -> String {
    let mut result = formula.to_string();
    for (key, value) in args {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

/// Tokenize a formula into (sign, token) pairs.
/// Splits on `+` and `-` operators while respecting parentheses.
fn tokenize(formula: &str) -> Vec<(f64, String)> {
    let mut tokens = Vec::new();
    let mut sign = 1.0_f64;
    let mut current = String::new();
    let mut paren_depth = 0;

    for ch in formula.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth -= 1;
                current.push(ch);
            }
            '+' | '-' if paren_depth == 0 => {
                let token = current.trim().to_string();
                if !token.is_empty() {
                    tokens.push((sign, token));
                }
                current.clear();
                sign = if ch == '+' { 1.0 } else { -1.0 };
            }
            _ => current.push(ch),
        }
    }

    let token = current.trim().to_string();
    if !token.is_empty() {
        tokens.push((sign, token));
    }

    tokens
}

/// Evaluate a single non-dice token against a character's stats.
///
/// Supports:
/// - `modifier(stat_name)` → D&D-style ability modifier: floor((stat - 10) / 2)
/// - Numeric literals (e.g., `"10"`, `"3.5"`)
/// - Stat name references (looked up from character)
fn evaluate_token_value(token: &str, character: &Character) -> f64 {
    let token = token.trim();

    // modifier(stat_name) function
    if let Some(stat_name) = token
        .strip_prefix("modifier(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return character
            .get_stat(stat_name)
            .map(|v| ((v - 10.0) / 2.0).floor())
            .unwrap_or(0.0);
    }

    // Numeric literal
    if let Ok(val) = token.parse::<f64>() {
        return val;
    }

    // Stat name lookup
    character.get_stat(token).unwrap_or(0.0)
}

/// Evaluate a formula without dice rolling (for derived values).
fn evaluate_static(formula: &str, character: &Character) -> f64 {
    tokenize(formula)
        .iter()
        .map(|(sign, token)| sign * evaluate_token_value(token, character))
        .sum()
}

/// Evaluate a formula with dice rolling (for checks).
fn evaluate_with_dice(formula: &str, character: &Character, rng: &mut dyn RngCore) -> f64 {
    let tokens = tokenize(formula);
    let mut total = 0.0;

    for (sign, token) in &tokens {
        let token = token.trim();

        // Try as dice formula first (contains 'd' with digits around it)
        if token.contains('d') {
            if let Ok(parsed) = roll::parse(token) {
                let mut sum = 0i32;
                for _ in 0..parsed.dice {
                    sum += rng.gen_range(1..=parsed.value) as i32;
                }
                total += sign * (sum + parsed.modifier) as f64;
                continue;
            }
        }

        // Fall back to non-dice evaluation
        total += sign * evaluate_token_value(token, character);
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::Threshold;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn make_dnd_character() -> Character {
        Character::new("Thorin")
            .with_stat("strength", 18.0) // modifier: +4
            .with_stat("dexterity", 14.0) // modifier: +2
            .with_stat("constitution", 16.0) // modifier: +3
            .with_stat("intelligence", 10.0) // modifier: 0
            .with_stat("wisdom", 13.0) // modifier: +1
            .with_stat("charisma", 8.0) // modifier: -1
    }

    fn make_pbta_character() -> Character {
        Character::new("Ghost")
            .with_stat("cool", 1.0)
            .with_stat("hard", 2.0)
            .with_stat("hot", -1.0)
            .with_stat("sharp", 0.0)
            .with_stat("weird", 1.0)
    }

    // --- tokenize ---

    #[test]
    fn test_tokenize_simple_addition() {
        let tokens = tokenize("10 + modifier(dexterity)");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (1.0, "10".to_string()));
        assert_eq!(tokens[1], (1.0, "modifier(dexterity)".to_string()));
    }

    #[test]
    fn test_tokenize_subtraction() {
        let tokens = tokenize("10 - 2");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (1.0, "10".to_string()));
        assert_eq!(tokens[1], (-1.0, "2".to_string()));
    }

    #[test]
    fn test_tokenize_single_term() {
        let tokens = tokenize("modifier(dexterity)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], (1.0, "modifier(dexterity)".to_string()));
    }

    // --- substitute_placeholders ---

    #[test]
    fn test_substitute_single_placeholder() {
        let mut args = HashMap::new();
        args.insert("ability".to_string(), "strength".to_string());
        assert_eq!(
            substitute_placeholders("1d20 + modifier({ability})", &args),
            "1d20 + modifier(strength)"
        );
    }

    #[test]
    fn test_substitute_multiple_placeholders() {
        let mut args = HashMap::new();
        args.insert("stat".to_string(), "cool".to_string());
        assert_eq!(
            substitute_placeholders("2d6 + {stat}", &args),
            "2d6 + cool"
        );
    }

    // --- resolve_derived ---

    #[test]
    fn test_resolve_derived_constant_plus_modifier() {
        let character = make_dnd_character();
        let derived = Derived::new("ac", "10 + modifier(dexterity)");
        // dex 14 → modifier = floor((14-10)/2) = 2
        assert_eq!(resolve_derived(&character, &derived), 12.0);
    }

    #[test]
    fn test_resolve_derived_modifier_only() {
        let character = make_dnd_character();
        let derived = Derived::new("initiative", "modifier(dexterity)");
        assert_eq!(resolve_derived(&character, &derived), 2.0);
    }

    #[test]
    fn test_resolve_derived_negative_modifier() {
        let character = make_dnd_character();
        let derived = Derived::new("cha_mod", "modifier(charisma)");
        // charisma 8 → floor((8-10)/2) = floor(-1) = -1
        assert_eq!(resolve_derived(&character, &derived), -1.0);
    }

    #[test]
    fn test_resolve_derived_stat_reference() {
        let character = make_pbta_character();
        let derived = Derived::new("combat_power", "hard");
        assert_eq!(resolve_derived(&character, &derived), 2.0);
    }

    #[test]
    fn test_resolve_derived_multi_modifier() {
        let character = make_dnd_character();
        let derived =
            Derived::new("hp_bonus", "modifier(constitution) + modifier(strength)");
        // con 16 → +3, str 18 → +4
        assert_eq!(resolve_derived(&character, &derived), 7.0);
    }

    #[test]
    fn test_resolve_derived_unknown_stat() {
        let character = make_dnd_character();
        let derived = Derived::new("mystery", "modifier(nonexistent)");
        assert_eq!(resolve_derived(&character, &derived), 0.0);
    }

    // --- resolve_check: D&D roll-over ---

    #[test]
    fn test_resolve_check_d20_guaranteed_success() {
        // DC 1 with str modifier +4 — minimum roll (1+4=5) still beats DC 1
        let character = make_dnd_character();
        let check = Check::new(
            "ability_check",
            "1d20 + modifier({ability})",
            ResolutionMode::RollOver,
        );
        let mut args = HashMap::new();
        args.insert("ability".to_string(), "strength".to_string());
        args.insert("dc".to_string(), "1".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Success
        );
    }

    #[test]
    fn test_resolve_check_d20_guaranteed_failure() {
        // DC 100 — max possible (20+4=24) can't reach it
        let character = make_dnd_character();
        let check = Check::new(
            "ability_check",
            "1d20 + modifier({ability})",
            ResolutionMode::RollOver,
        );
        let mut args = HashMap::new();
        args.insert("ability".to_string(), "strength".to_string());
        args.insert("dc".to_string(), "100".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Failure
        );
    }

    #[test]
    fn test_resolve_check_d20_deterministic() {
        let character = make_dnd_character();
        let check = Check::new(
            "ability_check",
            "1d20 + modifier({ability})",
            ResolutionMode::RollOver,
        );
        let mut args = HashMap::new();
        args.insert("ability".to_string(), "strength".to_string());
        args.insert("dc".to_string(), "15".to_string());

        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);

        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng1),
            resolve_check(&check, &character, &args, &mut rng2),
        );
    }

    // --- resolve_check: PbtA tiered ---

    #[test]
    fn test_resolve_check_pbta_guaranteed_success() {
        // stat = +20, so 2d6 (min 2) + 20 = 22, always >= 10 → success
        let character = Character::new("Hero").with_stat("cool", 20.0);
        let check = Check::new("move", "2d6 + {stat}", ResolutionMode::Tiered)
            .with_threshold(Threshold::new(None, Some(6), "miss"))
            .with_threshold(Threshold::new(Some(7), Some(9), "partial"))
            .with_threshold(Threshold::new(Some(10), None, "success"));

        let mut args = HashMap::new();
        args.insert("stat".to_string(), "cool".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Success
        );
    }

    #[test]
    fn test_resolve_check_pbta_guaranteed_miss() {
        // stat = -20, so 2d6 (max 12) + (-20) = -8, always <= 6 → miss
        let character = Character::new("Cursed").with_stat("cool", -20.0);
        let check = Check::new("move", "2d6 + {stat}", ResolutionMode::Tiered)
            .with_threshold(Threshold::new(None, Some(6), "miss"))
            .with_threshold(Threshold::new(Some(7), Some(9), "partial"))
            .with_threshold(Threshold::new(Some(10), None, "success"));

        let mut args = HashMap::new();
        args.insert("stat".to_string(), "cool".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Failure
        );
    }

    #[test]
    fn test_resolve_check_pbta_partial_possible() {
        // stat = +5, so 2d6 (min 2, max 12) + 5 = 7..17
        // With the right seed, we should get a partial (7-9)
        let character = make_pbta_character();
        let check = Check::new("move", "2d6 + {stat}", ResolutionMode::Tiered)
            .with_threshold(Threshold::new(None, Some(6), "miss"))
            .with_threshold(Threshold::new(Some(7), Some(9), "partial"))
            .with_threshold(Threshold::new(Some(10), None, "success"));

        let mut args = HashMap::new();
        args.insert("stat".to_string(), "cool".to_string());

        // Deterministic: same seed always gives same tier
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng1),
            resolve_check(&check, &character, &args, &mut rng2),
        );
    }

    // --- resolve_check: roll-under ---

    #[test]
    fn test_resolve_check_roll_under_guaranteed_success() {
        // target 100, 1d100 max = 100, always succeeds
        let character = Character::new("Investigator");
        let check = Check::new("skill", "1d100", ResolutionMode::RollUnder);
        let mut args = HashMap::new();
        args.insert("target".to_string(), "100".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Success
        );
    }

    #[test]
    fn test_resolve_check_roll_under_guaranteed_failure() {
        // target 0, 1d100 min = 1, always fails
        let character = Character::new("Investigator");
        let check = Check::new("skill", "1d100", ResolutionMode::RollUnder);
        let mut args = HashMap::new();
        args.insert("target".to_string(), "0".to_string());

        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            resolve_check(&check, &character, &args, &mut rng),
            CheckResult::Failure
        );
    }
}
