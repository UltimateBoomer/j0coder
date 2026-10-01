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
import subprocess
import tarfile
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

REPO = 'UltimateBoomer/j0coder'
ROOT = Path(__file__).resolve().parent.parent


def run(args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def download(url):
    if urllib.parse.urlsplit(url).scheme != 'https':
        raise ValueError('downloads require HTTPS')
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


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
    with open('/dev/tty', 'r+') as tty:
        tty.write(message + (f' [{default}]' if default else '') + ': ')
        tty.flush()
        answer = tty.readline()
        if not answer:
            raise ValueError('terminal input closed')
        return answer.strip() or default


def interactive():
    try:
        with open('/dev/tty', 'r+'):
            return True
    except OSError:
        return False


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
        if choice is None and interactive():
            choice = prompt('Install missing prerequisites? yes/no', 'no') == 'yes'
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
    env = read_env(path)
    if path.exists():
        required = ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD', 'PUBLIC_ORIGIN', 'PORT', 'COOKIE_SECURE')
        if any(not env.get(key) for key in required):
            raise ValueError('incomplete .env; restore original credentials/configuration before retrying')
    else:
        port = args.port or (prompt('Local port', '8080') if interactive() else '8080')
        origin = args.public_origin or (prompt('Public origin', f'http://localhost:{port}') if interactive() else f'http://localhost:{port}')
        parsed = urllib.parse.urlsplit(origin)
        if parsed.scheme not in ('http', 'https') or not parsed.hostname or parsed.username or parsed.password or parsed.path not in ('', '/') or parsed.query or parsed.fragment:
            raise ValueError('public origin must be an HTTP(S) origin without a path or credentials')
        if not str(port).isdigit() or not 1024 <= int(port) <= 65535:
            raise ValueError('local port must be between 1024 and 65535')
        env = dict(POSTGRES_PASSWORD=secrets.token_hex(32), VALKEY_PASSWORD=secrets.token_hex(32), PUBLIC_ORIGIN=origin.rstrip('/'), PORT=str(port), COOKIE_SECURE=str(parsed.scheme == 'https').lower())
    # Restrict secret alphabets to values safe in URLs, Valkey directives and dotenv.
    for key in ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD'):
        if not re.fullmatch('[A-Za-z0-9_-]+', env[key]):
            raise ValueError(f'{key} contains unsupported characters; preserve and configure manually')
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
    return env


def compose(root, *args, **kwargs):
    kwargs['env'] = {**os.environ, **read_env(root / '.env')}
    return run(['podman', 'compose', '-f', str(root / 'compose.yaml'), *args], cwd=root, **kwargs)


def bootstrap(root, args):
    state = compose(root, 'exec', '-T', 'api', 'api', 'bootstrap-status', capture_output=True).stdout.strip()
    if state == 'administrator-present':
        print('Existing administrator retained.')
        return
    if state != 'empty':
        raise ValueError('recovery required: existing users have no administrator')
    username = args.admin_username or (prompt('Administrator username', 'admin') if interactive() else None)
    if args.admin_password_file:
        password = Path(args.admin_password_file).read_text().removesuffix('\n')
    elif interactive():
        with open('/dev/tty', 'w') as tty:
            password = getpass.getpass('Administrator password: ', stream=tty)
            if password != getpass.getpass('Confirm password: ', stream=tty):
                raise ValueError('passwords do not match')
    else:
        raise ValueError('unattended onboarding requires ADMIN_USERNAME and ADMIN_PASSWORD_FILE')
    if not username or not re.fullmatch('[A-Za-z0-9_-]{1,64}', username) or not 12 <= len(password.encode()) <= 256 or '\n' in password or '\r' in password:
        raise ValueError('username must be 1–64 ASCII letters/digits/_/-; password must be 12–256 bytes and single-line')
    compose(root, 'exec', '-T', 'api', 'api', 'bootstrap-admin', username, '--if-empty', input=password + '\n')
    print(f'Administrator username: {username}')


def wait_ready(root, env):
    for _ in range(120):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{env['PORT']}/readyz", timeout=2) as response:
                if response.status == 200:
                    # Check every configured service, including worker/editor/controller.
                    for service in ('postgres', 'valkey', 'api', 'catalog-controller', 'worker', 'editor', 'proxy'):
                        ids = compose(root, 'ps', '-q', service, capture_output=True).stdout.split()
                        if len(ids) != 1:
                            raise ValueError(f'missing service: {service}')
                        state = json.loads(run(['podman', 'inspect', ids[0]], capture_output=True).stdout)[0]['State']
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
    if choice is None and interactive():
        choice = prompt('Enable reboot startup? yes/no', 'no') == 'yes'
    if not choice:
        return
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
        if interactive() and prompt('Enable lingering with sudo? yes/no', 'no') == 'yes':
            run(['sudo', 'loginctl', 'enable-linger', getpass.getuser()])
            linger = 'yes'
    print('Reboot startup fully configured.' if linger == 'yes' else 'User service enabled; reboot startup awaits lingering.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', nargs='?', default='up', choices=['up', 'install', 'down', 'status', 'logs', 'supervise'])
    parser.add_argument('--install-dir', default=os.environ.get('J0CODER_INSTALL_DIR', str(Path.home() / '.local/share/j0coder')))
    parser.add_argument('--version', default=os.environ.get('J0CODER_VERSION'))
    for key in ('public-origin', 'port', 'admin-username', 'admin-password-file'):
        parser.add_argument('--' + key, default=os.environ.get(key.replace('-', '_').upper()))
    parser.add_argument('--install-packages', action=argparse.BooleanOptionalAction, default=None)
    parser.add_argument('--autostart', action=argparse.BooleanOptionalAction, default=None)
    args = parser.parse_args()
    root = Path(args.install_dir).expanduser().resolve() if args.action == 'install' else ROOT
    root.mkdir(parents=True, exist_ok=True)
    os.umask(0o077)
    stage = 'installation lock'
    try:
        if args.action == 'supervise':
            # Supervision must start while onboarding holds its installation lock.
            stage = 'service supervision'
            supervise(root, read_env(root / '.env'))
            return
        with open(root / '.setup.lock', 'a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            env = read_env(root / '.env')
            if args.action in ('status', 'logs'):
                compose(root, 'ps' if args.action == 'status' else 'logs', *([] if args.action == 'status' else ['--tail=100']))
                return
            if args.action == 'down':
                subprocess.run(['systemctl', '--user', 'stop', 'j0coder.service'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                compose(root, 'down')
                run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'stop'], cwd=root, env={**os.environ, **env})
                return
            stage = 'prerequisites'
            if not interactive():
                # Ordinary starts work in terminals without a controlling /dev/tty.
                # Privileged setup and reboot integration remain explicit opt-ins.
                if args.install_packages is None:
                    args.install_packages = False
                if args.autostart is None:
                    args.autostart = False
            prerequisites(args)
            stage = 'release provisioning'
            state = root / '.release.json'
            manifest = manifest_valid(json.loads(state.read_text())) if state.exists() else resolve_release(args.version)
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
            if not os.access(runtime / 'runsc', os.X_OK) or not (runtime / 'gvisor-bin').is_dir():
                raise ValueError('gVisor archive lacks executable runsc or companion binaries')
            stage = 'configuration'
            env = configure(root, args, manifest)
            stage = 'image pulls'
            for image in (env['APP_IMAGE'], env['TOOLCHAIN_IMAGE']):
                if subprocess.run(['podman', 'image', 'exists', image]).returncode != 0:
                    run(['podman', 'pull', image])
            stage = 'controller and preflight'
            active = subprocess.run(['systemctl', '--user', 'is-active', '--quiet', 'j0coder.service']).returncode == 0
            if not active:
                run(['bash', str(root / 'scripts/gvisor-controller.sh'), 'start'], cwd=root, env={**os.environ, **env})
            run(['bash', str(root / 'scripts/preflight.sh')], cwd=root, env={**os.environ, **env})
            stage = 'Compose startup'
            compose(root, 'up', '-d', '--force-recreate')
            wait_ready(root, env)
            stage = 'administrator onboarding'
            bootstrap(root, args)
            stage = 'reboot integration'
            autostart(root, args, env)
            print(f"Login: {env['PUBLIC_ORIGIN']}\nInstallation: {root}\nLifecycle: make up | make down | make status | make logs")
    except (Exception, KeyboardInterrupt) as error:
        raise SystemExit(f'{stage} failed: {error}. Persistent data retained; rerun make up to resume.')


if __name__ == '__main__':
    main()
