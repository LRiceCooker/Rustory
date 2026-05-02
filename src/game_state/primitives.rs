use std::collections::HashMap;

/// A named numeric value on a character (e.g., strength: 16, cool: +1).
#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    pub name: String,
    pub value: f64,
}

impl Stat {
    pub fn new(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}

/// A value computed from other values (read-only).
/// e.g., ac = "10 + modifier(dexterity)"
#[derive(Debug, Clone, PartialEq)]
pub struct Derived {
    pub name: String,
    pub formula: String,
}

impl Derived {
    pub fn new(name: &str, formula: &str) -> Self {
        Self {
            name: name.to_string(),
            formula: formula.to_string(),
        }
    }
}

/// A current/max value that fluctuates (e.g., HP, Sanity, Stress).
/// Current is clamped to [0, max].
#[derive(Debug, Clone, PartialEq)]
pub struct Gauge {
    pub name: String,
    pub current: f64,
    pub max: f64,
}

impl Gauge {
    pub fn new(name: &str, max: f64) -> Self {
        Self {
            name: name.to_string(),
            current: max,
            max,
        }
    }

    pub fn damage(&mut self, amount: f64) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f64) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn set_current(&mut self, value: f64) {
        self.current = value.clamp(0.0, self.max);
    }

    pub fn set_max(&mut self, value: f64) {
        self.max = value;
        self.current = self.current.min(self.max).max(0.0);
    }
}

/// When a pool resets (e.g., spell slots reset on long rest).
#[derive(Debug, Clone, PartialEq)]
pub enum ResetTrigger {
    ShortRest,
    LongRest,
    Dawn,
    Manual,
    Custom(String),
}

/// A spendable/restorable resource (e.g., spell slots, Fate points).
/// Current is clamped to [0, max].
#[derive(Debug, Clone, PartialEq)]
pub struct Pool {
    pub name: String,
    pub current: f64,
    pub max: f64,
    pub resets_on: ResetTrigger,
}

impl Pool {
    pub fn new(name: &str, max: f64, resets_on: ResetTrigger) -> Self {
        Self {
            name: name.to_string(),
            current: max,
            max,
            resets_on,
        }
    }

    /// Spend from the pool. Returns false if insufficient resources.
    pub fn spend(&mut self, amount: f64) -> bool {
        if amount > self.current {
            return false;
        }
        self.current -= amount;
        true
    }

    pub fn restore(&mut self, amount: f64) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn reset(&mut self) {
        self.current = self.max;
    }
}

/// A dice formula (e.g., 2d6+3).
#[derive(Debug, Clone, PartialEq)]
pub struct RollFormula {
    pub dice: String,
    pub modifier: i32,
}

impl RollFormula {
    pub fn new(dice: &str, modifier: i32) -> Self {
        Self {
            dice: dice.to_string(),
            modifier,
        }
    }
}

/// How a check is resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionMode {
    /// Roll >= target (D&D style)
    RollOver,
    /// Roll <= target (CoC style)
    RollUnder,
    /// Multiple thresholds (PbtA style)
    Tiered,
}

/// A threshold range for tiered resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Threshold {
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub result: String,
}

impl Threshold {
    pub fn new(min: Option<i32>, max: Option<i32>, result: &str) -> Self {
        Self {
            min,
            max,
            result: result.to_string(),
        }
    }

    pub fn matches(&self, value: i32) -> bool {
        let above_min = self.min.is_none_or(|m| value >= m);
        let below_max = self.max.is_none_or(|m| value <= m);
        above_min && below_max
    }
}

/// How a roll is resolved against a target.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub name: String,
    pub roll: String,
    pub resolution_mode: ResolutionMode,
    pub thresholds: Vec<Threshold>,
}

impl Check {
    pub fn new(name: &str, roll: &str, resolution_mode: ResolutionMode) -> Self {
        Self {
            name: name.to_string(),
            roll: roll.to_string(),
            resolution_mode,
            thresholds: Vec::new(),
        }
    }

    pub fn with_threshold(mut self, threshold: Threshold) -> Self {
        self.thresholds.push(threshold);
        self
    }
}

/// A named state on a character (e.g., Stunned, Poisoned).
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub name: String,
    pub active: bool,
}

impl Condition {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            active: true,
        }
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }
}

/// What a modifier affects.
#[derive(Debug, Clone, PartialEq)]
pub enum ModifierEffect {
    Add(f64),
    Multiply(f64),
    Advantage,
    Disadvantage,
    Custom(String),
}

/// A temporary alteration to a roll or stat.
#[derive(Debug, Clone, PartialEq)]
pub struct Modifier {
    pub name: String,
    pub target: String,
    pub effect: ModifierEffect,
}

impl Modifier {
    pub fn new(name: &str, target: &str, effect: ModifierEffect) -> Self {
        Self {
            name: name.to_string(),
            target: target.to_string(),
            effect,
        }
    }
}

/// "When X happens, do Y" — automatic reactions.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub event: String,
    pub action: String,
}

impl Trigger {
    pub fn new(event: &str, action: &str) -> Self {
        Self {
            event: event.to_string(),
            action: action.to_string(),
        }
    }
}

/// A non-numeric narrative label (e.g., Aspects in FATE, traits, features).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

/// An item in a collection.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionItem {
    pub name: String,
    pub properties: HashMap<String, String>,
}

impl CollectionItem {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }
}

/// A list of items/spells/abilities (e.g., Inventory, spell list).
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub name: String,
    pub items: Vec<CollectionItem>,
}

impl Collection {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item: CollectionItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, item_name: &str) -> bool {
        let len_before = self.items.len();
        self.items.retain(|i| i.name != item_name);
        self.items.len() < len_before
    }

    pub fn get_item(&self, item_name: &str) -> Option<&CollectionItem> {
        self.items.iter().find(|i| i.name == item_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Stat ---

    #[test]
    fn test_stat_new() {
        let s = Stat::new("strength", 18.0);
        assert_eq!(s.name, "strength");
        assert_eq!(s.value, 18.0);
    }

    #[test]
    fn test_stat_modify() {
        let mut s = Stat::new("dexterity", 14.0);
        s.value = 16.0;
        assert_eq!(s.value, 16.0);
    }

    #[test]
    fn test_stat_negative_value() {
        let s = Stat::new("modifier", -2.0);
        assert_eq!(s.value, -2.0);
    }

    // --- Derived ---

    #[test]
    fn test_derived_new() {
        let d = Derived::new("ac", "10 + modifier(dexterity)");
        assert_eq!(d.name, "ac");
        assert_eq!(d.formula, "10 + modifier(dexterity)");
    }

    // --- Gauge ---

    #[test]
    fn test_gauge_new() {
        let g = Gauge::new("hp", 52.0);
        assert_eq!(g.name, "hp");
        assert_eq!(g.current, 52.0);
        assert_eq!(g.max, 52.0);
    }

    #[test]
    fn test_gauge_damage() {
        let mut g = Gauge::new("hp", 52.0);
        g.damage(15.0);
        assert_eq!(g.current, 37.0);
    }

    #[test]
    fn test_gauge_damage_cannot_go_below_zero() {
        let mut g = Gauge::new("hp", 10.0);
        g.damage(25.0);
        assert_eq!(g.current, 0.0);
    }

    #[test]
    fn test_gauge_heal() {
        let mut g = Gauge::new("hp", 52.0);
        g.damage(20.0);
        g.heal(10.0);
        assert_eq!(g.current, 42.0);
    }

    #[test]
    fn test_gauge_heal_cannot_exceed_max() {
        let mut g = Gauge::new("hp", 52.0);
        g.damage(5.0);
        g.heal(100.0);
        assert_eq!(g.current, 52.0);
    }

    #[test]
    fn test_gauge_set_current_clamped() {
        let mut g = Gauge::new("hp", 50.0);
        g.set_current(100.0);
        assert_eq!(g.current, 50.0);
        g.set_current(-10.0);
        assert_eq!(g.current, 0.0);
        g.set_current(25.0);
        assert_eq!(g.current, 25.0);
    }

    #[test]
    fn test_gauge_set_max_clamps_current() {
        let mut g = Gauge::new("hp", 50.0);
        g.set_max(30.0);
        assert_eq!(g.max, 30.0);
        assert_eq!(g.current, 30.0);
    }

    // --- Pool ---

    #[test]
    fn test_pool_new() {
        let p = Pool::new("spell_slots", 4.0, ResetTrigger::LongRest);
        assert_eq!(p.name, "spell_slots");
        assert_eq!(p.current, 4.0);
        assert_eq!(p.max, 4.0);
        assert_eq!(p.resets_on, ResetTrigger::LongRest);
    }

    #[test]
    fn test_pool_spend_success() {
        let mut p = Pool::new("slots", 3.0, ResetTrigger::LongRest);
        assert!(p.spend(2.0));
        assert_eq!(p.current, 1.0);
    }

    #[test]
    fn test_pool_spend_cannot_overspend() {
        let mut p = Pool::new("slots", 3.0, ResetTrigger::LongRest);
        assert!(!p.spend(5.0));
        assert_eq!(p.current, 3.0);
    }

    #[test]
    fn test_pool_spend_exact() {
        let mut p = Pool::new("slots", 3.0, ResetTrigger::LongRest);
        assert!(p.spend(3.0));
        assert_eq!(p.current, 0.0);
    }

    #[test]
    fn test_pool_restore() {
        let mut p = Pool::new("slots", 4.0, ResetTrigger::LongRest);
        p.spend(3.0);
        p.restore(2.0);
        assert_eq!(p.current, 3.0);
    }

    #[test]
    fn test_pool_restore_cannot_exceed_max() {
        let mut p = Pool::new("slots", 4.0, ResetTrigger::LongRest);
        p.spend(1.0);
        p.restore(100.0);
        assert_eq!(p.current, 4.0);
    }

    #[test]
    fn test_pool_reset() {
        let mut p = Pool::new("slots", 4.0, ResetTrigger::LongRest);
        p.spend(4.0);
        assert_eq!(p.current, 0.0);
        p.reset();
        assert_eq!(p.current, 4.0);
    }

    // --- RollFormula ---

    #[test]
    fn test_roll_formula_new() {
        let r = RollFormula::new("2d6", 3);
        assert_eq!(r.dice, "2d6");
        assert_eq!(r.modifier, 3);
    }

    #[test]
    fn test_roll_formula_negative_modifier() {
        let r = RollFormula::new("1d20", -2);
        assert_eq!(r.modifier, -2);
    }

    // --- Check ---

    #[test]
    fn test_check_roll_over() {
        let c = Check::new("ability_check", "1d20 + modifier", ResolutionMode::RollOver);
        assert_eq!(c.name, "ability_check");
        assert_eq!(c.resolution_mode, ResolutionMode::RollOver);
        assert!(c.thresholds.is_empty());
    }

    #[test]
    fn test_check_tiered_with_thresholds() {
        let c = Check::new("move", "2d6 + stat", ResolutionMode::Tiered)
            .with_threshold(Threshold::new(None, Some(6), "miss"))
            .with_threshold(Threshold::new(Some(7), Some(9), "partial"))
            .with_threshold(Threshold::new(Some(10), None, "success"));
        assert_eq!(c.thresholds.len(), 3);
    }

    #[test]
    fn test_threshold_matches() {
        let miss = Threshold::new(None, Some(6), "miss");
        let partial = Threshold::new(Some(7), Some(9), "partial");
        let success = Threshold::new(Some(10), None, "success");

        assert!(miss.matches(3));
        assert!(miss.matches(6));
        assert!(!miss.matches(7));

        assert!(!partial.matches(6));
        assert!(partial.matches(7));
        assert!(partial.matches(9));
        assert!(!partial.matches(10));

        assert!(!success.matches(9));
        assert!(success.matches(10));
        assert!(success.matches(15));
    }

    // --- Condition ---

    #[test]
    fn test_condition_new() {
        let c = Condition::new("Stunned");
        assert_eq!(c.name, "Stunned");
        assert!(c.active);
    }

    #[test]
    fn test_condition_toggle() {
        let mut c = Condition::new("Poisoned");
        assert!(c.active);
        c.toggle();
        assert!(!c.active);
        c.toggle();
        assert!(c.active);
    }

    // --- Modifier ---

    #[test]
    fn test_modifier_add() {
        let m = Modifier::new("bless", "attack_roll", ModifierEffect::Add(1.0));
        assert_eq!(m.name, "bless");
        assert_eq!(m.target, "attack_roll");
        assert_eq!(m.effect, ModifierEffect::Add(1.0));
    }

    #[test]
    fn test_modifier_advantage() {
        let m = Modifier::new("advantage", "attack_roll", ModifierEffect::Advantage);
        assert_eq!(m.effect, ModifierEffect::Advantage);
    }

    // --- Trigger ---

    #[test]
    fn test_trigger_new() {
        let t = Trigger::new("hp_reaches_0", "death_saves");
        assert_eq!(t.event, "hp_reaches_0");
        assert_eq!(t.action, "death_saves");
    }

    // --- Tag ---

    #[test]
    fn test_tag_new() {
        let t = Tag::new("Darkvision");
        assert_eq!(t.name, "Darkvision");
    }

    #[test]
    fn test_tag_equality() {
        let t1 = Tag::new("Flying");
        let t2 = Tag::new("Flying");
        let t3 = Tag::new("Swimming");
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    // --- Collection ---

    #[test]
    fn test_collection_new() {
        let c = Collection::new("inventory");
        assert_eq!(c.name, "inventory");
        assert!(c.items.is_empty());
    }

    #[test]
    fn test_collection_add_item() {
        let mut c = Collection::new("inventory");
        c.add_item(
            CollectionItem::new("Longsword")
                .with_property("weight", "3")
                .with_property("notes", "+1 magical"),
        );
        assert_eq!(c.items.len(), 1);
        assert_eq!(c.items[0].name, "Longsword");
        assert_eq!(c.items[0].properties["weight"], "3");
    }

    #[test]
    fn test_collection_remove_item() {
        let mut c = Collection::new("inventory");
        c.add_item(CollectionItem::new("Sword"));
        c.add_item(CollectionItem::new("Shield"));
        c.add_item(CollectionItem::new("Potion"));

        assert!(c.remove_item("Shield"));
        assert_eq!(c.items.len(), 2);
        assert!(c.get_item("Shield").is_none());
    }

    #[test]
    fn test_collection_remove_nonexistent_item() {
        let mut c = Collection::new("inventory");
        c.add_item(CollectionItem::new("Sword"));
        assert!(!c.remove_item("Ghost Item"));
        assert_eq!(c.items.len(), 1);
    }

    #[test]
    fn test_collection_get_item() {
        let mut c = Collection::new("inventory");
        c.add_item(CollectionItem::new("Healing Potion").with_property("quantity", "3"));

        let item = c.get_item("Healing Potion");
        assert!(item.is_some());
        assert_eq!(item.unwrap().properties["quantity"], "3");

        assert!(c.get_item("Missing").is_none());
    }

    #[test]
    fn test_collection_item_with_properties() {
        let item = CollectionItem::new("Rope")
            .with_property("length", "50ft")
            .with_property("weight", "10");
        assert_eq!(item.name, "Rope");
        assert_eq!(item.properties.len(), 2);
    }
}
