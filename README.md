# Spice of Life: Carrot Edition — Horizon port

A clean-room Rust port of **Spice of Life: Carrot Edition 1.21.1-1.16.6**
(`Cazsius/Spice-of-Life-Carrot-Edition`, LGPL-2.1) to Horizon / Minecraft Java 26.3.

Every player keeps the set of foods they have eaten. Each configured milestone of unique
foods raises their maximum health by `2 * hearts_per_milestone` — with the defaults
(5/10/15/20/25 foods, 2 hearts each) the fifth food takes a player from 20 to 24 health and
the twenty-fifth to 40. A re-eaten food, a filtered food (below `minimum_food_value`,
blacklisted, or outside a non-empty whitelist) and a consumable that is not food do not
count. `/foodlist size|sync|clear [target]` stands in for the food book item and its screen,
which this slice does not carry.

## The package (flat SDK layout, mod API 0.2)

```text
solcarrot.hmod          the delivered package (built by `cargo horizon-mod build`)
horizon-mod.toml        the manifest and the only identity: id, version, api, [native] pins
Cargo.toml              one crate; its only dependency is the pinned SDK's API crate
src/lib.rs              the one `impl mod_api::Hooks` — the nine moments, the health write,
                        the three /foodlist usages
src/config.rs           config/solcarrot.conf through the host's own config service
src/store.rs            the history file in the save's mod data folder
src/runtime.rs          the pure rules (food set, filters, milestones, health maths, the
                        config and history formats) with their unit tests
LICENSE                 LGPL-2.1, the upstream licence
LICENSE-NOTICE.md       licence provenance
tools/e2e.sh            the in-game scenario (needs a launcher-built game; see its header)
notes.md                scope, hook table, deviations, gaps, verification
FINDINGS.md             the upstream behaviour and where it was read
```

There is no `compatibility.json`, `mixins/`, `horizon-mixin.toml`, plan/injection schema or
`rust-toolchain.toml`: Horizon's mod-API merge retired the `.hnmod` container, and none of
them is part of an SDK package.

## Building

```sh
ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step test \
    -- bash -c 'cd ports/solcarrot && cargo test'
ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step build \
    -- bash -c 'cd ports/solcarrot && rm -f Cargo.lock && cargo horizon-mod build'
ports/tools/build-slot.sh run --job sdk-solcarrot --port ports/solcarrot --step check \
    -- bash -c 'cd ports/solcarrot && cargo horizon-mod check solcarrot.hmod &&
        cargo horizon-mod fits solcarrot.hmod --build nightly'
```

`cargo horizon-mod` fetches the pinned SDK into `.horizon/sdk/` (gitignored, never packed).
`build` packs the `.hmod`; the game build is the launcher's
(`horizon-launcher native-build . --instance <instance> --trust-native-code`).

## Licence

LGPL-2.1, the same licence as the original: this port may be redistributed and modified under
it, with the complete corresponding source travelling along — that is what this directory is.
Publishing still needs the campaign maintainer's approval.
