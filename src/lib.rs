//! Spice of Life: Carrot Edition for Horizon.
//!
//! Ported from Spice of Life: Carrot Edition 1.21.1-1.16.6
//! (`Cazsius/Spice-of-Life-Carrot-Edition`, LGPL-2.1) for Minecraft Java 1.21.1 NeoForge;
//! see `notes.md` and `FINDINGS.md` for the behaviour and its evidence.
//!
//! The pure rules live in [`runtime`] and are unit-tested by `cargo test`; this file is the
//! glue only — the one `impl Hooks`, the per-player state, the max-health write and the three
//! `/foodlist` usages. Nine moments:
//!
//! - `server_about_to_start`: read `config/solcarrot.conf` (writing the defaults).
//! - `server_started`: load the world's history file.
//! - `world_lifecycle`/`server_stopping`: flush it when the world or server closes.
//! - `living_entity_use_item_finish`: record a food when its use ran its time.
//! - `player_tick_post`: keep the max-health modifier in step (respawn, dimension change).
//! - `player_died`: honour `reset_on_death`.
//! - `register_commands`/`run_mod_command`: `/foodlist size|sync|clear [target]`.
//!
//! The jar's content is the food history itself, so nothing is carried: the package has no
//! `assets/` or `data/` and registers no id of its own.

pub mod config;
pub mod runtime;
pub mod store;

use std::collections::BTreeMap;
use std::sync::Mutex;

use mod_api::{
    self, AttributeModifier, AttributeOperation, Command, CommandArgumentValue, CommandSource,
    Flow, GameMode, Hooks, Host, HostSide, Identifier, ItemStack, LivingEntityUseItemFinishEvent,
    ModifierLifetime, PlayerDiedEvent, PlayerRef, PlayerTickPostEvent, RegisterCommandsEvent,
    RunModCommandEvent, ServerAboutToStartEvent, ServerHost, ServerStartedEvent,
    ServerStoppingEvent, Text, WorldLifecycleEvent, WorldPhase,
};

use crate::runtime::{
    Command as FoodCommand, Config, FoodHistory, HEALTH_MODIFIER_ID, format_uuid_hex,
    health_modifier_amount, milestones_achieved, progress,
};

/// The attribute this mod adds to (`MaxHealthHandler.java:21`).
const MAX_HEALTH: &str = "minecraft:max_health";

/// Vanilla's max health, the divisor the health scale uses when the read fails.
const VANILLA_MAX_HEALTH: f32 = 20.0;

/// The item component that makes an item food (`ItemInfo.food`'s source).
const FOOD_COMPONENT: &str = "minecraft:food";

/// The nutrition field inside `minecraft:food`, in half drumsticks.
const NUTRITION: &str = "nutrition";

/// One player's unique-food history.
#[derive(Default)]
struct PlayerState {
    history: FoodHistory,
}

/// Everything the hooks share: the settings in force and every loaded history.
#[derive(Default)]
struct Core {
    config: Config,
    players: BTreeMap<u128, PlayerState>,
}

/// The session state. `Hooks: Send + Sync`, so it is a `Mutex`, and the macro boxes one
/// `Default` instance for the session, so the state survives across calls.
static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn core() -> std::sync::MutexGuard<'static, Option<Core>> {
    CORE.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Before the server opens anything: read or create `config/solcarrot.conf`.
    fn server_about_to_start(
        &self,
        host: &mut dyn ServerHost,
        _event: &mut ServerAboutToStartEvent,
    ) {
        let config = config::load(&*host);
        let mut guard = core();
        guard.get_or_insert_with(Core::default).config = config;
    }

    /// The worlds are open and the tick loop has not run: load this world's history file.
    fn server_started(&self, host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        let players = store::load(&*host);
        let mut guard = core();
        let core = guard.get_or_insert_with(Core::default);
        core.players.clear();
        core.players.extend(
            players
                .into_iter()
                .map(|(uuid, history)| (uuid, PlayerState { history })),
        );
    }

    /// A world closed (finally, or with a failed save): flush the history file.
    fn world_lifecycle(&self, host: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
        match event.phase {
            WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
                let mut guard = core();
                let Some(core) = guard.as_mut() else {
                    return;
                };
                save(&*host, core);
                core.players.clear();
            }
            WorldPhase::WorldOpenFailed => {
                let mut guard = core();
                if let Some(core) = guard.as_mut() {
                    core.players.clear();
                }
            }
            _ => {}
        }
    }

    /// The server begins teardown: flush the history file (the worlds are still open).
    fn server_stopping(&self, host: &mut dyn ServerHost, _event: &mut ServerStoppingEvent) {
        let mut guard = core();
        let Some(core) = guard.as_mut() else {
            return;
        };
        save(&*host, core);
    }

    /// A use ran its time (`LivingEntityUseItemEvent.Finish`): the moment upstream's
    /// `FoodList` records a food. A use released early never fires it, so it is not counted.
    fn living_entity_use_item_finish(
        &self,
        host: HostSide<'_>,
        event: &mut LivingEntityUseItemFinishEvent<'_>,
    ) {
        // Fired on both sides; only the server writes the history.
        let HostSide::Server(host) = host else {
            return;
        };
        record_food(host, event.stack);
    }

    /// The end of a player's tick: keep the max-health modifier in step with the history (a
    /// respawn or a dimension change re-applies it here).
    fn player_tick_post(&self, host: HostSide<'_>, event: &mut PlayerTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let config = config_snapshot().unwrap_or_default();
        apply_health(host, event.player, &config);
    }

    /// A player died: upstream clears the list when `resetOnDeath` is on.
    fn player_died(&self, host: &mut dyn ServerHost, event: &mut PlayerDiedEvent<'_>) {
        let mut guard = core();
        let Some(core) = guard.as_mut() else {
            return;
        };
        if !core.config.reset_on_death {
            return;
        }
        if let Some(state) = core.players.get_mut(&event.player) {
            state.history.clear();
        }
        save(&*host, core);
    }

    /// Declare `/foodlist size|sync|clear [target]` (`FoodListCommand.java:49-55`).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        for usage in [
            "foodlist size [target:player]",
            "foodlist sync [target:player]",
        ] {
            event.commands.push(Command::new(usage));
        }
        event
            .commands
            .push(Command::op("foodlist clear [target:player]", 2));
    }

    /// Run one of them. The dispatcher has already parsed the usage and resolved the target,
    /// so this reads typed values instead of a line; the acting player's state is swapped
    /// in, so a self-targeted health change applies immediately.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let Some((command, target)) =
            requested(&event.command.literals, event.command.argument("target"))
        else {
            return Flow::Continue;
        };
        let sender = match event.source {
            CommandSource::Player { player, .. } => Some(*player),
            _ => None,
        };
        let Some(target) = target.or(sender) else {
            let _ = host
                .commands()
                .feedback(Text::literal("A player, or a player target, is needed"));
            return Flow::Cancel;
        };
        let uuid = target.uuid();
        match command {
            FoodCommand::Size(_) => {
                let (count, config) = counted(&*host, uuid);
                let progress = progress(count, &config);
                let name = target
                    .name(&*host)
                    .unwrap_or_else(|| format!("player {}", format_uuid_hex(uuid)));
                let mut lines = vec![format!("{name}: {count} unique foods eaten")];
                match (progress.has_reached_max, progress.next_milestone) {
                    (true, _) => lines.push("Every milestone has been reached".to_owned()),
                    (false, Some(next)) => lines.push(format!(
                        "Next milestone at {next} foods ({} to go)",
                        progress.foods_until_next.unwrap_or(0)
                    )),
                    (false, None) => {}
                }
                for line in lines {
                    let _ = host.commands().feedback(Text::literal(line));
                }
            }
            FoodCommand::Sync(_) => {
                if sender.map(PlayerRef::uuid) == Some(uuid) {
                    let (_, config) = counted(&*host, uuid);
                    apply_health(&mut *host, target, &config);
                }
                let _ = host
                    .commands()
                    .feedback(Text::literal("Food list synced".to_owned()));
            }
            FoodCommand::Clear(_) => {
                let config = {
                    let mut guard = core();
                    let core = guard.get_or_insert_with(Core::default);
                    if let Some(state) = core.players.get_mut(&uuid) {
                        state.history.clear();
                    }
                    save(&*host, core);
                    core.config.clone()
                };
                if sender.map(PlayerRef::uuid) == Some(uuid) {
                    apply_health(&mut *host, target, &config);
                }
                let _ = host
                    .commands()
                    .feedback(Text::literal("Food list cleared".to_owned()));
                if sender.map(PlayerRef::uuid) != Some(uuid) {
                    let _ = target.tell(&mut *host, Text::literal("Your food list was cleared"));
                }
            }
            FoodCommand::Unknown => return Flow::Continue,
        }
        Flow::Cancel
    }
}

/// The `/foodlist` usage the dispatcher parsed, with its typed target: the variant names the
/// subcommand (the runtime's own vocabulary) and the target is the resolved `player` argument.
/// `None` for a line this mod does not serve.
fn requested(
    literals: &[String],
    target: Option<&CommandArgumentValue>,
) -> Option<(FoodCommand, Option<PlayerRef>)> {
    let [root, sub] = literals else {
        return None;
    };
    if root != "foodlist" {
        return None;
    }
    let target = match target {
        Some(CommandArgumentValue::Player(player)) => Some(*player),
        Some(_) => return None,
        None => None,
    };
    let command = match sub.as_str() {
        "size" => FoodCommand::Size(None),
        "sync" => FoodCommand::Sync(None),
        "clear" => FoodCommand::Clear(None),
        _ => return None,
    };
    Some((command, target))
}

/// The settings in force, or `None` before the server start read them.
fn config_snapshot() -> Option<Config> {
    core().as_ref().map(|core| core.config.clone())
}

/// The nutrition (half drumsticks) of an item id, `None` for anything that is not a food.
fn nutrition_of(host: &dyn Host, id: &str) -> Option<i32> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    let food = host.item_default_component(key, FOOD_COMPONENT)?;
    match food.get(NUTRITION)? {
        mod_api::Tag::Int(nutrition) => Some(*nutrition),
        mod_api::Tag::Long(nutrition) => i32::try_from(*nutrition).ok(),
        mod_api::Tag::Double(bits) => Some(f64::from_bits(*bits) as i32),
        _ => None,
    }
}

/// Record one finished food for the acting player, save the file and answer the milestone
/// line.
///
/// The hook fires for every player of the world with that player's state swapped in, so the
/// acting player is the eater. It also fires for potions, spyglasses and mods' items: a stack
/// without a `minecraft:food` component is not a food and is skipped.
fn record_food(host: &mut dyn ServerHost, stack: &ItemStack) {
    let acting = host.players().acting_player();
    let Some(player) = acting else {
        return;
    };
    let uuid = player.uuid();
    let item = stack.item().id(&*host);
    let Some(nutrition) = nutrition_of(&*host, &item) else {
        return;
    };
    let config = config_snapshot().unwrap_or_default();
    if config.limit_progression_to_survival && player.game_mode(&*host) == Some(GameMode::Creative)
    {
        return;
    }
    let message = {
        let mut guard = core();
        let core = guard.get_or_insert_with(Core::default);
        let before = counted_in(core, &*host, uuid);
        let added = {
            let state = core.players.entry(uuid).or_default();
            state.history.record(&item, nutrition, &config)
        };
        let after = counted_in(core, &*host, uuid);
        if !added.newly_added {
            None
        } else {
            save(&*host, core);
            let reached = milestones_achieved(after, &config);
            (reached > milestones_achieved(before, &config)).then(|| {
                format!(
                    "Milestone {reached} reached: {after} foods eaten (+{} hearts)",
                    config.hearts_per_milestone
                )
            })
        }
    };
    if let Some(message) = message {
        let _ = player.action_bar(&mut *host, Text::literal(message));
    }
    apply_health(host, player, &config);
}

/// The stored counted-food total and the active settings.
fn counted(host: &dyn Host, uuid: u128) -> (usize, Config) {
    let guard = core();
    let Some(core) = guard.as_ref() else {
        return (0, Config::default());
    };
    (counted_in(core, host, uuid), core.config.clone())
}

/// The same count against an already locked core (a hook that also writes must not lock
/// twice).
fn counted_in(core: &Core, host: &dyn Host, uuid: u128) -> usize {
    core.players.get(&uuid).map_or(0, |state| {
        state
            .history
            .counted(|id| nutrition_of(host, id), &core.config)
    })
}

/// Apply (or update) this mod's max-health modifier and scale current health by the ratio of
/// the new maximum to the old, as upstream's `MaxHealthHandler` does.
///
/// The modifier is transient: the history file is authoritative and the next tick after a
/// respawn or a dimension change reapplies it, so nothing is saved twice.
fn apply_health(host: &mut dyn ServerHost, player: PlayerRef, config: &Config) {
    let (count, _) = counted(&*host, player.uuid());
    let desired = f64::from(health_modifier_amount(count, config));
    let Ok(identifier) = Identifier::parse(HEALTH_MODIFIER_ID) else {
        return;
    };
    let Some(living) = host.living(player.entity()) else {
        return;
    };
    let current = living
        .attribute_modifiers(&*host, MAX_HEALTH)
        .unwrap_or_default()
        .into_iter()
        .find(|modifier| modifier.id == identifier)
        .map_or(0.0, |modifier| modifier.amount);
    if (current - desired).abs() < f64::EPSILON {
        return;
    }
    let old_max = living.max_health(&*host).unwrap_or(VANILLA_MAX_HEALTH);
    let old_health = living.health(&*host).unwrap_or(old_max);
    let modifier = AttributeModifier::new(identifier, desired, AttributeOperation::AddValue);
    if !living.replace_attribute_modifier(
        &mut *host,
        MAX_HEALTH,
        modifier,
        ModifierLifetime::Transient,
    ) {
        return;
    }
    let new_max = living.max_health(&*host).unwrap_or(old_max);
    let new_health = if old_max > 0.0 {
        old_health * new_max / old_max
    } else {
        old_health
    };
    let _ = living.set_health(&mut *host, new_health.clamp(0.0, new_max));
}

/// Write the history file with every non-empty player's foods.
fn save(host: &dyn ServerHost, core: &Core) {
    let players: BTreeMap<u128, FoodHistory> = core
        .players
        .iter()
        .filter(|(_, state)| !state.history.is_empty())
        .map(|(uuid, state)| (*uuid, state.history.clone()))
        .collect();
    store::save(host, &players);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_three_usages_map_to_their_variants() {
        assert_eq!(
            requested(&words("foodlist size"), None).map(|(command, _)| command),
            Some(FoodCommand::Size(None))
        );
        assert_eq!(
            requested(&words("foodlist sync"), None).map(|(command, _)| command),
            Some(FoodCommand::Sync(None))
        );
        assert_eq!(
            requested(&words("foodlist clear"), None).map(|(command, _)| command),
            Some(FoodCommand::Clear(None))
        );
        assert_eq!(
            requested(&words("foodlist size"), None).map(|(_, target)| target),
            Some(None)
        );
    }

    #[test]
    fn other_lines_are_left_to_the_game() {
        for line in [
            "",
            "foodlist",
            "foodlist nope",
            "foodlist size extra",
            "food list size",
            "list",
        ] {
            assert_eq!(requested(&words(line), None), None, "{line}");
        }
    }

    #[test]
    fn an_argument_that_is_not_a_player_is_refused() {
        let argument = CommandArgumentValue::String("Alex".to_owned());
        assert_eq!(requested(&words("foodlist size"), Some(&argument)), None);
    }
}
