use std::collections::HashMap;

use super::primitives::{
    Collection, CollectionItem, Condition, Derived, Gauge, Modifier, Pool, Stat, Tag,
};

#[derive(Debug, Clone)]
pub struct Character {
    pub name: String,
    pub stats: Vec<Stat>,
    pub derived: Vec<Derived>,
    pub gauges: HashMap<String, Gauge>,
    pub pools: HashMap<String, Pool>,
    pub conditions: Vec<Condition>,
    pub modifiers: Vec<Modifier>,
    pub tags: Vec<Tag>,
    pub inventory: Collection,
    pub lore: Option<String>,
    pub dialogues: Option<String>,
    pub location: Option<String>,
}

impl Character {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stats: Vec::new(),
            derived: Vec::new(),
            gauges: HashMap::new(),
            pools: HashMap::new(),
            conditions: Vec::new(),
            modifiers: Vec::new(),
            tags: Vec::new(),
            inventory: Collection::new("inventory"),
            lore: None,
            dialogues: None,
            location: None,
        }
    }

    /// Builder: add a stat to the character.
    pub fn with_stat(mut self, name: &str, value: f64) -> Self {
        self.stats.push(Stat::new(name, value));
        self
    }

    /// Builder: add a gauge to the character.
    pub fn with_gauge(mut self, name: &str, max: f64) -> Self {
        self.gauges.insert(name.to_string(), Gauge::new(name, max));
        self
    }

    /// Builder: add a pool to the character.
    pub fn with_pool(
        mut self,
        name: &str,
        max: f64,
        resets_on: super::primitives::ResetTrigger,
    ) -> Self {
        self.pools
            .insert(name.to_string(), Pool::new(name, max, resets_on));
        self
    }

    // --- Stat accessors ---

    pub fn get_stat(&self, name: &str) -> Option<f64> {
        self.stats.iter().find(|s| s.name == name).map(|s| s.value)
    }

    pub fn set_stat(&mut self, name: &str, value: f64) {
        if let Some(stat) = self.stats.iter_mut().find(|s| s.name == name) {
            stat.value = value;
        } else {
            self.stats.push(Stat::new(name, value));
        }
    }

    // --- Gauge accessors ---

    pub fn get_gauge(&self, name: &str) -> Option<&Gauge> {
        self.gauges.get(name)
    }

    pub fn get_gauge_mut(&mut self, name: &str) -> Option<&mut Gauge> {
        self.gauges.get_mut(name)
    }

    pub fn damage(&mut self, gauge_name: &str, amount: f64) -> bool {
        if let Some(gauge) = self.gauges.get_mut(gauge_name) {
            gauge.damage(amount);
            true
        } else {
            false
        }
    }

    pub fn heal(&mut self, gauge_name: &str, amount: f64) -> bool {
        if let Some(gauge) = self.gauges.get_mut(gauge_name) {
            gauge.heal(amount);
            true
        } else {
            false
        }
    }

    // --- Pool accessors ---

    pub fn get_pool(&self, name: &str) -> Option<&Pool> {
        self.pools.get(name)
    }

    pub fn spend_pool(&mut self, name: &str, amount: f64) -> Option<bool> {
        self.pools.get_mut(name).map(|pool| pool.spend(amount))
    }

    pub fn restore_pool(&mut self, name: &str, amount: f64) -> bool {
        if let Some(pool) = self.pools.get_mut(name) {
            pool.restore(amount);
            true
        } else {
            false
        }
    }

    // --- Condition accessors ---

    pub fn add_condition(&mut self, name: &str) {
        if !self.has_condition(name) {
            self.conditions.push(Condition::new(name));
        }
    }

    pub fn remove_condition(&mut self, name: &str) -> bool {
        let len_before = self.conditions.len();
        self.conditions.retain(|c| c.name != name);
        self.conditions.len() < len_before
    }

    pub fn has_condition(&self, name: &str) -> bool {
        self.conditions.iter().any(|c| c.name == name && c.active)
    }

    // --- Modifier accessors ---

    pub fn add_modifier(&mut self, modifier: Modifier) {
        self.modifiers.push(modifier);
    }

    pub fn remove_modifier(&mut self, name: &str) -> bool {
        let len_before = self.modifiers.len();
        self.modifiers.retain(|m| m.name != name);
        self.modifiers.len() < len_before
    }

    pub fn get_modifiers_for(&self, target: &str) -> Vec<&Modifier> {
        self.modifiers
            .iter()
            .filter(|m| m.target == target)
            .collect()
    }

    // --- Tag accessors ---

    pub fn add_tag(&mut self, name: &str) {
        if !self.has_tag(name) {
            self.tags.push(Tag::new(name));
        }
    }

    pub fn remove_tag(&mut self, name: &str) -> bool {
        let len_before = self.tags.len();
        self.tags.retain(|t| t.name != name);
        self.tags.len() < len_before
    }

    pub fn has_tag(&self, name: &str) -> bool {
        self.tags.iter().any(|t| t.name == name)
    }

    // --- Inventory accessors ---

    pub fn add_item(&mut self, item: CollectionItem) {
        self.inventory.add_item(item);
    }

    pub fn remove_item(&mut self, item_name: &str) -> bool {
        self.inventory.remove_item(item_name)
    }

    pub fn get_item(&self, item_name: &str) -> Option<&CollectionItem> {
        self.inventory.get_item(item_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::{ModifierEffect, ResetTrigger};

    // --- Construction ---

    #[test]
    fn test_character_new() {
        let c = Character::new("Thorin");
        assert_eq!(c.name, "Thorin");
        assert!(c.stats.is_empty());
        assert!(c.derived.is_empty());
        assert!(c.gauges.is_empty());
        assert!(c.pools.is_empty());
        assert!(c.conditions.is_empty());
        assert!(c.modifiers.is_empty());
        assert!(c.tags.is_empty());
        assert!(c.inventory.items.is_empty());
        assert!(c.lore.is_none());
        assert!(c.dialogues.is_none());
    }

    #[test]
    fn test_character_with_stat_chaining() {
        let c = Character::new("Hero")
            .with_stat("strength", 18.0)
            .with_stat("dexterity", 14.0)
            .with_stat("constitution", 12.0);
        assert_eq!(c.stats.len(), 3);
        assert_eq!(c.get_stat("strength"), Some(18.0));
        assert_eq!(c.get_stat("dexterity"), Some(14.0));
        assert_eq!(c.get_stat("constitution"), Some(12.0));
    }

    #[test]
    fn test_character_with_gauge() {
        let c = Character::new("Hero").with_gauge("hp", 52.0);
        let gauge = c.get_gauge("hp").unwrap();
        assert_eq!(gauge.current, 52.0);
        assert_eq!(gauge.max, 52.0);
    }

    #[test]
    fn test_character_with_pool() {
        let c = Character::new("Hero").with_pool("spell_slots", 4.0, ResetTrigger::LongRest);
        let pool = c.get_pool("spell_slots").unwrap();
        assert_eq!(pool.current, 4.0);
        assert_eq!(pool.max, 4.0);
    }

    // --- Stat methods ---

    #[test]
    fn test_get_stat_existing() {
        let c = Character::new("Hero").with_stat("strength", 18.0);
        assert_eq!(c.get_stat("strength"), Some(18.0));
    }

    #[test]
    fn test_get_stat_missing() {
        let c = Character::new("Hero");
        assert_eq!(c.get_stat("strength"), None);
    }

    #[test]
    fn test_set_stat_existing() {
        let mut c = Character::new("Hero").with_stat("strength", 18.0);
        c.set_stat("strength", 20.0);
        assert_eq!(c.get_stat("strength"), Some(20.0));
        assert_eq!(c.stats.len(), 1);
    }

    #[test]
    fn test_set_stat_new() {
        let mut c = Character::new("Hero");
        c.set_stat("charisma", 16.0);
        assert_eq!(c.get_stat("charisma"), Some(16.0));
        assert_eq!(c.stats.len(), 1);
    }

    // --- Gauge methods ---

    #[test]
    fn test_damage_existing_gauge() {
        let mut c = Character::new("Hero").with_gauge("hp", 52.0);
        assert!(c.damage("hp", 15.0));
        assert_eq!(c.get_gauge("hp").unwrap().current, 37.0);
    }

    #[test]
    fn test_damage_missing_gauge() {
        let mut c = Character::new("Hero");
        assert!(!c.damage("hp", 15.0));
    }

    #[test]
    fn test_damage_cannot_go_below_zero() {
        let mut c = Character::new("Hero").with_gauge("hp", 10.0);
        c.damage("hp", 25.0);
        assert_eq!(c.get_gauge("hp").unwrap().current, 0.0);
    }

    #[test]
    fn test_heal_existing_gauge() {
        let mut c = Character::new("Hero").with_gauge("hp", 52.0);
        c.damage("hp", 20.0);
        assert!(c.heal("hp", 10.0));
        assert_eq!(c.get_gauge("hp").unwrap().current, 42.0);
    }

    #[test]
    fn test_heal_missing_gauge() {
        let mut c = Character::new("Hero");
        assert!(!c.heal("hp", 10.0));
    }

    #[test]
    fn test_heal_cannot_exceed_max() {
        let mut c = Character::new("Hero").with_gauge("hp", 52.0);
        c.damage("hp", 5.0);
        c.heal("hp", 100.0);
        assert_eq!(c.get_gauge("hp").unwrap().current, 52.0);
    }

    // --- Pool methods ---

    #[test]
    fn test_spend_pool_success() {
        let mut c = Character::new("Hero").with_pool("slots", 4.0, ResetTrigger::LongRest);
        assert_eq!(c.spend_pool("slots", 2.0), Some(true));
        assert_eq!(c.get_pool("slots").unwrap().current, 2.0);
    }

    #[test]
    fn test_spend_pool_insufficient() {
        let mut c = Character::new("Hero").with_pool("slots", 2.0, ResetTrigger::LongRest);
        assert_eq!(c.spend_pool("slots", 5.0), Some(false));
        assert_eq!(c.get_pool("slots").unwrap().current, 2.0);
    }

    #[test]
    fn test_spend_pool_missing() {
        let mut c = Character::new("Hero");
        assert_eq!(c.spend_pool("slots", 1.0), None);
    }

    #[test]
    fn test_restore_pool() {
        let mut c = Character::new("Hero").with_pool("slots", 4.0, ResetTrigger::LongRest);
        c.spend_pool("slots", 3.0);
        assert!(c.restore_pool("slots", 2.0));
        assert_eq!(c.get_pool("slots").unwrap().current, 3.0);
    }

    #[test]
    fn test_restore_pool_missing() {
        let mut c = Character::new("Hero");
        assert!(!c.restore_pool("slots", 1.0));
    }

    // --- Condition methods ---

    #[test]
    fn test_add_condition() {
        let mut c = Character::new("Hero");
        c.add_condition("Stunned");
        assert!(c.has_condition("Stunned"));
        assert_eq!(c.conditions.len(), 1);
    }

    #[test]
    fn test_add_condition_duplicate_ignored() {
        let mut c = Character::new("Hero");
        c.add_condition("Stunned");
        c.add_condition("Stunned");
        assert_eq!(c.conditions.len(), 1);
    }

    #[test]
    fn test_remove_condition() {
        let mut c = Character::new("Hero");
        c.add_condition("Stunned");
        c.add_condition("Poisoned");
        assert!(c.remove_condition("Stunned"));
        assert!(!c.has_condition("Stunned"));
        assert!(c.has_condition("Poisoned"));
    }

    #[test]
    fn test_remove_condition_missing() {
        let mut c = Character::new("Hero");
        assert!(!c.remove_condition("Stunned"));
    }

    #[test]
    fn test_has_condition_inactive() {
        let mut c = Character::new("Hero");
        c.add_condition("Stunned");
        c.conditions[0].toggle();
        assert!(!c.has_condition("Stunned"));
    }

    // --- Modifier methods ---

    #[test]
    fn test_add_modifier() {
        let mut c = Character::new("Hero");
        c.add_modifier(Modifier::new(
            "bless",
            "attack_roll",
            ModifierEffect::Add(1.0),
        ));
        assert_eq!(c.modifiers.len(), 1);
    }

    #[test]
    fn test_remove_modifier() {
        let mut c = Character::new("Hero");
        c.add_modifier(Modifier::new(
            "bless",
            "attack_roll",
            ModifierEffect::Add(1.0),
        ));
        c.add_modifier(Modifier::new("shield", "ac", ModifierEffect::Add(2.0)));
        assert!(c.remove_modifier("bless"));
        assert_eq!(c.modifiers.len(), 1);
        assert_eq!(c.modifiers[0].name, "shield");
    }

    #[test]
    fn test_remove_modifier_missing() {
        let mut c = Character::new("Hero");
        assert!(!c.remove_modifier("bless"));
    }

    #[test]
    fn test_get_modifiers_for_target() {
        let mut c = Character::new("Hero");
        c.add_modifier(Modifier::new(
            "bless",
            "attack_roll",
            ModifierEffect::Add(1.0),
        ));
        c.add_modifier(Modifier::new("shield", "ac", ModifierEffect::Add(2.0)));
        c.add_modifier(Modifier::new(
            "guidance",
            "attack_roll",
            ModifierEffect::Add(1.0),
        ));
        let mods = c.get_modifiers_for("attack_roll");
        assert_eq!(mods.len(), 2);
    }

    // --- Tag methods ---

    #[test]
    fn test_add_tag() {
        let mut c = Character::new("Hero");
        c.add_tag("Darkvision");
        assert!(c.has_tag("Darkvision"));
    }

    #[test]
    fn test_add_tag_duplicate_ignored() {
        let mut c = Character::new("Hero");
        c.add_tag("Darkvision");
        c.add_tag("Darkvision");
        assert_eq!(c.tags.len(), 1);
    }

    #[test]
    fn test_remove_tag() {
        let mut c = Character::new("Hero");
        c.add_tag("Darkvision");
        c.add_tag("Flying");
        assert!(c.remove_tag("Darkvision"));
        assert!(!c.has_tag("Darkvision"));
        assert!(c.has_tag("Flying"));
    }

    #[test]
    fn test_remove_tag_missing() {
        let mut c = Character::new("Hero");
        assert!(!c.remove_tag("Darkvision"));
    }

    // --- Inventory methods ---

    #[test]
    fn test_add_item() {
        let mut c = Character::new("Hero");
        c.add_item(CollectionItem::new("Longsword").with_property("weight", "3"));
        assert_eq!(c.inventory.items.len(), 1);
        assert!(c.get_item("Longsword").is_some());
    }

    #[test]
    fn test_remove_item() {
        let mut c = Character::new("Hero");
        c.add_item(CollectionItem::new("Longsword"));
        c.add_item(CollectionItem::new("Shield"));
        assert!(c.remove_item("Longsword"));
        assert!(c.get_item("Longsword").is_none());
        assert!(c.get_item("Shield").is_some());
    }

    #[test]
    fn test_remove_item_missing() {
        let mut c = Character::new("Hero");
        assert!(!c.remove_item("Ghost Item"));
    }

    #[test]
    fn test_get_item() {
        let mut c = Character::new("Hero");
        c.add_item(CollectionItem::new("Potion").with_property("quantity", "3"));
        let item = c.get_item("Potion").unwrap();
        assert_eq!(item.properties["quantity"], "3");
    }

    // --- Lore and dialogues ---

    #[test]
    fn test_lore_and_dialogues() {
        let mut c = Character::new("Goblin King");
        c.lore = Some("A fearsome goblin ruler.".to_string());
        c.dialogues = Some("You dare enter my domain?".to_string());
        assert_eq!(c.lore.as_deref(), Some("A fearsome goblin ruler."));
        assert_eq!(c.dialogues.as_deref(), Some("You dare enter my domain?"));
    }
}
