# Spice of Life: Carrot Edition — re-pin audit: Horizon `83ad384f`

Read-only source audit. **No port code was changed and nothing was built.**

| | |
|---|---|
| New Horizon | `83ad384f48d0742746d518737511882680aab5b7` — 427 hooks / 159 helpers, at `ports/_shared/pins/horizon-83ad384f` |
| New mixins | `e8cd0aa38cffef49cb2e088ef716baa107148c02`, at `ports/_shared/pins/horizon-mixins-e8cd0aa` |
| Old pin (baseline) | `c9d474d` — 108 hooks, **0 helpers** |
| Port state | slice 1: 17 runtime tests, 7 injections, 7 hooks (all v1, all still present at the new pin) |

Classification: **usable** = a helper a plan can call, a writable/cancellable hook output, or a
content field the loader reads; **hook-only** = listen, cannot write or deny; **absent** = no
symbol. Paths are relative to the pin root.

## Summary

| | |
|---|---|
| Deferred items in `notes.md` | **10** rows (1 functional gap, 8 feature/parity items, 1 pin limitation) |
| Newly realizable at `83ad384f` | **3** (exact finished-use; per-player attachments instead of the world tsv; client data sync) |
| Realizable already, effort only | 5 (food book item, book GUI, tooltips, sounds, particles) |
| Still blocked | 1 (`native_server_players` cannot see the single-player host) + one permission nuance to fix in port code |
| Highest-value next step | **replace the `player_tick` use-sampling heuristic with `living_entity_use_item_finish`** |

## 1. Deferred items, one row each

| # | Deferred (`notes.md:82-104`) | Pin symbol / file:line | Class | Verdict |
|---:|---|---|---|---|
| 1 | **Food-completion detection**: the port samples `Game::using`/`Game::use_duration` at `player_tick` and infers "finished" (`support.rs:5-12, 152-197`) | `native_living_entity_use_item_finish` `native_hooks.rs:6597`, catalog `native-hooks.json:1704` (v1, `contract: listen`, `result` writable), doc `docs/native-mods.md:939` | usable (listen + writable `result`) | **newly realizable — exact, no heuristic** |
| 2 | Persistence: `<world>/solcarrot/foods.tsv` keyed by UUID instead of a per-player attachment | `native_attachments` `native_hooks.rs:8406` (**new**) → `Attachments::get/set` (`horizon-world/src/attachments.rs:250-400`), `Holder::Player(u128)` (`attachments.rs:45-51`), `AttachmentType` with codec/`copy_on_death`/`sync` (`docs/native-mods.md:3386-3455`); saved in the player's data | usable (helper, **new**) | **newly realizable** (loses offline lookups, see §3) |
| 3 | `/foodlist clear <other>` at level 2 for every target (upstream lets a player clear only themselves below level 2) | `NativeServerPlayer.permission` in `perform_command` (`native_server.rs:444-475`), `NativeCommand::op(..., 2)` | usable (glue only) | realizable (port code, no pin) |
| 4 | Food book item + recipe | content item `components`, `docs/mods.md:203-230` (was available) | usable (content) | realizable (effort) |
| 5 | Food book GUI | `native_mod_screen_open` `native_hooks.rs:8968` (API 111) + screen hooks + `native_quad`/`native_item_model`; or `native_panel_items` `:9224`; container menu `native_open_menu` `native_menus.rs:124` (LAN guests only) | usable (helpers) | realizable (the host needs the mod screen, not a guest menu — see §3) |
| 6 | Tooltips | `tooltip` hook (present at `c9d474d` too) | hook-only | realizable (effort) |
| 7 | Client sync packets | attachment `sync: true` (`docs/native-mods.md:3428-3440`), `native_send_packet` `native_net.rs:201` (**new**) | usable (new) | **newly realizable** (and no longer needed for max health, which already syncs) |
| 8 | Sounds | `native_play_sound` `native_hooks.rs:8193` (exists at both pins), `native_play_local_sound` `:8782` (client-local; function existed, now catalogued as helper `play_local_sound`) | usable | realizable (effort) |
| 9 | Particles | `native_send_particles` `native_hooks.rs:8233` (old; item/trail/vibration kinds unsupported) | usable | realizable (effort, vanilla particle kinds) |
| 10 | Target resolution in a single-player world | `native_server_players` `native_server.rs:204-226` maps `lan.hub.guests` only | **absent** (host has no guest entry) | **still blocked** |

## 2. The finish hook in detail (the item that matters)

The function existed at `c9d474d` (`git show c9d474d:…/native_hooks.rs:2391`) but was **not in
that pin's `native-hooks.json`**, so a plan could not bind it — exactly what `notes.md:84-87`
records. At `83ad384f` it is a catalogued v1 hook with `"contract": "listen"`:

```json
{ "name": "living_entity_use_item_finish", "version": 1,
  "target": "horizon_client::game::native_hooks::Game<'_>::native_living_entity_use_item_finish",
  "neoforge": "LivingEntityUseItemEvent.Finish", "side": "server",
  "mod_api": { "contract": "listen" } }
```

Signature (`native_hooks.rs:6597-6617`): `stack: &ItemStack`, `entity: Option<i32>` (None for a
player; `native_acting_player()` says who), `hand: Hand`, `duration: i32` (always 0), `result:
&mut Option<ItemStack>`. It fires for consumables, potions, spyglasses and mods' items, server-side,
for every player of the world. `dispatch-tests/src/lib.rs:281-301` proves a mod can also replace
`result` (`item_finish_can_clear_vanilla_result`).

Consequences for the port:

- Record the food on the **exact** finish: the item id is `stack.item`, the eater is
  `native_acting_player().uuid`, the food test is the existing
  `game.items().get(&item).and_then(|i| i.food)` (`support.rs:190-206`).
- Drop `UseSample`, `use_sample`, the `game.using`/`game.use_duration` reads and the
  "last tick ran and the use is gone" inference (`support.rs:5-12, 29-41, 161-197, 243`).
- Two recorded gaps disappear on their own: a zero-duration food now fires the hook, and a use
  that is replaced by another use in the same tick can no longer be mis-detected.
- Potions/spyglasses are filtered out by `info.food` being `Some`, so `native_consume_effect`
  stays unnecessary.

## 3. Still blocked / trade-offs

1. **The single-player host is not in `native_server_players`** (`native_server.rs:204-226`):
   `/foodlist size <other>` and `/foodlist clear <other>` cannot resolve anyone in a
   single-player world, and `native_open_menu` (`native_menus.rs:112-124`) needs a
   `hub.guests` entry too. The port's self-targeting works (`player.uuid` from
   `perform_command`). Unchanged at this pin; a food-book GUI for the host must be the mod's own
   screen (`native_mod_screen_open`), not a server menu.
2. **Attachments are online-only.** `Holder::Player(uuid)` covers the acting player and online
   guests (`docs/native-mods.md:3428-3434`); an offline player is `MissingHolder`. Migrating
   persistence to an attachment therefore loses `/foodlist size <offline>` and
   `/foodlist clear <offline>`, which the world tsv supports today. Recommended: keep the tsv as
   the source of truth and use an attachment (with `sync: true`) only as the **client-facing
   mirror** for the book — or accept online-only and delete the tsv. This is a product call for
   the orchestrator, not a pin blocker.
3. **Permission nuance (port code, not pin).** `NativeServerPlayer.permission` is available in
   `perform_command` (`native_server.rs:447-452`), so the upstream rule (self below level 2,
   others at level 2) can be enforced instead of registering `clear` at level 2 for all targets.

## 4. Next step (highest value)

One code job, no content change:

1. Migrate first (shared recipe): `compatibility.json`, `[compatibility] horizon_revision` and
   mixins revision to the new pins; `tools/horizon.py` embedded-test target `--lib`, ctl package
   `horizon-client-lib`; ladder with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`.
2. `mixins/solcarrot/horizon-mixin.toml`: add

   ```toml
   [[injection]]
   id = "food-finished"
   hook = "living_entity_use_item_finish"
   hook_version = 1
   kind = "HEAD"
   helper = "finished.rs"
   bindings = [
     { name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" },
     { name = "stack", ty = "&ItemStack", value = "stack" },
   ]
   ```

   (`ItemStack` is already in scope in `native_hooks.rs`, as `simplemagnets/use.rs` shows.)
3. `finished.rs` → `solcarrot_state::on_use_item_finish(game, stack)`; move the recording block of
   `on_player_tick` (`support.rs:199-249`) into it, keep `player_tick` for `apply_health` only,
   and delete `UseSample` + the sampling code. Runtime tests: keep all 17, add one for "the same
   food recorded twice is one entry" if not already covered.
4. e2e (`tools/e2e.sh`): the existing scenario must stay green (4 foods → 20, 5 → 24, re-eat →
   24, potion ignored, clear → 20, restart → 24). Add a **release-early** case that only the
   new hook gets right: `ctl use-item --hold 0.6` (≈12 of 32 ticks) then `ctl stop` must not
   count the food (`/foodlist size` unchanged, max health unchanged). The control run
   (`hearts_per_milestone = 0`) must still end at 20.
5. Do **not** bundle the attachment migration into this job: it changes persistence format and
   the offline-command behaviour, so it needs its own brief and its own restart e2e.

## 5. Migration notes for this port

- All 7 bound hooks are still v1 with the same names (checked against the new pin's
  `native-hooks.json`, entry by entry):
  `server_about_to_start`, `world_lifecycle`, `player_tick`, `player_died`,
  `server_stopping`, `register_commands`, `perform_command` (`native-hooks.json` at the new pin).
- `perform_command`'s `ran` is now a documented `cancel`/`out: ran` contract; the port already
  writes `*ran = true`, which stays correct.
- The port reads `Game::using`/`Game::use_duration` (private fields of `Game` in module `game`);
  the injected module is a child of that module, so this still compiles at the new pin
  (`game.rs:2672-2675`) — but step 2 of §4 removes those reads anyway.
- `native_world_dir` is still stale after a close (`game.rs:12451` sets it, nothing clears it;
  `native_server.rs:492-496`). The port's existing "trust only between open and close" guard
  stays required.
