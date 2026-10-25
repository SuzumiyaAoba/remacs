#!/usr/bin/env python3
"""Validate workspace boundaries without building or fetching dependencies."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "remacs": {"remacs-runtime", "remacs-cli", "remacs-gui"},
    "remacs-runtime": {"remacs-libraries"},
    "remacs-libraries": set(),
    "remacs-cli": {"remacs-runtime"},
    "remacs-gui": {"remacs-runtime", "remacs-cli"},
}


def main():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1"], cwd=ROOT
    ))
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    errors = []
    if set(packages) != set(ALLOWED):
        errors.append(f"unexpected workspace members: {set(packages) ^ set(ALLOWED)}")
    for name, package in packages.items():
        deps = [d for d in package["dependencies"] if d["kind"] != "dev"]
        internal = {d["name"] for d in deps if d["name"] in packages}
        if internal != ALLOWED.get(name):
            errors.append(f"{name}: workspace dependencies {internal}, expected {ALLOWED.get(name)}")
        for dep in deps:
            if dep["name"] in {"gpui", "smol"} and name != "remacs-gui":
                errors.append(f"{name}: GUI dependency {dep['name']} outside GUI")
            if dep["name"] == "crossterm" and (name != "remacs-cli" or not dep["optional"]):
                errors.append(f"{name}: terminal dependency must be optional and owned by CLI")
        if name == "remacs-libraries" and deps:
            errors.append("remacs-libraries must remain data-only with no dependencies")
        if name == "remacs-gui":
            cli = next(d for d in deps if d["name"] == "remacs-cli")
            if cli["uses_default_features"] or "terminal" in cli["features"]:
                errors.append("GUI must use CLI startup without the terminal feature")
        if name == "remacs":
            gui = next(d for d in deps if d["name"] == "remacs-gui")
            if not gui["optional"]:
                errors.append("integrated launcher must allow builds without GUI")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Workspace boundaries OK: runtime, libraries, CLI, GUI; no reverse dependencies")
    return 0


if __name__ == "__main__":
    sys.exit(main())
