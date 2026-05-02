use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use lci::types::Value;
use rand::Rng;
use rand::RngCore;

use crate::commands::parsers::roll;
use crate::game_state::primitives::{CollectionItem, Modifier, ModifierEffect};
use crate::game_state::{Character, GameState};
use crate::rules::resolver;

use super::engine::ScriptEngine;

/// A sound command queued during LOLCODE script execution.
#[derive(Debug, Clone, PartialEq)]
pub enum SoundCommand {
    Play(String),
    PlayLoop(String),
    Stop,
}

/// Context shared by all RUSTORY_* API functions during script execution.
///
/// Temporarily owns the GameState (moved from App) and provides RNG,
/// output collection, and sound command buffering.
pub struct ScriptContext {
    pub game_state: GameState,
    pub rng: Box<dyn RngCore>,
    /// Display messages collected via RUSTORY_DISPLAY.
    pub output: Vec<String>,
    /// Sound commands queued via RUSTORY_PLAY_SOUND/PLAY_LOOP/STOP_SOUND.
    pub sound_commands: Vec<SoundCommand>,
    /// Currently active character: (name, is_player).
    /// Set by RUSTORY_GET_PLAYER / RUSTORY_GET_NPC.
    active_character: Option<(String, bool)>,
}

impl ScriptContext {
    pub fn new(game_state: GameState, rng: Box<dyn RngCore>) -> Self {
        Self {
            game_state,
            rng,
            output: Vec::new(),
            sound_commands: Vec::new(),
            active_character: None,
        }
    }

    /// Register all RUSTORY_* API functions on a ScriptEngine.
    ///
    /// The context is wrapped in `Rc<RefCell<>>` so multiple closures can share it.
    pub fn register_api(ctx: Rc<RefCell<ScriptContext>>, engine: &mut ScriptEngine) {
        // --- Dice ---
        let c = ctx.clone();
        engine.register("RUSTORY_ROLL", Some(1), move |args| {
            c.borrow_mut().rustory_roll(args)
        });

        // --- Stats ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_STAT", Some(1), move |args| {
            c.borrow().rustory_get_stat(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_SET_STAT", Some(2), move |args| {
            c.borrow_mut().rustory_set_stat(args)
        });

        // --- Derived ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_DERIVED", Some(1), move |args| {
            c.borrow().rustory_get_derived(args)
        });

        // --- Gauges ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_GAUGE", Some(1), move |args| {
            c.borrow().rustory_get_gauge(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_DAMAGE", Some(2), move |args| {
            c.borrow_mut().rustory_damage(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_HEAL", Some(2), move |args| {
            c.borrow_mut().rustory_heal(args)
        });

        // --- Pools ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_POOL", Some(1), move |args| {
            c.borrow().rustory_get_pool(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_SPEND", Some(2), move |args| {
            c.borrow_mut().rustory_spend(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_RESTORE", Some(2), move |args| {
            c.borrow_mut().rustory_restore(args)
        });

        // --- Checks ---
        let c = ctx.clone();
        engine.register("RUSTORY_CHECK", None, move |args| {
            c.borrow_mut().rustory_check(args)
        });

        // --- Conditions ---
        let c = ctx.clone();
        engine.register("RUSTORY_ADD_CONDITION", Some(1), move |args| {
            c.borrow_mut().rustory_add_condition(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_REMOVE_CONDITION", Some(1), move |args| {
            c.borrow_mut().rustory_remove_condition(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_HAS_CONDITION", Some(1), move |args| {
            c.borrow().rustory_has_condition(args)
        });

        // --- Modifiers ---
        let c = ctx.clone();
        engine.register("RUSTORY_ADD_MODIFIER", Some(1), move |args| {
            c.borrow_mut().rustory_add_modifier(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_REMOVE_MODIFIER", Some(1), move |args| {
            c.borrow_mut().rustory_remove_modifier(args)
        });

        // --- Tags ---
        let c = ctx.clone();
        engine.register("RUSTORY_ADD_TAG", Some(1), move |args| {
            c.borrow_mut().rustory_add_tag(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_REMOVE_TAG", Some(1), move |args| {
            c.borrow_mut().rustory_remove_tag(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_HAS_TAG", Some(1), move |args| {
            c.borrow().rustory_has_tag(args)
        });

        // --- Collections ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_INVENTORY", Some(0), move |args| {
            c.borrow().rustory_get_inventory(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_ADD_ITEM", Some(1), move |args| {
            c.borrow_mut().rustory_add_item(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_REMOVE_ITEM", Some(1), move |args| {
            c.borrow_mut().rustory_remove_item(args)
        });

        // --- Targeting ---
        let c = ctx.clone();
        engine.register("RUSTORY_GET_PLAYER", Some(1), move |args| {
            c.borrow_mut().rustory_get_player(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_GET_NPC", Some(1), move |args| {
            c.borrow_mut().rustory_get_npc(args)
        });

        // --- Display ---
        let c = ctx.clone();
        engine.register("RUSTORY_DISPLAY", Some(1), move |args| {
            c.borrow_mut().rustory_display(args)
        });

        // --- Sound ---
        let c = ctx.clone();
        engine.register("RUSTORY_PLAY_SOUND", Some(1), move |args| {
            c.borrow_mut().rustory_play_sound(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_PLAY_LOOP", Some(1), move |args| {
            c.borrow_mut().rustory_play_loop(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_STOP_SOUND", Some(0), move |args| {
            c.borrow_mut().rustory_stop_sound(args)
        });

        // --- Bestiary ---
        let c = ctx.clone();
        engine.register("RUSTORY_SPAWN", None, move |args| {
            c.borrow_mut().rustory_spawn(args)
        });

        let c = ctx.clone();
        engine.register("RUSTORY_ENCOUNTER", Some(1), move |args| {
            c.borrow_mut().rustory_encounter(args)
        });

        // --- Input ---
        let c = ctx.clone();
        engine.register("RUSTORY_ASK", Some(1), move |args| {
            c.borrow_mut().rustory_ask(args)
        });
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Find a character by name (searches both players and NPCs).
    fn find_character(&self, name: &str) -> Option<&Character> {
        let name_lower = name.to_lowercase();
        self.game_state
            .players
            .iter()
            .find(|c| c.name.to_lowercase() == name_lower)
            .or_else(|| {
                self.game_state
                    .npcs
                    .iter()
                    .find(|c| c.name.to_lowercase() == name_lower)
            })
    }

    /// Find a mutable character by name (searches both players and NPCs).
    fn find_character_mut(&mut self, name: &str) -> Option<&mut Character> {
        let name_lower = name.to_lowercase();
        if let Some(idx) = self
            .game_state
            .players
            .iter()
            .position(|c| c.name.to_lowercase() == name_lower)
        {
            return Some(&mut self.game_state.players[idx]);
        }
        if let Some(idx) = self
            .game_state
            .npcs
            .iter()
            .position(|c| c.name.to_lowercase() == name_lower)
        {
            return Some(&mut self.game_state.npcs[idx]);
        }
        None
    }

    /// Get a reference to the active character (set by GET_PLAYER/GET_NPC).
    fn active_char(&self) -> Option<&Character> {
        let (name, _) = self.active_character.as_ref()?;
        self.find_character(name)
    }

    /// Get a mutable reference to the active character.
    fn active_char_mut(&mut self) -> Option<&mut Character> {
        let (name, _) = self.active_character.clone()?;
        self.find_character_mut(&name)
    }

    // =========================================================================
    // RUSTORY_* API implementations
    // =========================================================================

    // --- Dice ---

    /// RUSTORY_ROLL(formula) -> Numbr
    /// Roll dice using a formula like "2d6+3". Returns the total.
    fn rustory_roll(&mut self, args: Vec<Value>) -> Value {
        let formula = value_to_string(&args[0]);
        match roll::parse(&formula) {
            Ok(parsed) => {
                let mut sum = 0i64;
                for _ in 0..parsed.dice {
                    sum += self.rng.gen_range(1..=parsed.value) as i64;
                }
                Value::Numbr(sum + parsed.modifier as i64)
            }
            Err(_) => Value::Noob,
        }
    }

    // --- Stats ---

    /// RUSTORY_GET_STAT(name) -> Numbar
    /// Get a stat value from the active character.
    fn rustory_get_stat(&self, args: Vec<Value>) -> Value {
        let stat_name = value_to_string(&args[0]);
        match self.active_char() {
            Some(ch) => match ch.get_stat(&stat_name) {
                Some(val) => Value::Numbar(val),
                None => Value::Noob,
            },
            None => Value::Noob,
        }
    }

    /// RUSTORY_SET_STAT(name, value) -> Troof
    /// Set a stat value on the active character.
    fn rustory_set_stat(&mut self, args: Vec<Value>) -> Value {
        let stat_name = value_to_string(&args[0]);
        let new_value = value_to_f64(&args[1]);
        match self.active_char_mut() {
            Some(ch) => {
                ch.set_stat(&stat_name, new_value);
                Value::Troof(true)
            }
            None => Value::Troof(false),
        }
    }

    // --- Derived ---

    /// RUSTORY_GET_DERIVED(name) -> Numbar
    /// Get a derived value from the active character using the rules engine.
    fn rustory_get_derived(&self, args: Vec<Value>) -> Value {
        let derived_name = value_to_string(&args[0]);
        let character = match self.active_char() {
            Some(ch) => ch,
            None => return Value::Noob,
        };
        let rules = match &self.game_state.rules {
            Some(r) => r,
            None => return Value::Noob,
        };
        match rules.derived.iter().find(|d| d.name == derived_name) {
            Some(derived) => Value::Numbar(resolver::resolve_derived(character, derived)),
            None => Value::Noob,
        }
    }

    // --- Gauges ---

    /// RUSTORY_GET_GAUGE(name) -> Numbar
    /// Get the current value of a gauge on the active character.
    fn rustory_get_gauge(&self, args: Vec<Value>) -> Value {
        let gauge_name = value_to_string(&args[0]);
        match self.active_char() {
            Some(ch) => match ch.get_gauge(&gauge_name) {
                Some(gauge) => Value::Numbar(gauge.current),
                None => Value::Noob,
            },
            None => Value::Noob,
        }
    }

    /// RUSTORY_DAMAGE(target, amount) -> Troof
    /// Deal damage to a named character's first gauge (typically "hp").
    /// Target is looked up by name in both players and NPCs.
    fn rustory_damage(&mut self, args: Vec<Value>) -> Value {
        let target_name = value_to_string(&args[0]);
        let amount = value_to_f64(&args[1]);
        match self.find_character_mut(&target_name) {
            Some(ch) => {
                // Damage the first gauge (conventionally "hp")
                let gauge_name = ch.gauges.keys().next().cloned();
                match gauge_name {
                    Some(name) => Value::Troof(ch.damage(&name, amount)),
                    None => Value::Troof(false),
                }
            }
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_HEAL(target, amount) -> Troof
    /// Heal a named character's first gauge (typically "hp").
    fn rustory_heal(&mut self, args: Vec<Value>) -> Value {
        let target_name = value_to_string(&args[0]);
        let amount = value_to_f64(&args[1]);
        match self.find_character_mut(&target_name) {
            Some(ch) => {
                let gauge_name = ch.gauges.keys().next().cloned();
                match gauge_name {
                    Some(name) => Value::Troof(ch.heal(&name, amount)),
                    None => Value::Troof(false),
                }
            }
            None => Value::Troof(false),
        }
    }

    // --- Pools ---

    /// RUSTORY_GET_POOL(name) -> Numbar
    /// Get the current value of a pool on the active character.
    fn rustory_get_pool(&self, args: Vec<Value>) -> Value {
        let pool_name = value_to_string(&args[0]);
        match self.active_char() {
            Some(ch) => match ch.get_pool(&pool_name) {
                Some(pool) => Value::Numbar(pool.current),
                None => Value::Noob,
            },
            None => Value::Noob,
        }
    }

    /// RUSTORY_SPEND(pool, amount) -> Troof
    /// Spend from a pool on the active character. Returns false if insufficient.
    fn rustory_spend(&mut self, args: Vec<Value>) -> Value {
        let pool_name = value_to_string(&args[0]);
        let amount = value_to_f64(&args[1]);
        match self.active_char_mut() {
            Some(ch) => match ch.spend_pool(&pool_name, amount) {
                Some(success) => Value::Troof(success),
                None => Value::Troof(false),
            },
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_RESTORE(pool, amount) -> Troof
    /// Restore a pool on the active character.
    fn rustory_restore(&mut self, args: Vec<Value>) -> Value {
        let pool_name = value_to_string(&args[0]);
        let amount = value_to_f64(&args[1]);
        match self.active_char_mut() {
            Some(ch) => Value::Troof(ch.restore_pool(&pool_name, amount)),
            None => Value::Troof(false),
        }
    }

    // --- Checks ---

    /// RUSTORY_CHECK(check_name, key1, val1, key2, val2, ...) -> Yarn
    /// Run a check from the rules engine. Args are alternating key/value pairs
    /// for placeholder substitution. Returns "success", "failure", "partial:<detail>", or "critical".
    fn rustory_check(&mut self, args: Vec<Value>) -> Value {
        if args.is_empty() {
            return Value::Noob;
        }
        let check_name = value_to_string(&args[0]);

        let character = match self.active_char() {
            Some(ch) => ch.clone(),
            None => return Value::Noob,
        };

        let rules = match &self.game_state.rules {
            Some(r) => r,
            None => return Value::Noob,
        };

        let check = match rules.checks.iter().find(|c| c.name == check_name) {
            Some(c) => c.clone(),
            None => return Value::Noob,
        };

        // Build args HashMap from alternating key/value pairs
        let mut check_args = HashMap::new();
        let mut i = 1;
        while i + 1 < args.len() {
            let key = value_to_string(&args[i]);
            let val = value_to_string(&args[i + 1]);
            check_args.insert(key, val);
            i += 2;
        }

        let result = resolver::resolve_check(&check, &character, &check_args, &mut *self.rng);

        match result {
            resolver::CheckResult::Success => Value::Yarn("success".to_string()),
            resolver::CheckResult::Failure => Value::Yarn("failure".to_string()),
            resolver::CheckResult::Partial(detail) => Value::Yarn(format!("partial:{detail}")),
            resolver::CheckResult::Critical => Value::Yarn("critical".to_string()),
        }
    }

    // --- Conditions ---

    /// RUSTORY_ADD_CONDITION(name) -> Troof
    fn rustory_add_condition(&mut self, args: Vec<Value>) -> Value {
        let condition_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => {
                ch.add_condition(&condition_name);
                Value::Troof(true)
            }
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_REMOVE_CONDITION(name) -> Troof
    fn rustory_remove_condition(&mut self, args: Vec<Value>) -> Value {
        let condition_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => Value::Troof(ch.remove_condition(&condition_name)),
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_HAS_CONDITION(name) -> Troof
    fn rustory_has_condition(&self, args: Vec<Value>) -> Value {
        let condition_name = value_to_string(&args[0]);
        match self.active_char() {
            Some(ch) => Value::Troof(ch.has_condition(&condition_name)),
            None => Value::Troof(false),
        }
    }

    // --- Modifiers ---

    /// RUSTORY_ADD_MODIFIER(name) -> Troof
    /// Adds a modifier by name. Looks up the definition from the rules.
    /// If no rules or no matching modifier definition, adds a basic modifier.
    fn rustory_add_modifier(&mut self, args: Vec<Value>) -> Value {
        let modifier_name = value_to_string(&args[0]);

        // Try to find modifier definition in rules
        let modifier = if let Some(rules) = &self.game_state.rules {
            rules
                .modifier_defs
                .iter()
                .find(|m| m.name == modifier_name)
                .map(|def| Modifier::new(&def.name, &def.target, def.effect.clone()))
        } else {
            None
        };

        let modifier =
            modifier.unwrap_or_else(|| Modifier::new(&modifier_name, "", ModifierEffect::Add(0.0)));

        match self.active_char_mut() {
            Some(ch) => {
                ch.add_modifier(modifier);
                Value::Troof(true)
            }
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_REMOVE_MODIFIER(name) -> Troof
    fn rustory_remove_modifier(&mut self, args: Vec<Value>) -> Value {
        let modifier_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => Value::Troof(ch.remove_modifier(&modifier_name)),
            None => Value::Troof(false),
        }
    }

    // --- Tags ---

    /// RUSTORY_ADD_TAG(tag) -> Troof
    fn rustory_add_tag(&mut self, args: Vec<Value>) -> Value {
        let tag_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => {
                ch.add_tag(&tag_name);
                Value::Troof(true)
            }
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_REMOVE_TAG(tag) -> Troof
    fn rustory_remove_tag(&mut self, args: Vec<Value>) -> Value {
        let tag_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => Value::Troof(ch.remove_tag(&tag_name)),
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_HAS_TAG(tag) -> Troof
    fn rustory_has_tag(&self, args: Vec<Value>) -> Value {
        let tag_name = value_to_string(&args[0]);
        match self.active_char() {
            Some(ch) => Value::Troof(ch.has_tag(&tag_name)),
            None => Value::Troof(false),
        }
    }

    // --- Collections ---

    /// RUSTORY_GET_INVENTORY() -> Yarn
    /// Returns a formatted inventory list for the active character.
    fn rustory_get_inventory(&self, _args: Vec<Value>) -> Value {
        match self.active_char() {
            Some(ch) => {
                if ch.inventory.items.is_empty() {
                    return Value::Yarn("(empty)".to_string());
                }
                let items: Vec<String> =
                    ch.inventory.items.iter().map(|i| i.name.clone()).collect();
                Value::Yarn(items.join(", "))
            }
            None => Value::Noob,
        }
    }

    /// RUSTORY_ADD_ITEM(item_name) -> Troof
    fn rustory_add_item(&mut self, args: Vec<Value>) -> Value {
        let item_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => {
                ch.add_item(CollectionItem::new(&item_name));
                Value::Troof(true)
            }
            None => Value::Troof(false),
        }
    }

    /// RUSTORY_REMOVE_ITEM(item_name) -> Troof
    fn rustory_remove_item(&mut self, args: Vec<Value>) -> Value {
        let item_name = value_to_string(&args[0]);
        match self.active_char_mut() {
            Some(ch) => Value::Troof(ch.remove_item(&item_name)),
            None => Value::Troof(false),
        }
    }

    // --- Targeting ---

    /// RUSTORY_GET_PLAYER(name) -> Troof
    /// Set the active character to a player by name.
    fn rustory_get_player(&mut self, args: Vec<Value>) -> Value {
        let name = value_to_string(&args[0]);
        let found = self
            .game_state
            .players
            .iter()
            .any(|c| c.name.to_lowercase() == name.to_lowercase());
        if found {
            self.active_character = Some((name, true));
            Value::Troof(true)
        } else {
            Value::Troof(false)
        }
    }

    /// RUSTORY_GET_NPC(name) -> Troof
    /// Set the active character to an NPC by name.
    fn rustory_get_npc(&mut self, args: Vec<Value>) -> Value {
        let name = value_to_string(&args[0]);
        let found = self
            .game_state
            .npcs
            .iter()
            .any(|c| c.name.to_lowercase() == name.to_lowercase());
        if found {
            self.active_character = Some((name, false));
            Value::Troof(true)
        } else {
            Value::Troof(false)
        }
    }

    // --- Display ---

    /// RUSTORY_DISPLAY(text) -> Noob
    /// Append a display message to the output buffer.
    fn rustory_display(&mut self, args: Vec<Value>) -> Value {
        let text = value_to_string(&args[0]);
        self.output.push(text);
        Value::Noob
    }

    // --- Sound ---

    /// RUSTORY_PLAY_SOUND(filename) -> Noob
    /// Queue a sound file for single playback.
    fn rustory_play_sound(&mut self, args: Vec<Value>) -> Value {
        let filename = value_to_string(&args[0]);
        self.sound_commands.push(SoundCommand::Play(filename));
        Value::Noob
    }

    /// RUSTORY_PLAY_LOOP(filename) -> Noob
    /// Queue a sound file for looped playback.
    fn rustory_play_loop(&mut self, args: Vec<Value>) -> Value {
        let filename = value_to_string(&args[0]);
        self.sound_commands.push(SoundCommand::PlayLoop(filename));
        Value::Noob
    }

    /// RUSTORY_STOP_SOUND() -> Noob
    /// Queue a stop-all-audio command.
    fn rustory_stop_sound(&mut self, _args: Vec<Value>) -> Value {
        self.sound_commands.push(SoundCommand::Stop);
        Value::Noob
    }

    // --- Bestiary ---

    /// RUSTORY_SPAWN(template) or RUSTORY_SPAWN(template, name) -> Yarn
    /// Spawn an NPC from a bestiary template. Returns the NPC name.
    fn rustory_spawn(&mut self, args: Vec<Value>) -> Value {
        if args.is_empty() {
            return Value::Noob;
        }

        let template_name = value_to_string(&args[0]);
        let custom_name = args.get(1).map(value_to_string).filter(|s| !s.is_empty());

        let entry = match crate::bestiary::find_entry(&self.game_state.bestiary_entries, &template_name) {
            Some(e) => e.clone(),
            None => return Value::Noob,
        };

        let npc_name = match custom_name {
            Some(name) => name,
            None => {
                let base = &entry.name;
                let mut counter = 1u32;
                loop {
                    let candidate = format!("{base} #{counter}");
                    if self.game_state.get_npc(&candidate).is_none() {
                        break candidate;
                    }
                    counter += 1;
                }
            }
        };

        let mut character = Character::new(&npc_name);
        for stat in &entry.stats {
            character.stats.push(stat.clone());
        }

        // Apply resource_defs (gauges/pools) from rules
        if let Some(rules) = &self.game_state.rules {
            for def in &rules.resource_defs {
                match def {
                    crate::rules::loader::ResourceDef::Gauge { name, max_stat } => {
                        if !character.gauges.contains_key(name) {
                            let max_val = character.get_stat(max_stat).unwrap_or(0.0);
                            if max_val > 0.0 {
                                character.gauges.insert(
                                    name.clone(),
                                    crate::game_state::primitives::Gauge::new(name, max_val),
                                );
                            }
                        }
                    }
                    crate::rules::loader::ResourceDef::Pool {
                        name,
                        max,
                        resets_on,
                    } => {
                        if !character.pools.contains_key(name) {
                            character.pools.insert(
                                name.clone(),
                                crate::game_state::primitives::Pool::new(name, *max, resets_on.clone()),
                            );
                        }
                    }
                }
            }
        }

        let result_name = npc_name.clone();
        self.game_state.add_npc(character);
        self.output.push(format!("Spawned {result_name} from bestiary."));

        Value::Yarn(result_name)
    }

    /// RUSTORY_ENCOUNTER(encounter_name) -> Numbr (count of spawned creatures)
    /// Spawn all creatures defined in an encounter TOML.
    fn rustory_encounter(&mut self, args: Vec<Value>) -> Value {
        if args.is_empty() {
            return Value::Noob;
        }

        let encounter_name = value_to_string(&args[0]);

        let encounter = match crate::bestiary::find_encounter(
            &self.game_state.bestiary_encounters,
            &encounter_name,
        ) {
            Some(e) => e.clone(),
            None => return Value::Noob,
        };

        let mut spawned_count = 0i64;

        for creature_def in &encounter.creatures {
            let entry = match crate::bestiary::find_entry(
                &self.game_state.bestiary_entries,
                &creature_def.template,
            ) {
                Some(e) => e.clone(),
                None => continue,
            };

            for i in 0..creature_def.count {
                let npc_name = if let Some(ref override_name) = creature_def.name_override {
                    if creature_def.count == 1 {
                        override_name.clone()
                    } else {
                        format!("{override_name} #{}", i + 1)
                    }
                } else {
                    let base = &entry.name;
                    let mut counter = 1u32;
                    loop {
                        let candidate = format!("{base} #{counter}");
                        if self.game_state.get_npc(&candidate).is_none() {
                            break candidate;
                        }
                        counter += 1;
                    }
                };

                let mut character = Character::new(&npc_name);
                for stat in &entry.stats {
                    character.stats.push(stat.clone());
                }

                // Apply resource_defs (gauges/pools) from rules
                if let Some(rules) = &self.game_state.rules {
                    for def in &rules.resource_defs {
                        match def {
                            crate::rules::loader::ResourceDef::Gauge { name, max_stat } => {
                                if !character.gauges.contains_key(name) {
                                    let max_val = character.get_stat(max_stat).unwrap_or(0.0);
                                    if max_val > 0.0 {
                                        character.gauges.insert(
                                            name.clone(),
                                            crate::game_state::primitives::Gauge::new(name, max_val),
                                        );
                                    }
                                }
                            }
                            crate::rules::loader::ResourceDef::Pool {
                                name,
                                max,
                                resets_on,
                            } => {
                                if !character.pools.contains_key(name) {
                                    character.pools.insert(
                                        name.clone(),
                                        crate::game_state::primitives::Pool::new(
                                            name,
                                            *max,
                                            resets_on.clone(),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }

                self.output.push(format!("Spawned {} from encounter.", npc_name));
                self.game_state.add_npc(character);
                spawned_count += 1;
            }
        }

        Value::Numbr(spawned_count)
    }

    // --- Input ---

    /// RUSTORY_ASK(prompt) -> Yarn
    /// Ask the user for input. Currently returns an empty string as interactive
    /// input during script execution is not yet supported.
    fn rustory_ask(&mut self, args: Vec<Value>) -> Value {
        let prompt = value_to_string(&args[0]);
        self.output.push(format!("[ASK] {prompt}"));
        Value::Yarn(String::new())
    }
}

// =============================================================================
// Value conversion helpers
// =============================================================================

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Yarn(s) => s.clone(),
        Value::Numbr(n) => n.to_string(),
        Value::Numbar(f) => f.to_string(),
        Value::Troof(b) => b.to_string(),
        Value::Noob => String::new(),
        _ => String::new(),
    }
}

fn value_to_f64(v: &Value) -> f64 {
    match v {
        Value::Numbr(n) => *n as f64,
        Value::Numbar(f) => *f,
        Value::Yarn(s) => s.parse().unwrap_or(0.0),
        Value::Troof(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Noob => 0.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::primitives::{
        Check, Derived, ModifierEffect, ResetTrigger, ResolutionMode,
    };
    use crate::rules::loader::{CampaignRules, ModifierDef, ResourceDef};
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::path::Path;

    fn make_rng() -> Box<dyn RngCore> {
        Box::new(StdRng::seed_from_u64(42))
    }

    fn make_game_state() -> GameState {
        let mut gs = GameState::new(Path::new("/tmp/test"));
        gs.add_player(
            Character::new("Thorin")
                .with_stat("strength", 18.0)
                .with_stat("dexterity", 14.0)
                .with_stat("hp_max", 52.0)
                .with_gauge("hp", 52.0)
                .with_pool("spell_slots", 3.0, ResetTrigger::LongRest),
        );
        gs.add_npc(
            Character::new("Goblin")
                .with_stat("strength", 8.0)
                .with_stat("ac", 15.0)
                .with_gauge("hp", 7.0),
        );
        gs
    }

    fn make_game_state_with_rules() -> GameState {
        let mut gs = make_game_state();
        gs.rules = Some(CampaignRules {
            system_name: "Test".to_string(),
            system_version: None,
            stat_names: vec!["strength".to_string(), "dexterity".to_string()],
            derived: vec![Derived::new("ac", "10 + modifier(dexterity)")],
            checks: vec![Check::new(
                "ability_check",
                "1d20 + modifier({ability})",
                ResolutionMode::RollOver,
            )],
            modifier_defs: vec![ModifierDef {
                name: "bless".to_string(),
                target: "attack_roll".to_string(),
                effect: ModifierEffect::Add(1.0),
            }],
            resource_defs: vec![ResourceDef::Gauge {
                name: "hp".to_string(),
                max_stat: "hp_max".to_string(),
            }],
        });
        gs
    }

    fn make_ctx() -> ScriptContext {
        ScriptContext::new(make_game_state(), make_rng())
    }

    fn make_ctx_with_rules() -> ScriptContext {
        ScriptContext::new(make_game_state_with_rules(), make_rng())
    }

    // ---- Dice ----

    #[test]
    fn test_rustory_roll_valid() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_roll(vec![Value::Yarn("2d6".to_string())]);
        match result {
            Value::Numbr(n) => assert!((2..=12).contains(&n)),
            _ => panic!("Expected Numbr, got {result:?}"),
        }
    }

    #[test]
    fn test_rustory_roll_with_modifier() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_roll(vec![Value::Yarn("1d20+5".to_string())]);
        match result {
            Value::Numbr(n) => assert!((6..=25).contains(&n)),
            _ => panic!("Expected Numbr, got {result:?}"),
        }
    }

    #[test]
    fn test_rustory_roll_deterministic() {
        let mut ctx1 = ScriptContext::new(make_game_state(), Box::new(StdRng::seed_from_u64(99)));
        let mut ctx2 = ScriptContext::new(make_game_state(), Box::new(StdRng::seed_from_u64(99)));
        let r1 = ctx1.rustory_roll(vec![Value::Yarn("3d8+2".to_string())]);
        let r2 = ctx2.rustory_roll(vec![Value::Yarn("3d8+2".to_string())]);
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_rustory_roll_invalid_formula() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_roll(vec![Value::Yarn("invalid".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Targeting ----

    #[test]
    fn test_rustory_get_player_found() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        assert_eq!(result, Value::Troof(true));
        assert!(ctx.active_character.is_some());
    }

    #[test]
    fn test_rustory_get_player_not_found() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_get_player(vec![Value::Yarn("Nobody".to_string())]);
        assert_eq!(result, Value::Troof(false));
        assert!(ctx.active_character.is_none());
    }

    #[test]
    fn test_rustory_get_player_case_insensitive() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_get_player(vec![Value::Yarn("thorin".to_string())]);
        assert_eq!(result, Value::Troof(true));
    }

    #[test]
    fn test_rustory_get_npc_found() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_get_npc(vec![Value::Yarn("Goblin".to_string())]);
        assert_eq!(result, Value::Troof(true));
        assert!(ctx.active_character.is_some());
    }

    #[test]
    fn test_rustory_get_npc_not_found() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_get_npc(vec![Value::Yarn("Dragon".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    // ---- Stats ----

    #[test]
    fn test_rustory_get_stat_existing() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_stat(vec![Value::Yarn("strength".to_string())]);
        assert_eq!(result, Value::Numbar(18.0));
    }

    #[test]
    fn test_rustory_get_stat_missing() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_stat(vec![Value::Yarn("charisma".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_get_stat_no_active() {
        let ctx = make_ctx();
        let result = ctx.rustory_get_stat(vec![Value::Yarn("strength".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_set_stat() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result =
            ctx.rustory_set_stat(vec![Value::Yarn("strength".to_string()), Value::Numbr(20)]);
        assert_eq!(result, Value::Troof(true));
        let val = ctx.rustory_get_stat(vec![Value::Yarn("strength".to_string())]);
        assert_eq!(val, Value::Numbar(20.0));
    }

    #[test]
    fn test_rustory_set_stat_new() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_set_stat(vec![Value::Yarn("charisma".to_string()), Value::Numbr(12)]);
        let val = ctx.rustory_get_stat(vec![Value::Yarn("charisma".to_string())]);
        assert_eq!(val, Value::Numbar(12.0));
    }

    #[test]
    fn test_rustory_set_stat_no_active() {
        let mut ctx = make_ctx();
        let result =
            ctx.rustory_set_stat(vec![Value::Yarn("strength".to_string()), Value::Numbr(20)]);
        assert_eq!(result, Value::Troof(false));
    }

    // ---- Derived ----

    #[test]
    fn test_rustory_get_derived() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_derived(vec![Value::Yarn("ac".to_string())]);
        // ac = 10 + modifier(dexterity) = 10 + floor((14-10)/2) = 10 + 2 = 12
        assert_eq!(result, Value::Numbar(12.0));
    }

    #[test]
    fn test_rustory_get_derived_no_rules() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_derived(vec![Value::Yarn("ac".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_get_derived_unknown_name() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_derived(vec![Value::Yarn("nonexistent".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Gauges ----

    #[test]
    fn test_rustory_get_gauge() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_gauge(vec![Value::Yarn("hp".to_string())]);
        assert_eq!(result, Value::Numbar(52.0));
    }

    #[test]
    fn test_rustory_get_gauge_missing() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_gauge(vec![Value::Yarn("sanity".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_damage() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_damage(vec![Value::Yarn("Goblin".to_string()), Value::Numbr(3)]);
        assert_eq!(result, Value::Troof(true));
        // Verify HP decreased
        ctx.rustory_get_npc(vec![Value::Yarn("Goblin".to_string())]);
        let hp = ctx.rustory_get_gauge(vec![Value::Yarn("hp".to_string())]);
        assert_eq!(hp, Value::Numbar(4.0));
    }

    #[test]
    fn test_rustory_damage_unknown_target() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_damage(vec![Value::Yarn("Ghost".to_string()), Value::Numbr(5)]);
        assert_eq!(result, Value::Troof(false));
    }

    #[test]
    fn test_rustory_damage_clamped_to_zero() {
        let mut ctx = make_ctx();
        ctx.rustory_damage(vec![Value::Yarn("Goblin".to_string()), Value::Numbr(100)]);
        ctx.rustory_get_npc(vec![Value::Yarn("Goblin".to_string())]);
        let hp = ctx.rustory_get_gauge(vec![Value::Yarn("hp".to_string())]);
        assert_eq!(hp, Value::Numbar(0.0));
    }

    #[test]
    fn test_rustory_heal() {
        let mut ctx = make_ctx();
        ctx.rustory_damage(vec![Value::Yarn("Thorin".to_string()), Value::Numbr(20)]);
        let result = ctx.rustory_heal(vec![Value::Yarn("Thorin".to_string()), Value::Numbr(10)]);
        assert_eq!(result, Value::Troof(true));
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let hp = ctx.rustory_get_gauge(vec![Value::Yarn("hp".to_string())]);
        assert_eq!(hp, Value::Numbar(42.0));
    }

    #[test]
    fn test_rustory_heal_clamped_to_max() {
        let mut ctx = make_ctx();
        ctx.rustory_damage(vec![Value::Yarn("Thorin".to_string()), Value::Numbr(5)]);
        ctx.rustory_heal(vec![Value::Yarn("Thorin".to_string()), Value::Numbr(100)]);
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let hp = ctx.rustory_get_gauge(vec![Value::Yarn("hp".to_string())]);
        assert_eq!(hp, Value::Numbar(52.0));
    }

    // ---- Pools ----

    #[test]
    fn test_rustory_get_pool() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_pool(vec![Value::Yarn("spell_slots".to_string())]);
        assert_eq!(result, Value::Numbar(3.0));
    }

    #[test]
    fn test_rustory_get_pool_missing() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_pool(vec![Value::Yarn("ki_points".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_spend_success() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_spend(vec![
            Value::Yarn("spell_slots".to_string()),
            Value::Numbr(2),
        ]);
        assert_eq!(result, Value::Troof(true));
        let remaining = ctx.rustory_get_pool(vec![Value::Yarn("spell_slots".to_string())]);
        assert_eq!(remaining, Value::Numbar(1.0));
    }

    #[test]
    fn test_rustory_spend_insufficient() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_spend(vec![
            Value::Yarn("spell_slots".to_string()),
            Value::Numbr(10),
        ]);
        assert_eq!(result, Value::Troof(false));
        // Pool unchanged
        let remaining = ctx.rustory_get_pool(vec![Value::Yarn("spell_slots".to_string())]);
        assert_eq!(remaining, Value::Numbar(3.0));
    }

    #[test]
    fn test_rustory_restore() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_spend(vec![
            Value::Yarn("spell_slots".to_string()),
            Value::Numbr(2),
        ]);
        let result = ctx.rustory_restore(vec![
            Value::Yarn("spell_slots".to_string()),
            Value::Numbr(1),
        ]);
        assert_eq!(result, Value::Troof(true));
        let val = ctx.rustory_get_pool(vec![Value::Yarn("spell_slots".to_string())]);
        assert_eq!(val, Value::Numbar(2.0));
    }

    // ---- Checks ----

    #[test]
    fn test_rustory_check_guaranteed_success() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        // DC 1 with str mod +4, always succeeds
        let result = ctx.rustory_check(vec![
            Value::Yarn("ability_check".to_string()),
            Value::Yarn("ability".to_string()),
            Value::Yarn("strength".to_string()),
            Value::Yarn("dc".to_string()),
            Value::Yarn("1".to_string()),
        ]);
        assert_eq!(result, Value::Yarn("success".to_string()));
    }

    #[test]
    fn test_rustory_check_guaranteed_failure() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        // DC 100, impossible to reach
        let result = ctx.rustory_check(vec![
            Value::Yarn("ability_check".to_string()),
            Value::Yarn("ability".to_string()),
            Value::Yarn("strength".to_string()),
            Value::Yarn("dc".to_string()),
            Value::Yarn("100".to_string()),
        ]);
        assert_eq!(result, Value::Yarn("failure".to_string()));
    }

    #[test]
    fn test_rustory_check_no_rules() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_check(vec![Value::Yarn("ability_check".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_check_unknown_check() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_check(vec![Value::Yarn("nonexistent".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_check_no_active() {
        let mut ctx = make_ctx_with_rules();
        let result = ctx.rustory_check(vec![Value::Yarn("ability_check".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Conditions ----

    #[test]
    fn test_rustory_add_condition() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_add_condition(vec![Value::Yarn("Stunned".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let has = ctx.rustory_has_condition(vec![Value::Yarn("Stunned".to_string())]);
        assert_eq!(has, Value::Troof(true));
    }

    #[test]
    fn test_rustory_remove_condition() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_condition(vec![Value::Yarn("Poisoned".to_string())]);
        let result = ctx.rustory_remove_condition(vec![Value::Yarn("Poisoned".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let has = ctx.rustory_has_condition(vec![Value::Yarn("Poisoned".to_string())]);
        assert_eq!(has, Value::Troof(false));
    }

    #[test]
    fn test_rustory_remove_condition_not_found() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_remove_condition(vec![Value::Yarn("Invisible".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    #[test]
    fn test_rustory_has_condition_no_active() {
        let ctx = make_ctx();
        let result = ctx.rustory_has_condition(vec![Value::Yarn("Stunned".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    // ---- Modifiers ----

    #[test]
    fn test_rustory_add_modifier_from_rules() {
        let mut ctx = make_ctx_with_rules();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_add_modifier(vec![Value::Yarn("bless".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let ch = ctx.active_char().unwrap();
        assert_eq!(ch.modifiers.len(), 1);
        assert_eq!(ch.modifiers[0].name, "bless");
        assert_eq!(ch.modifiers[0].target, "attack_roll");
    }

    #[test]
    fn test_rustory_add_modifier_unknown() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_add_modifier(vec![Value::Yarn("custom_buff".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let ch = ctx.active_char().unwrap();
        assert_eq!(ch.modifiers.len(), 1);
        assert_eq!(ch.modifiers[0].name, "custom_buff");
    }

    #[test]
    fn test_rustory_remove_modifier() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_modifier(vec![Value::Yarn("shield".to_string())]);
        let result = ctx.rustory_remove_modifier(vec![Value::Yarn("shield".to_string())]);
        assert_eq!(result, Value::Troof(true));
    }

    #[test]
    fn test_rustory_remove_modifier_not_found() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_remove_modifier(vec![Value::Yarn("invisible".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    // ---- Tags ----

    #[test]
    fn test_rustory_add_tag() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_add_tag(vec![Value::Yarn("Darkvision".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let has = ctx.rustory_has_tag(vec![Value::Yarn("Darkvision".to_string())]);
        assert_eq!(has, Value::Troof(true));
    }

    #[test]
    fn test_rustory_add_tag_duplicate() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_tag(vec![Value::Yarn("Darkvision".to_string())]);
        ctx.rustory_add_tag(vec![Value::Yarn("Darkvision".to_string())]);
        let ch = ctx.active_char().unwrap();
        assert_eq!(ch.tags.len(), 1);
    }

    #[test]
    fn test_rustory_remove_tag() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_tag(vec![Value::Yarn("Flying".to_string())]);
        let result = ctx.rustory_remove_tag(vec![Value::Yarn("Flying".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let has = ctx.rustory_has_tag(vec![Value::Yarn("Flying".to_string())]);
        assert_eq!(has, Value::Troof(false));
    }

    #[test]
    fn test_rustory_remove_tag_not_found() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_remove_tag(vec![Value::Yarn("Ghost".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    #[test]
    fn test_rustory_has_tag_no_active() {
        let ctx = make_ctx();
        let result = ctx.rustory_has_tag(vec![Value::Yarn("Darkvision".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    // ---- Collections ----

    #[test]
    fn test_rustory_get_inventory_empty() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_get_inventory(vec![]);
        assert_eq!(result, Value::Yarn("(empty)".to_string()));
    }

    #[test]
    fn test_rustory_add_and_get_inventory() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_item(vec![Value::Yarn("Longsword".to_string())]);
        ctx.rustory_add_item(vec![Value::Yarn("Shield".to_string())]);
        let result = ctx.rustory_get_inventory(vec![]);
        assert_eq!(result, Value::Yarn("Longsword, Shield".to_string()));
    }

    #[test]
    fn test_rustory_remove_item() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        ctx.rustory_add_item(vec![Value::Yarn("Potion".to_string())]);
        let result = ctx.rustory_remove_item(vec![Value::Yarn("Potion".to_string())]);
        assert_eq!(result, Value::Troof(true));
        let inv = ctx.rustory_get_inventory(vec![]);
        assert_eq!(inv, Value::Yarn("(empty)".to_string()));
    }

    #[test]
    fn test_rustory_remove_item_not_found() {
        let mut ctx = make_ctx();
        ctx.rustory_get_player(vec![Value::Yarn("Thorin".to_string())]);
        let result = ctx.rustory_remove_item(vec![Value::Yarn("Ghost".to_string())]);
        assert_eq!(result, Value::Troof(false));
    }

    #[test]
    fn test_rustory_get_inventory_no_active() {
        let ctx = make_ctx();
        let result = ctx.rustory_get_inventory(vec![]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Display ----

    #[test]
    fn test_rustory_display() {
        let mut ctx = make_ctx();
        ctx.rustory_display(vec![Value::Yarn("Hello, world!".to_string())]);
        ctx.rustory_display(vec![Value::Yarn("Second message".to_string())]);
        assert_eq!(ctx.output.len(), 2);
        assert_eq!(ctx.output[0], "Hello, world!");
        assert_eq!(ctx.output[1], "Second message");
    }

    #[test]
    fn test_rustory_display_returns_noob() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_display(vec![Value::Yarn("test".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Sound ----

    #[test]
    fn test_rustory_play_sound() {
        let mut ctx = make_ctx();
        ctx.rustory_play_sound(vec![Value::Yarn("tavern.mp3".to_string())]);
        assert_eq!(ctx.sound_commands.len(), 1);
        assert_eq!(ctx.sound_commands[0], SoundCommand::Play("tavern.mp3".to_string()));
    }

    #[test]
    fn test_rustory_play_sound_returns_noob() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_play_sound(vec![Value::Yarn("battle.mp3".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_play_loop() {
        let mut ctx = make_ctx();
        ctx.rustory_play_loop(vec![Value::Yarn("ambiance.ogg".to_string())]);
        assert_eq!(ctx.sound_commands.len(), 1);
        assert_eq!(ctx.sound_commands[0], SoundCommand::PlayLoop("ambiance.ogg".to_string()));
    }

    #[test]
    fn test_rustory_play_loop_returns_noob() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_play_loop(vec![Value::Yarn("music.mp3".to_string())]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_rustory_stop_sound() {
        let mut ctx = make_ctx();
        ctx.rustory_stop_sound(vec![]);
        assert_eq!(ctx.sound_commands.len(), 1);
        assert_eq!(ctx.sound_commands[0], SoundCommand::Stop);
    }

    #[test]
    fn test_rustory_stop_sound_returns_noob() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_stop_sound(vec![]);
        assert_eq!(result, Value::Noob);
    }

    #[test]
    fn test_sound_commands_sequence() {
        let mut ctx = make_ctx();
        ctx.rustory_play_sound(vec![Value::Yarn("battle.mp3".to_string())]);
        ctx.rustory_play_loop(vec![Value::Yarn("ambiance.ogg".to_string())]);
        ctx.rustory_stop_sound(vec![]);
        assert_eq!(ctx.sound_commands.len(), 3);
        assert_eq!(ctx.sound_commands[0], SoundCommand::Play("battle.mp3".to_string()));
        assert_eq!(ctx.sound_commands[1], SoundCommand::PlayLoop("ambiance.ogg".to_string()));
        assert_eq!(ctx.sound_commands[2], SoundCommand::Stop);
    }

    // ---- Input ----

    #[test]
    fn test_rustory_ask() {
        let mut ctx = make_ctx();
        let result = ctx.rustory_ask(vec![Value::Yarn("What do you do?".to_string())]);
        assert_eq!(result, Value::Yarn(String::new()));
        assert_eq!(ctx.output.len(), 1);
        assert!(ctx.output[0].contains("What do you do?"));
    }

    // ---- Integration: register_api with ScriptEngine ----

    #[test]
    fn test_register_api_and_execute_display() {
        let ctx = Rc::new(RefCell::new(make_ctx()));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);

        let output = engine
            .execute(
                "\
HAI 1.2
I IZ RUSTORY_DISPLAY YR \"Hello from LOLCODE\" MKAY
KTHXBYE",
            )
            .unwrap();

        // RUSTORY_DISPLAY writes to ctx.output, not stdout
        assert_eq!(output, "");
        assert_eq!(ctx.borrow().output.len(), 1);
        assert_eq!(ctx.borrow().output[0], "Hello from LOLCODE");
    }

    #[test]
    fn test_register_api_and_execute_get_stat() {
        let ctx = Rc::new(RefCell::new(make_ctx()));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);

        let output = engine
            .execute(
                "\
HAI 1.2
I IZ RUSTORY_GET_PLAYER YR \"Thorin\" MKAY
I HAS A STR ITZ I IZ RUSTORY_GET_STAT YR \"strength\" MKAY
VISIBLE STR
KTHXBYE",
            )
            .unwrap();

        assert_eq!(output, "18\n");
    }

    #[test]
    fn test_register_api_and_execute_damage() {
        let ctx = Rc::new(RefCell::new(make_ctx()));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);

        engine
            .execute(
                "\
HAI 1.2
I IZ RUSTORY_DAMAGE YR \"Goblin\" AN YR 3 MKAY
I IZ RUSTORY_GET_NPC YR \"Goblin\" MKAY
I HAS A HP ITZ I IZ RUSTORY_GET_GAUGE YR \"hp\" MKAY
VISIBLE HP
KTHXBYE",
            )
            .unwrap();

        let borrowed = ctx.borrow();
        let goblin = borrowed.game_state.get_npc("Goblin").unwrap();
        assert_eq!(goblin.get_gauge("hp").unwrap().current, 4.0);
    }

    #[test]
    fn test_register_api_and_execute_roll() {
        let ctx = Rc::new(RefCell::new(ScriptContext::new(
            make_game_state(),
            Box::new(StdRng::seed_from_u64(42)),
        )));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);

        let output = engine
            .execute(
                "\
HAI 1.2
I HAS A RESULT ITZ I IZ RUSTORY_ROLL YR \"1d20\" MKAY
VISIBLE RESULT
KTHXBYE",
            )
            .unwrap();

        // With seed 42, should be deterministic
        let value: i64 = output.trim().parse().unwrap();
        assert!((1..=20).contains(&value));
    }

    // ---- Value conversion helpers ----

    #[test]
    fn test_value_to_string_yarn() {
        assert_eq!(value_to_string(&Value::Yarn("hello".to_string())), "hello");
    }

    #[test]
    fn test_value_to_string_numbr() {
        assert_eq!(value_to_string(&Value::Numbr(42)), "42");
    }

    #[test]
    fn test_value_to_string_numbar() {
        assert_eq!(value_to_string(&Value::Numbar(3.14)), "3.14");
    }

    #[test]
    fn test_value_to_string_noob() {
        assert_eq!(value_to_string(&Value::Noob), "");
    }

    #[test]
    fn test_value_to_f64_numbr() {
        assert_eq!(value_to_f64(&Value::Numbr(10)), 10.0);
    }

    #[test]
    fn test_value_to_f64_numbar() {
        assert_eq!(value_to_f64(&Value::Numbar(3.14)), 3.14);
    }

    #[test]
    fn test_value_to_f64_yarn_valid() {
        assert_eq!(value_to_f64(&Value::Yarn("2.5".to_string())), 2.5);
    }

    #[test]
    fn test_value_to_f64_yarn_invalid() {
        assert_eq!(value_to_f64(&Value::Yarn("abc".to_string())), 0.0);
    }

    #[test]
    fn test_value_to_f64_noob() {
        assert_eq!(value_to_f64(&Value::Noob), 0.0);
    }

    // ---- Sample script integration tests ----

    fn make_sample_game_state() -> GameState {
        // Matches sample/ campaign: Thorin (player) and Goblin King (NPC) with D&D rules
        let mut gs = GameState::new(Path::new("sample"));
        gs.add_player(
            Character::new("Thorin")
                .with_stat("strength", 18.0)
                .with_stat("dexterity", 12.0)
                .with_stat("constitution", 16.0)
                .with_stat("intelligence", 10.0)
                .with_stat("wisdom", 13.0)
                .with_stat("charisma", 8.0)
                .with_stat("hp_max", 52.0)
                .with_stat("ac", 18.0)
                .with_gauge("hp", 52.0)
                .with_pool("spell_slots", 4.0, ResetTrigger::LongRest),
        );
        gs.add_npc(
            Character::new("Goblin King")
                .with_stat("strength", 16.0)
                .with_stat("dexterity", 14.0)
                .with_stat("constitution", 14.0)
                .with_stat("intelligence", 12.0)
                .with_stat("wisdom", 11.0)
                .with_stat("charisma", 10.0)
                .with_stat("hp_max", 45.0)
                .with_stat("ac", 17.0)
                .with_gauge("hp", 45.0),
        );
        gs.rules = Some(CampaignRules {
            system_name: "D&D 5e Basic".to_string(),
            system_version: Some("1.0".to_string()),
            stat_names: vec![
                "strength".into(),
                "dexterity".into(),
                "constitution".into(),
                "intelligence".into(),
                "wisdom".into(),
                "charisma".into(),
            ],
            derived: vec![
                Derived::new("ac", "10 + modifier(dexterity)"),
                Derived::new("initiative", "modifier(dexterity)"),
            ],
            checks: vec![Check::new(
                "ability_check",
                "1d20 + modifier({ability})",
                ResolutionMode::RollOver,
            )],
            modifier_defs: vec![],
            resource_defs: vec![ResourceDef::Gauge {
                name: "hp".to_string(),
                max_stat: "hp_max".to_string(),
            }],
        });
        gs
    }

    #[test]
    fn test_sample_smite_executes_without_error() {
        let source = std::fs::read_to_string("sample/rules/commands/smite.lol").unwrap();
        let ctx = Rc::new(RefCell::new(ScriptContext::new(
            make_sample_game_state(),
            Box::new(StdRng::seed_from_u64(42)),
        )));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);
        let result = engine.execute(&source);
        assert!(result.is_ok(), "smite.lol failed: {:?}", result.err());
        // Should produce RUSTORY_DISPLAY output (hit or miss message)
        let output = ctx.borrow().output.clone();
        assert!(
            !output.is_empty(),
            "smite.lol should produce display output"
        );
        assert!(
            output[0].contains("SMITE"),
            "smite output should mention SMITE: {:?}",
            output
        );
    }

    #[test]
    fn test_sample_heal_executes_without_error() {
        let source = std::fs::read_to_string("sample/rules/commands/heal.lol").unwrap();
        let ctx = Rc::new(RefCell::new(ScriptContext::new(
            make_sample_game_state(),
            Box::new(StdRng::seed_from_u64(42)),
        )));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);
        let result = engine.execute(&source);
        assert!(result.is_ok(), "heal.lol failed: {:?}", result.err());
        let output = ctx.borrow().output.clone();
        assert!(
            !output.is_empty(),
            "heal.lol should produce display output"
        );
        assert!(
            output[0].contains("Heal") || output[0].contains("spell"),
            "heal output should mention healing or spell slots: {:?}",
            output
        );
    }

    #[test]
    fn test_sample_perception_executes_without_error() {
        let source =
            std::fs::read_to_string("sample/rules/commands/perception.lol").unwrap();
        let ctx = Rc::new(RefCell::new(ScriptContext::new(
            make_sample_game_state(),
            Box::new(StdRng::seed_from_u64(42)),
        )));
        let mut engine = ScriptEngine::new();
        ScriptContext::register_api(ctx.clone(), &mut engine);
        let result = engine.execute(&source);
        assert!(
            result.is_ok(),
            "perception.lol failed: {:?}",
            result.err()
        );
        let output = ctx.borrow().output.clone();
        assert!(
            !output.is_empty(),
            "perception.lol should produce display output"
        );
        assert!(
            output[0].contains("perception") || output[0].contains("Thorin"),
            "perception output should mention the check: {:?}",
            output
        );
    }

    // ---- Spawn ----

    fn make_game_state_with_bestiary() -> GameState {
        let mut gs = make_game_state_with_rules();
        gs.bestiary_entries = vec![
            crate::bestiary::BestiaryEntry {
                name: "Goblin Warrior".to_string(),
                stats: vec![
                    crate::game_state::primitives::Stat::new("strength", 8.0),
                    crate::game_state::primitives::Stat::new("dexterity", 14.0),
                    crate::game_state::primitives::Stat::new("hp_max", 7.0),
                    crate::game_state::primitives::Stat::new("ac", 15.0),
                ],
            },
        ];
        gs
    }

    #[test]
    fn test_rustory_spawn_with_name() {
        let gs = make_game_state_with_bestiary();
        let initial_npcs = gs.npcs.len();
        let mut ctx = ScriptContext::new(gs, make_rng());

        let result = ctx.rustory_spawn(vec![
            Value::Yarn("Goblin Warrior".to_string()),
            Value::Yarn("Guard".to_string()),
        ]);
        assert_eq!(result, Value::Yarn("Guard".to_string()));
        assert_eq!(ctx.game_state.npcs.len(), initial_npcs + 1);
        let spawned = ctx.game_state.get_npc("Guard").unwrap();
        assert_eq!(spawned.get_stat("strength"), Some(8.0));
        assert_eq!(spawned.get_stat("hp_max"), Some(7.0));
        // Gauge should be applied from resource_defs
        assert!(spawned.gauges.contains_key("hp"));
        assert_eq!(spawned.gauges["hp"].max, 7.0);
    }

    #[test]
    fn test_rustory_spawn_auto_name() {
        let gs = make_game_state_with_bestiary();
        let mut ctx = ScriptContext::new(gs, make_rng());

        let result = ctx.rustory_spawn(vec![Value::Yarn("Goblin Warrior".to_string())]);
        assert_eq!(result, Value::Yarn("Goblin Warrior #1".to_string()));
        assert!(ctx.game_state.get_npc("Goblin Warrior #1").is_some());
    }

    #[test]
    fn test_rustory_spawn_auto_name_increments() {
        let gs = make_game_state_with_bestiary();
        let mut ctx = ScriptContext::new(gs, make_rng());

        ctx.rustory_spawn(vec![Value::Yarn("Goblin Warrior".to_string())]);
        ctx.rustory_spawn(vec![Value::Yarn("Goblin Warrior".to_string())]);
        let result = ctx.rustory_spawn(vec![Value::Yarn("Goblin Warrior".to_string())]);

        assert_eq!(result, Value::Yarn("Goblin Warrior #3".to_string()));
        assert!(ctx.game_state.get_npc("Goblin Warrior #1").is_some());
        assert!(ctx.game_state.get_npc("Goblin Warrior #2").is_some());
        assert!(ctx.game_state.get_npc("Goblin Warrior #3").is_some());
    }

    #[test]
    fn test_rustory_spawn_unknown_template() {
        let gs = make_game_state_with_bestiary();
        let mut ctx = ScriptContext::new(gs, make_rng());
        let initial_npcs = ctx.game_state.npcs.len();

        let result = ctx.rustory_spawn(vec![Value::Yarn("Dragon".to_string())]);
        assert_eq!(result, Value::Noob);
        assert_eq!(ctx.game_state.npcs.len(), initial_npcs);
    }

    #[test]
    fn test_rustory_spawn_no_args() {
        let gs = make_game_state_with_bestiary();
        let mut ctx = ScriptContext::new(gs, make_rng());

        let result = ctx.rustory_spawn(vec![]);
        assert_eq!(result, Value::Noob);
    }

    // ---- Encounter ----

    fn make_game_state_with_encounters() -> GameState {
        let mut gs = make_game_state_with_bestiary();
        // Add orc entry
        gs.bestiary_entries.push(crate::bestiary::BestiaryEntry {
            name: "Orc".to_string(),
            stats: vec![
                crate::game_state::primitives::Stat::new("strength", 16.0),
                crate::game_state::primitives::Stat::new("hp_max", 15.0),
                crate::game_state::primitives::Stat::new("ac", 13.0),
            ],
        });
        gs.bestiary_encounters = vec![crate::bestiary::Encounter {
            name: "Goblin Patrol".to_string(),
            description: "A small group of goblins".to_string(),
            creatures: vec![
                crate::bestiary::EncounterCreature {
                    template: "Goblin Warrior".to_string(),
                    count: 2,
                    name_override: None,
                },
                crate::bestiary::EncounterCreature {
                    template: "Orc".to_string(),
                    count: 1,
                    name_override: Some("Orc Chieftain".to_string()),
                },
            ],
        }];
        gs
    }

    #[test]
    fn test_rustory_encounter_spawns_all_creatures() {
        let gs = make_game_state_with_encounters();
        let initial_npcs = gs.npcs.len();
        let mut ctx = ScriptContext::new(gs, make_rng());

        let result = ctx.rustory_encounter(vec![Value::Yarn("Goblin Patrol".to_string())]);
        assert_eq!(result, Value::Numbr(3));
        assert_eq!(ctx.game_state.npcs.len(), initial_npcs + 3);
    }

    #[test]
    fn test_rustory_encounter_auto_names() {
        let gs = make_game_state_with_encounters();
        let mut ctx = ScriptContext::new(gs, make_rng());

        ctx.rustory_encounter(vec![Value::Yarn("Goblin Patrol".to_string())]);

        assert!(ctx.game_state.get_npc("Goblin Warrior #1").is_some());
        assert!(ctx.game_state.get_npc("Goblin Warrior #2").is_some());
        assert!(ctx.game_state.get_npc("Orc Chieftain").is_some());
    }

    #[test]
    fn test_rustory_encounter_unknown_name() {
        let gs = make_game_state_with_encounters();
        let mut ctx = ScriptContext::new(gs, make_rng());
        let initial_npcs = ctx.game_state.npcs.len();

        let result = ctx.rustory_encounter(vec![Value::Yarn("Dragon Horde".to_string())]);
        assert_eq!(result, Value::Noob);
        assert_eq!(ctx.game_state.npcs.len(), initial_npcs);
    }

    #[test]
    fn test_rustory_encounter_no_args() {
        let gs = make_game_state_with_encounters();
        let mut ctx = ScriptContext::new(gs, make_rng());

        let result = ctx.rustory_encounter(vec![]);
        assert_eq!(result, Value::Noob);
    }
}
