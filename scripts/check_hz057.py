#!/usr/bin/env python3
"""HZ-057 local check: app integration suite gates."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEST = ROOT / "crates" / "hertzline-app" / "tests" / "hz057_integration.rs"
CHECKLIST = ROOT / "docs" / "HZ057_CHECKLIST.md"
CI = ROOT / ".github" / "workflows" / "ci.yml"
LIB = ROOT / "crates" / "hertzline-app" / "src" / "lib.rs"

REQUIRED = [
    "ac_shell_unprivileged_routes",
    "ac1_ac8_measure_export",
    "ac2_benchmark_labeled_mock_verdict",
    "ac4_ac8_catalog_explanations",
    "ac5_ac6_ac7_serial_env_smoke",
    "required_route_ids",
    "restore_all_explicit",
    "EasyAntiCheat",
]


def main() -> int:
    for path in (TEST, CHECKLIST, CI, LIB):
        if not path.is_file():
            print("FAIL missing", path.relative_to(ROOT))
            return 1

    text = TEST.read_text(encoding="utf-8")
    missing = [m for m in REQUIRED if m not in text]
    if missing:
        print("FAIL hz057_integration.rs missing:", ", ".join(missing))
        return 1

    lib = LIB.read_text(encoding="utf-8")
    if "required_route_ids" not in lib:
        print("FAIL lib.rs must export required_route_ids")
        return 1

    ci = CI.read_text(encoding="utf-8")
    for marker in ("check_hz057.py", "hz057_integration"):
        if marker not in ci:
            print("FAIL ci.yml missing", marker)
            return 1

    print("check_hz057: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
