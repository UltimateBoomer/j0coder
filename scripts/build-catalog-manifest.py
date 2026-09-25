#!/usr/bin/env python3
"""Update or check the checksum manifest of a sibling problem catalog."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


def build(root: Path) -> dict:
    manifest_path = root / "catalog.json"
    previous = json.loads(manifest_path.read_text())
    if previous.get("schema") != 1:
        raise ValueError("expected schema-1 catalog manifest")
    prior_keys = {entry["path"]: entry["key"] for entry in previous["problems"]}
    paths = sorted((root / "problems").glob("*.json"))
    if not paths:
        raise ValueError("no problem artifacts found")
    entries = []
    keys = set()
    for path in paths:
        relative = path.relative_to(root).as_posix()
        raw = path.read_bytes()
        problem = json.loads(raw)
        if relative in prior_keys:
            key = prior_keys[relative]
        else:
            tags = problem.get("tags") or []
            if not tags:
                raise ValueError(f"{relative}: missing primary tag")
            slug = re.sub(r"([a-z0-9])([A-Z])", r"\1-\2", path.stem).replace("_", "-").lower()
            key = f"{tags[0]}/{slug}"
        if not re.fullmatch(r"[a-z0-9][a-z0-9._:/-]{2,199}", key):
            raise ValueError(f"{relative}: invalid key {key!r}")
        if key in keys:
            raise ValueError(f"duplicate catalog key: {key}")
        keys.add(key)
        entries.append({"key": key, "path": relative, "sha256": hashlib.sha256(raw).hexdigest()})
    return {"schema": 1, "problems": sorted(entries, key=lambda entry: entry["key"])}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    parser.add_argument("--check", action="store_true", help="fail if catalog.json needs updating")
    parser.add_argument("--expect", type=int, help="required number of problems")
    args = parser.parse_args()
    manifest = build(args.catalog)
    if args.expect is not None and len(manifest["problems"]) != args.expect:
        raise ValueError(f"expected {args.expect} problems, found {len(manifest['problems'])}")
    output = json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"
    path = args.catalog / "catalog.json"
    if args.check:
        if path.read_text() != output:
            print(f"{path} is out of date", file=sys.stderr)
            return 1
    else:
        path.write_text(output)
    print(f"{len(manifest['problems'])} catalog entries checked")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1) from exc
