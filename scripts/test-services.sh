#!/usr/bin/env bash
# Start disposable integration dependencies only. Does not create an API/admin or execute submissions.
set -euo pipefail
podman run -d --name practice-test-pg -p 127.0.0.1:15432:5432 \
 -e POSTGRES_PASSWORD=integration-only -e POSTGRES_DB=practice docker.io/library/postgres@sha256:747d5ed1fdeeb124b880fbe3d7c6557d2c4064ae41d6b6297d417882effce4be
podman run -d --name practice-test-redis -p 127.0.0.1:16379:6379 \
 docker.io/library/redis@sha256:0302cccee2b2043e61b497c8f4075467c5f7ba27a9f38be7e092634f2734baed redis-server --appendonly yes
printf 'Test dependencies ready; remove only practice-test-* containers when finished.\n'
