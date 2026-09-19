#!/usr/bin/env python3
"""Pin the images built in the current Podman engine into this deployment's .env."""
import pathlib,subprocess
root=pathlib.Path(__file__).resolve().parent.parent
path=root/'.env'
if not path.exists():raise SystemExit('Run configure.py first')
pins={}
for key,image in [('APP_IMAGE','localhost/practice-app:1'),('TOOLCHAIN_IMAGE','localhost/practice-toolchain:1')]:
 value=subprocess.check_output(['podman','image','inspect',image,'--format','{{.Id}}'],text=True).strip()
 if value.startswith('sha256:'):value=value[7:]
 if len(value)!=64 or any(c not in '0123456789abcdef' for c in value):raise SystemExit('Unexpected image ID')
 pins[key]='sha256:'+value
lines=[line for line in path.read_text().splitlines() if line.split('=',1)[0] not in pins]
path.write_text('\n'.join(lines+[f'{key}={value}' for key,value in pins.items()])+'\n')
print('Pinned app and toolchain to immutable local image IDs in .env')
