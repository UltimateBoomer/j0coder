#!/usr/bin/env python3
"""Release-backed, resumable rootless Compose installation (Python stdlib only)."""
import argparse
import contextlib
import fcntl
import getpass
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import shutil
import signal
import selectors
import codecs
import threading
import subprocess
import tarfile
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

REPO = 'UltimateBoomer/j0coder'
ROOT = Path(__file__).resolve().parent.parent


class Reporter:
    def __init__(self, root=None, verbose=False):
        self.path = None
        self.log = None
        if root is not None:
            directory = root / '.dev/setup-logs'
            directory.mkdir(parents=True, exist_ok=True)
            directory.chmod(0o700)
            self.path = directory / (time.strftime('%Y%m%d-%H%M%S') + '-' + secrets.token_hex(6) + '.log')
            fd = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            self.log = os.fdopen(fd, 'w')
        self.verbose = verbose
        self.secrets = set()
        self.lines = []
        self.stage = 'initialization'

    def clean(self, value):
        for secret in sorted(self.secrets, key=len, reverse=True):
            if secret:
                value = value.replace(secret, '[REDACTED]')
        return value

    def say(self, value):
        value = self.clean(value)
        self.write_log(value)
        print(value, flush=True)

    def write_log(self, value):
        if self.log is not None:
            self.log.write(self.clean(value) + '\n')
            self.log.flush()

    def diagnostic(self, value):
        value = self.clean(value)
        self.write_log(value)
        self.lines.extend(value.splitlines())
        self.lines = self.lines[-30:]
        if self.verbose:
            print(value, flush=True)

    def begin(self, number, title):
        self.stage = title
        self.lines = []
        self.say(f'\n[{number}/8] {title}')

    def done(self, outcome):
        self.say('DONE — ' + outcome)


REPORTER = None


def say(value):
    if REPORTER:
        REPORTER.say(value)
    else:
        print(value, flush=True)


@contextlib.contextmanager
def progress(label):
    stopped = threading.Event()
    started = time.monotonic()
    def update():
        while not stopped.wait(10):
            say(f'{label}: {int(time.monotonic() - started)} seconds elapsed')
    thread = threading.Thread(target=update, daemon=True)
    thread.start()
    try:
        yield
    finally:
        stopped.set()
        thread.join()


def run(args, **kwargs):
    if REPORTER is None:
        return subprocess.run(args, check=True, text=True, **kwargs)
    captured = kwargs.pop('capture_output', False)
    suppressed = kwargs.pop('stdout', None) == subprocess.DEVNULL
    kwargs.pop('stderr', None)
    input_value = kwargs.pop('input', None)
    # Captured stdout is a private machine-readable channel, never a diagnostic.
    with tempfile.TemporaryFile(mode='w+') as private:
        process = subprocess.Popen(args, text=True, stdout=private if captured else subprocess.DEVNULL if suppressed else subprocess.PIPE,
                                   stderr=subprocess.PIPE, stdin=subprocess.PIPE if input_value is not None else None, **kwargs)
        if input_value is not None:
            process.stdin.write(input_value)
            process.stdin.close()
        selector = selectors.DefaultSelector()
        buffers = {}
        decoders = {}
        for stream in (process.stderr, None if captured or suppressed else process.stdout):
            if stream:
                selector.register(stream, selectors.EVENT_READ)
                buffers[stream] = ''
                decoders[stream] = codecs.getincrementaldecoder('utf-8')('replace')
        try:
            with progress('Command running'):
                while selector.get_map():
                    for key, _ in selector.select(timeout=1):
                        stream = key.fileobj
                        chunk = os.read(stream.fileno(), 65536)
                        buffers[stream] += decoders[stream].decode(chunk, final=not chunk)
                        while '\n' in buffers[stream]:
                            line, buffers[stream] = buffers[stream].split('\n', 1)
                            if not captured:
                                REPORTER.diagnostic(line.rstrip('\r'))
                        if not chunk:
                            if buffers[stream] and not captured:
                                REPORTER.diagnostic(buffers[stream])
                            selector.unregister(stream)
                            stream.close()
                code = process.wait()
        finally:
            selector.close()
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        if code:
            raise subprocess.CalledProcessError(code, args)
        private.seek(0)
        return subprocess.CompletedProcess(args, code, stdout=private.read() if captured else None)


def download(url):
    if urllib.parse.urlsplit(url).scheme != 'https':
        raise ValueError('downloads require HTTPS')
    say('Downloading: ' + url)
    with progress('Download'), urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    say('DONE — Downloaded ' + url)
    return data


def atomic(path, content, mode=0o600):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(dir=path.parent)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, 'w') as stream:
            stream.write(content)
        os.replace(temp, path)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def read_env(path):
    result = {}
    if path.exists():
        for line in path.read_text().splitlines():
            if not line.strip() or line.lstrip().startswith('#'):
                continue
            key, value = line.split('=', 1)
            parsed = shlex.split(value)
            result[key] = parsed[0] if parsed else ''
    return result


def write_env(path, env):
    # Compatible with both Compose dotenv and existing shell source consumers.
    if any('\n' in value or '\r' in value for value in env.values()):
        raise ValueError('configuration values must be single-line')
    atomic(path, ''.join(f'{key}={shlex.quote(value)}\n' for key, value in env.items()))


def prompt(message, default=None):
    say('\nINPUT REQUIRED — ' + message)
    say(f'Press Enter to use {default}.' if default is not None else 'Enter the requested answer.')
    try:
        with open('/dev/tty', 'r') as reader, open('/dev/tty', 'w') as tty:
            tty.write('> ' + message + (f' [{default}]' if default is not None else '') + ': ')
            tty.flush()
            answer = reader.readline()
            if not answer:
                raise ValueError('terminal input closed')
            return answer.strip() or default
    except OSError as error:
        raise OSError(f'{message} requires /dev/tty input; supply the corresponding command-line option: {error}') from error


def yesno(message, default='no'):
    while True:
        answer = prompt(message + ' (yes/y or no/n)', default).lower()
        if answer in ('yes', 'y', 'no', 'n'):
            return answer in ('yes', 'y')
        say('Invalid answer; enter yes/y or no/n.')


def validate_port(port):
    if not str(port).isdigit() or not 1024 <= int(port) <= 65535:
        raise ValueError('local port must be between 1024 and 65535')
    return str(port)


def validate_origin(origin):
    parsed = urllib.parse.urlsplit(origin)
    try:
        parsed.port
    except ValueError:
        raise ValueError('public origin has an invalid port')
    if parsed.scheme not in ('http', 'https') or not parsed.hostname or parsed.username or parsed.password or parsed.path not in ('', '/') or parsed.query or parsed.fragment or any(c.isspace() for c in origin):
        raise ValueError('public origin must be an HTTP(S) origin without a path or credentials')
    return origin.rstrip('/')


def field(value, label, default, validator):
    if value is not None:
        result = validator(value)
        say(f'{label}: {result} (supplied)')
        return result
    while True:
        answer = prompt(label, default)
        try:
            return validator(answer)
        except ValueError as error:
            say(str(error))


def manifest_valid(manifest):
    if manifest.get('installer_version') != 1:
        raise ValueError('release has no compatible installer manifest')
    for key in ('app_image', 'toolchain_image'):
        if not re.fullmatch(r'ghcr\.io/[a-z0-9/_.-]+@sha256:[0-9a-f]{64}', manifest[key]):
            raise ValueError('release images must be immutable GHCR references')
    for key in ('bundle', 'gvisor'):
        if not re.fullmatch('[0-9a-f]{64}', manifest[key]['sha256']):
            raise ValueError('invalid archive checksum')
    return manifest


def archive(asset, destination):
    data = download(asset['url'])
    if hashlib.sha256(data).hexdigest() != asset['sha256']:
        raise ValueError('archive checksum mismatch')
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryFile() as stream:
        stream.write(data)
        stream.seek(0)
        with tarfile.open(fileobj=stream) as tar:
            # Release archives intentionally contain only directories and regular files.
            for member in tar.getmembers():
                target = destination / member.name
                if member.name.startswith('/') or '..' in Path(member.name).parts or not (member.isfile() or member.isdir()):
                    raise ValueError('unsafe archive member')
                if target.is_symlink():
                    raise ValueError('refusing extraction over a symlink')
            for member in tar.getmembers():
                target = destination / member.name
                target.resolve().relative_to(destination.resolve())
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with tar.extractfile(member) as source, open(target, 'wb') as output:
                        shutil.copyfileobj(source, output)
                    target.chmod(member.mode & 0o777)


def resolve_release(version):
    base = f'https://api.github.com/repos/{REPO}/releases'
    if version:
        releases = [json.loads(download(base + '/tags/' + urllib.parse.quote(version, safe='')))]
    else:
        # Walk complete stable releases, rather than trusting the latest tag.
        releases = []
        for page in range(1, 11):
            batch = json.loads(download(f'{base}?per_page=100&page={page}'))
            releases.extend(batch)
            if len(batch) < 100:
                break
    for release in releases:
        if release['draft'] or (release['prerelease'] and not version):
            continue
        assets = {a['name']: a['browser_download_url'] for a in release['assets']}
        if 'release-manifest.json' not in assets:
            continue
        candidate = json.loads(download(assets['release-manifest.json']))
        if candidate.get('installer_version') != 1 and not version:
            continue
        manifest = manifest_valid(candidate)
        if manifest['version'] != release['tag_name']:
            raise ValueError('manifest version does not match release')
        return manifest
    raise ValueError('no complete compatible release found')


def prerequisites(args):
    if os.uname().sysname != 'Linux' or os.uname().machine != 'x86_64' or os.getuid() == 0:
        raise ValueError('requires Linux x86_64 and a normal, non-root user')
    missing = [name for name in ('podman', 'systemctl', 'make', 'newuidmap', 'newgidmap') if not shutil.which(name)]
    if shutil.which('podman') and subprocess.run(['podman', 'compose', 'version'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
        missing.append('Compose provider')
    if missing:
        manager = 'apt' if shutil.which('apt-get') else 'dnf' if shutil.which('dnf') else None
        if manager is None:
            raise ValueError('install Python 3.9+, rootless Podman, podman-compose, make and systemd using your distribution package manager')
        commands = [['sudo', 'apt-get', 'update'], ['sudo', 'apt-get', 'install', '-y', 'podman', 'podman-compose', 'uidmap', 'make', 'systemd']] if manager == 'apt' else [['sudo', 'dnf', 'install', '-y', 'podman', 'podman-compose', 'shadow-utils', 'make', 'systemd']]
        print('Package commands:\n' + '\n'.join(shlex.join(c) for c in commands), flush=True)
        choice = args.install_packages
        if choice is None:
            choice = yesno('Install missing prerequisites (--install-packages/--no-install-packages) with the sudo commands shown above?')
        if not choice:
            raise ValueError('missing prerequisites: ' + ', '.join(missing))
        for command in commands:
            run(command)
    info = json.loads(run(['podman', 'info', '--format', 'json'], capture_output=True).stdout)
    host = info['host']
    if not host['security']['rootless'] or host['cgroupVersion'] != 'v2' or host['cgroupManager'] != 'systemd':
        raise ValueError('Podman must be rootless with cgroup v2 and the systemd cgroup manager')
    run(['systemctl', '--user', 'show-environment'], stdout=subprocess.DEVNULL)
    run(['podman', 'unshare', 'true'])
    if run(['podman', 'compose', 'version'], capture_output=True).returncode != 0:
        raise ValueError('install podman-compose or a supported Compose provider')


def configure(root, args, manifest):
    path = root / '.env'
    existed = path.exists()
    repaired = existed and any(not (root / name).exists() for name in ('data/config/valkey.conf', 'data/config/catalog-key', 'data/config/catalog-hosts'))
    env = read_env(path)
    if path.exists():
        required = ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD', 'PUBLIC_ORIGIN', 'PORT', 'COOKIE_SECURE')
        if any(not env.get(key) for key in required):
            raise ValueError('incomplete .env; restore original credentials/configuration before retrying')
    if not path.exists() or getattr(args, 'action', None) == 'modify':
        say('The proxy listens on loopback. Choose its local port; use an HTTPS public origin when an external proxy terminates TLS.')
        port = field(args.port, 'Local port (--port)', env.get('PORT', '8080'), validate_port)
        origin = field(args.public_origin, 'Public origin (--public-origin)', env.get('PUBLIC_ORIGIN', f'http://localhost:{port}'), validate_origin)
        parsed = urllib.parse.urlsplit(origin)
        env.setdefault('POSTGRES_PASSWORD', secrets.token_hex(32))
        env.setdefault('VALKEY_PASSWORD', secrets.token_hex(32))
        env.update(PUBLIC_ORIGIN=origin.rstrip('/'), PORT=str(port), COOKIE_SECURE=str(parsed.scheme == 'https').lower())
    # Restrict secret alphabets to values safe in URLs, Valkey directives and dotenv.
    for key in ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD'):
        if not re.fullmatch('[A-Za-z0-9_-]+', env[key]):
            raise ValueError(f'{key} contains unsupported characters; preserve and configure manually')
    if REPORTER:
        REPORTER.secrets.update(env[key] for key in ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD'))
    env.setdefault('CATALOG_ENABLED', 'false')
    # Persist the source credentials before creating any derived secret files.
    write_env(path, env)
    config = root / 'data/config'
    config.mkdir(parents=True, exist_ok=True)
    config.parent.chmod(0o700)
    if env['CATALOG_ENABLED'] == 'false':
        for key, name in [('CATALOG_SSH_KEY_PATH', 'catalog-key'), ('CATALOG_KNOWN_HOSTS_PATH', 'catalog-hosts')]:
            placeholder = config / name
            if not placeholder.exists():
                atomic(placeholder, '', 0o644)
            env.setdefault(key, str(placeholder))
    valkey = config / 'valkey.conf'
    if valkey.exists() and f"requirepass {env['VALKEY_PASSWORD']}" not in valkey.read_text().splitlines():
        raise ValueError('valkey.conf differs from preserved credentials; restore or remove this derived file')
    if not valkey.exists():
        atomic(valkey, f"bind 0.0.0.0\nprotected-mode yes\nrequirepass {env['VALKEY_PASSWORD']}\nappendonly yes\nappendfsync everysec\ndir /data\nmaxmemory-policy noeviction\n", 0o644)
    env.update(APP_IMAGE=manifest['app_image'], TOOLCHAIN_IMAGE=manifest['toolchain_image'])
    env['SANDBOX_RUNTIME'] = str(root / '.dev/gvisor/current/runsc')
    env['PODMAN_SOCKET'] = str(Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}')) / 'j0coder-podman.sock')
    write_env(path, env)
    say(f"DONE — Configuration {'repaired using preserved credentials' if repaired else 'preserved' if existed and getattr(args, 'action', None) != 'modify' else 'saved'}\nOrigin: {env['PUBLIC_ORIGIN']}\nPort: {env['PORT']}\nSecure cookies: {env['COOKIE_SECURE']}")
    return env


def compose(root, *args, **kwargs):
    kwargs['env'] = {**os.environ, **read_env(root / '.env')}
    return run(['podman', 'compose', '-f', str(root / 'compose.yaml'), *args], cwd=root, **kwargs)


def bootstrap(root, args):
    state = compose(root, 'exec', '-T', 'api', 'api', 'bootstrap-status', capture_output=True).stdout.strip()
    if state == 'administrator-present':
        say('DONE — Existing administrator retained.')
        return
    if state != 'empty':
        raise ValueError('recovery required: existing users have no administrator')
    say('Username: 1–64 ASCII letters, digits, _ or -. Password: 12–256 bytes, single line. Password typing will not display characters.')
    username = field(args.admin_username, 'Administrator username (--admin-username)', 'admin', lambda value: value if re.fullmatch('[A-Za-z0-9_-]{1,64}', value) else (_ for _ in ()).throw(ValueError('invalid administrator username')))
    if args.admin_password_file:
        password = Path(args.admin_password_file).read_text().removesuffix('\n')
    else:
        while True:
            say('\nINPUT REQUIRED — Administrator password (or supply --admin-password-file)')
            try:
                with open('/dev/tty', 'w') as tty:
                    password = getpass.getpass('> Administrator password: ', stream=tty)
                    confirmation = getpass.getpass('> Confirm password: ', stream=tty)
            except OSError as error:
                raise OSError('Administrator password requires /dev/tty; supply --admin-password-file') from error
            if password != confirmation:
                say('Passwords do not match; try again.')
            elif not 12 <= len(password.encode()) <= 256 or '\n' in password or '\r' in password:
                say('Password must be 12–256 bytes and single-line; try again.')
            else:
                break
    if REPORTER:
        REPORTER.secrets.add(password)
    if not username or not re.fullmatch('[A-Za-z0-9_-]{1,64}', username) or not 12 <= len(password.encode()) <= 256 or '\n' in password or '\r' in password:
        raise ValueError('username must be 1–64 ASCII letters/digits/_/-; password must be 12–256 bytes and single-line')
    compose(root, 'exec', '-T', 'api', 'api', 'bootstrap-admin', username, '--if-empty', input=password + '\n')
    say(f'Administrator username: {username}')
    return username


def wait_ready(root, env):
    with progress('Waiting for readiness'):
        return _wait_ready(root, env)


def _wait_ready(root, env):
    for attempt in range(120):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{env['PORT']}/readyz", timeout=2) as response:
                if response.status == 200:
                    # podman-compose ps does not accept a service argument.
                    ids = compose(root, 'ps', '-q', capture_output=True).stdout.split()
                    containers = json.loads(run(['podman', 'inspect', *ids], capture_output=True).stdout) if ids else []
                    services = {}
                    for container in containers:
                        labels = container['Config'].get('Labels') or {}
                        service = labels.get('com.docker.compose.service') or labels.get('io.podman.compose.service')
                        services.setdefault(service, []).append(container)
                    # Check every configured service, including worker/editor/controller.
                    for service in ('postgres', 'valkey', 'api', 'catalog-controller', 'worker', 'editor', 'proxy'):
                        matches = services.get(service, [])
                        if len(matches) != 1:
                            raise ValueError(f'expected one container for service: {service}; found {len(matches)}')
                        state = matches[0]['State']
                        if not state['Running']:
                            raise ValueError(f'failed service: {service}')
                    return
        except (urllib.error.URLError, TimeoutError):
            pass
        time.sleep(1)
    raise ValueError('readiness timed out; inspect make status and make logs')


def supervise(root, env):
    # Controller death causes systemd to restart everything, refreshing bind-mounted sockets.
    controller = subprocess.Popen(['podman', '--runtime=' + env['SANDBOX_RUNTIME'], 'system', 'service', '--time=0', 'unix://' + env['PODMAN_SOCKET']], cwd=root)
    def stop(*_):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, stop)
    try:
        for _ in range(100):
            if controller.poll() is not None:
                raise ValueError('controller exited')
            if subprocess.run(['podman', '--remote', '--url', 'unix://' + env['PODMAN_SOCKET'], 'info'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(.1)
        else:
            raise ValueError('controller startup timed out')
        run(['bash', str(root / 'scripts/preflight.sh')], cwd=root, env={**os.environ, **env})
        compose(root, 'up', '-d', '--force-recreate')
        wait_ready(root, env)
        while controller.poll() is None:
            time.sleep(1)
        raise ValueError('controller exited')
    finally:
        compose(root, 'down')
        controller.terminate()
        controller.wait(timeout=15)


def autostart(root, args, env):
    choice = args.autostart
    if choice is not None:
        say('Reboot startup choice supplied: ' + ('yes' if choice else 'no'))
    if choice is None:
        choice = yesno('Enable reboot startup (--autostart)? The user service supervises the application.')
    if not choice:
        say('DONE — Reboot startup skipped (existing service settings retained).')
        return 'skipped; existing settings retained'
    if subprocess.run(['systemctl', '--user', 'is-active', '--quiet', 'j0coder.service']).returncode == 0:
        run(['systemctl', '--user', 'stop', 'j0coder.service'])
    compose(root, 'down')
    run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'stop'], cwd=root, env={**os.environ, **env})
    def unit_quote(value):
        return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'
    unit = Path.home() / '.config/systemd/user/j0coder.service'
    atomic(unit, '[Unit]\nDescription=j0coder Compose installation\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart=' + unit_quote(shutil.which('python3')) + ' ' + unit_quote(root / 'scripts/setup.py') + ' supervise\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=120\n\n[Install]\nWantedBy=default.target\n')
    run(['systemctl', '--user', 'daemon-reload'])
    run(['systemctl', '--user', 'enable', '--now', 'j0coder.service'])
    wait_ready(root, env)
    run(['systemctl', '--user', 'is-active', '--quiet', 'j0coder.service'])
    linger = run(['loginctl', 'show-user', str(os.getuid()), '-p', 'Linger', '--value'], capture_output=True).stdout.strip()
    if linger != 'yes':
        print('Reboot startup requires: sudo loginctl enable-linger ' + shlex.quote(getpass.getuser()))
        if yesno('Enable lingering with sudo? Lingering starts the user service before login.'):
            run(['sudo', 'loginctl', 'enable-linger', getpass.getuser()])
            linger = 'yes'
    result = 'enabled' if linger == 'yes' else 'user service enabled; awaiting lingering'
    say('DONE — Reboot startup: ' + result)
    return result


def stop_installation(root, env, disable=False, purge=False):
    unit = Path.home() / '.config/systemd/user/j0coder.service'
    if unit.exists():
        run(['systemctl', '--user', 'disable' if disable else 'stop', *(['--now'] if disable else []), 'j0coder.service'])
    compose(root, 'down', *(['--volumes'] if purge else []))
    run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'stop'], cwd=root, env={**os.environ, **env})
    if disable and unit.exists():
        unit.unlink()
        run(['systemctl', '--user', 'daemon-reload'])


def start_installation(root, env):
    if subprocess.run(['systemctl', '--user', 'is-enabled', '--quiet', 'j0coder.service']).returncode == 0:
        run(['systemctl', '--user', 'start', 'j0coder.service'])
    else:
        run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'start'], cwd=root, env={**os.environ, **env})
        run(['bash', str(root / 'scripts/preflight.sh')], cwd=root, env={**os.environ, **env})
        compose(root, 'up', '-d', '--force-recreate')
    wait_ready(root, env)


def upgrade(root, args):
    if not (root / '.deployment-installation').exists():
        raise ValueError('release upgrades require a deployment installation; update source checkouts with Git')
    manifest = resolve_release(args.version)
    # Validate every download before stopping the current installation.
    with tempfile.TemporaryDirectory(dir=root) as staging:
        bundle, runtime = Path(staging) / 'bundle', Path(staging) / 'runtime'
        archive(manifest['bundle'], bundle)
        archive(manifest['gvisor'], runtime)
        files = ('compose.yaml', 'Makefile', 'deploy/nginx.conf', 'scripts/setup.py', 'scripts/setup.sh', 'scripts/install.sh', 'scripts/preflight.sh', 'scripts/gvisor-controller.sh')
        if any(not (bundle / name).is_file() for name in files) or not os.access(runtime / 'runsc', os.X_OK) or not (runtime / 'gvisor-bin').is_dir():
            raise ValueError('incomplete release archives')
        for image in (manifest['app_image'], manifest['toolchain_image']):
            run(['podman', 'pull', image])
        stop_installation(root, read_env(root / '.env'))
        # Backups retain the previous release and configuration for recovery.
        backup = root / '.dev' / ('upgrade-backup-' + secrets.token_hex(6))
        backup.mkdir(parents=True)
        for name in (*files, '.env', '.release.json'):
            source = root / name
            if source.exists():
                target = backup / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
        current = root / '.dev/gvisor/current'
        if current.exists() or current.is_symlink():
            os.replace(current, backup / 'runtime')
        os.replace(runtime, current)
        atomic(current / '.complete', manifest['version'])
        for name in files:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            os.replace(bundle / name, target)
        atomic(root / '.release.json', json.dumps(manifest) + '\n')
        atomic(root / '.bundle-complete', manifest['version'])
        atomic(root / '.deployment-installation', manifest['version'])
        env = configure(root, args, manifest)
        print(f'Previous release backup: {backup}. Database migrations may require a database backup to roll back.')
        start_installation(root, env)


def uninstall(root, args, env):
    message = 'Delete persistent volumes and local configuration? Type delete' if args.purge else 'Uninstall services and preserve data? yes/no'
    if not args.yes and not (prompt(message, 'no') == 'delete' if args.purge else yesno(message)):
        print('Uninstall cancelled.')
        return
    stop_installation(root, env, disable=True, purge=args.purge)
    if args.purge:
        # Only remove installer-owned configuration, never a checkout or arbitrary directory.
        for name in ('.env', '.release.json', '.bundle-complete', '.deployment-installation'):
            (root / name).unlink(missing_ok=True)
        for name in ('data/config', '.dev/gvisor'):
            path = root / name
            if path.is_symlink():
                path.unlink()
            elif path.exists():
                shutil.rmtree(path)
    say('Services uninstalled. ' + ('Persistent volumes and configuration deleted.' if args.purge else 'Data, configuration, and installation files retained; make setup reinstalls.'))


def main():
    global REPORTER
    REPORTER = None
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', nargs='?', default='setup', choices=['setup', 'up', 'install', 'modify', 'upgrade', 'uninstall', 'down', 'status', 'logs', 'supervise'])
    parser.add_argument('--install-dir', default=os.environ.get('J0CODER_INSTALL_DIR', str(Path.home() / '.local/share/j0coder')))
    parser.add_argument('--version', default=os.environ.get('J0CODER_VERSION'))
    for key in ('public-origin', 'port', 'admin-username', 'admin-password-file'):
        parser.add_argument('--' + key, default=os.environ.get(key.replace('-', '_').upper()))
    parser.add_argument('--install-packages', action=argparse.BooleanOptionalAction, default=None)
    parser.add_argument('--autostart', action=argparse.BooleanOptionalAction, default=None)
    parser.add_argument('--action', dest='installer_action', choices=['install', 'modify', 'upgrade', 'uninstall'], help='downloadable installer operation')
    parser.add_argument('--purge', action='store_true', help='uninstall: delete persistent volumes and configuration')
    parser.add_argument('--verbose', action='store_true', help='stream diagnostic output; private logs are always saved for setup')
    parser.add_argument('--yes', action='store_true', help='confirm uninstall without a prompt')
    args = parser.parse_args()
    downloadable = args.action == 'install'
    if args.installer_action:
        args.action = args.installer_action
    if args.purge and args.action != 'uninstall':
        parser.error('--purge is only valid for uninstall')
    root = Path(args.install_dir).expanduser().resolve() if downloadable else ROOT
    root.mkdir(parents=True, exist_ok=True)
    os.umask(0o077)
    stage = 'initialization'
    try:
        if args.action not in ('supervise', 'status', 'logs'):
            REPORTER = Reporter(root, args.verbose)
            REPORTER.secrets.update(value for key, value in read_env(root / '.env').items() if 'PASSWORD' in key)
            pinned = json.loads((root / '.release.json').read_text()).get('version') if (root / '.release.json').exists() else None
            say(f"j0coder — {args.action}\nInstallation: {root}\nRelease: {args.version or pinned or 'latest compatible stable'}\nLog: {REPORTER.path}\nSetup pauses for missing answers. Enter accepts displayed defaults.")
        stage = 'installation lock'
        if args.action == 'supervise':
            # Supervision must start while onboarding holds its installation lock.
            stage = 'service supervision'
            env = read_env(root / '.env')
            # Stream redacted diagnostics to the journal without creating setup logs.
            REPORTER = Reporter(verbose=True)
            REPORTER.secrets.update(value for key, value in env.items() if 'PASSWORD' in key)
            supervise(root, env)
            return
        with open(root / '.setup.lock', 'a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            env = read_env(root / '.env')
            if args.port is not None:
                validate_port(args.port)
            if args.public_origin is not None:
                validate_origin(args.public_origin)
            if args.action in ('status', 'logs'):
                compose(root, 'ps' if args.action == 'status' else 'logs', *([] if args.action == 'status' else ['--tail=100']))
                return
            if args.action == 'down':
                subprocess.run(['systemctl', '--user', 'stop', 'j0coder.service'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                compose(root, 'down')
                run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'stop'], cwd=root, env={**os.environ, **env})
                return
            if args.action in ('up', 'modify', 'upgrade', 'uninstall'):
                if not env or not (root / '.release.json').exists():
                    raise ValueError('installation is not configured; run make setup first')
                stage = args.action
                if args.action == 'up' and args.version and args.version != json.loads((root / '.release.json').read_text())['version']:
                    raise ValueError('use make upgrade to select a different release')
                if args.action == 'uninstall':
                    uninstall(root, args, env)
                elif args.action == 'upgrade':
                    upgrade(root, args)
                else:
                    if args.action == 'modify':
                        manifest = manifest_valid(json.loads((root / '.release.json').read_text()))
                        env = configure(root, args, manifest)
                        stop_installation(root, env)
                    start_installation(root, env)
                    if args.action == 'modify' and args.autostart is None:
                        autostart(root, args, env)
                    if args.action == 'modify' and args.autostart is not None:
                        if args.autostart:
                            autostart(root, args, env)
                        else:
                            run(['systemctl', '--user', 'disable', '--now', 'j0coder.service'])
                            start_installation(root, env)
                return
            stage = 'prerequisites'
            REPORTER.begin(1, 'Prerequisites')
            prerequisites(args)
            REPORTER.done('Prerequisites checked')
            stage = 'release provisioning'
            REPORTER.begin(2, 'Release artifacts')
            state = root / '.release.json'
            manifest = manifest_valid(json.loads(state.read_text())) if state.exists() else resolve_release(args.version)
            say('Release ' + manifest['version'] + (' reused from saved pin' if state.exists() else ' selected'))
            if args.version and args.version != manifest['version']:
                raise ValueError('installation is pinned to ' + manifest['version'] + '; explicit upgrades are not supported')
            if not state.exists():
                atomic(state, json.dumps(manifest, indent=2) + '\n')
            if args.action == 'install':
                atomic(root / '.deployment-installation', manifest['version'])
            if (root / '.deployment-installation').exists() and not (root / '.bundle-complete').exists():
                with tempfile.TemporaryDirectory(dir=root) as staging:
                    archive(manifest['bundle'], Path(staging))
                    for name in ('compose.yaml', 'Makefile', 'deploy/nginx.conf', 'scripts/setup.py', 'scripts/setup.sh', 'scripts/install.sh', 'scripts/preflight.sh', 'scripts/gvisor-controller.sh'):
                        source = Path(staging) / name
                        if not source.is_file():
                            raise ValueError('incomplete deployment archive: ' + name)
                        target = root / name
                        target.parent.mkdir(parents=True, exist_ok=True)
                        os.replace(source, target)
                atomic(root / '.bundle-complete', manifest['version'])
                REPORTER.done('Deployment bundle downloaded')
            else:
                REPORTER.done('Deployment bundle reused' if (root / '.deployment-installation').exists() else 'Deployment bundle skipped for source checkout')
            runtime = root / '.dev/gvisor/current'
            if not (runtime / '.complete').exists():
                runtime.parent.mkdir(parents=True, exist_ok=True)
                with tempfile.TemporaryDirectory(dir=runtime.parent) as staging:
                    archive(manifest['gvisor'], Path(staging))
                    if not os.access(Path(staging) / 'runsc', os.X_OK) or not (Path(staging) / 'gvisor-bin').is_dir():
                        raise ValueError('incomplete gVisor archive')
                    if runtime.exists() or runtime.is_symlink():
                        # Preserve incomplete or previously built artifacts.
                        os.replace(runtime, runtime.parent / ('previous-' + secrets.token_hex(6)))
                    os.replace(staging, runtime)
                    atomic(runtime / '.complete', manifest['version'])
                REPORTER.done('Sandbox runtime downloaded')
            else:
                REPORTER.done('Sandbox runtime reused')
            if not os.access(runtime / 'runsc', os.X_OK) or not (runtime / 'gvisor-bin').is_dir():
                raise ValueError('gVisor archive lacks executable runsc or companion binaries')
            REPORTER.done('Release artifacts checked: ' + manifest['version'])
            stage = 'configuration'
            REPORTER.begin(3, 'Configure application')
            env = configure(root, args, manifest)
            stage = 'image pulls'
            REPORTER.begin(4, 'Image downloads')
            for image in (env['APP_IMAGE'], env['TOOLCHAIN_IMAGE']):
                if subprocess.run(['podman', 'image', 'exists', image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
                    say('Pulling image: ' + image)
                    run(['podman', 'pull', image])
                    REPORTER.done('Image downloaded: ' + image)
                else:
                    REPORTER.done('Image reused: ' + image)
            stage = 'controller and preflight'
            REPORTER.begin(5, 'Sandbox checks')
            active = subprocess.run(['systemctl', '--user', 'is-active', '--quiet', 'j0coder.service']).returncode == 0
            if not active:
                run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'start'], cwd=root, env={**os.environ, **env})
            run(['bash', str(root / 'scripts/preflight.sh')], cwd=root, env={**os.environ, **env})
            REPORTER.done('Sandbox checked')
            stage = 'Compose startup'
            REPORTER.begin(6, 'Application startup and readiness')
            compose(root, 'up', '-d', '--force-recreate')
            wait_ready(root, env)
            REPORTER.done('Application ready')
            stage = 'administrator onboarding'
            REPORTER.begin(7, 'Administrator onboarding')
            username = bootstrap(root, args)
            REPORTER.done('Administrator created' if username else 'Existing administrator retained')
            stage = 'reboot integration'
            REPORTER.begin(8, 'Reboot startup')
            reboot = autostart(root, args, env)
            say(f"Release: {manifest['version']}\nAdministrator: {username or 'existing administrator retained'}\nReboot startup: {reboot}\nLogin: {env['PUBLIC_ORIGIN']}\nInstallation: {root}\nLifecycle: make up | make down | make status | make logs")
    except (Exception, KeyboardInterrupt) as error:
        cause = str(error) or 'interrupted'
        details = ''
        if isinstance(error, subprocess.CalledProcessError):
            details = f'\nCommand: {shlex.join(map(str, error.cmd))}\nExit code: {error.returncode}'
        recovery = ['python3', str(root / 'scripts/setup.py'), args.action]
        if downloadable:
            recovery = ['sh', 'install.sh', '--action', args.action, '--install-dir', str(root)]
        version = args.version or (manifest['version'] if 'manifest' in locals() else None)
        if version:
            recovery += ['--version', version]
        message = f'{stage} failed: {cause}{details}'
        if REPORTER:
            message += '\nRecent diagnostics:\n' + '\n'.join(REPORTER.lines)
            message += f'\nFull log: {REPORTER.path}' if REPORTER.path else '\nJournal: journalctl --user -u j0coder.service'
            message = REPORTER.clean(message)
        if args.purge:
            recovery.append('--purge')
        for name in ('port', 'public_origin', 'admin_username', 'admin_password_file'):
            if getattr(args, name) is not None:
                recovery += ['--' + name.replace('_', '-'), getattr(args, name)]
        for name in ('install_packages', 'autostart'):
            if getattr(args, name) is not None:
                recovery.append('--' + ('' if getattr(args, name) else 'no-') + name.replace('_', '-'))
        if downloadable:
            recovery_text = 'curl -fsSL https://raw.githubusercontent.com/UltimateBoomer/j0coder/main/scripts/install.sh | sh -s -- ' + shlex.join(recovery[2:])
        else:
            recovery_text = shlex.join(recovery)
        if args.action == 'supervise':
            recovery_text = 'systemctl --user restart j0coder.service'
        message += '\nRecovery: ' + recovery_text
        if REPORTER:
            message = REPORTER.clean(message)
            REPORTER.write_log(message)
        raise SystemExit(message)
    finally:
        if REPORTER:
            if REPORTER.log is not None:
                REPORTER.log.close()
            REPORTER = None


if __name__ == '__main__':
    main()
