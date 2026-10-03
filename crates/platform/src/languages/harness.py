"""Untrusted protocol adapter; no expected answers or credentials enter this process.
The controller independently enforces wall time, cgroup memory, and protocol bounds.
"""
import base64
import json
import os
import selectors
import resource
import subprocess
import sys
import zipfile

mode, cap = sys.argv[1], int(sys.argv[2])
heap_mib = int(sys.argv[3]) if len(sys.argv) > 3 else 256
os.chdir('/work')
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
resource.setrlimit(resource.RLIMIT_NOFILE, (64, 64))
resource.setrlimit(resource.RLIMIT_FSIZE, (64 * 1024 * 1024, 64 * 1024 * 1024))
try:
    resource.setrlimit(resource.RLIMIT_NPROC, (64, 64))
except (ValueError, OSError):
    pass

# Kubernetes cannot copy into an unstarted Pod. Its controller sends one bounded
# request containing the single input file and case input over attach stdin.
protocol = os.environ.get('J0CODER_STREAM_PROTOCOL') == '1'
request = None
if protocol:
    raw = sys.stdin.buffer.read(64 * 1024 * 1024 + 1)
    if len(raw) > 64 * 1024 * 1024:
        raise SystemExit('request too large')
    request = json.loads(raw)
    if set(request) != {'name', 'data', 'input'} or request['name'] not in ('solution.cpp', 'solution.py', 'solution.java', 'solution.kt', 'program.b64', 'program.jar.b64'):
        raise SystemExit('invalid request')
    with open('/input/' + request['name'], 'wb') as f:
        f.write(base64.b64decode(request['data'], validate=True))
jvm_source = next((x for x in ('solution.java', 'solution.kt') if os.path.isfile('/input/' + x)), None)
if mode == 'compile':
    data = b''
    if jvm_source:
        with open('/input/' + jvm_source, encoding='utf-8') as f:
            payload = json.load(f)
        if set(payload) != {'source', 'interface', 'type_definitions', 'language'} or payload['language'] != ('java' if jvm_source.endswith('.java') else 'kotlin'):
            raise SystemExit('invalid JVM source payload')
        source_file = '/work/Solution.java' if jvm_source.endswith('.java') else '/work/Solution.kt'
        with open(source_file, 'w', encoding='utf-8') as f:
            f.write(payload['source'])
        with open('/work/judge-schema.json', 'w', encoding='utf-8') as f:
            json.dump({k: payload[k] for k in ('interface', 'type_definitions', 'language')}, f)
        os.mkdir('/work/classes')
        if jvm_source.endswith('.java'):
            command = ['javac', '--release', '21', '-d', '/work/classes', source_file]
        else:
            command = ['kotlinc', '-J-Xmx1536m', '-jvm-target', '21', '-d', '/work/classes', source_file]
    else:
        command = ['clang++', '-std=c++20', '-O2', '-pipe', '/input/solution.cpp', '-o', '/work/program']
else:
    data = request['input'].encode() if protocol else sys.stdin.buffer.read(2 * 1024 * 1024)
    if os.path.isfile('/input/program.jar.b64'):
        with open('/input/program.jar.b64', 'rb') as f:
            binary = base64.b64decode(f.read(), validate=True)
        with open('/work/program.jar', 'wb') as f:
            f.write(binary)
        command = ['java', '-XX:+UseSerialGC', '-Xmx%dm' % heap_mib,
                   '-cp', '/opt/judge:/work/program.jar:/opt/jackson/*:/opt/kotlinc/lib/*', 'JudgeMain']
    elif os.path.isfile('/input/program.b64'):
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
    if jvm_source:
        with zipfile.ZipFile('/work/program.jar', 'w', zipfile.ZIP_DEFLATED) as jar:
            jar.write('/work/judge-schema.json', 'judge-schema.json')
            for root, _, files in os.walk('/work/classes'):
                for filename in files:
                    path = os.path.join(root, filename)
                    jar.write(path, os.path.relpath(path, '/work/classes'))
        artifact_path = '/work/program.jar'
    else:
        artifact_path = '/work/program'
    with open(artifact_path, 'rb') as f:
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
