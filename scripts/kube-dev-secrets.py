#!/usr/bin/env python3
"""Render local Kubernetes credentials from .env without exposing them in argv."""
import base64
import json
import pathlib
import sys
import urllib.parse


def read_env(path: pathlib.Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for number, raw in enumerate(path.read_text().splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise SystemExit(f"{path}:{number}: expected KEY=VALUE")
        key, value = line.split("=", 1)
        values[key] = value
    return values


def encoded(value: str) -> str:
    return base64.b64encode(value.encode()).decode()


path = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".env")
values = read_env(path)
missing = [key for key in ("POSTGRES_PASSWORD", "VALKEY_PASSWORD") if not values.get(key)]
if missing:
    raise SystemExit(f"{path}: missing required values: {', '.join(missing)}")

postgres = urllib.parse.quote(values["POSTGRES_PASSWORD"], safe="")
valkey = urllib.parse.quote(values["VALKEY_PASSWORD"], safe="")
data = {
    "POSTGRES_PASSWORD": encoded(values["POSTGRES_PASSWORD"]),
    "VALKEY_PASSWORD": encoded(values["VALKEY_PASSWORD"]),
    "DATABASE_URL": encoded(f"postgres://practice:{postgres}@locoder-postgres:5432/practice"),
    "VALKEY_URL": encoded(f"redis://:{valkey}@locoder-valkey:6379"),
}
print(json.dumps({
    "apiVersion": "v1",
    "kind": "Secret",
    "metadata": {"name": "locoder-dev", "namespace": "locoder"},
    "type": "Opaque",
    "data": data,
}))
