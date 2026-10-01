#!/usr/bin/env python3
"""Build a deployment-only archive and the installer contract manifest."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
parser.add_argument('--app-image', required=True)
parser.add_argument('--toolchain-image', required=True)
parser.add_argument('--gvisor-dir', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('release-assets'))
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
args.output.mkdir(parents=True, exist_ok=True)
with tarfile.open(args.output / 'deployment.tar.gz', 'w:gz', dereference=True) as tar:
    for name in ['compose.yaml', 'deploy/nginx.conf', 'scripts/setup.py', 'scripts/setup.sh', 'scripts/install.sh', 'scripts/preflight.sh', 'scripts/gvisor-controller.sh']:
        tar.add(root / name, arcname=name)
    makefile = args.output / 'Makefile'
    makefile.write_text('SETUP_ARGS ?=\n.PHONY: up down status logs\nup down status logs:\n\t./scripts/setup.sh $@ $(SETUP_ARGS)\n')
    tar.add(makefile, arcname='Makefile')
makefile.unlink()
with tarfile.open(args.output / 'gvisor-linux-x86_64.tar.gz', 'w:gz', dereference=True) as tar:
    for name in ['runsc', 'gvisor-bin']:
        tar.add(args.gvisor_dir / name, arcname=name)
base = f'https://github.com/UltimateBoomer/j0coder/releases/download/{args.version}/'
manifest = dict(version=args.version, installer_version=1, app_image=args.app_image, toolchain_image=args.toolchain_image)
for key, filename in [('bundle', 'deployment.tar.gz'), ('gvisor', 'gvisor-linux-x86_64.tar.gz')]:
    manifest[key] = dict(url=base + filename, sha256=hashlib.sha256((args.output / filename).read_bytes()).hexdigest())
(args.output / 'release-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
