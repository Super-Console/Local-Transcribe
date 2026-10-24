#!/usr/bin/env python3
"""Generate THIRD-PARTY-NOTICES.md from cargo metadata and package.json.

No network and no extra tools: `cargo metadata --format-version 1 --all-features`
is parsed here, plus the frontend runtime dependencies in
apps/desktop/package.json. Dev-only Rust edges are skipped so the file lists what
can actually ship in the binary. Re-run from the repository root:

    python3 scripts/generate-third-party-notices.py
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "THIRD-PARTY-NOTICES.md"
FRONTEND = ROOT / "apps" / "desktop"


def cargo_packages() -> list[dict]:
    raw = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--all-features"],
        cwd=ROOT,
    )
    meta = json.loads(raw)
    packages = {package["id"]: package for package in meta["packages"]}
    nodes = {node["id"]: node for node in meta["resolve"]["nodes"]}

    reachable: set[str] = set()
    stack = list(meta["workspace_members"])
    while stack:
        package_id = stack.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        node = nodes.get(package_id)
        if not node:
            continue
        for dependency in node.get("deps", []):
            kinds = {kind.get("kind") for kind in dependency.get("dep_kinds", [])}
            if kinds and kinds <= {"dev"}:
                continue
            stack.append(dependency["pkg"])

    result = []
    for package_id in sorted(reachable):
        package = packages[package_id]
        if package.get("source") is None:
            continue  # workspace members are covered by LICENSE
        result.append(
            {
                "name": package["name"],
                "version": package["version"],
                "license": package.get("license") or "未标注",
                "repository": (package.get("repository") or "").strip(),
            }
        )
    return result


def frontend_packages() -> list[dict]:
    manifest = json.loads((FRONTEND / "package.json").read_text())
    result = []
    for name in sorted(manifest.get("dependencies", {})):
        folder = FRONTEND / "node_modules" / name
        package_json = folder / "package.json"
        if not package_json.is_file():
            result.append(
                {"name": name, "version": "", "license": "未标注", "repository": ""}
            )
            continue
        entry = json.loads(package_json.read_text())
        repository = entry.get("repository")
        if isinstance(repository, dict):
            repository = repository.get("url", "")
        result.append(
            {
                "name": name,
                "version": entry.get("version", ""),
                "license": entry.get("license") or "未标注",
                "repository": (repository or "").strip(),
            }
        )
    return result


def group(license_expression: str) -> str:
    expression = license_expression.upper()
    if "MPL-2.0" in expression:
        return "MPL-2.0"
    if "BSD" in expression:
        return "BSD"
    if "APACHE" in expression and "MIT" not in expression:
        return "Apache-2.0"
    if "MIT" in expression and "APACHE" not in expression:
        return "MIT"
    if expression.strip() == "MIT":
        return "MIT"
    return "双许可与其他"


def line(package: dict) -> str:
    version = f" {package['version']}" if package["version"] else ""
    repository = package["repository"] or "仓库地址未标注"
    return f"- {package['name']}{version} — {repository} ({package['license']})"


def main() -> None:
    rust = cargo_packages()
    front = frontend_packages()
    everything = rust + front

    groups: dict[str, list[dict]] = {}
    for package in everything:
        groups.setdefault(group(package["license"]), []).append(package)
    for packages in groups.values():
        packages.sort(key=lambda item: (item["name"].lower(), item["version"]))

    order = ["MIT", "Apache-2.0", "MPL-2.0", "BSD", "双许可与其他"]
    lines = [
        "# Third-party notices",
        "",
        "Local Transcribe is MIT-licensed; the components below keep their own licenses.",
        "Generated from `cargo metadata --format-version 1 --all-features` (normal and",
        "build dependency closure, dev-only edges excluded) plus the desktop app's",
        "runtime `dependencies`. Re-run `python3 scripts/generate-third-party-notices.py`",
        "after changing dependencies.",
        "",
        "## Components called out explicitly",
        "",
        "- **sherpa-onnx** — Apache-2.0. This project statically links its runtime",
        "  library through `sherpa-onnx-sys`; the license text and upstream notices",
        "  travel with that crate.",
        "- **symphonia** — MPL-2.0 (file-level copyleft). This project uses it",
        "  unmodified and does not include any modified Symphonia source file. If a",
        "  Symphonia file is ever modified, that file must be published under",
        "  MPL-2.0.",
        "",
        f"Total: {len(everything)} packages ({len(rust)} Rust, {len(front)} frontend).",
        "",
    ]

    titles = {
        "MIT": "MIT",
        "Apache-2.0": "Apache-2.0",
        "MPL-2.0": "MPL-2.0",
        "BSD": "BSD",
        "双许可与其他": "Dual licenses and other terms",
    }
    for key in order:
        packages = groups.get(key)
        if not packages:
            continue
        lines.append(f"## {titles[key]}")
        lines.append("")
        lines.extend(line(package) for package in packages)
        lines.append("")

    OUTPUT.write_text("\n".join(lines).rstrip() + "\n")
    print(f"wrote {OUTPUT} ({len(everything)} packages)")


if __name__ == "__main__":
    main()
