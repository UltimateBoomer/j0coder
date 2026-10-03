#!/usr/bin/env python3
"""Select CI checks from changed paths; unknown changes run every check."""

import json
import os
import re
import subprocess
import sys

CHECKS = ("rust", "web", "integration", "helm", "config", "app_image", "toolchain_image")
ALL = set(CHECKS)


def checks_for(path):
    if path.startswith("docs/") or path in {"README.md", "LICENSE"}:
        return set()
    if path == ".github/workflows/publish-images.yml":
        return {"app_image", "toolchain_image"}
    if path.startswith(".github/") or path == "Makefile":
        return ALL
    if path.startswith("crates/platform/src/languages/"):
        return {"rust", "web", "integration", "app_image", "toolchain_image"}
    if path in {"crates/platform/src/contract.rs", "crates/platform/src/schema.rs", "crates/platform/src/accounts.rs", "crates/platform/src/api.rs", "crates/platform/src/bin/schema-export.rs", "crates/platform/Cargo.toml", "Cargo.toml", "Cargo.lock", "problem.schema.json"}:
        return {"rust", "web", "integration", "app_image"}
    if path in {"Cargo.toml", "Cargo.lock"} or path.startswith(("crates/", "migrations/")):
        return {"rust", "integration", "app_image"}
    if path == "openapi.json" or path == "scripts/generate-openapi.py":
        return {"rust", "web", "integration", "app_image"}
    if path.startswith("web/"):
        return {"web", "integration", "app_image"}
    if path.startswith("tests/browser/"):
        return {"web", "integration"}
    if path == "tests/api_acceptance.py" or path.startswith("samples/"):
        return {"integration"}
    if path.startswith("deploy/helm/"):
        return {"helm"}
    if path.startswith("deploy/local-kubernetes/"):
        return {"helm", "config"}
    if path.startswith("deploy/gvisor/"):
        return {"config"}
    if path == "deploy/App.Containerfile":
        return {"app_image"}
    if path == "deploy/nginx.conf":
        return {"config"}
    if path in {"deploy/Toolchain.Containerfile"}:
        return {"toolchain_image"}
    if path in {".dockerignore", ".containerignore"}:
        return {"app_image", "toolchain_image"}
    if path in {"compose.yaml", "compose.dev.yaml", ".env.example"}:
        return {"config"}
    if path.startswith("scripts/"):
        if path.startswith(("scripts/kube-", "scripts/lima-")):
            return {"helm", "config"}
        return {"config"}
    if path.startswith("tests/"):
        return {"config"}
    return ALL


def changed_paths():
    event = os.environ["GITHUB_EVENT_NAME"]
    if event == "workflow_dispatch":
        return None
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as file:
        payload = json.load(file)
    base = payload["pull_request"]["base"]["sha"] if event == "pull_request" else payload["before"]
    if not re.fullmatch(r"[0-9a-fA-F]{40}", base) or set(base) == {"0"}:
        return None
    result = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", "-z", base, "HEAD"],
        check=True, capture_output=True,
    )
    return [os.fsdecode(path) for path in result.stdout.split(b"\0") if path]


def main():
    try:
        paths = changed_paths()
        selected = ALL if paths is None else set().union(*(checks_for(path) for path in paths))
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Could not classify changes ({error}); running every check", file=sys.stderr)
        selected = ALL
    output = "".join(f"{check}={'true' if check in selected else 'false'}\n" for check in CHECKS)
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as file:
        file.write(output)
    print(output, end="")


if __name__ == "__main__":
    main()
