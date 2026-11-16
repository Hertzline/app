# Changelog

All notable changes to the Hertzline desktop app are recorded here.
The website `/changelog/` mirrors release highlights from `releases.json`
after a signed public release is published.

## [1.0.3] - 2026-11-16

### Fixed

- Home loads a lite overview (startup + thermal + power) so score/stats appear without waiting on hog/cleanup scans

## [1.0.2] - 2026-11-16

### Fixed

- Home no longer freezes the UI while everyday overview samples CPU hogs (timeout + skeleton + CTAs first)

### Changed

- Dark precision-instrument visual system with bundled Instrument Sans + IBM Plex Mono
- Rail navigation layout and sharper product chrome

## [1.0.1] - 2026-11-16

### Changed

- Italian product UI with Home dashboard: system health score %, big stats tiles, and a clear Measure → Optimize → Verify flow
- Actions ranked by priority; benchmark verdict shows median FPS gain as a percentage from measured before/after only
- Advanced PresentMon/MangoHud and raw tables moved behind details

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
