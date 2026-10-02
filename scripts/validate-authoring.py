#!/usr/bin/env python3
"""Validate the published catalog and unpublished example against Rust's schema."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

import jsonschema
import yaml


def main():
    root = Path(sys.argv[1])
    generated = subprocess.check_output([
        "cargo", "run", "--locked", "--quiet", "--bin", "schema-export", "--", "authoring"
    ])
    if generated != (root / "problem.schema.json").read_bytes():
        raise SystemExit("Catalog schema differs from the pinned application's generated schema")
    schema = json.loads(generated)
    jsonschema.Draft7Validator.check_schema(schema)
    validator = jsonschema.Draft7Validator(schema)
    paths = sorted((root / "problems").glob("*/problem.yaml")) + [root / "example-problem/problem.yaml"]
    for path in paths:
        validator.validate(yaml.safe_load(path.read_text()))
    # Run the same semantic/filesystem validator for the unpublished example.
    with tempfile.TemporaryDirectory(prefix="j0coder-example-") as scratch:
        import shutil
        shutil.copytree(root / "example-problem", Path(scratch) / "problems/example")
        subprocess.run(["cargo", "run", "--locked", "--quiet", "--bin", "catalog-validate", "--", scratch], check=True)
    print(f"Validated {len(paths)} authoring documents, including the unpublished example")


if __name__ == "__main__":
    main()
