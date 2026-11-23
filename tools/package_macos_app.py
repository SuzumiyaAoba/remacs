#!/usr/bin/env python3
"""Assemble the GUI executable and its Services declaration into a macOS app."""

from __future__ import annotations

import argparse
import plistlib
import shutil
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", default="fast-dev")
    args = parser.parse_args()

    if sys.platform != "darwin":
        parser.error("Remacs.app can only be assembled on macOS")

    root = Path(__file__).resolve().parents[1]
    profile_dir = root / "target" / args.profile
    executable = profile_dir / "remacs"
    info_plist = root / "packaging" / "macos" / "Info.plist"
    if not executable.is_file():
        parser.error(f"GUI executable not found: {executable}")

    with info_plist.open("rb") as source:
        metadata = plistlib.load(source)
    if metadata.get("CFBundleExecutable") != executable.name:
        parser.error("CFBundleExecutable does not match the built Remacs binary")

    bundle_contents = profile_dir / "Remacs.app" / "Contents"
    macos_dir = bundle_contents / "MacOS"
    macos_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, macos_dir / executable.name)
    shutil.copy2(info_plist, bundle_contents / "Info.plist")
    print(f"Created {bundle_contents.parent}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
