import argparse
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import setup


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='setup with spaces ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.args = argparse.Namespace(port='8080', public_origin='https://practice.example', version=None)
        self.manifest = dict(version='v1', installer_version=1, app_image='ghcr.io/example/app@sha256:' + 'a' * 64, toolchain_image='ghcr.io/example/toolchain@sha256:' + 'b' * 64, bundle=dict(sha256='c' * 64), gvisor=dict(sha256='d' * 64))

    def test_modify_preserves_secrets_and_updates_cookie(self):
        before = setup.configure(self.root, self.args, self.manifest)
        self.args.action = 'modify'
        self.args.port = '9090'
        self.args.public_origin = 'http://localhost:9090'
        after = setup.configure(self.root, self.args, self.manifest)
        for key in ('POSTGRES_PASSWORD', 'VALKEY_PASSWORD'):
            self.assertEqual(before[key], after[key])
        self.assertEqual(after['PORT'], '9090')
        self.assertEqual(after['COOKIE_SECURE'], 'false')
        self.args.port = '80'
        with self.assertRaises(ValueError):
            setup.configure(self.root, self.args, self.manifest)
        self.assertEqual(setup.read_env(self.root / '.env'), after)

    def test_up_requires_setup_and_never_installs(self):
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'up']), patch.object(setup, 'prerequisites') as prerequisites:
            with self.assertRaisesRegex(SystemExit, 'run make setup first'):
                setup.main()
            prerequisites.assert_not_called()
        setup.configure(self.root, self.args, self.manifest)
        (self.root / '.release.json').write_text(json.dumps(self.manifest))
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'up']), patch.object(setup, 'start_installation') as start, patch.object(setup, 'prerequisites') as prerequisites, patch.object(setup, 'bootstrap') as bootstrap, patch.object(setup, 'prompt', side_effect=AssertionError('unexpected prompt')):
            setup.main()
            start.assert_called_once()
            prerequisites.assert_not_called()
            bootstrap.assert_not_called()

    def test_uninstall_preserves_data_unless_purged(self):
        env = setup.configure(self.root, self.args, self.manifest)
        args = argparse.Namespace(purge=False, yes=True)
        with patch.object(setup, 'stop_installation') as stop:
            setup.uninstall(self.root, args, env)
            stop.assert_called_once_with(self.root, env, disable=True, purge=False)
            self.assertTrue((self.root / '.env').exists())
            args.purge = True
            unrelated = self.root / 'source.rs'
            unrelated.write_text('keep')
            setup.uninstall(self.root, args, env)
            self.assertFalse((self.root / '.env').exists())
            self.assertFalse((self.root / 'data/config').exists())
            self.assertTrue(unrelated.exists())

    def test_uninstall_cancellation_does_not_stop(self):
        with patch.object(setup, 'prompt', return_value='no'), patch.object(setup, 'stop_installation') as stop:
            setup.uninstall(self.root, argparse.Namespace(purge=True, yes=False), {})
            stop.assert_not_called()

    def test_upgrade_replaces_release_and_preserves_credentials(self):
        before = setup.configure(self.root, self.args, self.manifest)
        (self.root / '.deployment-installation').write_text('v1')
        (self.root / '.release.json').write_text(json.dumps(self.manifest))
        runtime = self.root / '.dev/gvisor/current'
        runtime.mkdir(parents=True)
        (runtime / 'runsc').write_text('old runtime')
        updated = dict(self.manifest, version='v2', app_image='ghcr.io/example/app@sha256:' + 'e' * 64)
        def extract(asset, destination):
            destination.mkdir(parents=True)
            if asset == updated['bundle']:
                for name in ('compose.yaml', 'Makefile', 'deploy/nginx.conf', 'scripts/setup.py', 'scripts/setup.sh', 'scripts/install.sh', 'scripts/preflight.sh', 'scripts/gvisor-controller.sh'):
                    target = destination / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text('new release')
            else:
                (destination / 'runsc').write_text('new runtime')
                (destination / 'runsc').chmod(0o755)
                (destination / 'gvisor-bin').mkdir()
        with patch.object(setup, 'resolve_release', return_value=updated), patch.object(setup, 'archive', side_effect=extract), patch.object(setup, 'run'), patch.object(setup, 'stop_installation') as stop, patch.object(setup, 'start_installation') as start:
            setup.upgrade(self.root, self.args)
            stop.assert_called_once()
            start.assert_called_once()
        after = setup.read_env(self.root / '.env')
        self.assertEqual(after['POSTGRES_PASSWORD'], before['POSTGRES_PASSWORD'])
        self.assertEqual(after['VALKEY_PASSWORD'], before['VALKEY_PASSWORD'])
        self.assertEqual(after['APP_IMAGE'], updated['app_image'])
        self.assertEqual(json.loads((self.root / '.release.json').read_text())['version'], 'v2')
        backup = next((self.root / '.dev').glob('upgrade-backup-*'))
        self.assertEqual((backup / 'runtime/runsc').read_text(), 'old runtime')
        self.assertEqual(setup.read_env(backup / '.env'), before)

    def test_upgrade_bad_archive_does_not_stop_or_change_pins(self):
        (self.root / '.deployment-installation').write_text('v1')
        (self.root / '.release.json').write_text(json.dumps(self.manifest))
        with patch.object(setup, 'resolve_release', return_value=self.manifest), patch.object(setup, 'archive', side_effect=ValueError('archive checksum mismatch')), patch.object(setup, 'stop_installation') as stop:
            with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                setup.upgrade(self.root, argparse.Namespace(version='v2'))
            stop.assert_not_called()
        self.assertEqual(json.loads((self.root / '.release.json').read_text()), self.manifest)

    def test_prompt_works_on_nonseekable_terminal(self):
        import os
        import pty
        import select
        import subprocess
        import sys
        master, slave = pty.openpty()
        def attach_terminal():
            import fcntl
            import termios
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        process = subprocess.Popen([sys.executable, '-c', "import setup; print('ANSWER=' + setup.prompt('Choice', 'no'))"], cwd=Path(__file__).parent, stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach_terminal)
        os.close(slave)
        try:
            self.assertTrue(select.select([master], [], [], 5)[0], 'prompt did not appear')
            output = b''
            while b'Choice [no]: ' not in output:
                self.assertTrue(select.select([master], [], [], 5)[0])
                output += os.read(master, 4096)
            self.assertIn(b'INPUT REQUIRED', output)
            os.write(master, b'yes\n')
            process.wait(timeout=5)
            while select.select([master], [], [], 0.1)[0]:
                try:
                    output += os.read(master, 4096)
                except OSError:
                    break
            self.assertEqual(process.returncode, 0, output.decode())
            self.assertIn(b'ANSWER=yes', output)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)

    def test_readiness_supports_provider_without_service_filter(self):
        from unittest.mock import MagicMock
        services = ('postgres', 'valkey', 'api', 'catalog-controller', 'worker', 'editor', 'proxy')
        containers = [dict(Config=dict(Labels={('com.docker.compose.service' if i % 2 else 'io.podman.compose.service'): service}), State=dict(Running=True)) for i, service in enumerate(services)]
        response = MagicMock()
        response.__enter__.return_value.status = 200
        with patch.object(setup.urllib.request, 'urlopen', return_value=response), patch.object(setup, 'compose', return_value=argparse.Namespace(stdout=' '.join(services))) as compose, patch.object(setup, 'run', return_value=argparse.Namespace(stdout=json.dumps(containers))) as inspect:
            setup.wait_ready(self.root, {'PORT': '8080'})
            compose.assert_called_once_with(self.root, 'ps', '-q', capture_output=True)
            containers[4]['State']['Running'] = False
            inspect.return_value.stdout = json.dumps(containers)
            with self.assertRaisesRegex(ValueError, 'failed service: worker'):
                setup.wait_ready(self.root, {'PORT': '8080'})
            containers.pop(4)
            inspect.return_value.stdout = json.dumps(containers)
            with self.assertRaisesRegex(ValueError, 'service: worker; found 0'):
                setup.wait_ready(self.root, {'PORT': '8080'})
            containers.append(containers[0])
            inspect.return_value.stdout = json.dumps(containers)
            with self.assertRaisesRegex(ValueError, 'service: postgres; found 2'):
                setup.wait_ready(self.root, {'PORT': '8080'})

    def test_repeat_and_repair_preserves_secrets(self):
        first = setup.configure(self.root, self.args, self.manifest)
        self.assertEqual(first['COOKIE_SECURE'], 'true')
        (self.root / 'data/config/valkey.conf').unlink()
        (self.root / 'data/config/catalog-key').unlink()
        second = setup.configure(self.root, self.args, self.manifest)
        self.assertEqual(first, second)
        self.assertIn(first['VALKEY_PASSWORD'], (self.root / 'data/config/valkey.conf').read_text())
        self.assertEqual(setup.read_env(self.root / '.env'), first)

    def test_incomplete_credentials_refused(self):
        (self.root / '.env').write_text('POSTGRES_PASSWORD=original\n')
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            setup.configure(self.root, self.args, self.manifest)
        self.assertEqual((self.root / '.env').read_text(), 'POSTGRES_PASSWORD=original\n')

    def test_origin_and_port_validation(self):
        for origin in ['https://example/path', 'ftp://example', 'https://user:pass@example', 'https://example?x=1']:
            self.args.public_origin = origin
            with self.assertRaises(ValueError):
                setup.configure(self.root, self.args, self.manifest)

    def test_incompatible_and_mutable_releases(self):
        setup.manifest_valid(self.manifest)
        self.manifest['installer_version'] = 2
        with self.assertRaises(ValueError):
            setup.manifest_valid(self.manifest)
        self.manifest['installer_version'] = 1
        self.manifest['app_image'] = 'ghcr.io/example/app:latest'
        with self.assertRaises(ValueError):
            setup.manifest_valid(self.manifest)

    def test_checksum_failure_does_not_extract(self):
        with patch.object(setup, 'download', return_value=b'corrupt'):
            with self.assertRaisesRegex(ValueError, 'checksum'):
                setup.archive(dict(url='https://example', sha256='a' * 64), self.root / 'output')
        self.assertFalse((self.root / 'output').exists())

    def test_unsafe_archive_rejected(self):
        for name, kind in [('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE), ('link', tarfile.SYMTYPE)]:
            stream = io.BytesIO()
            with tarfile.open(fileobj=stream, mode='w:gz') as tar:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = '/etc/passwd'
                tar.addfile(member)
            data = stream.getvalue()
            with patch.object(setup, 'download', return_value=data):
                with self.assertRaisesRegex(ValueError, 'unsafe'):
                    setup.archive(dict(url='https://example', sha256=hashlib.sha256(data).hexdigest()), self.root / 'output')

    def test_restored_database_never_prompts(self):
        result = argparse.Namespace(stdout='administrator-present\n')
        with patch.object(setup, 'compose', return_value=result), patch.object(setup, 'prompt', side_effect=AssertionError):
            setup.bootstrap(self.root, argparse.Namespace())
        result.stdout = 'recovery-required\n'
        with patch.object(setup, 'compose', return_value=result):
            with self.assertRaisesRegex(ValueError, 'recovery'):
                setup.bootstrap(self.root, argparse.Namespace())

    def test_password_file_sent_only_via_stdin(self):
        secret = self.root / 'password'
        secret.write_text('secret password 123\n')
        args = argparse.Namespace(admin_username='admin', admin_password_file=str(secret))
        with patch.object(setup, 'compose', return_value=argparse.Namespace(stdout='empty')) as command:
            setup.bootstrap(self.root, args)
        call = command.call_args
        self.assertEqual(call.kwargs['input'], 'secret password 123\n')
        self.assertNotIn('secret password 123', call.args)

    def test_latest_skips_incomplete_and_prerelease(self):
        releases = [dict(draft=False, prerelease=False, tag_name='v3', assets=[]), dict(draft=False, prerelease=True, tag_name='v2', assets=[]), dict(draft=False, prerelease=False, tag_name='v1', assets=[dict(name='release-manifest.json', browser_download_url='https://example/manifest')])]
        with patch.object(setup, 'download', side_effect=[json.dumps(releases).encode(), json.dumps(self.manifest).encode()]):
            self.assertEqual(setup.resolve_release(None)['version'], 'v1')

    def test_interruption_after_secret_persistence_resumes(self):
        original = setup.atomic
        def interrupted(path, content, mode=0o600):
            if path.name == 'valkey.conf':
                raise OSError('simulated interruption')
            return original(path, content, mode)
        with patch.object(setup, 'atomic', side_effect=interrupted):
            with self.assertRaises(OSError):
                setup.configure(self.root, self.args, self.manifest)
        before = setup.read_env(self.root / '.env')
        after = setup.configure(self.root, self.args, self.manifest)
        self.assertEqual(before['VALKEY_PASSWORD'], after['VALKEY_PASSWORD'])
        self.assertEqual(before['POSTGRES_PASSWORD'], after['POSTGRES_PASSWORD'])

    def test_compose_uses_saved_config_over_environment(self):
        setup.write_env(self.root / '.env', dict(APP_IMAGE='pinned', PORT='8080'))
        with patch.dict(setup.os.environ, dict(APP_IMAGE='mutable', PORT='9999')), patch.object(setup, 'run') as command:
            setup.compose(self.root, 'up', '-d')
        self.assertEqual(command.call_args.kwargs['env']['APP_IMAGE'], 'pinned')
        self.assertEqual(command.call_args.kwargs['env']['PORT'], '8080')

    def test_corrupt_derived_secret_is_refused(self):
        setup.configure(self.root, self.args, self.manifest)
        (self.root / 'data/config/valkey.conf').write_text('requirepass wrong\n')
        with self.assertRaisesRegex(ValueError, 'differs'):
            setup.configure(self.root, self.args, self.manifest)

    def test_supervisor_never_takes_install_lock(self):
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'supervise']), patch.object(setup, 'supervise') as service, patch.object(setup.fcntl, 'flock', side_effect=AssertionError('lock acquired')):
            setup.main()
        service.assert_called_once()

    def test_piped_installer_preserves_arguments_with_spaces(self):
        import subprocess
        stub = self.root / 'coordinator.py'
        stub.write_text('import sys, json; print(json.dumps(sys.argv[1:]))')
        fake_bin = self.root / 'bin'
        fake_bin.mkdir()
        curl = fake_bin / 'curl'
        curl.write_text('#!/bin/sh\ncp "$INSTALL_TEST_COORDINATOR" "$4"\n')
        curl.chmod(0o755)
        result = subprocess.run(['sh', str(Path(__file__).parent / 'install.sh'), '--install-dir', str(self.root / 'install with spaces'), '--no-install-packages', '--no-autostart'], input='', text=True, capture_output=True, check=True, env={**setup.os.environ, 'PATH': str(fake_bin) + ':' + setup.os.environ['PATH'], 'INSTALL_TEST_COORDINATOR': str(stub)})
        self.assertEqual(json.loads(result.stdout), ['install', '--install-dir', str(self.root / 'install with spaces'), '--no-install-packages', '--no-autostart'])

    def test_interrupted_install_resumes_and_reuses_cached_release(self):
        from contextlib import ExitStack
        args = ['setup.py', 'install', '--install-dir', str(self.root), '--public-origin', 'http://localhost:8080', '--port', '8080']
        def extract(asset, destination):
            if asset == self.manifest['bundle']:
                for name in ('compose.yaml', 'Makefile', 'deploy/nginx.conf', 'scripts/setup.py', 'scripts/setup.sh', 'scripts/install.sh', 'scripts/preflight.sh', 'scripts/gvisor-controller.sh'):
                    target = destination / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text('release content')
                    target.chmod(0o755)
            else:
                (destination / 'runsc').write_text('runtime')
                (destination / 'runsc').chmod(0o755)
                (destination / 'gvisor-bin').mkdir()
        with ExitStack() as stack:
            from contextlib import redirect_stdout
            output = stack.enter_context(redirect_stdout(io.StringIO()))
            stack.enter_context(patch('sys.argv', args))
            prerequisites = stack.enter_context(patch.object(setup, 'prerequisites'))
            resolve = stack.enter_context(patch.object(setup, 'resolve_release', return_value=self.manifest))
            stack.enter_context(patch.object(setup, 'run'))
            stack.enter_context(patch.object(setup.subprocess, 'run', return_value=argparse.Namespace(returncode=1)))
            stack.enter_context(patch.object(setup, 'compose'))
            stack.enter_context(patch.object(setup, 'wait_ready'))
            stack.enter_context(patch.object(setup, 'bootstrap'))
            autostart = stack.enter_context(patch.object(setup, 'autostart'))
            with patch.object(setup, 'archive', side_effect=OSError('interrupted download')):
                with self.assertRaisesRegex(SystemExit, 'release provisioning failed'):
                    setup.main()
            self.assertFalse((self.root / '.bundle-complete').exists())
            with patch.object(setup, 'archive', side_effect=extract) as archives:
                setup.main()
                self.assertEqual(archives.call_count, 2)
                archives.reset_mock()
                setup.main()
                archives.assert_not_called()
            resolve.assert_called_once()
            self.assertIsNone(prerequisites.call_args.args[0].install_packages)
            self.assertIsNone(autostart.call_args.args[1].autostart)
            self.assertTrue((self.root / '.dev/gvisor/current/.complete').exists())
            transcript = output.getvalue()
            completed = transcript.split('[1/8] Prerequisites')[-1]
            positions = [completed.index(f'[{number}/8]') for number in range(2, 9)]
            self.assertEqual(positions, sorted(positions))
            self.assertIn('Deployment bundle reused', completed)
            self.assertIn('Sandbox runtime reused', completed)

    def test_configuration_always_prompts_for_missing_values(self):
        self.args.port = None
        self.args.public_origin = None
        with patch.object(setup, 'prompt', side_effect=['8080', 'http://localhost:8080']) as prompt:
            env = setup.configure(self.root, self.args, self.manifest)
        self.assertEqual(prompt.call_count, 2)
        self.assertEqual(env['PUBLIC_ORIGIN'], 'http://localhost:8080')

    def test_missing_terminal_does_not_silently_default_configuration(self):
        self.args.port = None
        self.args.public_origin = None
        with patch('builtins.open', side_effect=OSError('no controlling terminal')):
            with self.assertRaisesRegex(OSError, 'no controlling terminal'):
                setup.configure(self.root, self.args, self.manifest)
        self.assertFalse((self.root / '.env').exists())

    def test_setup_keeps_interactive_choices_unspecified(self):
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'setup']), patch.object(setup, 'prerequisites', side_effect=ValueError('prerequisites reached')) as prerequisites:
            with self.assertRaisesRegex(SystemExit, 'prerequisites reached'):
                setup.main()
        self.assertIsNone(prerequisites.call_args.args[0].install_packages)
        self.assertIsNone(prerequisites.call_args.args[0].autostart)

    def test_shell_entry_has_no_terminal_guard(self):
        import subprocess
        result = subprocess.run(['sh', str(Path(__file__).parent / 'setup.sh'), '--help'], input='', text=True, capture_output=True, start_new_session=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn('--install-packages', result.stdout)

    def test_lifecycle_commands_do_not_prompt(self):
        for action in ('status', 'logs', 'down'):
            with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', action]), patch.object(setup, 'prompt', side_effect=AssertionError('unexpected prompt')), patch.object(setup, 'compose') as compose, patch.object(setup, 'run'), patch.object(setup.subprocess, 'run'):
                setup.main()
                compose.assert_called_once()

    def test_missing_packages_prompt_before_normal_sudo(self):
        args = argparse.Namespace(install_packages=None)
        info = argparse.Namespace(returncode=0, stdout=json.dumps(dict(host=dict(security=dict(rootless=True), cgroupVersion='v2', cgroupManager='systemd'))))
        with patch.object(setup.os, 'uname', return_value=argparse.Namespace(sysname='Linux', machine='x86_64')), patch.object(setup.os, 'getuid', return_value=1000), patch.object(setup.shutil, 'which', side_effect=lambda name: None if name == 'newuidmap' else '/bin/' + name), patch.object(setup.subprocess, 'run', return_value=info), patch.object(setup, 'run', return_value=info) as run, patch.object(setup, 'prompt', return_value='yes') as prompt:
            setup.prerequisites(args)
        prompt.assert_called_once()
        self.assertEqual(run.call_args_list[0].args[0], ['sudo', 'apt-get', 'update'])
        self.assertEqual(run.call_args_list[1].args[0][:2], ['sudo', 'apt-get'])

    def reporter(self, verbose=False):
        reporter = setup.Reporter(self.root, verbose)
        self.addCleanup(reporter.log.close)
        self.addCleanup(setattr, setup, 'REPORTER', None)
        setup.REPORTER = reporter
        return reporter

    def test_private_logs_redaction_and_private_capture(self):
        import contextlib
        import sys
        reporter = self.reporter(True)
        reporter.secrets.add('sensitive-password')
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            setup.run([sys.executable, '-c', "print('sensitive-password')"])
            result = setup.run([sys.executable, '-c', "import sys; print('private-inspection'); print('private-stderr', file=sys.stderr)"], capture_output=True)
        self.assertEqual(result.stdout, 'private-inspection\n')
        self.assertNotIn('sensitive-password', output.getvalue())
        self.assertNotIn('private-inspection', reporter.path.read_text())
        self.assertNotIn('private-stderr', reporter.path.read_text())
        self.assertIn('[REDACTED]', reporter.path.read_text())
        self.assertEqual(reporter.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(reporter.path.parent.stat().st_mode & 0o777, 0o700)

    def test_summary_suppresses_commands_and_failure_excerpt_is_bounded(self):
        import contextlib
        import sys
        reporter = self.reporter()
        with contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaises(setup.subprocess.CalledProcessError) as failure:
                setup.run([sys.executable, '-c', "import sys; [print('line-' + str(i)) for i in range(40)]; sys.exit(7)"])
        self.assertEqual(output.getvalue(), '')
        self.assertEqual(failure.exception.returncode, 7)
        self.assertEqual(len(reporter.lines), 30)
        self.assertEqual(reporter.lines[0], 'line-10')

    def test_verbose_streams_before_command_finishes(self):
        import contextlib
        import sys
        import threading
        reporter = self.reporter(True)
        appeared = threading.Event()
        class Sink(io.StringIO):
            def write(self, value):
                if 'streamed' in value:
                    appeared.set()
                return super().write(value)
        with contextlib.redirect_stdout(Sink()):
            worker = threading.Thread(target=setup.run, args=([sys.executable, '-c', "import time; print('streamed', flush=True); time.sleep(.5)"],))
            worker.start()
            self.assertTrue(appeared.wait(.4))
            self.assertTrue(worker.is_alive())
            worker.join(2)

    def test_field_retries_only_invalid_answer_and_yesno_aliases(self):
        with patch.object(setup, 'prompt', side_effect=['80', '8081']) as prompt:
            self.assertEqual(setup.field(None, 'Local port', '8080', setup.validate_port), '8081')
            self.assertEqual(prompt.call_count, 2)
        with patch.object(setup, 'prompt', side_effect=['perhaps', 'Y']) as prompt:
            self.assertTrue(setup.yesno('Choice'))
            self.assertEqual(prompt.call_count, 2)
        with patch.object(setup, 'prompt', return_value='N'):
            self.assertFalse(setup.yesno('Choice'))

    def terminal_session(self, code, exchanges):
        import os
        import pty
        import select
        import subprocess
        import sys
        import time
        master, slave = pty.openpty()
        def attach():
            import fcntl
            import termios
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        process = subprocess.Popen([sys.executable, '-c', code], cwd=Path(__file__).parent, stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach)
        os.close(slave)
        output = b''
        cursor = 0
        try:
            for marker, answer in exchanges:
                deadline = time.monotonic() + 5
                while marker not in output[cursor:]:
                    self.assertLess(time.monotonic(), deadline, output.decode())
                    if select.select([master], [], [], .1)[0]:
                        output += os.read(master, 4096)
                cursor = len(output)
                os.write(master, answer)
            process.wait(timeout=5)
            while select.select([master], [], [], .1)[0]:
                try:
                    output += os.read(master, 4096)
                except OSError:
                    break
            self.assertEqual(process.returncode, 0, output.decode())
            return output.decode()
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)

    def test_terminal_defaults_and_invalid_answers(self):
        output = self.terminal_session("import setup; print('RESULT=' + str(setup.yesno('Choice'))); print('PORT=' + setup.field(None, 'Local port', '8080', setup.validate_port))", [(b'> Choice', b'maybe\n'), (b'> Choice', b'Y\n'), (b'> Local port', b'80\n'), (b'> Local port', b'\n')])
        self.assertIn('Invalid answer', output)
        self.assertIn('RESULT=True', output)
        self.assertIn('PORT=8080', output)

    def test_terminal_password_hidden_and_mismatch_retried(self):
        code = "import setup, argparse; from pathlib import Path; setup.compose=lambda *a, **k: argparse.Namespace(stdout='empty'); setup.bootstrap(Path('.'), argparse.Namespace(admin_username='admin', admin_password_file=None))"
        output = self.terminal_session(code, [(b'> Administrator password:', b'hidden-secret-123\n'), (b'> Confirm password:', b'different-secret-456\n'), (b'> Administrator password:', b'hidden-secret-123\n'), (b'> Confirm password:', b'hidden-secret-123\n')])
        self.assertIn('Passwords do not match', output)
        self.assertNotIn('hidden-secret-123', output)
        self.assertNotIn('different-secret-456', output)
        self.assertIn('Administrator username: admin', output)

    def test_readiness_elapsed_and_timeout(self):
        import contextlib
        with patch.object(setup.urllib.request, 'urlopen', side_effect=setup.urllib.error.URLError('unready')), patch.object(setup.time, 'sleep'), patch.object(setup, 'progress', wraps=setup.progress) as progress, contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaisesRegex(ValueError, 'readiness timed out'):
                setup.wait_ready(self.root, {'PORT': '8080'})
        progress.assert_called_once_with('Waiting for readiness')

    def test_failure_recovery_retains_install_path_and_version(self):
        args = ['setup.py', 'install', '--install-dir', str(self.root), '--version', 'v1']
        with patch('sys.argv', args), patch.object(setup, 'prerequisites', side_effect=ValueError('readable cause')):
            with self.assertRaises(SystemExit) as failure:
                setup.main()
        message = str(failure.exception)
        self.assertIn('prerequisites failed: readable cause', message)
        self.assertIn('--install-dir', message)
        self.assertIn(str(self.root), message)
        self.assertIn('--version v1', message)
        self.assertIn('Full log:', message)
        self.assertNotIn('Persistent data retained', message)

    def test_explicit_stdout_suppression_never_logs_environment(self):
        import sys
        reporter = self.reporter(True)
        setup.run([sys.executable, '-c', "print('UNRELATED_TOKEN=private-token')"], stdout=setup.subprocess.DEVNULL)
        self.assertNotIn('private-token', reporter.path.read_text())

    def test_runner_drains_unterminated_output_without_deadlock(self):
        import sys
        reporter = self.reporter()
        setup.run([sys.executable, '-c', "import sys; sys.stdout.write('partial'); sys.stdout.flush(); sys.stderr.write('x'*1000000); sys.stderr.flush(); print('done')"])
        self.assertIn('partialdone', reporter.path.read_text())

    def test_field_terminal_eof_is_not_retried(self):
        with patch.object(setup, 'prompt', side_effect=ValueError('terminal input closed')) as prompt:
            with self.assertRaisesRegex(ValueError, 'terminal input closed'):
                setup.field(None, 'Local port', '8080', setup.validate_port)
            prompt.assert_called_once()

    def test_elapsed_progress_uses_ten_second_intervals(self):
        import threading
        appeared = threading.Event()
        with patch.object(setup, 'say', side_effect=lambda value: appeared.set()):
            with setup.progress('Work'):
                self.assertFalse(appeared.wait(.1))
                self.assertTrue(appeared.wait(11))

    def test_actual_piped_shell_installer_prompts_on_controlling_terminal(self):
        import shlex
        coordinator = self.root / 'coordinator.py'
        script_dir = str(Path(__file__).resolve().parent)
        coordinator.write_text("import sys; sys.path.insert(0, " + repr(script_dir) + "); import setup; print('PIPE-ANSWER=' + setup.prompt('Piped choice', 'default'))")
        fake_bin = self.root / 'bin'
        fake_bin.mkdir()
        curl = fake_bin / 'curl'
        curl.write_text('#!/bin/sh\ncp ' + shlex.quote(str(coordinator)) + ' "$4"\n')
        curl.chmod(0o755)
        installer = Path(__file__).parent.resolve() / 'install.sh'
        command = 'cat ' + shlex.quote(str(installer)) + ' | sh -s -- --no-install-packages --no-autostart'
        code = 'import os, subprocess; os.environ["PATH"] = ' + repr(str(fake_bin) + ':' + setup.os.environ['PATH']) + '; raise SystemExit(subprocess.call(' + repr(command) + ', shell=True))'
        output = self.terminal_session(code, [(b'> Piped choice [default]:', b'\n')])
        self.assertIn('PIPE-ANSWER=default', output)
        self.assertIn('Downloading setup coordinator', output)

    def test_failure_redacts_diagnostics_and_preserves_purge_recovery(self):
        setup.configure(self.root, self.args, self.manifest)
        (self.root / '.release.json').write_text(json.dumps(self.manifest))
        secret = setup.read_env(self.root / '.env')['POSTGRES_PASSWORD']
        def fail(*args, **kwargs):
            setup.REPORTER.diagnostic('diagnostic ' + secret)
            raise setup.subprocess.CalledProcessError(9, ['fake-command', secret])
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'uninstall', '--purge', '--yes']), patch.object(setup, 'stop_installation', side_effect=fail):
            with self.assertRaises(SystemExit) as failure:
                setup.main()
        message = str(failure.exception)
        self.assertNotIn(secret, message)
        self.assertIn('--purge', message)
        self.assertIn('Exit code: 9', message)
        self.assertIn('diagnostic [REDACTED]', message)
        self.assertNotIn(secret, next((self.root / '.dev/setup-logs').glob('*.log')).read_text())

    def test_corrupt_pin_has_structured_failure_and_closed_log(self):
        (self.root / '.release.json').write_text('{broken')
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'setup']):
            with self.assertRaises(SystemExit) as failure:
                setup.main()
        self.assertIn('initialization failed', str(failure.exception))
        self.assertIn('Full log:', str(failure.exception))
        self.assertIn('Recovery:', str(failure.exception))
        self.assertIsNone(setup.REPORTER)

    def test_supervision_redacts_streamed_credentials_without_setup_logs(self):
        import contextlib
        import sys
        setup.write_env(self.root / '.env', {'POSTGRES_PASSWORD': 'journal-db-secret', 'VALKEY_PASSWORD': 'journal-queue-secret'})
        def service(root, env):
            setup.run([sys.executable, '-c', "import sys; print('DATABASE_URL=postgres://journal-db-secret@postgres'); print('VALKEY_URL=redis://journal-queue-secret@valkey', file=sys.stderr)"])
            private = setup.run([sys.executable, '-c', "print('private-inspection')"], capture_output=True)
            self.assertEqual(private.stdout, 'private-inspection\n')
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'supervise']), patch.object(setup, 'supervise', side_effect=service), patch.object(setup, 'prompt', side_effect=AssertionError('unexpected prompt')), contextlib.redirect_stdout(io.StringIO()) as output:
            setup.main()
        text = output.getvalue()
        self.assertIn('DATABASE_URL=postgres://[REDACTED]@postgres', text)
        self.assertIn('VALKEY_URL=redis://[REDACTED]@valkey', text)
        for private in ('journal-db-secret', 'journal-queue-secret', 'private-inspection', 'INPUT REQUIRED'):
            self.assertNotIn(private, text)
        self.assertFalse((self.root / '.dev/setup-logs').exists())
        self.assertIsNone(setup.REPORTER)

    def test_supervision_failure_redacts_credentials_and_points_to_journal(self):
        setup.write_env(self.root / '.env', {'POSTGRES_PASSWORD': 'journal-db-secret'})
        def service(*args):
            setup.REPORTER.diagnostic('error journal-db-secret')
            raise setup.subprocess.CalledProcessError(17, ['podman', 'journal-db-secret'])
        with patch.object(setup, 'ROOT', self.root), patch('sys.argv', ['setup.py', 'supervise']), patch.object(setup, 'supervise', side_effect=service):
            with self.assertRaises(SystemExit) as failure:
                setup.main()
        text = str(failure.exception)
        self.assertNotIn('journal-db-secret', text)
        self.assertIn('error [REDACTED]', text)
        self.assertIn('Exit code: 17', text)
        self.assertIn('Journal: journalctl --user -u j0coder.service', text)
        self.assertIn('Recovery: systemctl --user restart j0coder.service', text)
        self.assertIsNone(setup.REPORTER)


if __name__ == '__main__':
    unittest.main()
