# Licence provenance

- Mod: Spice of Life: Carrot Edition 1.21.1-1.16.6 by Cazsius —
  <https://github.com/Cazsius/Spice-of-Life-Carrot-Edition>
- Licence: **LGPL-2.1**, as the campaign's jar audit recorded for
  `solcarrot-1.21.1-1.16.6.jar` and as the upstream repository's `LICENSE`
  carries. `LICENSE` in this package is that text.
- Upstream behaviour was read at commit
  `412fa94a07d93f09378fc0e3393de38eb0099824` (branch `1.21`) for analysis only;
  this port is a clean-room Rust rewrite, not a translation. `FINDINGS.md`
  records what was read and where.
- LGPL-2.1 obligations: this package ships the **complete corresponding source**
  of the port (`src/lib.rs`, `src/config.rs`, `src/store.rs`, `src/runtime.rs`,
  `Cargo.toml`, `horizon-mod.toml`) inside the same `.hmod`, under the same
  licence, with this notice. Publishing still needs the campaign maintainer's
  approval (ARCHITECTURE.md §8).
