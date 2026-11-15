#!/usr/bin/env python3
"""HZ-059 local check: app v1.0.0 release candidate metadata."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = "1.0.0"
CI = ROOT / ".github" / "workflows" / "ci.yml"

REQUIRED = [
    ROOT / "CHANGELOG.md",
    ROOT / "docs" / "HZ059_CHECKLIST.md",
    ROOT / "src-tauri" / "tauri.conf.json",
    ROOT / "src-tauri" / "Cargo.toml",
    ROOT / "crates" / "hertzline-app" / "Cargo.toml",
    ROOT / "package.json",
]


def fail(msg: str) -> int:
    print(f"FAIL {msg}")
    return 1


def main() -> int:
    missing = [str(p.relative_to(ROOT)) for p in REQUIRED if not p.is_file()]
    if missing:
        return fail("missing: " + ", ".join(missing))

    conf = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    if conf.get("version") != VERSION:
        return fail(f"tauri.conf.json version must be {VERSION}")

    pkg = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    if pkg.get("version") != VERSION:
        return fail(f"package.json version must be {VERSION}")

    for rel in ("src-tauri/Cargo.toml", "crates/hertzline-app/Cargo.toml"):
        text = (ROOT / rel).read_text(encoding="utf-8")
        if not re.search(rf'(?m)^version = "{re.escape(VERSION)}"', text):
            return fail(f"{rel} version must be {VERSION}")

    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    if f"[{VERSION}]" not in changelog:
        return fail("CHANGELOG.md missing [1.0.0] section")
    if re.search(r"\+\s*\d+(\.\d+)?\s*%?\s*FPS", changelog, re.I):
        return fail("CHANGELOG must not invent +N FPS claims")

    checklist = (ROOT / "docs" / "HZ059_CHECKLIST.md").read_text(encoding="utf-8")
    for marker in ("1.0.0", "HZ-054", "Vercel", "sign", "not_released"):
        if marker.lower() not in checklist.lower():
            return fail(f"HZ059_CHECKLIST.md missing marker: {marker}")

    if not CI.is_file():
        return fail("missing .github/workflows/ci.yml")
    ci = CI.read_text(encoding="utf-8")
    if "check_hz059.py" not in ci:
        return fail("ci.yml must run check_hz059.py")

    print(f"PASS check_hz059 version={VERSION}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
