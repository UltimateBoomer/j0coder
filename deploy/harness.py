"""Untrusted protocol adapter; no expected answers or credentials enter this process.
The controller independently enforces wall time, cgroup memory, and protocol bounds.
"""
import base64
import json
import os
import selectors
import subprocess
import sys

mode, cap = sys.argv[1], int(sys.argv[2])
os.chdir('/work')
if mode == 'compile':
    command = ['clang++', '-std=c++20', '-O2', '-pipe', '/input/solution.cpp', '-o', '/work/program']
    data = b''
else:
    data = sys.stdin.buffer.read(2 * 1024 * 1024)
    if os.path.isfile('/input/program.b64'):
        with open('/input/program.b64', 'rb') as f:
            binary = base64.b64decode(f.read(), validate=True)
        with open('/work/program', 'wb') as f:
            f.write(binary)
        os.chmod('/work/program', 0o700)
        command = ['/work/program']
    else:
        command = ['python3', '-I', '-B', '/input/solution.py']
p = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
# Inputs are bounded, but use a background writer so programs that never read stdin cannot deadlock capture.
import threading

def feed():
    try:
        p.stdin.write(data + b'\n')
        p.stdin.close()
    except (BrokenPipeError, OSError):
        pass
threading.Thread(target=feed, daemon=True).start()
sel = selectors.DefaultSelector()
sel.register(p.stdout, selectors.EVENT_READ)
sel.register(p.stderr, selectors.EVENT_READ)
logs = bytearray()
overflow = False
while sel.get_map():
    for key, _ in sel.select(0.05):
        part = os.read(key.fileobj.fileno(), 8192)
        if not part:
            sel.unregister(key.fileobj)
            continue
        logs.extend(part)
        if len(logs) > cap:
            overflow = True
            import signal
            os.killpg(p.pid, signal.SIGKILL)
            logs = logs[:cap]
    try:
        if os.stat('/work/result').st_size + len(logs) > cap:
            overflow = True
            p.kill()
    except FileNotFoundError:
        pass
code = p.wait()
output = None
artifact = None
if mode == 'compile' and code == 0:
    with open('/work/program', 'rb') as f:
        binary = f.read(32 * 1024 * 1024 + 1)
    if len(binary) > 32 * 1024 * 1024:
        overflow = True
    else:
        artifact = base64.b64encode(binary).decode('ascii')
elif mode == 'run' and code == 0 and not overflow:
    try:
        with open('/work/result', 'rb') as f:
            raw = f.read(cap + 1)
        if len(raw) + len(logs) > cap:
            overflow = True
        else:
            output = json.loads(raw)
    except (OSError, ValueError):
        code = 1
print(json.dumps(dict(exit=code, output=output, log=logs.decode('utf-8', 'replace'), overflow=overflow, artifact=artifact), ensure_ascii=True))
