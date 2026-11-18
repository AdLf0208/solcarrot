# Validation

Validated on Linux with Rust 1.95.0 on 2026-11-03, using the exact revisions in
`compatibility.json` and the real bundle in this directory.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | Pass, 0 warnings |
| `cargo test --locked --offline` | 17 passed, 0 failed |
| `tools/horizon.py stage` | Staged at pin `c9d474dd…`, `stage_changed: true` |
| `tools/horizon.py check` | `cargo_exit: 0`, 0 warnings in `target/horizon-check.log` |
| `tools/horizon.py test` | `cargo_exit: 0`, 17 passed, 531 filtered out, 0 warnings |
| `tools/horizon.py build` | `cargo_exit: 0`, `target/horizon-cargo/fast/horizon-client` |
| `tools/e2e.sh` (default) | `PASS`, exit 0 |
| `MODE=control tools/e2e.sh` | `CONTROL PASS`, exit 0 |

The ladder used `HORIZON_MIXINS=<campaign>/horizon-mixins` and
`--horizon <campaign>/horizon` (the template defaults point inside `ports/`,
where those checkouts do not exist). `horizon-ctl` was built separately in the
staged tree:

```sh
cargo build --profile fast --locked --offline -p horizon-client \
  --bin horizon-ctl --no-default-features \
  --target-dir <port>/target/horizon-cargo
```

`tools/test_template.py` and `tools/test_stage.py` are the template's
single-injection self-tests; they assert one injection and a `break_block`
fixture and therefore do not describe this seven-injection bundle. They are not
part of the port ladder.

No game resource pack or player save is committed; every run used a scratch
`HORIZON_HOME`, `XDG_DATA_HOME` and `XDG_CONFIG_HOME`, and the worlds live under
`/tmp/opencode` and are removed by the e2e trap.
