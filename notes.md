# Spice of Life: Carrot Edition — port notes

> **Format (2026-11-18): the port is now a flat SDK `.hmod`.** Horizon retired `.hnmod` in the
> mod-API merge (#16941, launcher ≥ 0.2.70); the bundle this file described below (PR #809,
> closed) was migrated to `horizon-mod.toml` + `Cargo.toml` + `src/lib.rs`
> (`impl mod_api::Hooks`) at Horizon **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** /
> horizon-mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, mod API `0.2`. Read
> **`## Migration to the SDK format`** at the end of this file first: it is the current state.
> Everything above it is the history of the retired bundle (pins `c9d474d`/`83ad384f`/`1d63e0a5`,
> `.hnmod`, `mixins/`, the staging ladder) and is kept as the record, not as instructions —
> `tools/horizon.py`, `compatibility.json`, `mixins/<id>/` and the `.hnmod` no longer exist here.

Ported from **Spice of Life: Carrot Edition 1.21.1-1.16.6**
(`Cazsius/Spice-of-Life-Carrot-Edition`, **LGPL-2.1**) for Minecraft Java 1.21.1
NeoForge. Upstream read at commit `412fa94a07d93f09378fc0e3393de38eb0099824`
(branch `1.21`); full area list and pin evidence in
[`../_shared/upstream/solcarrot/FINDINGS.md`](../_shared/upstream/solcarrot/FINDINGS.md).

Status: **implemented and verified in-game** — 17 runtime tests, staged
`stage → check → test → build`, and the e2e scenarios below. Local-only until
the maintainer approves publication (LGPL-2.1, ARCHITECTURE.md §8).

**Class + tier** (from `ports/ledger.csv`, per `horizon/docs/modpacks.md`):
**content / logic**. The approach is therefore the **logic shape**: a pure
tracking core in `runtime.rs`, `use`/tick/lifecycle hooks in the glue next to it, and
commands. The mod's "carried content" is the food history, not assets: the jar holds no
blocks, items, recipes or tags this slice needs, so the SDK package ships **no `assets/` or
`data/`** either (the retired `.hmod` did not exist for the same reason).

## Behaviour (slice 1)

- **History**: one per player, a set of food **item ids**. A food is recorded
  when it finishes being eaten; re-eating it adds nothing. Filtered foods
  (below the nutrition floor, blacklisted, or outside a non-empty whitelist)
  are still remembered but do not count, exactly as upstream's
  `FoodList.addFood` does.
- **Milestones**: `milestones` (default `5,10,15,20,25`) count unique counted
  foods. Each reached milestone grants `2 * hearts_per_milestone` health points
  on top of vanilla's 20; `base_hearts` other than 10 adds
  `2 * (base_hearts - 10)` to the same modifier. At the default settings:
  5 foods → 24 max health, 10 → 28, 25 → 40.
- **Health update**: the modifier id is `solcarrot:health_gained` and current
  health is scaled by `new_max / old_max`, as upstream. It is reapplied on the
  player's next tick after a respawn or dimension change.
- **Feed back**: the first food of a run and every new milestone stay silent in
  chat except for one action-bar line on a milestone. (Upstream's sounds,
  particles, book and tooltip are not in this slice.)
- **Reset**: `/foodlist clear [target]` (level 2) clears the history and drops
  the bonus; `reset_on_death = true` clears it on death.
- **Status**: `/foodlist size [target]` reports the counted total and the next
  milestone; `/foodlist sync [target]` re-applies the bonus.
- **Creative**: `limit_progression_to_survival = true` skips tracking when the
  eater is in creative (default false, as upstream).

## Config (`config/solcarrot.conf`)

A missing file is written with these defaults:

```
base_hearts = 10
hearts_per_milestone = 2
milestones = 5,10,15,20,25
blacklist =
whitelist =
minimum_food_value = 1
reset_on_death = false
limit_progression_to_survival = false
```

`*` is the only wildcard in the filter lists. An empty `milestones` list means
no bonus (upstream would panic; recorded deviation).

## Persistence

`<world>/solcarrot/foods.tsv`, one line per `(uuid, food id)`:

```
# solcarrot v1
0123456789abcdef0123456789abcdef	minecraft:apple
```

Loaded at `WorldOpenReady`, saved immediately after every new food, on
`server_stopping` and on the final world close (write to `.tmp`, then rename).
The format is the port's own: `83ad384f` now offers `native_attachments`, but
an attachment is online-only (`Holder::Player(uuid)` covers the acting player
and online guests; an offline player is `MissingHolder`), while the world file
keeps `/foodlist size|clear <offline>` working. The attachment was therefore
**not** adopted in the re-pin slice; moving to one is its own job (see the
audit §3.2).

## Hooks

| Hook | Helper | What it does |
|---|---|---|
| `server_about_to_start` (v1, API 44) | `about.rs` | Read/create `config/solcarrot.conf`. |
| `world_lifecycle` (v1, API 44) | `world.rs` | Load at open, save/flush on close. |
| `living_entity_use_item_finish` (v1, API 90) | `finished.rs` | Record a food exactly once when its use completes. |
| `player_tick` (v1, API 31) | `player.rs` | Apply the max-health modifier (respawn, dimension change). |
| `player_died` (v1, API 21) | `dying.rs` | Clear the history when `reset_on_death` is on. |
| `server_stopping` (v1, API 44) | `stopping.rs` | Flush the history file. |
| `register_commands` (v1, API 8) | `register.rs` | Declare `/foodlist size|sync|clear [target]`. |
| `perform_command` (v1, API 8) | `perform.rs` | Run them. |

### Food completion at the re-pin

Slice 1 ran at `c9d474d`, where the finished-use hook existed in the source but
was **not catalogued** in `native-hooks.json`, so a plan could not bind it; the
port sampled `Game::using`/`use_duration` in `player_tick` and inferred
"finished" from "the last tick ran and the use is gone".

At `83ad384f` `living_entity_use_item_finish` is a catalogued v1 hook
(`native_hooks.rs:6597`, `contract: listen`) that fires once, server-side, for
every player when a use ran its time. `on_use_item_finish` records the food
there: the item is `stack.item`, the eater is `native_acting_player().uuid`,
and `info.food` filters out potions, spyglasses and mods' items. The sampling
state (`UseSample`, `use_sample`, the `using`/`use_duration` reads) is gone.

Consequences: a released-early use never fires the hook and is not counted
(proved in the e2e with `use-item --hold 0.6` on a 1.6 s food), a zero-duration
food is counted, and a use replaced by another in the same tick can no longer
be mis-detected. `player_tick` keeps only the max-health reapply.

## Differences from upstream (deliberate)

1. Persistence is a world text file, not a NeoForge player attachment; config
   is a `key = value` file, not a `ModConfigSpec`.
2. `/foodlist clear` is registered at permission level 2 for every target
   (upstream lets a player clear only themselves below level 2).
3. Plain-text feedback; the food book item/recipe/GUI, tooltips, client sync
   packets, sounds and particles are deferred.
4. Target resolution uses `native_server_players()`; in a single-player world
   only the sender's own name is resolvable (the host is not listed).

Skills: superpowers/using-superpowers,verification-before-completion; BeautifulRust/rust-best-practices

## Verification (re-pin `83ad384f`, 2026-11-06; migrated to `1d63e0a5`, 2026-11-10)

### Campaign-pin migration — Horizon `1d63e0a5`, mixins `5a64af93` (2026-11-10)

`compatibility.json` and `mixins/solcarrot/horizon-mixin.toml` now name the
campaign pin `1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9` (`game.rs` SHA-256
`64813cf4…cef80f`) / horizon-mixins `5a64af938ec86b00d53d16a408aabc0038fb3e57`;
`tools/horizon.py` was replaced by the campaign template's wrapper, whose
embedded-test form is `--lib game::solcarrot::`. The full ladder was re-run
through `ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0`:

| Check | Result |
|---|---|
| `cargo fmt --all --check` + `rustfmt --edition 2024 --check mixins/solcarrot/*.rs` | clean |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **17 passed**, 0 failed |
| `tools/horizon.py stage/check/test/build --horizon …/pins/horizon-1d63e0a` | `cargo_exit: 0`, **0 warnings**, 17 embedded tests, fast build |
| `tools/e2e.sh` (default, shared 1d63e0a5 `horizon-ctl`, port game) | **PASS**, `EXIT:0` — `ports/_shared/evidence/solcarrot/e2e.log` |
| `MODE=control tools/e2e.sh` (differential) | **CONTROL PASS**, `EXIT:0` — `e2e-control.log` |
| `tools/package.py` | `.hnmod` **14,811 B**, SHA-256 `a6a96c5f7d40af4b094e52134fa9c8638f949ff39563a0fd6ad22593f40e8d2f`, 12 entries; reproducible |

The e2e log ends with `EXIT:0` (the script now prints it from its cleanup trap),
and `python3 ports/tools/check-e2e-logs.py solcarrot --strict` is green.

Staged at Horizon `83ad384f48d0742746d518737511882680aab5b7` / horizon-mixins
`e8cd0aa38cffef49cb2e088ef716baa107148c02`, via
`ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0`, the slot stage/target
and `/home` temp dirs.

| Command | Evidence |
|---|---|
| `python3 tools/horizon.py check --horizon <pin>` | `cargo_exit: 0`, `stage_changed: false`; `grep -c warning: target/horizon-check.log` → **0** |
| `python3 tools/horizon.py test --horizon <pin>` | `cargo_exit: 0`; `test result: ok. 17 passed; 0 failed; 0 ignored; 913 filtered out`; 0 warnings |
| `python3 tools/horizon.py build --horizon <pin>` | exit 0; fast build finished; `target/horizon-cargo/fast/horizon-client` |
| `cargo build -p horizon-client-lib --bin horizon-ctl` | exit 0; `target/horizon-cargo/fast/horizon-ctl` |
| `tools/e2e.sh` (default) | exit 0, `PASS` (below), log `target/G9-e2e.log` |
| `MODE=control tools/e2e.sh` | exit 0, `CONTROL PASS` (below), log `target/G9-e2e-control.log` |
| `python3 tools/package.py` | `.hnmod` **14,814 B**, SHA-256 `061e8fc828329bd7f815450b75e593a35bbb5cfa625c596349ae2cfde792f73b` |

The release-early phase (`use-item --hold 0.6` on a 1.6 s golden carrot) is a
new assertion only the finished-use hook can satisfy: the e2e requires
`/foodlist size` to stay **0** and max health **20**, and it passed:

```
== a use released before it finishes is not counted ==
max health 20 (released early)
```

Default run (`target/G9-e2e.log`), 8 assertions:

```
max health 20 (baseline)
max health 20 (released early)
max health 20 (four foods: below the first milestone)
max health 24 (five foods: first milestone (+2 hearts))
max health 24 (re-eating a known food)
max health 24 (sixth new food below the next milestone)
max health 20 (clear drops the bonus)
max health 24 (regrown five foods)
max health 24 (history reloaded after restart)
PASS: 4 foods stayed 20, 5 reached 24, a re-eat and a sixth stayed 24, a potion was ignored, clear returned to 20, and 24 survived a restart
```

Control run (`target/G9-e2e-control.log`, `hearts_per_milestone = 0`):

```
max health 20 (baseline)
max health 20 (released early)
max health 20 (four foods: below the first milestone)
max health 20 (control: config turns the bonus off)
max health 20 (re-eating a known food)
max health 20 (sixth new food below the next milestone)
max health 20 (clear drops the bonus)
CONTROL PASS: five foods, a re-eat and a sixth food stayed at 20 max health with hearts_per_milestone=0
```

That is the differential: the identical scenario with the default config reaches
24, so a broken/ignored health path fails the default run, and a broken config
path fails the control. The in-game run does not exercise survival hunger, the
blacklist/whitelist filters or `reset_on_death`; those stay covered by the
runtime tests.

## Review repairs (PR #576, 2026-11-11)

Review: `ports/_shared/horizon-mods/reviews/pr-576-solcarrot.md` (CHANGES_REQUESTED, 2 items).

| Reviewer's item | This job |
|---|---|
| 1. artifact not staged | **orchestrator** (no write access) |
| 2. relative link; milestones text ends at "base_hearts other than 10 adds"; "Not yet ported: nothing recorded yet" hides the first-slice gaps | **fixed**: link was already absolute; the description now completes the milestone arithmetic and lists the real gaps (the food book and its screen, the client sync packet, sounds/particles, the attachment-vs-file trade-off) |

The reviewer could not read LGPL source; the port is LGPL-2.1-only like the original with
the licence and the complete corresponding source in the archive.

Artifact: `solcarrot.hnmod` **24,321 B** `dae8511db6b2a92a0f52f5aa00982bc58eb23451f20ac7248992b060c84e3fcd`.

## Migration to the SDK format (2026-11-18, SDK `0b14f8a`)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, registry PR #17992) retired the `.hnmod`
container, and the registry closed this port's submission (**PR #809**) with *"republish this
as a native `.hmod` with the SDK from the next Nightly"*. The port is now one flat package,
following the verified recipe in `ports/imfast/notes.md` (§"Migration to the SDK format") and
`ports/repeatable_trial_vaults/notes.md` (the second migration's trap list).

- **Pin:** Horizon **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (nightly 2026-11-18),
  horizon-mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, mod API **`0.2`**, package
  format **6**, surface `72aaa95c…` (749 hooks / 215 traits, cached at
  `ports/_shared/sdk/0b14f8a/`). The SDK is fetched into `.horizon/sdk/` by
  `cargo horizon-mod` and never packed.
- **Version:** the port's own **0.2.0** (the retired bundle was 0.1.0; a resubmission is always
  a new version).
- **Class + tier unchanged:** class **content**, tier **logic** — nine server-side moments
  over the same pure runtime. No machine, deep or client feature is added.

### File map — created, deleted, where the evidence went

| Created | What it holds |
|---|---|
| `horizon-mod.toml` | the only manifest: `id = "solcarrot"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, the required `capabilities = ["registries"]`, `[native]` with the exact `game`/`horizon`/`mixins` |
| `Cargo.toml` | one crate, edition 2024 (`license = "LGPL-2.1"`, no `rust-version`); its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: the nine moments, the session state, the max-health write, the `/foodlist` usages, and 3 glue tests |
| `src/runtime.rs` | the retired `mixins/solcarrot/runtime.rs`, **byte-identical** (sha256 `526cbcc2…8216e`, 717 lines, 17 tests) — `cmp` clean before the old tree was deleted, and the same file inside the package |
| `src/config.rs` | `config/solcarrot.conf` through the host's own checked config service (new file: the retired `support.rs` used `Game::native_config_dir()` + `std::fs`) |
| `src/store.rs` | the history file through the host's save-scoped data service (new file: the retired `support.rs` wrote the path itself) |
| `LICENSE`, `LICENSE-NOTICE.md` | the LGPL-2.1 text and its provenance, **inside** the package |
| `README.md`, `AGENTS.md` | rewritten for the flat layout (the old ones named `compatibility.json` and the mixin plan) |
| `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` |

| Deleted | Why | Where it went |
|---|---|---|
| `mixins/solcarrot/` (`horizon-mixin.toml`, `runtime.rs`, `support.rs`, `about.rs`, `world.rs`, `player.rs`, `finished.rs`, `dying.rs`, `stopping.rs`, `register.rs`, `perform.rs`) | the plan/injection schema and the one-helper-per-file layout are not part of any SDK package | `runtime.rs` → `src/`; `support.rs` → `src/lib.rs` + `src/config.rs` + `src/store.rs`; the eight hook files → `impl Hooks` methods |
| `solcarrot.hnmod` (24,321 B) | unloadable on launcher ≥ 0.2.70 | `ports/_shared/evidence/solcarrot/retired-hnmod-0.1.0/` |
| `compatibility.json`, `rust-toolchain.toml` | the pin is `[native]` in the manifest; "not part of any new package" | same folder |
| `tools/horizon.py`, `package.py`, `package.sh`, `notice.py`, `test_stage.py`, `test_template.py`, `audit/` | the staged mixin ladder and `.hnmod` packaging are gone; there is no stage, plan or module-check step | same folder (`tools/`) |
| `Cargo.toml`, `Cargo.lock` (the old crate) | replaced by the SDK crate; a packed lock changes the artifact | old `Cargo.toml` kept as evidence, the lock not kept |

Kept on purpose: `tools/e2e.sh` (the scenario and its assertions are format-independent; its
header now says what a launcher-built game is needed for and the two paths the SDK moved),
`FINDINGS.md`, `VALIDATION.md`, `REPIN-AUDIT.md`, `README.md`, `notes.md`, `LICENSE`.

### Hook → `impl Hooks` mapping

| Retired | SDK (0.2) | Notes |
|---|---|---|
| `server_about_to_start` + `creates the config` helper | `fn server_about_to_start(&self, host, _: &mut ServerAboutToStartEvent)` | `config::load(&*host)`; `ConfigHost::config_files()` gives the mod its own `config/solcarrot.conf` |
| `world_lifecycle` (open) | **`fn server_started(&self, host, _: &mut ServerStartedEvent)`** | `WorldPhase::WorldOpenReady` **no longer exists**: the SDK's own migration note says a mod matching it implements `server_started`, fired at the same moment |
| `world_lifecycle` (close phases) | `fn world_lifecycle(&self, host, event: &mut WorldLifecycleEvent<'_>)` | `event.phase` is `WorldPhase`; only `WorldCloseFinal`/`WorldCloseFailed`/`WorldOpenFailed` exist |
| `server_stopping` | `fn server_stopping(&self, host, _: &mut ServerStoppingEvent)` | the worlds are still open, so the save still works there |
| `living_entity_use_item_finish` + `stack` binding | `fn living_entity_use_item_finish(&self, host: HostSide<'_>, event: &mut LivingEntityUseItemFinishEvent<'_>)` | the hook fires on **both** sides and hands `HostSide`; only `HostSide::Server` writes. `event.stack` is the finished stack, and `host.players().acting_player()` is the eater (as the old `native_acting_player()` was) |
| `player_tick` | **`fn player_tick_post(&self, host: HostSide<'_>, event: &mut PlayerTickPostEvent)`** | also both sides; `event.player` names the ticked player, so no acting-player read is needed |
| `player_died` + `player` binding | `fn player_died(&self, host, event: &mut PlayerDiedEvent<'_>)` | `event.player` is the UUID |
| `register_commands` + `&mut Vec<NativeCommand>` | `fn register_commands(&self, _host, event: &mut RegisterCommandsEvent<'_>)` | `event.commands: &mut Vec<mod_api::Command>`; `Command::new(usage)` / `Command::op(usage, 2)` are the old `NativeCommand::new/op` |
| `perform_command` + a raw line | `fn run_mod_command(&self, host, event: &mut RunModCommandEvent<'_>) -> Flow` | the dispatcher parses: `event.command.literals` is `["foodlist", "size"]`, `event.command.argument("target")` is a typed `CommandArgumentValue::Player`; answering is `Flow::Cancel` |
| `game.native_feedback(Text)` | `host.commands().feedback(Text)` | routed to the command's source with vanilla's silence/track-output rules |
| `game.native_action_bar(uuid, Text)` | `player.action_bar(&mut *host, Text)` | the milestone line |
| `game.native_tell(uuid, Text)` | `player.tell(&mut *host, Text)` | "Your food list was cleared" |
| `game.items().get(id).food` | `host.item_default_component(key, "minecraft:food")` | read `nutrition` out of the component (Int/Long/Double); there is **no** nutrition accessor in the SDK |
| `stack.item` (`&str` id) | `stack.item().id(&*host)` | `ItemKey` is an opaque u32 slot; `ItemKey::id(host)` is the canonical id |
| `player.set_modifier(... "add_value", true)` | `living.replace_attribute_modifier(host, "minecraft:max_health", AttributeModifier::new(id, amount, AttributeOperation::AddValue), ModifierLifetime::Transient)` | read back through `living.attribute_modifiers`, scale health with `living.set_health` |
| `static CORE: Mutex<Option<Core>>` in `support.rs` | the same `static` in `src/lib.rs` | `Hooks: Any + Send + Sync`; the macro boxes one `Default` instance for the session |

`runtime.rs` is untouched: `parse_config`/`format_config`, `should_count`, `counted_foods`,
`milestones_achieved`, `health_modifier_amount`, `progress`, `FoodHistory`, `format_store`/
`parse_store`, `format_uuid_hex`, the `Command` enum and its constants are called exactly as
before. `parse_command` stays as the record of the three usages (the dispatcher now supplies
the same information parsed); the new glue only converts between the SDK's types and theirs.

### Build and check (verbatim)

```sh
# runtime + glue tests (the runtime's 17 ran green before any glue existed)
ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step test \
    -- bash -c 'cd ports/solcarrot && cargo test'
# → test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step lint \
    -- bash -c 'cd ports/solcarrot && cargo fmt --check &&
        cargo clippy --all-targets --offline -- -D warnings'
# → FMT_EXIT:0, Finished `dev` profile (0 warnings, 0 errors), CLIPPY_EXIT:0

ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step build \
    -- bash -c 'cd ports/solcarrot && rm -f Cargo.lock && cargo horizon-mod build'
# → SDK 0b14f8a (build 0b14f8acfd97fa2c750e42795249dcc981178ca4) in ./.horizon/sdk
# → Built ./target/horizon-mod/solcarrot.hmod

ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step check \
    -- bash -c 'cd ports/solcarrot && cargo horizon-mod check solcarrot.hmod &&
        cargo horizon-mod fits solcarrot.hmod --build nightly'
# → solcarrot 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)
# →   32704 bytes, SHA-256 e73918dc…8754, unsigned          CHECK_EXIT:0
# → Spice of Life: Carrot Edition: / 0b14f8a  made for        FITS_EXIT:0
```

`BUILD_LOCAL=1` on every step: the automatic forward to the second machine failed with
`bash: line 1: cd: ports/solcarrot: No such file or directory` (that box has no copy of the
port), so each step ran locally in a lease (`CARGO_INCREMENTAL=0`, released by the runner).

### Traps this migration cost (the next port must know)

1. **`WorldPhase::WorldOpenReady` is gone.** `world_lifecycle` now carries only the three
   close/failure phases; the open-ready moment is the **`server_started` hook**. The SDK's own
   `hooks/notes.rs` says so explicitly (`WorldOpenReady` → `server_started`, same moment).
2. **Two hooks hand `HostSide`, not a server host.** `living_entity_use_item_finish` and
   `player_tick_post` fire on **both** logical sides, so their `host` is
   `HostSide<'_>` (`Server(&mut dyn ServerHost) | Client(&mut dyn ClientHost)`, exhaustive).
   `let HostSide::Server(host) = host else { return }` is the only correct gate before a write.
3. **`server_about_to_start`/`world_lifecycle`/`server_stopping` hand `&mut dyn ServerHost`
   directly** — do not add a `HostSide` gate there; it does not compile.
4. **`item_key_id` is not a `ServerHost` method.** `ItemKey` is an opaque u32 slot; the id comes
   from the handle method `stack.item().id(&*host)` (`ItemKey::id(host)`, `&dyn Host`).
   `host.item_key(&Identifier)` is the other direction.
5. **There is no nutrition/food accessor.** `host.item_default_component(key, "minecraft:food")`
   returns the component as a `Tag`; read `nutrition` yourself (`Tag::Int`, and `Long`/`Double`
   because the manifest JSON may give a number in any of the three; `Tag::Double` holds
   `f64` **bits**).
6. **`CommandInvocation` is typed, not a line.** `event.command.literals` is a `Vec<String>` of
   the path and `argument(name)` returns a `CommandArgumentValue`; there is no raw command text
   in `run_mod_command` (that is `player_command`, a different, cancel-only hook).
7. **`CommandSource` is not `Copy`** and is `#[non_exhaustive]`: match `event.source` with a
   `_ => …` arm and copy the `PlayerRef` out.
8. **`host.commands()` returns `&mut dyn Commands`** (the route that honours silence and
   track-output), while `host.messages()` is read-only — `feedback` lives on `Commands`.
9. **`AttributeModifier` is a struct, not five arguments.** `AttributeModifier::new(id, amount,
   AttributeOperation::AddValue)` plus `ModifierLifetime::{Transient, Permanent}`; the old
   `set_modifier(...)` call has no equivalent. Read the current amount back through
   `living.attribute_modifiers(host, "minecraft:max_health")` before writing, or the modifier is
   rewritten (and health rescaled) every tick.
10. **`Hooks::command` appears in `uses.json`** even though this mod implements
    `run_mod_command`; the scanner maps `event.command`, so do not read that entry as a hook the
    port declared.
11. **`capabilities` is a required manifest field** (`missing field capabilities`) even though
    this mod registers nothing: the check report's own `capabilities` is `[]`. It is a
    permission, not a claim.
12. **Do not declare `rust-version`** (the box's rustc 1.97.0 must build the crate; the
    scaffold declares none) and **do not keep a `Cargo.lock`**: `cargo test` writes one, and a
    packed lock changes the artifact (imfast: 7,570 B → 23,133 B). `rm -f Cargo.lock` right
    before `cargo horizon-mod build`, and `/Cargo.lock` in `.gitignore`.
13. **`cargo fmt --all --check` reports the SDK's own files** (`.horizon/sdk` is a path
    dependency). Use `cargo fmt --check`; never "fix" the fetched SDK.
14. **`build-slot.sh` does not `cd` into the port** and its automatic forward fails while the
    second machine has no copy: `--port` is a label, so wrap every command as
    `bash -c 'cd ports/<id> && …'` and pass `BUILD_LOCAL=1`.
15. **The packer includes root files by name**: `LICENSE`, `LICENSE-NOTICE.md` and `README.md`
    travel inside the `.hmod` (10 entries here); `notes.md`/`FINDINGS.md`/`VALIDATION.md` do not.
    That is what makes an LGPL package licence-compliant (source + notice + licence in the
    archive).
16. **Two paths moved with the SDK, and the e2e's assertions depend on one of them**: the
    history is now `<save>/horizon-mod-data/solcarrot/foods.tsv`
    (`ConfigServerHost::world_data_files()`, written atomically and carried with the save) and
    the settings `config/solcarrot.conf` (`ConfigHost::config_files()`), not
    `<save>/solcarrot/foods.tsv` and `XDG_CONFIG_HOME/solcarrot.conf`. `tools/e2e.sh` takes
    `CONFIG_DIR`/`STORE_DIR` overrides for the first launcher run.
17. **No log service in the SDK.** The retired `support.rs` logged the config it read and the
    file it wrote (`log::info!`); the port now says nothing. The state is observable through
    `/foodlist size` and the file itself, which is what the e2e asserts.

### Verification (SDK `0b14f8a`)

Skills: superpowers/using-superpowers,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

| Check | Command | Result |
|---|---|---|
| Runtime tests before glue | `cargo test` with only `pub mod runtime;` | **17 passed**, 0 failed |
| Runtime + glue tests | `cargo test` | **20 passed** (17 runtime + 3 glue), 0 failed (`evidence/solcarrot/sdk-0b14f8a/unit.log`) |
| Formatting | `cargo fmt --check` | clean, `FMT_EXIT:0` (`fmt.log`) |
| Lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings (`clippy.log`) |
| Package | `cargo horizon-mod build` | `BUILD_EXIT:0`, **32,704 B**, 10 entries (`build.log`, `package-entries.log`) |
| Registry check | `cargo horizon-mod check solcarrot.hmod` | `solcarrot 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)`, sha256 `e73918dc…8754`, **CHECK_EXIT:0** (`check.log`, `check-json.log`) |
| Fits the pinned build | `cargo horizon-mod fits solcarrot.hmod --build nightly` | `0b14f8a  made for`, **FITS_EXIT:0** (`fits.log`) |
| `runtime.rs` untouched | `unzip -p solcarrot.hmod src/runtime.rs \| sha256sum` + `cmp` | `526cbcc2…8216e` = the retired file, **byte for byte** (`runtime-carry.log`) |
| Reproducible | second `cargo horizon-mod build` | same 32,704 B, same sha256 |
| Symbols line | `python3 ports/tools/check-skills.py --notes ports/solcarrot` | see the job report |

The package's own `uses.json` (210 entries) names the nine hooks the port implements —
`server_about_to_start`, `server_started`, `world_lifecycle`, `server_stopping`,
`living_entity_use_item_finish`, `player_tick_post`, `player_died`, `register_commands`,
`run_mod_command` — plus `ConfigHost::config_files`, `ConfigServerHost::world_data_files`,
`ItemsHost::item_default_component`, `LivingRef::replace_attribute_modifier`,
`Commands::feedback` and `AttributeModifier`, so the checker read the glue as intended.

### Gaps after the migration

1. **No in-game run at `0b14f8a`.** `horizon-launcher` is not installed here and no game build
   exists at this pin (the shared binaries are at `1d63e0a5`/`41f822b6`), so the behaviour
   claims in this section are the *mapping* (each retired hook to a typed method at the same
   call site), the type-checked glue and the carried rules — not a gameplay run. The last
   in-game evidence is the retired bundle's (`evidence/solcarrot/e2e.log`, `e2e-control.log`,
   `EXIT:0` both, `check-e2e-logs.py --strict` green): the scenario, the foods and the
   assertion are unchanged, but the transport is not. The next slice is exactly that: build
   the game with the launcher, run `tools/e2e.sh` with `GAME=` pointed at it, record the
   transcript, then claim `ported`.
2. **The stderr log line is gone** (no log service in the SDK; trap 17). The e2e never
   asserted it, so nothing is lost there.
3. **The config is read once**, at `server_about_to_start`, as upstream's server config was;
   a later edit needs a restart. No reload hook is used (`add_server_reload_listeners` exists
   and could pick it up — a small next-slice improvement, not a gap against upstream).
4. **No nutrition accessor in the SDK** (trap 5): the port reads the `minecraft:food`
   component per stored food id. Costly for a long history on a 25-milestone player, but
   correct; a `item_food(id) -> Option<Food>` host method is the issue-shaped ask.
5. **`/foodlist <sub> <target>` now takes a vanilla `player` argument**, so an unknown name is
   refused by the dispatcher with vanilla's own message instead of the port's
   `No player named …`, and an offline player still cannot be targeted (the retired
   `native_server_players()` resolution could not either).
6. Still deferred from slice 1 (unchanged): the food book item/recipe/GUI, tooltips, client
   sync packets, milestone sounds and particles, exact self-clear permission parity, and the
   attachment-vs-file trade-off (the file keeps `/foodlist size|clear <offline>` readable;
   `PlayerRef::persistent_data`/attachments are online-only).

### Changelog for the resubmission

> Supersedes #809, closed because Horizon retired `.hnmod` (#17992). Changes: flat SDK package
> with `api = "0.2"` and `[native]` pinned to `0b14f8a` / `e59da90`; the eight retired
> injections are now `impl mod_api::Hooks` methods against typed hosts and events (the
> world-open phase moved to `server_started`, the two both-sides hooks gate on `HostSide`), the
> command line's raw text is replaced by the dispatcher's typed `literals`/`arguments`, the max
> health write goes through `LivingRef::replace_attribute_modifier` instead of a pin-internal
> attribute map, and the history/config files go through the host's own save/config services
> (`<save>/horizon-mod-data/solcarrot/foods.tsv`, `config/solcarrot.conf`). `runtime.rs` and
> its 17 tests are unchanged, byte for byte; 3 glue tests are added. Version 0.2.0.
