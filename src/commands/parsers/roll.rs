#[derive(Debug, PartialEq)]
pub struct Roll {
    pub dice: u32,
    pub value: u32,
    pub modifier: i32,
}

pub fn parse(input: &str) -> Result<Roll, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Empty roll input".to_string());
    }

    // Split on 'd' to get dice count and the rest
    let parts: Vec<&str> = input.splitn(2, 'd').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid roll format: \"{}\". Expected format: NdV or NdV+M", input));
    }

    let dice: u32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid dice count: \"{}\"", parts[0]))?;

    if dice == 0 {
        return Err("Dice count must be at least 1".to_string());
    }

    // Parse value and optional modifier from the right side
    let right = parts[1];
    if right.is_empty() {
        return Err("Missing die value after 'd'".to_string());
    }
    let (value_str, modifier) = if let Some(pos) = right.find('+') {
        let (v, m) = right.split_at(pos);
        let mod_val: i32 = m[1..]
            .parse()
            .map_err(|_| format!("Invalid modifier: \"{}\"", &m[1..]))?;
        (v, mod_val)
    } else if let Some(pos) = right[1..].find('-') {
        // Skip first char to avoid matching negative sign at start
        let pos = pos + 1;
        let (v, m) = right.split_at(pos);
        let mod_val: i32 = m
            .parse()
            .map_err(|_| format!("Invalid modifier: \"{}\"", m))?;
        (v, mod_val)
    } else {
        (right, 0)
    };

    let value: u32 = value_str
        .parse()
        .map_err(|_| format!("Invalid die value: \"{}\"", value_str))?;

    if value == 0 {
        return Err("Die value must be at least 1".to_string());
    }

    Ok(Roll { dice, value, modifier })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        assert_eq!(
            parse("2d6").unwrap(),
            Roll { dice: 2, value: 6, modifier: 0 }
        );
    }

    #[test]
    fn test_parse_with_positive_modifier() {
        assert_eq!(
            parse("1d20+3").unwrap(),
            Roll { dice: 1, value: 20, modifier: 3 }
        );
    }

    #[test]
    fn test_parse_with_negative_modifier() {
        assert_eq!(
            parse("3d8-2").unwrap(),
            Roll { dice: 3, value: 8, modifier: -2 }
        );
    }

    #[test]
    fn test_parse_invalid_no_d() {
        assert!(parse("abc").is_err());
    }

    #[test]
    fn test_parse_missing_value() {
        assert!(parse("2d").is_err());
    }

    #[test]
    fn test_parse_empty_input() {
        assert!(parse("").is_err());
    }

    #[test]
    fn test_parse_zero_dice() {
        assert!(parse("0d6").is_err());
    }

    #[test]
    fn test_parse_zero_value() {
        assert!(parse("2d0").is_err());
    }

    #[test]
    fn test_parse_single_die() {
        assert_eq!(
            parse("1d100").unwrap(),
            Roll { dice: 1, value: 100, modifier: 0 }
        );
    }

    #[test]
    fn test_parse_missing_count() {
        // "d6" has empty string before 'd'
        assert!(parse("d6").is_err());
    }

    #[test]
    fn test_parse_negative_modifier() {
        assert_eq!(
            parse("1d20-5").unwrap(),
            Roll { dice: 1, value: 20, modifier: -5 }
        );
    }
}
