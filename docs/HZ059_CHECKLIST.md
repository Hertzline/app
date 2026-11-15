# HZ-059 -- app release candidate checklist

Version: **1.0.0**

## Required in this repo

- [x] `src-tauri/tauri.conf.json` version `1.0.0`
- [x] `src-tauri/Cargo.toml` and `crates/hertzline-app/Cargo.toml` version `1.0.0`
- [x] `package.json` version `1.0.0`
- [x] `CHANGELOG.md` section `[1.0.0]`
- [x] Forbidden-claim / anti-cheat freeze still green (`scripts/check_hz058.py`)

## With packaging (sibling)

- Windows MSI/NSIS via `../packaging/scripts/build_windows.py`
- Checksums + public signing fail-closed without owner certs
- winget draft `Hertzline.Hertzline` `1.0.0` validated locally; no PR without approval

## Owner after CI green

1. Sign / notarize / GPG-sign platform artifacts
2. Tag `v1.0.0` and publish GitHub Release assets
3. Flip website `releases.json` from `not_released` to `released` via packaging `write_release_metadata.py`
4. Deploy website on Vercel (owner only)
5. Approve HZ-054 before any winget-pkgs PR

## Gate

```bash
python scripts/check_hz059.py
```
