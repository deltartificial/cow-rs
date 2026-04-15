#!/usr/bin/env python3
"""Validate the cow-rs workspace layer DAG.

Each crate's `Cargo.toml` declares its layer in
`[package.metadata.workspace.layer]`. This script walks every workspace
crate, parses its internal dependencies, and enforces:

1. **Acyclic layering** — a crate on layer N may only depend on crates on
   layers strictly less than N (Rule 1 of the architecture doc).
2. **No sibling dependencies** — two crates declared on the same layer
   may not depend on each other (Rule 2). This is stricter than Rule 1
   because it also forbids "N depends on N" edges.
3. **Infrastructure exception** — `cow-errors` is treated as workspace
   infrastructure: any layer may depend on it without violating Rule 2.
4. **Orthogonal crates** are ignored for DAG purposes (they are adapters,
   not ordered layers). They are still required to parse cleanly.

Usage:
    python3 scripts/check-workspace-layers.py

Exits 0 on success, 1 on any violation. No external dependencies — only
Python stdlib.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

ROOT = Path(__file__).resolve().parent.parent
CRATES_DIR = ROOT / "crates"
INFRA_CRATES = {"cow-errors"}  # universally dependable
# Legacy compat shim — excluded from layer enforcement because it intentionally
# bundles everything for backwards compatibility.
EXCLUDED_CRATES = {"cow-rs"}
# Known sibling-dep migration debt: (crate, dep) pairs that the strict DAG
# check would flag but are tolerated while the underlying refactor is in
# flight. Each entry **must** have a tracking reason. Shrink this set to
# zero before the next major release.
ALLOWED_SIBLING_EDGES: Set[Tuple[str, str]] = set()


def parse_cargo_toml(path: Path) -> Tuple[Optional[str], Optional[object], List[str]]:
    """Return (crate_name, layer, internal_cow_deps) for a crate's Cargo.toml.

    Layer is parsed as either an int (normal layers) or the literal string
    "orthogonal". Internal deps are the workspace-internal crate names
    referenced via `.workspace = true` or a `path =` line in the
    `[dependencies]` table.
    """
    text = path.read_text(encoding="utf-8")

    # --- Package name
    name_match = re.search(r'^\s*name\s*=\s*"([^"]+)"', text, re.MULTILINE)
    name = name_match.group(1) if name_match else None

    # --- Layer
    layer: Optional[object] = None
    layer_match = re.search(
        r"\[package\.metadata\.workspace\.layer\]\s*\n\s*layer\s*=\s*(.+)",
        text,
    )
    if layer_match:
        raw = layer_match.group(1).strip()
        if raw.startswith('"'):
            layer = raw.strip('"')
        else:
            try:
                layer = int(raw)
            except ValueError:
                layer = None

    # --- Internal deps
    # Find the [dependencies] section only (skip dev-dependencies, build-
    # dependencies, features, etc.). We look for `cow-*` crate names, and
    # skip `cow-rs` (the legacy compat shim) which doesn't participate.
    deps: List[str] = []
    deps_section = re.search(
        r"^\[dependencies\]\s*\n(.*?)(?=^\[|\Z)",
        text,
        re.DOTALL | re.MULTILINE,
    )
    if deps_section:
        body = deps_section.group(1)
        for line in body.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            m = re.match(r"^(cow-[a-z0-9-]+)\s*[=.]", line)
            if m and m.group(1) != "cow-rs":
                deps.append(m.group(1))
    return name, layer, deps


def layer_rank(layer: object) -> Optional[int]:
    """Return a comparable integer for normal layers, or None for orthogonal."""
    if isinstance(layer, int):
        return layer
    return None


def main() -> int:
    crates: Dict[str, Tuple[object, List[str]]] = {}
    unparsed: List[Path] = []

    for manifest in sorted(CRATES_DIR.glob("*/Cargo.toml")):
        name, layer, deps = parse_cargo_toml(manifest)
        if name is None:
            unparsed.append(manifest)
            continue
        if name in EXCLUDED_CRATES:
            continue
        crates[name] = (layer, deps)

    errors: List[str] = []

    if unparsed:
        for p in unparsed:
            errors.append(f"could not parse package name in {p}")

    # --- Per-crate layer declaration
    for name, (layer, _) in crates.items():
        if layer is None:
            errors.append(
                f"{name}: missing [package.metadata.workspace.layer]"
                " (set `layer = N` or `layer = \"orthogonal\"`)"
            )

    # --- DAG / sibling checks
    for name, (layer, deps) in crates.items():
        own_rank = layer_rank(layer)
        for dep in deps:
            if dep in INFRA_CRATES:
                continue
            if dep == name:
                continue
            if dep not in crates:
                # Unknown cow-* dep — probably a typo or a stub not yet added.
                errors.append(f"{name}: unknown workspace dep `{dep}`")
                continue
            dep_layer = crates[dep][0]
            dep_rank = layer_rank(dep_layer)

            # Orthogonal crates may depend on any layer, and vice versa.
            if own_rank is None or dep_rank is None:
                continue

            if dep_rank > own_rank:
                errors.append(
                    f"{name} (L{own_rank}) depends on {dep} (L{dep_rank}) — "
                    "layer violation: a crate may only depend on strictly "
                    "lower layers"
                )
            elif dep_rank == own_rank:
                if (name, dep) in ALLOWED_SIBLING_EDGES:
                    print(
                        f"note: allowed migration debt — {name} (L{own_rank}) "
                        f"depends on sibling {dep} (L{dep_rank})",
                        file=sys.stderr,
                    )
                    continue
                errors.append(
                    f"{name} (L{own_rank}) depends on sibling {dep} (L{dep_rank}) — "
                    "sibling violation (Rule 2)"
                )

    if errors:
        print("workspace layer check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(f"workspace layer check OK ({len(crates)} crates)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
