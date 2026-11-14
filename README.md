# Hertzline / app

Desktop host for [Hertzline](https://github.com/Hertzline).

Unprivileged by default. Profile apply-on-start / restore-on-exit / crash recovery live here and in `hertzline-core` (HZ-028). The Tauri UI shell is HZ-029.

## Layout

| Path | Role |
|---|---|
| `crates/hertzline-app` | Profile lifecycle host (`ProfileHost`) |

## Build

```bash
cargo test --locked
cargo build --release --locked
```

**Build note:** this crate path-depends on sibling `../core` (`Hertzline/core`). CI checks out that repo next to `app`.

## Profile lifecycle (HZ-028)

1. `ProfileHost::apply_on_start` applies the game profile and writes an active-profile marker.
2. `restore_on_exit` / `poll_game_exit` restores every applied setting on normal exit (AC-3.3).
3. `recover_from_prior_crash` uses HZ-018 `recover_from_crash` when a marker remains (AC-3.4).

## License

[MIT](LICENSE)

Third-party notices: [NOTICE](NOTICE)
