#!/usr/bin/env bash
# Start disposable integration dependencies only. Does not create an API/admin or execute submissions.
set -euo pipefail
podman run -d --name locoder-test-pg -p 127.0.0.1:15432:5432 \
 -e POSTGRES_PASSWORD=integration-only -e POSTGRES_DB=practice docker.io/library/postgres@sha256:747d5ed1fdeeb124b880fbe3d7c6557d2c4064ae41d6b6297d417882effce4be
# Valkey 9.1.2, pinned to its multi-architecture manifest.
podman run -d --name locoder-test-valkey -p 127.0.0.1:16379:6379 \
 docker.io/valkey/valkey@sha256:c123e3715db63d06d4ad6964884037aa0d5d4d703939b9929954112889708e1d valkey-server --appendonly yes
printf 'Test dependencies ready; remove only locoder-test-* containers when finished.\n'
