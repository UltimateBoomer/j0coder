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
        args = ['setup.py', 'install', '--install-dir', str(self.root), '--no-install-packages', '--no-autostart', '--public-origin', 'http://localhost:8080', '--port', '8080']
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
            stack.enter_context(patch('sys.argv', args))
            stack.enter_context(patch.object(setup, 'interactive', return_value=False))
            stack.enter_context(patch.object(setup, 'prerequisites'))
            resolve = stack.enter_context(patch.object(setup, 'resolve_release', return_value=self.manifest))
            stack.enter_context(patch.object(setup, 'run'))
            stack.enter_context(patch.object(setup.subprocess, 'run', return_value=argparse.Namespace(returncode=1)))
            stack.enter_context(patch.object(setup, 'compose'))
            stack.enter_context(patch.object(setup, 'wait_ready'))
            stack.enter_context(patch.object(setup, 'bootstrap'))
            stack.enter_context(patch.object(setup, 'autostart'))
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
            self.assertTrue((self.root / '.dev/gvisor/current/.complete').exists())


if __name__ == '__main__':
    unittest.main()
