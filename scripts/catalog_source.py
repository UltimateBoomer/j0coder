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
    if type(metadata["difficulty"]) is int:
        score = metadata["difficulty"]
        if not 1 <= score <= 5 or "difficulty_score" in metadata:
            raise ValueError(f"{path}: integer difficulty must be 1-5 without difficulty_score")
        metadata["difficulty"] = "easy" if score <= 2 else "medium" if score == 3 else "hard"
        metadata["difficulty_score"] = score
    statement_path = path / "statement.md"
    if "statement" in metadata:
        if statement_path.exists():
            raise ValueError(f"{path}: statement is defined in both locations")
    else:
        metadata["statement"] = statement_path.read_text()
    hint_dir = path / "hints"
    if "hints" in metadata:
        if hint_dir.exists():
            raise ValueError(f"{path}: hints are defined in both locations")
    else:
        metadata["hints"] = [p.read_text() for p in sorted(hint_dir.glob("*.md"))]
    source_tests = metadata.pop("tests", None)
    if source_tests is not None and (path / "tests").exists():
        raise ValueError(f"{path}: tests are defined in both locations")
    if source_tests is not None and set(source_tests) - {"visible", "hidden"}:
        raise ValueError(f"{path}: unknown tests group")
    tests = []
    for visibility in ("visible", "hidden"):
        if source_tests is None:
            cases = [
                json.loads(case.read_text()) if case.suffix == ".json" else yaml.safe_load(case.read_text())
                for case in sorted((path / "tests" / visibility).glob("*"))
            ]
        else:
            cases = source_tests.get(visibility, [])
        for item in cases:
            if "hidden" in item:
                raise ValueError(f"{path}: visibility comes from tests group")
            item["hidden"] = visibility == "hidden"
            tests.append(item)
    metadata["tests"] = tests
    return metadata


def problem_paths(root: Path):
    paths = sorted((Path(root) / "problems").glob("*/problem.yaml"))
    if paths:
        return [p.parent for p in paths]
    return sorted((Path(root) / "problems").glob("*.json"))
