# Hertzline / app

Desktop shell for [Hertzline](https://github.com/Hertzline).

Unprivileged by default. Elevation is requested per action later, never for the whole process.

## Layout

| Path | Role |
|---|---|
| `crates/hertzline-app` | Profile lifecycle host + shell route model |
| `src-tauri` | Tauri 2 binary (`hertzline`) |
| `ui` | Static shell UI (hash routes) |

## Shell routes (HZ-029)

`#/measure` · `#/benchmark` · `#/actions` · `#/profiles` · `#/changelog` · `#/restore-all`

## Versions (verified 2026-11-14)

| Package | Version | License | Source |
|---|---|---|---|
| tauri | 2.12.5 | Apache-2.0 OR MIT | crates.io `max_stable` |
| tauri-build | 2.7.4 | Apache-2.0 OR MIT | crates.io |
| @tauri-apps/cli | 2.12.4 | Apache-2.0 OR MIT | npm latest |

## Build

Requires sibling checkout of `Hertzline/core` at `../core`.

```bash
npm ci
cargo test --locked
npm run build
```

Headless smoke (no window):

```bash
# after release build
set HERTZLINE_APP_SMOKE=1
target/release/hertzline.exe
```

## Profile lifecycle (HZ-028)

1. `ProfileHost::apply_on_start` applies the game profile and writes an active-profile marker.
2. `restore_on_exit` / `poll_game_exit` restores every applied setting on normal exit (AC-3.3).
3. `recover_from_prior_crash` uses HZ-018 `recover_from_crash` when a marker remains (AC-3.4).

## License

[MIT](LICENSE)

Third-party notices: [NOTICE](NOTICE)
