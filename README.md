# solcarrot (Horizon port)

A Horizon port of [solcarrot](Cazsius/Spice-of-Life-Carrot-Edition) — LGPL-2.1-only.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- History**: one per player, a set of food **item ids**. A food is recorded
- when it finishes being eaten; re-eating it adds nothing. Filtered foods
- (below the nutrition floor, blacklisted, or outside a non-empty whitelist)
- are still remembered but do not count, exactly as upstream's
- `FoodList.addFood` does.
- Milestones**: `milestones` (default `5,10,15,20,25`) count unique counted
- foods. Each reached milestone grants `2 * hearts_per_milestone` health points
- on top of vanilla's 20; `base_hearts` other than 10 adds

## Not yet ported

- nothing recorded yet

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: Cazsius/Spice-of-Life-Carrot-Edition. Port: AdLf0208. Licence: LGPL-2.1-only — see [LICENSE](LICENSE);
the original notices are preserved.
