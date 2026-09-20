#!/usr/bin/env python3
"""Pin the images built in the current Podman engine into this deployment's .env."""
import argparse
import pathlib
import shlex
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--podman', default='podman')
parser.add_argument('--app-image', default='localhost/practice-app:1')
parser.add_argument('--toolchain-image', default='localhost/practice-toolchain:1')
args = parser.parse_args()

root=pathlib.Path(__file__).resolve().parent.parent
path=root/'.env'
if not path.exists():raise SystemExit('Run configure.py first')
pins={}
podman=shlex.split(args.podman)
if not podman:raise SystemExit('Podman command cannot be empty')
for key,image in [('APP_IMAGE',args.app_image),('TOOLCHAIN_IMAGE',args.toolchain_image)]:
 value=subprocess.check_output([*podman,'image','inspect',image,'--format','{{.Id}}'],text=True).strip()
 if value.startswith('sha256:'):value=value[7:]
 if len(value)!=64 or any(c not in '0123456789abcdef' for c in value):raise SystemExit('Unexpected image ID')
 pins[key]='sha256:'+value
lines=[line for line in path.read_text().splitlines() if line.split('=',1)[0] not in pins]
path.write_text('\n'.join(lines+[f'{key}={value}' for key,value in pins.items()])+'\n')
print('Pinned app and toolchain to immutable local image IDs in .env')
