#!/usr/bin/env python3
import os, pathlib, secrets
root = pathlib.Path(__file__).resolve().parent.parent
if (root / '.env').exists(): raise SystemExit('.env already exists; refusing to overwrite credentials')
os.umask(0o077)
config = root / 'data/config'; config.mkdir(parents=True, exist_ok=True)
pg, valkey = [secrets.token_hex(32) for _ in range(2)]
runtime_dir = pathlib.Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}'))
(root / '.env').write_text(
    f'POSTGRES_PASSWORD={pg}\nVALKEY_PASSWORD={valkey}\n'
    'PUBLIC_ORIGIN=http://localhost:8080\nCOOKIE_SECURE=false\nPORT=8080\n'
    f'PODMAN_SOCKET={runtime_dir}/practice-podman.sock\n'
    f'SANDBOX_RUNTIME={root}/.dev/gvisor/current/runsc\n'
)
(config / 'valkey.conf').write_text(f'bind 0.0.0.0\nprotected-mode yes\nrequirepass {valkey}\nappendonly yes\nappendfsync everysec\ndir /data\nmaxmemory-policy noeviction\n')
# Container services must read these files after UID mapping; enclosing data directory remains private.
for p in config.iterdir():p.chmod(0o644)
print('Created .env and rootless service configuration. Set origin and TLS cookie policy before deployment.')
