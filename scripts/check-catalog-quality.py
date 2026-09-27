#!/usr/bin/env python3
"""Check the curation target and basic test-suite quality for the problem catalog."""

import argparse
from collections import Counter
import json
from pathlib import Path
import sys
from catalog_source import load_problem, problem_paths


ORIGINAL_PATHS = {
    "problems/between-the-markers.json",
    "problems/quiet-peaks.json",
    "problems/folded-rows.json",
    "problems/lru-cache.json",
}
TARGET_GROUPS = {"easy": 24, "medium": 61, "hard": 35, "extra-hard": 5}


def check(root: Path) -> list[str]:
    paths = problem_paths(root)
    errors = []
    if len(paths) != 125:
        errors.append(f"expected 125 entries, found {len(paths)}")
    scores = Counter()
    titles = set()
    for artifact in paths:
        path = artifact.relative_to(root).as_posix()
        problem = load_problem(artifact)
        title = problem["title"].strip().casefold()
        if title in titles:
            errors.append(f"{path}: duplicate title")
        titles.add(title)
        band = problem["difficulty"]
        score = problem.get("difficulty_score", {"easy": 2, "medium": 3, "hard": 4}[band])
        scores[score] += 1
        if band != ("easy" if score <= 2 else "medium" if score == 3 else "hard"):
            errors.append(f"{path}: difficulty and score disagree")
        if artifact.stem in {Path(p).stem for p in ORIGINAL_PATHS}:
            continue
        if artifact.is_file() and artifact.stat().st_size > 1_000_000:
            errors.append(f"{path}: artifact exceeds 1 MB")
        tests = problem["tests"]
        visible = sum(not case.get("hidden", False) for case in tests)
        hidden = len(tests) - visible
        if visible < 2 or hidden < 3:
            errors.append(f"{path}: need at least 2 visible and 3 hidden tests, found {visible}/{hidden}")
        if not problem.get("tags"):
            errors.append(f"{path}: missing tags")
        if len(problem.get("tags", [])) != len(set(problem.get("tags", []))):
            errors.append(f"{path}: duplicate tags")
        if len(problem.get("statement", "").strip()) < 150:
            errors.append(f"{path}: statement too short to specify the task and constraints")
        inputs = set()
        for case in tests:
            key = json.dumps(
                [case.get("args"), case.get("constructor_args"), case.get("operations")],
                sort_keys=True,
                ensure_ascii=False,
            )
            if key in inputs:
                errors.append(f"{path}: duplicate test input")
                break
            inputs.add(key)
    groups = {
        "easy": scores[1] + scores[2],
        "medium": scores[3],
        "hard": scores[4],
        "extra-hard": scores[5],
    }
    for group, target in TARGET_GROUPS.items():
        if groups[group] != target:
            errors.append(f"{group}: expected {target}, found {groups[group]}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    args = parser.parse_args()
    errors = check(args.catalog)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("curation target and basic test-suite quality checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
