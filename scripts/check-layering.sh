#!/usr/bin/env bash
set -euo pipefail

# Lightweight architectural checks that do not build or test the workspace.
# Cargo enforces direct dependencies; this script also catches accidental
# imports and reintroduced app-level channel polling.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$repo_root" <<'PY'
import json
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(sys.argv[1])
metadata = json.loads(
    subprocess.check_output(
        [
            "cargo",
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            str(root / "Cargo.toml"),
        ],
        text=True,
    )
)
packages = {package["name"]: package for package in metadata["packages"]}
errors = []


def normal_dependencies(package_name):
    package = packages[package_name]
    return {
        dependency["name"]
        for dependency in package["dependencies"]
        if dependency.get("kind") is None
    }


backend_dependencies = {"hdf5-metno", "hdf5-metno-sys", "ndarray", "netcdf", "netcdf-reader"}
for package_name in ("h5jive", "h5jive-data"):
    leaked = sorted(normal_dependencies(package_name) & backend_dependencies)
    if package_name == "h5jive" and leaked:
        errors.append(
            "h5jive has normal backend dependencies: " + ", ".join(leaked)
        )

ui_dependencies = {"dear-app", "dear-implot", "winit"}
leaked = sorted(normal_dependencies("h5jive-data") & ui_dependencies)
if leaked:
    errors.append("h5jive-data must not depend on UI crates: " + ", ".join(leaked))


def production_source(path):
    text = path.read_text(encoding="utf-8")
    marker = text.find("#[cfg(test)]")
    return text if marker < 0 else text[:marker]


for directory in (
    root / "crates/h5jive/src/app",
    root / "crates/h5jive/src/plugins",
    root / "crates/h5jive/src/core",
):
    for path in directory.rglob("*.rs"):
        source = production_source(path)
        for number, line in enumerate(source.splitlines(), start=1):
            if re.search(r"\b(?:hdf5_metno|hdf5_metno_sys|ndarray|netcdf)::", line) or re.match(
                r"^\s*(?:use|extern\s+crate)\s+(?:hdf5_metno|hdf5_metno_sys|ndarray|netcdf)\b",
                line,
            ):
                errors.append(
                    f"{path.relative_to(root)}:{number}: import backend API through h5jive-data"
                )
            if directory.name == "core" and (
                re.search(r"\b(?:dear_app|dear_implot|winit)::", line)
                or re.match(r"^\s*(?:use|extern\s+crate)\s+(?:dear_app|dear_implot|winit)\b", line)
            ):
                errors.append(
                    f"{path.relative_to(root)}:{number}: core must not use UI libraries"
                )
            if (
                directory.name in {"app", "plugins"}
                and path.name != "background_job.rs"
                and re.search(r"\bmpsc::|\.try_recv\s*\(", line)
            ):
                errors.append(
                    f"{path.relative_to(root)}:{number}: use BackgroundJob instead of raw channel polling"
                )

for path in (root / "crates/h5jive/src/core").rglob("*.rs"):
    source = production_source(path)
    for number, line in enumerate(source.splitlines(), start=1):
        if re.search(r"\bcrate::(?:app|plugins)\b", line):
            errors.append(
                f"{path.relative_to(root)}:{number}: core must not depend on app or plugins"
            )

if errors:
    print("Layering check failed:", file=sys.stderr)
    for error in errors:
        print(f"  - {error}", file=sys.stderr)
    sys.exit(1)

print("Layering check passed.")
PY
