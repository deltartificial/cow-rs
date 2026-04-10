#!/usr/bin/env python3
# ruff: noqa: T201
"""Bundle upstream CoW Protocol AppData JSON Schemas into self-contained files.

Fetches the raw schema files from the `cowprotocol/app-data` repository at
the pinned commit, walks every ``$ref`` recursively, inlines referenced
sub-schemas, strips ``$id`` fields, and writes one bundled JSON document
per version into ``specs/app-data/vX.Y.Z.json``.

The bundled outputs are what ``crates/cow-rs/src/app_data/schema.rs``
``include_str!``s at compile time, so this script must be run any time
the upstream schemas change (``make fetch-appdata-schema``).

Usage
-----
    scripts/bundle-appdata-schemas.py [--versions VERSION [VERSION ...]]
                                      [--commit SHA]
                                      [--out-dir DIR]

Defaults
--------
* Commit: ``main`` on ``cowprotocol/app-data`` (override with
  ``--commit`` for reproducible builds).
* Versions: every version currently registered in
  :func:`cow_rs::app_data::schema` — keep this list in sync when
  adding or removing a bundled version in the Rust side.
* Output directory: ``specs/app-data/`` relative to the repo root.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import urllib.request
from pathlib import Path

# ── Configuration ────────────────────────────────────────────────────────────

DEFAULT_COMMIT = "main"
DEFAULT_VERSIONS = [
    "v1.0.0",
    "v1.5.0",
    "v1.6.0",
    "v1.10.0",
    "v1.13.0",
    "v1.14.0",
]
REPO_BASE_URL = "https://raw.githubusercontent.com/cowprotocol/app-data"

# Every sub-schema path that appears via ``$ref`` somewhere in the root
# versioned schemas. Maintained by hand — if a new sub-schema directory is
# added upstream, extend this list.
SUB_SCHEMA_PATHS = [
    "definitions.json",
    "bridging/v0.1.0.json",
    "flashloan/v0.1.0.json",
    "hook/v0.2.0.json",
    "hooks/v0.2.0.json",
    "orderClass/v0.3.0.json",
    "partnerFee/v0.1.0.json",
    "partnerFee/v1.0.0.json",
    "quote/v0.1.0.json",
    "quote/v0.2.0.json",
    "quote/v0.3.0.json",
    "quote/v1.0.0.json",
    "quote/v1.1.0.json",
    "referrer/v0.1.0.json",
    "referrer/v0.2.0.json",
    "referrer/v1.0.0.json",
    "replacedOrder/v0.1.0.json",
    "signer/v0.1.0.json",
    "userConsents/v0.1.0.json",
    "utm/v0.1.0.json",
    "utm/v0.2.0.json",
    "widget/v0.1.0.json",
    "wrappers/v0.1.0.json",
]

MAX_DEPTH = 30


# ── Core bundling logic ──────────────────────────────────────────────────────


def _load(path: Path) -> dict:
    with path.open() as f:
        return json.load(f)


def _preresolve_local_definitions(schema: dict) -> dict:
    """Flatten any ``#/definitions/X`` refs inside ``schema`` against
    its own top-level ``definitions`` block, in place.

    Upstream ``cowprotocol/app-data`` sub-schemas (``partnerFee/v1.0.0.json``
    etc.) carry their own root-level ``definitions`` and reference them
    via ``"$ref": "#/definitions/maxVolumeBps"``. When the main bundler
    inlines a sub-schema into a parent document, the sub-schema's
    ``definitions`` block is discarded and those refs become dangling.
    This helper inlines them BEFORE handing the schema to the outer
    bundler, producing a self-contained snippet where every reference
    has been replaced by its target.

    The ``definitions`` block itself is **kept** so that external
    fragment refs (``other.json#/definitions/ethereumAddress``) can
    still walk into it after this function returns.

    Non-dict / non-list leaves pass through unchanged. Refs that do not
    begin with ``#/definitions/`` (e.g. ``#/properties/x``) are left
    alone — they are either still valid in the bundled document or
    something the outer bundler already handles.
    """
    if not isinstance(schema, dict) or "definitions" not in schema:
        return schema

    defs: dict = schema["definitions"]

    def walk(obj, depth: int = 0):
        if depth > MAX_DEPTH:
            return obj
        if isinstance(obj, dict):
            if "$ref" in obj and obj["$ref"].startswith("#/definitions/"):
                key = obj["$ref"][len("#/definitions/") :]
                if key in defs:
                    other = {k: v for k, v in obj.items() if k != "$ref"}
                    resolved = walk(defs[key], depth + 1)
                    if isinstance(resolved, dict) and other:
                        resolved = {
                            **resolved,
                            **{k: walk(v, depth + 1) for k, v in other.items()},
                        }
                    return resolved
                return obj
            return {k: walk(v, depth + 1) for k, v in obj.items()}
        if isinstance(obj, list):
            return [walk(item, depth + 1) for item in obj]
        return obj

    return {k: walk(v) for k, v in schema.items()}


def _resolve(obj, base_dir: Path, depth: int = 0):
    """Inline every file-based ``$ref`` encountered in ``obj``.

    File refs are loaded from disk, pre-flattened via
    :func:`_preresolve_local_definitions` to kill any local
    ``#/definitions/...`` refs, then recursed into so nested file refs
    are also resolved. In-document refs (``#/foo``) are left alone
    because every downstream JSON Schema validator understands them.
    Refs that cannot be resolved on disk are also left alone so the
    final validator fails loudly with a useful error instead of silently
    dropping them.
    """
    if depth > MAX_DEPTH:
        return obj
    if isinstance(obj, dict):
        if "$ref" in obj:
            ref = obj["$ref"]
            other = {k: v for k, v in obj.items() if k != "$ref"}
            if ref.startswith("#/"):
                return obj
            ref_path, _, fragment = ref.partition("#")
            target = (base_dir / ref_path).resolve()
            if target.exists():
                ext = _preresolve_local_definitions(_load(target))
                if fragment.strip("/"):
                    for key in fragment.strip("/").split("/"):
                        ext = ext.get(key, {})
                resolved = _resolve(ext, target.parent, depth + 1)
                if other and isinstance(resolved, dict):
                    resolved.update(
                        {k: _resolve(v, base_dir, depth + 1) for k, v in other.items()}
                    )
                return resolved
            return obj
        return {k: _resolve(v, base_dir, depth + 1) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_resolve(item, base_dir, depth + 1) for item in obj]
    return obj


def _clean(obj):
    """Strip ``$id`` fields — they anchor the schema to a remote URL that
    the bundled snapshot no longer matches and confuse draft-07 validators."""
    if isinstance(obj, dict):
        obj.pop("$id", None)
        return {k: _clean(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_clean(item) for item in obj]
    return obj


def bundle(version: str, source_dir: Path, out_dir: Path) -> None:
    """Bundle a single ``vX.Y.Z`` root schema into ``out_dir``."""
    src = source_dir / f"{version}.json"
    if not src.exists():
        print(f"  MISSING: {src}", file=sys.stderr)
        sys.exit(1)
    raw = _load(src)
    bundled = _clean(_resolve(raw, source_dir))
    bundled["$schema"] = "http://json-schema.org/draft-07/schema"
    out_path = out_dir / f"{version}.json"
    with out_path.open("w") as f:
        json.dump(bundled, f, indent=2)
        f.write("\n")
    print(f"  Bundled {version} -> {out_path.relative_to(Path.cwd())}")


# ── Fetching ─────────────────────────────────────────────────────────────────


def _download(commit: str, rel_path: str, dest: Path) -> None:
    url = f"{REPO_BASE_URL}/{commit}/src/schemas/{rel_path}"
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(url, headers={"User-Agent": "cow-rs/bundler"})
    with urllib.request.urlopen(req) as resp:  # noqa: S310 — URL is hard-coded
        dest.write_bytes(resp.read())


def fetch_sources(commit: str, versions: list[str], cache_dir: Path) -> None:
    print(f"Fetching upstream schemas from cowprotocol/app-data@{commit} ...")
    paths = [f"{v}.json" for v in versions] + SUB_SCHEMA_PATHS
    for rel in paths:
        dest = cache_dir / rel
        if not dest.exists():
            try:
                _download(commit, rel, dest)
            except Exception as exc:  # noqa: BLE001
                print(f"  WARN: failed to fetch {rel}: {exc}", file=sys.stderr)


# ── Entry point ──────────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--commit",
        default=DEFAULT_COMMIT,
        help="Upstream cowprotocol/app-data commit SHA or branch (default: main)",
    )
    parser.add_argument(
        "--versions",
        nargs="+",
        default=DEFAULT_VERSIONS,
        help="Versions to bundle (default: all registered in schema.rs)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("specs/app-data"),
        help="Output directory for bundled schemas (default: specs/app-data)",
    )
    parser.add_argument(
        "--source-dir",
        type=Path,
        default=None,
        help=(
            "Skip fetching and use a local directory of upstream source "
            "schemas (must contain vX.Y.Z.json + sub-schema directories)"
        ),
    )
    args = parser.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)

    if args.source_dir is not None:
        source_dir = args.source_dir
        print(f"Using local source schemas from {source_dir}")
    else:
        cache_dir = Path(tempfile.gettempdir()) / "cow-rs-appdata-sources"
        cache_dir.mkdir(parents=True, exist_ok=True)
        fetch_sources(args.commit, args.versions, cache_dir)
        source_dir = cache_dir

    print(f"Bundling {len(args.versions)} version(s) -> {args.out_dir}")
    for version in args.versions:
        bundle(version, source_dir, args.out_dir)

    print("Done.")


if __name__ == "__main__":
    main()
