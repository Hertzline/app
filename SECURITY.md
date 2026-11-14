# Security Policy

Report vulnerabilities privately as described in the organization
[SECURITY.md](https://github.com/Hertzline/.github/blob/main/SECURITY.md).

Do not open public issues for undisclosed vulnerabilities.

## App surface

This repository owns the unprivileged desktop host. It must never run fully
elevated. Privileged work goes through the `helper` allow-list broker per action.

Profile crash recovery (HZ-028) uses the same headless restore path as HZ-018.
