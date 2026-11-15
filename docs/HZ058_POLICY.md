# Hertzline HZ-058 -- Forbidden-claim and anti-cheat policy freeze

Frozen 2026-11-15. Changing this policy needs a new roadmap task and owner review.
Source of truth: workspace `RULES.md` / `PROJECT_SPEC.md` Strictly forbidden.

## Forbidden claims

Product UI, docs shipped with the app, and user-visible strings must not assert:

- Any `+N FPS` or `+N% FPS` marketing number without a completed local before/after on this machine
- Cross-machine "average FPS gains"
- Guaranteed or instant FPS / boost language

Local measured gains from a completed before/after pair may be shown with the measured numbers and noise band (AC-2.3, AC-2.4, AC-8.2). That is not a marketing claim.

## Forbidden product surface

Not shipped as Hertzline actions (v1):

- Registry "FPS boost" packs
- Disabling Defender, updates, mitigations, or core services
- Unmeasured TCP "optimizers"
- Universal timer-resolution hacks
- BCDEdit changes
- Scheduled standby-list "RAM cleaners"
- Overclock, undervolt, fan or power-limit changes (link vendor tools)
- Injection, game memory reading, game file modification
- Telemetry by default, ads, bundled software, mining, data selling
- Running the whole app elevated

Exception path (RULES.md): an otherwise forbidden tweak may ship only with a cited reproducible measurement, marked advanced, off by default, and reversible. None of those exceptions are in v1.

## Anti-cheat (frozen)

1. No process injection into games
2. No game memory reading
3. No unauthorized game file modification
4. Frame capture uses external documented methods only (PresentMon ETW / MangoHud opt-in / macOS partial)
5. Overlay off by default when filesystem anti-cheat markers are found (AC-6.2)
6. One global overlay disable control (AC-6.3)
7. `injects_into_game` stays false

Named markers scanned (filesystem paths only): EasyAntiCheat, BattlEye, Vanguard, Ricochet and equivalents already coded in core/app.

## Enforcement

- `scripts/check_hz058.py` greps UI and Rust string surfaces
- `tests/hz058_policy.rs` asserts overlay, catalog, and claim flags
- CI runs both on the app matrix
