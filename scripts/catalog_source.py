"""Read catalog problems for quality and independent oracle checks.

The Rust catalog validator remains authoritative for source security and types.
"""

import json
from pathlib import Path

import yaml


def load_problem(path: Path) -> dict:
    path = Path(path)
    if path.is_file():
        return json.loads(path.read_text())
    metadata = yaml.safe_load((path / "problem.yaml").read_text())
    metadata.pop("key")
    metadata["schema"] = 3
    metadata["statement"] = (path / "statement.md").read_text()
    hint_dir = path / "hints"
    metadata["hints"] = [p.read_text() for p in sorted(hint_dir.glob("*.md"))]
    tests = []
    for visibility in ("visible", "hidden"):
        for case in sorted((path / "tests" / visibility).glob("*")):
            item = json.loads(case.read_text()) if case.suffix == ".json" else yaml.safe_load(case.read_text())
            item["hidden"] = visibility == "hidden"
            tests.append(item)
    metadata["tests"] = tests
    return metadata


def problem_paths(root: Path):
    paths = sorted((Path(root) / "problems").glob("*/problem.yaml"))
    if paths:
        return [p.parent for p in paths]
    return sorted((Path(root) / "problems").glob("*.json"))
