#!/usr/bin/env python3
"""HZ-058: forbidden-claim and anti-cheat policy freeze gates (app)."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "docs" / "HZ058_POLICY.md"
CI = ROOT / ".github" / "workflows" / "ci.yml"
TEST = ROOT / "crates" / "hertzline-app" / "tests" / "hz058_policy.rs"
UI = ROOT / "ui" / "app.js"

POLICY_MARKERS = [
    "Forbidden claims",
    "Anti-cheat (frozen)",
    "injects_into_game",
    "No process injection",
    "+N FPS",
    "BCDEdit",
    "EasyAntiCheat",
    "AC-6.2",
    "AC-6.3",
]

# Numeric marketing FPS claims (letter X form is the forbid phrase, not a claim).
BANNED_CLAIM = [
    re.compile(r"\+\s*\d+(\.\d+)?\s*%?\s*FPS", re.I),
    re.compile(r"\d+(\.\d+)?\s*%?\s*more\s*FPS", re.I),
    re.compile(r"guaranteed\s+.{0,24}FPS", re.I),
    re.compile(r"instant\s+.{0,24}(FPS|boost)", re.I),
    re.compile(r"average\s+FPS\s+gains?", re.I),
]

# Product surface that must not appear as action identifiers or enable paths.
BANNED_SURFACE = re.compile(
    r"(?i)\b("
    r"bcdedit|disable_defender|timer_resolution|standby_list|ram_cleaner|"
    r"tcp_optimizer|fps_boost_pack|writeprocessmemory|createremotethread|"
    r"readprocessmemory"
    r")\b"
)

SCAN_GLOBS = [
    "ui/**/*.js",
    "ui/**/*.html",
    "ui/**/*.css",
    "crates/hertzline-app/src/**/*.rs",
    "src-tauri/**/*.rs",
    "README.md",
]


def _fail(msg: str) -> int:
    print(f"FAIL {msg}")
    return 1


def _iter_scan_files() -> list[Path]:
    files: list[Path] = []
    for pattern in SCAN_GLOBS:
        files.extend(ROOT.glob(pattern))
    out: list[Path] = []
    for path in files:
        if not path.is_file():
            continue
        if "target" in path.parts:
            continue
        out.append(path)
    return sorted(set(out))


def _line_is_comment_or_doc(path: Path, line: str) -> bool:
    s = line.lstrip()
    if path.suffix == ".rs":
        return s.startswith("//") or s.startswith("///") or s.startswith("//!")
    if path.suffix == ".js":
        return s.startswith("//")
    return False


def main() -> int:
    for path in (POLICY, CI, TEST, UI):
        if not path.is_file():
            return _fail(f"missing {path.relative_to(ROOT)}")

    policy = POLICY.read_text(encoding="utf-8")
    missing = [m for m in POLICY_MARKERS if m not in policy]
    if missing:
        return _fail(f"HZ058_POLICY.md missing markers: {', '.join(missing)}")

    ci = CI.read_text(encoding="utf-8")
    for marker in ("check_hz058.py", "hz058_policy"):
        if marker not in ci:
            return _fail(f"ci.yml missing {marker}")

    test = TEST.read_text(encoding="utf-8")
    for marker in (
        "injects_into_game",
        "fps_claim_allowed",
        "forbidden_action",
        "EasyAntiCheat",
        "no measurable effect",
    ):
        if marker not in test:
            return _fail(f"hz058_policy.rs missing {marker}")

    hits: list[str] = []
    for path in _iter_scan_files():
        text = path.read_text(encoding="utf-8")
        for i, line in enumerate(text.splitlines(), 1):
            if _line_is_comment_or_doc(path, line):
                # Still ban numeric +N FPS even in comments (easy to copy into UI).
                for rx in BANNED_CLAIM:
                    if rx.search(line):
                        hits.append(f"{path.relative_to(ROOT)}:{i}:{line.strip()}")
                        break
                continue
            for rx in BANNED_CLAIM:
                if rx.search(line):
                    hits.append(f"{path.relative_to(ROOT)}:{i}:{line.strip()}")
                    break
            if BANNED_SURFACE.search(line) and path.suffix in {".js", ".rs", ".html"}:
                # Allow deny-lists / policy strings that mention the token as forbidden.
                low = line.lower()
                if any(
                    k in low
                    for k in (
                        "forbid",
                        "banned",
                        "never",
                        "must not",
                        "not ship",
                        "out of",
                        "reject",
                        "denied",
                        "hz058",
                        "policy",
                    )
                ):
                    continue
                hits.append(f"{path.relative_to(ROOT)}:{i}:{line.strip()}")

    if hits:
        print("FAIL banned claim/surface hits:")
        for h in hits[:40]:
            print(" ", h)
        return 1

    ui = UI.read_text(encoding="utf-8")
    for marker in (
        "Never injects into the game",
        "anti-cheat",
        "No marketing +X FPS claim",
        "Offer revert",
        "noise band",
    ):
        if marker not in ui:
            return _fail(f"ui/app.js missing anti-cheat/claim marker: {marker}")

    print("check_hz058: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
