# HZ-057 -- App integration checklist

Companion to `core/docs/HZ057_CHECKLIST.md`.

| OS | Check | Notes |
|---|---|---|
| Windows / Linux / macOS | `cargo test -p hertzline-app --test hz057_integration --locked` | shell routes, measure export, mock bench verdict, catalog, restore-all, everyday, overlay |
| All | `shell_info.unprivileged == true` | Never whole-app elevation |

Sign-off is recorded when CI `app-test` passes on windows/ubuntu/macos and
local `scripts/check_hz057.py` is green.
