"""Run inside the pinned toolchain image with writable cache mounts.

Checks isolated Java and Kotlin LSP initialization and completion over stdio.
"""
import json
import os
import queue
import subprocess
import sys
import threading
import time


def check(command, language, filename, source, line, character):
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd='/workspace')
    messages = queue.Queue()
    errors = []

    def read():
        try:
            while True:
                length = None
                while True:
                    header = process.stdout.readline()
                    if not header:
                        return
                    if header == b'\r\n':
                        break
                    if header.lower().startswith(b'content-length:'):
                        length = int(header.split(b':')[1])
                if length is None:
                    continue
                messages.put(json.loads(process.stdout.read(length)))
        except Exception as error:
            errors.append(error)

    threading.Thread(target=read, daemon=True).start()

    def send(value):
        data = json.dumps({'jsonrpc': '2.0', **value}).encode()
        process.stdin.write(f'Content-Length: {len(data)}\r\n\r\n'.encode() + data)
        process.stdin.flush()

    def wait(ident, timeout=120):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise AssertionError(f'{language} server exited {process.returncode}: {process.stderr.read().decode()[-2000:]}')
            try:
                value = messages.get(timeout=min(5, deadline-time.monotonic()))
            except queue.Empty:
                continue
            if value.get('id') == ident and 'method' not in value:
                return value
            if 'id' in value and 'method' in value:
                send({'id': value['id'], 'result': [{}] if value['method'] == 'workspace/configuration' else None})
        raise AssertionError(f'{language} LSP response timeout; reader errors: {errors}')

    try:
        send({'id': 1, 'method': 'initialize', 'params': {'processId': None, 'rootUri': 'file:///workspace', 'capabilities': {'textDocument': {'completion': {'completionItem': {'snippetSupport': True}}}}}})
        initialized = wait(1)
        assert 'result' in initialized, initialized
        send({'method': 'initialized', 'params': {}})
        uri = 'file:///workspace/' + filename
        send({'method': 'textDocument/didOpen', 'params': {'textDocument': {'uri': uri, 'languageId': language, 'version': 1, 'text': source}}})
        time.sleep(3)
        send({'id': 2, 'method': 'textDocument/completion', 'params': {'textDocument': {'uri': uri}, 'position': {'line': line, 'character': character}}})
        completion = wait(2)
        result = completion.get('result')
        items = result.get('items', []) if isinstance(result, dict) else result
        assert items, completion
        print(language, 'semantic completion:', len(items), 'items')
    finally:
        process.kill()
        process.wait()


if __name__ == '__main__':
    language = sys.argv[1]
    if language == 'java':
        check(['/opt/jdtls/bin/jdtls', '-configuration', '/workspace/.cache/jdtls-config', '-data', '/workspace/.cache/jdtls'], 'java', 'Solution.java', 'class Solution { void solve() { String value = "x"; value. } }', 0, 58)
    elif language == 'kotlin':
        check(['/usr/local/bin/j0coder-kotlin-lsp', '--stdio', '--system-path=/workspace/.cache/kotlin-lsp', '--data-sharing=none', '--region=americas'], 'kotlin', 'solution.kt', 'fun solve() { val value = "x"; value. }', 0, 37)
    else:
        raise SystemExit('expected java or kotlin')
