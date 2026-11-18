# Spice of Life: Carrot Edition — a Horizon port

**Spice of Life: Carrot Edition** is a Minecraft mod. Motivating people to explore new foods with the carrot, not the stick. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Cazsius/Spice-of-Life-Carrot-Edition](https://github.com/Cazsius/Spice-of-Life-Carrot-Edition) — licence **LGPL-2.1-only**.

| | |
|---|---|
| Upstream mod | Spice of Life: Carrot Edition ([source](https://github.com/Cazsius/Spice-of-Life-Carrot-Edition)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `solcarrot.hmod` (install it with the Horizon launcher) |
| Licence | **LGPL-2.1-only** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- **History**: one per player, a set of food **item ids**. A food is recorded
- **Milestones**: `milestones` (default `5,10,15,20,25`) count unique counted
- **Health update**: the modifier id is `solcarrot:health_gained` and current
- **Feed back**: the first food of a run and every new milestone stay silent in
- **Reset**: `/foodlist clear [target]` (level 2) clears the history and drops
- **Status**: `/foodlist size [target]` reports the counted total and the next
- **Creative**: `limit_progression_to_survival = true` skips tracking when the

## Not yet ported

- **No in-game run at `0b14f8a`.** `horizon-launcher` is not installed here and no game build
- **The stderr log line is gone** (no log service in the SDK; trap 17). The e2e never
- **The config is read once**, at `server_about_to_start`, as upstream's server config was;
- **No nutrition accessor in the SDK** (trap 5): the port reads the `minecraft:food`
- **`/foodlist <sub> <target>` now takes a vanilla `player` argument**, so an unknown name is
- Still deferred from slice 1 (unchanged): the food book item/recipe/GUI, tooltips, client

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `solcarrot.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Spice of Life: Carrot Edition was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **LGPL-2.1-only**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Cazsius/Spice-of-Life-Carrot-Edition

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run solcarrot` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/solcarrot/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
