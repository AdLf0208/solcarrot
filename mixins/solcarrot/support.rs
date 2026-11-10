// Game-side glue for Spice of Life: Carrot Edition. Injected into
// `crate::game` as `solcarrot_state`; it needs the game, so it is not part of
// the standalone library (only `runtime.rs` is).
//
// Food completion: the pin's `living_entity_use_item_finish` hook (API 90,
// `LivingEntityUseItemEvent.Finish`) fires once, on the server, when a use ran
// its time; `on_use_item_finish` records the food there. A use released early
// never fires it, so it is not counted. `player_tick` only keeps the max-health
// modifier in step (respawn, dimension change, another player's history).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use horizon_client::items::ItemStack;
use horizon_client::text::Text;

use crate::game::native_lifecycle::WorldPhase;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};
use crate::game::solcarrot::{
    Command as FoodCommand, Config, FoodHistory, HEALTH_MODIFIER_ID, STORE_DIR, STORE_FILE,
    format_config, format_store, health_modifier_amount, milestones_achieved, parse_command,
    parse_config, parse_store, progress,
};

#[derive(Default)]
struct PlayerState {
    history: FoodHistory,
}

#[derive(Default)]
struct Core {
    config: Config,
    world: Option<PathBuf>,
    players: BTreeMap<u128, PlayerState>,
}

static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn core() -> std::sync::MutexGuard<'static, Option<Core>> {
    CORE.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Before the server starts: read `config/solcarrot.conf`, writing upstream's
/// defaults when it is absent.
#[allow(private_interfaces)]
pub fn on_server_about_to_start(_game: &mut crate::game::Game<'_>) {
    let mut config = Config::default();
    match crate::game::Game::native_config_dir() {
        Ok(dir) => {
            let path = dir.join(crate::game::solcarrot::CONFIG_FILE);
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    let text = format_config(&config);
                    if fs::create_dir_all(&dir)
                        .and_then(|()| fs::write(&path, &text))
                        .is_err()
                    {
                        log::warn!("[solcarrot] could not write {}", path.display());
                    }
                    text
                }
            };
            config = parse_config(&text);
            log::info!(
                "[solcarrot] config {} (base_hearts={}, hearts_per_milestone={}, milestones={:?})",
                path.display(),
                config.base_hearts,
                config.hearts_per_milestone,
                config.milestones
            );
        }
        Err(error) => log::warn!("[solcarrot] no config dir: {error}"),
    }
    let mut guard = core();
    let core = guard.get_or_insert_with(Core::default);
    core.config = config;
}

/// The world opened or closed: load/save `<world>/solcarrot/foods.tsv`.
#[allow(private_interfaces)]
pub fn on_world_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    match phase {
        WorldPhase::WorldOpenReady => {
            let world = game.native_world_dir();
            let mut guard = core();
            let core = guard.get_or_insert_with(Core::default);
            core.world.clone_from(&world);
            core.players.clear();
            if let Some(world) = &world {
                let path = world.join(STORE_DIR).join(STORE_FILE);
                match fs::read_to_string(&path) {
                    Ok(text) => {
                        for (uuid, history) in parse_store(&text) {
                            core.players.insert(uuid, PlayerState { history });
                        }
                        log::info!("[solcarrot] loaded {}", path.display());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => log::warn!("[solcarrot] read {}: {error}", path.display()),
                }
            }
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            let mut guard = core();
            if let Some(core) = guard.as_mut() {
                save(core);
                core.world = None;
                core.players.clear();
            }
        }
        WorldPhase::WorldOpenFailed => {
            let mut guard = core();
            if let Some(core) = guard.as_mut() {
                core.world = None;
                core.players.clear();
            }
        }
    }
}

#[allow(private_interfaces)]
pub fn on_server_stopping(_game: &mut crate::game::Game<'_>) {
    let mut guard = core();
    if let Some(core) = guard.as_mut() {
        save(core);
    }
}

/// End of a player's server tick: keep the max-health modifier in step with
/// the stored history (a respawn or dimension change re-applies it here).
#[allow(private_interfaces)]
pub fn on_player_tick(game: &mut crate::game::Game<'_>) {
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let uuid = player.uuid;
    let config = {
        let guard = core();
        guard
            .as_ref()
            .map(|core| core.config.clone())
            .unwrap_or_default()
    };
    apply_health(game, uuid, &config);
}

/// A food's use ran its time (`living_entity_use_item_finish`, API 90): record
/// it exactly once, then keep the max-health modifier in step.
///
/// The hook fires for every player of the world with that player's state
/// swapped in, so `native_acting_player` names the eater. It also fires for
/// potions, spyglasses and mods' items; `info.food` is the filter.
#[allow(private_interfaces)]
pub fn on_use_item_finish(game: &mut crate::game::Game<'_>, stack: &ItemStack) {
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let uuid = player.uuid;
    let game_mode = player.game_mode;
    let item = stack.item.to_string();
    let Some(nutrition) = food_value(game, &item) else {
        return;
    };
    let config = {
        let guard = core();
        guard
            .as_ref()
            .map(|core| core.config.clone())
            .unwrap_or_default()
    };
    if config.limit_progression_to_survival && game_mode == 1 {
        return;
    }
    let milestone_message = {
        let mut guard = core();
        let core = guard.get_or_insert_with(Core::default);
        let state = core.players.entry(uuid).or_default();
        let before = state.history.counted(|id| food_value(game, id), &config);
        let added = state.history.record(&item, nutrition, &config);
        let after = state.history.counted(|id| food_value(game, id), &config);
        if !added.newly_added {
            None
        } else {
            save(core);
            let reached = milestones_achieved(after, &config);
            (reached > milestones_achieved(before, &config)).then(|| {
                format!(
                    "Milestone {reached} reached: {after} foods eaten (+{} hearts)",
                    config.hearts_per_milestone
                )
            })
        }
    };
    if let Some(message) = milestone_message {
        game.native_action_bar(uuid, Text::literal(message));
    }
    apply_health(game, uuid, &config);
}

/// The nutrition (half drumsticks) of a food id, None for anything that is not
/// a food (`ItemInfo.food`).
fn food_value(game: &crate::game::Game<'_>, id: &str) -> Option<i32> {
    game.items()
        .get(id)
        .and_then(|info| info.food)
        .map(|(nutrition, _)| nutrition as i32)
}

/// A player died: upstream clears the list when `resetOnDeath` is on.
#[allow(private_interfaces)]
pub fn on_player_died(_game: &mut crate::game::Game<'_>, player: u128) {
    let config = {
        let guard = core();
        guard.as_ref().map(|core| core.config.clone())
    };
    let Some(config) = config else {
        return;
    };
    if !config.reset_on_death {
        return;
    }
    let mut guard = core();
    let Some(core) = guard.as_mut() else {
        return;
    };
    if let Some(state) = core.players.get_mut(&player) {
        state.history.clear();
    }
    save(core);
}

/// Register `/foodlist size|sync|clear [target]` (`FoodListCommand.java:49-55`).
#[allow(private_interfaces)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    for usage in [
        "foodlist size [target:player]",
        "foodlist sync [target:player]",
    ] {
        commands.push(NativeCommand::new(usage));
    }
    commands.push(NativeCommand::op("foodlist clear [target:player]", 2));
}

/// Run a `/foodlist` command. `perform_command` swaps the sender's state in, so
/// self-targeted health changes apply immediately; another player's history is
/// applied on their next tick.
#[allow(private_interfaces)]
pub fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let action = parse_command(command);
    if action == FoodCommand::Unknown {
        return;
    }
    *ran = true;
    let (target, subcommand) = match &action {
        FoodCommand::Size(target) => (target.as_deref(), "size"),
        FoodCommand::Sync(target) => (target.as_deref(), "sync"),
        FoodCommand::Clear(target) => (target.as_deref(), "clear"),
        FoodCommand::Unknown => return,
    };
    let Some(uuid) = resolve(game, player, target) else {
        game.native_feedback(Text::literal(format!(
            "No player named {}",
            target.unwrap_or("?")
        )));
        return;
    };
    match subcommand {
        "size" => {
            let (count, config) = counted(game, uuid);
            let progress = progress(count, &config);
            game.native_feedback(Text::literal(format!(
                "{}: {} unique foods eaten",
                command_name(uuid, player),
                count
            )));
            match (progress.has_reached_max, progress.next_milestone) {
                (true, _) => game
                    .native_feedback(Text::literal("Every milestone has been reached".to_owned())),
                (false, Some(next)) => game.native_feedback(Text::literal(format!(
                    "Next milestone at {next} foods ({} to go)",
                    progress.foods_until_next.unwrap_or(0)
                ))),
                (false, None) => {}
            }
        }
        "sync" => {
            if uuid == player.uuid {
                let (_, config) = counted(game, uuid);
                apply_health(game, uuid, &config);
            }
            game.native_feedback(Text::literal("Food list synced".to_owned()));
        }
        "clear" => {
            let config = {
                let mut guard = core();
                let core = guard.get_or_insert_with(Core::default);
                if let Some(state) = core.players.get_mut(&uuid) {
                    state.history.clear();
                }
                save(core);
                core.config.clone()
            };
            if uuid == player.uuid {
                apply_health(game, uuid, &config);
            }
            game.native_feedback(Text::literal("Food list cleared".to_owned()));
            if uuid != player.uuid {
                let _ =
                    game.native_tell(uuid, Text::literal("Your food list was cleared".to_owned()));
            }
        }
        _ => {}
    }
}

fn resolve(
    game: &crate::game::Game<'_>,
    player: &NativeServerPlayer,
    target: Option<&str>,
) -> Option<u128> {
    match target {
        None => Some(player.uuid),
        Some(name) if name == player.name => Some(player.uuid),
        Some(name) => game
            .native_server_players()
            .into_iter()
            .find(|candidate| candidate.name == name)
            .map(|candidate| candidate.uuid),
    }
}

fn command_name(uuid: u128, player: &NativeServerPlayer) -> String {
    if uuid == player.uuid {
        player.name.clone()
    } else {
        format!("player {}", crate::game::solcarrot::format_uuid_hex(uuid))
    }
}

/// The stored counted-food total and the active config.
fn counted(game: &crate::game::Game<'_>, uuid: u128) -> (usize, Config) {
    let guard = core();
    let Some(core) = guard.as_ref() else {
        return (0, Config::default());
    };
    let config = core.config.clone();
    let count = core.players.get(&uuid).map_or(0, |state| {
        state.history.counted(
            |id| {
                game.items()
                    .get(id)
                    .and_then(|info| info.food)
                    .map(|(nutrition, _)| nutrition as i32)
            },
            &config,
        )
    });
    (count, config)
}

/// Apply (or update) this mod's permanent-ish max-health modifier. The pin's
/// `PlayerMotion::set_modifier` adds a transient modifier, which is enough: the
/// history file is authoritative and the glue reapplies it on the next tick
/// after a respawn or a dimension change.
fn apply_health(game: &mut crate::game::Game<'_>, uuid: u128, config: &Config) {
    let (count, _) = counted(game, uuid);
    let desired = f64::from(health_modifier_amount(count, config));
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let key = player.attributes.key("minecraft:max_health");
    let current = key
        .as_ref()
        .and_then(|key| player.attributes.lookup(Some(key)))
        .and_then(|handle| player.attributes.instance(&handle).ok())
        .and_then(|instance| instance.modifier(HEALTH_MODIFIER_ID))
        .map_or(0.0, |modifier| modifier.amount());
    if (current - desired).abs() < f64::EPSILON {
        return;
    }
    let old_max = player.attribute("minecraft:max_health").unwrap_or(20.0);
    let old_health = player.health.health();
    if let Err(error) = player.set_modifier(
        "minecraft:max_health",
        HEALTH_MODIFIER_ID,
        desired,
        "add_value",
        true,
    ) {
        log::warn!("[solcarrot] max health modifier: {error}");
        return;
    }
    let new_max = player.attribute("minecraft:max_health").unwrap_or(old_max);
    let new_health = if old_max > 0.0 {
        (f64::from(old_health) * new_max / old_max) as f32
    } else {
        old_health
    };
    let _ = player.set_health(new_health.clamp(0.0, new_max as f32), false);
}

/// Write the history file atomically. Called after every new food and on
/// orderly shutdown; a missing world (a client) is a no-op.
fn save(core: &Core) {
    let Some(world) = &core.world else {
        return;
    };
    let dir = world.join(STORE_DIR);
    let players: BTreeMap<u128, FoodHistory> = core
        .players
        .iter()
        .filter(|(_, state)| !state.history.is_empty())
        .map(|(uuid, state)| (*uuid, state.history.clone()))
        .collect();
    let text = format_store(&players);
    let path = dir.join(STORE_FILE);
    let temporary = dir.join("foods.tsv.tmp");
    let result = fs::create_dir_all(&dir)
        .and_then(|()| fs::write(&temporary, text))
        .and_then(|()| fs::rename(&temporary, &path));
    if let Err(error) = result {
        log::warn!("[solcarrot] write {}: {error}", path.display());
    }
}
