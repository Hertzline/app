# Changelog

All notable changes to the Hertzline desktop app are recorded here.
The website `/changelog/` mirrors release highlights from `releases.json`
after a signed public release is published.

## [1.0.0] - 2026-11-15 (release candidate)

First v1 release candidate. Measurement, benchmark, reversible actions,
profiles, everyday mode, and optional overlay are feature-complete for
Windows, Linux, and macOS capability surfaces documented in the project.

### Added

- Measure sessions with CSV/JSON export (identical fields and units)
- Before/after benchmark with noise-band verdict and revert offer
- Per-OS optimization actions with apply / restore / verify
- Per-game profiles from documented launcher paths
- Everyday mode (startup review, cleanup preview, weekly health)
- Optional overlay (PresentMon / MangoHud / macOS partial; never injects)
- Crash and uninstall restore-all hooks

### Honest release notes

- Public download links stay empty until Authenticode / notarized /
  GPG-signed artifacts are published by the owner
- winget package `Hertzline.Hertzline` is prepared locally; no
  winget-pkgs PR without explicit owner approval (HZ-054)
- No "+X FPS" marketing claims without the user's own measurements
