# Security Policy

Report vulnerabilities privately as described in the organization
[SECURITY.md](https://github.com/Hertzline/.github/blob/main/SECURITY.md).

Do not open public issues for undisclosed vulnerabilities.

## App surface

This repository owns the unprivileged desktop host. It must never run fully
elevated. Privileged work goes through the `helper` allow-list broker per action.

Profile crash recovery (HZ-028) uses the same headless restore path as HZ-018.

HZ-055 threat model and helper audit live in the sibling
[`helper` docs](https://github.com/Hertzline/helper/blob/main/docs/THREAT_MODEL.md).

Rules:

- `ShellInfo.unprivileged` is always true.
- Elevation prompts are per action, never whole-app.
- Overlay is off by default when anti-cheat install markers are found (AC-6.2).
- No product telemetry by default; UI does not call remote analytics endpoints.
