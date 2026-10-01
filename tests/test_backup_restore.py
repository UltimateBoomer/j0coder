"""Exercise backup/restore safety with a fake Podman engine and disposable files."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
FAKE_PODMAN = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
state = pathlib.Path(os.environ['PODMAN_TEST_STATE'])
with (state / 'calls').open('a') as log:
    log.write(json.dumps(args) + '\n')
if args[0] == 'compose':
    sys.exit(0)
assert args[0] == 'volume', args
action, name = args[1:3]
volume = state / name
if action == 'exists':
    sys.exit(int(os.environ.get('PODMAN_EXISTS_ERROR', '0')) or (0 if volume.exists() else 1))
if action == 'export':
    if os.environ.get('PODMAN_EXPORT_ERROR'):
        sys.exit(125)
    pathlib.Path(args[4]).write_bytes(volume.read_bytes())
elif action == 'create':
    with volume.open('xb'):
        pass
elif action == 'import':
    volume.write_bytes(pathlib.Path(args[3]).read_bytes())
else:
    raise AssertionError(args)
'''


class BackupRestoreTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='j0coder-backup-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.state = self.root / 'engine'
        self.state.mkdir()
        binaries = self.root / 'bin'
        binaries.mkdir()
        executable = binaries / 'podman'
        executable.write_text(FAKE_PODMAN)
        executable.chmod(0o755)
        self.env = {**os.environ, 'PATH': f'{binaries}:{os.environ["PATH"]}',
                    'PODMAN_TEST_STATE': str(self.state)}
        self.source = self.checkout('source', configured=True)
        self.target = self.checkout('target')
        self.backup = self.root / 'backup'
        (self.state / 'locoder_postgres').write_bytes(b'accounts drafts submissions catalog revisions')
        (self.state / 'locoder_valkey').write_bytes(b'pending execution queue')

    def checkout(self, name, configured=False):
        root = self.root / name
        (root / 'scripts').mkdir(parents=True)
        for script in ('backup.sh', 'restore.sh'):
            shutil.copy2(ROOT / 'scripts' / script, root / 'scripts' / script)
        (root / 'compose.yaml').write_text('name: j0coder\n')
        (root / 'compose.dev.yaml').write_text('name: j0coder\n')
        (root / 'Cargo.lock').write_text('lockfile\n')
        (root / 'web').mkdir()
        (root / 'web/package-lock.json').write_text('{}\n')
        if configured:
            (root / '.env').write_text('POSTGRES_PASSWORD=preserved\n')
            (root / 'data/config').mkdir(parents=True)
            (root / 'data/config/valkey.conf').write_text('requirepass preserved\n')
        return root

    def run_script(self, root, script, *args, **env):
        return subprocess.run(['bash', str(root / 'scripts' / script), *map(str, args)],
                              env={**self.env, **env}, capture_output=True, text=True)

    def calls(self):
        log = self.state / 'calls'
        return [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []

    def make_backup(self, *args, **env):
        return self.run_script(self.source, 'backup.sh', '--project-name', 'locoder',
                               *args, self.backup, **env)

    def test_legacy_backup_restores_new_volumes_without_touching_source(self):
        result = self.make_backup('--compose-file', 'compose.dev.yaml', '--leave-stopped')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(['compose', '-f', 'compose.dev.yaml', '-p', 'locoder', 'stop'], self.calls())
        self.assertFalse(any(call[-1] == 'start' for call in self.calls()))
        result = self.run_script(self.target, 'restore.sh', self.backup)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ('postgres', 'valkey'):
            self.assertEqual((self.state / f'j0coder_{name}').read_bytes(),
                             (self.state / f'locoder_{name}').read_bytes())
        self.assertEqual((self.target / '.env').read_bytes(), (self.source / '.env').read_bytes())
        self.assertEqual((self.backup / 'env').stat().st_mode & 0o077, 0)
        self.assertEqual((self.target / '.env').stat().st_mode & 0o077, 0)

    def test_backup_restarts_selected_project_even_on_export_failure(self):
        result = self.make_backup(PODMAN_EXPORT_ERROR='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls()[-1], ['compose', '-f', 'compose.yaml', '-p', 'locoder', 'start'])

    def test_backup_refuses_existing_archive_before_stopping_services(self):
        self.backup.mkdir()
        (self.backup / 'postgres.tar').write_bytes(b'previous backup')
        result = self.make_backup()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])
        self.assertEqual((self.backup / 'postgres.tar').read_bytes(), b'previous backup')

    def test_restore_preflights_both_volumes_before_writing_configuration(self):
        self.assertEqual(self.make_backup().returncode, 0)
        (self.state / 'j0coder_valkey').write_bytes(b'keep me')
        result = self.run_script(self.target, 'restore.sh', self.backup)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.target / '.env').exists())
        self.assertFalse((self.state / 'j0coder_postgres').exists())
        self.assertEqual((self.state / 'j0coder_valkey').read_bytes(), b'keep me')

    def test_restore_refuses_engine_errors_and_incomplete_archives(self):
        self.assertEqual(self.make_backup().returncode, 0)
        result = self.run_script(self.target, 'restore.sh', self.backup, PODMAN_EXISTS_ERROR='125')
        self.assertEqual(result.returncode, 125)
        self.assertFalse((self.target / '.env').exists())
        (self.backup / 'valkey.tar').unlink()
        result = self.run_script(self.target, 'restore.sh', self.backup)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.target / '.env').exists())
        self.assertFalse((self.state / 'j0coder_postgres').exists())

    def test_restore_rejects_existing_configuration_and_relative_paths(self):
        self.assertEqual(self.make_backup().returncode, 0)
        result = self.run_script(self.source, 'restore.sh', self.backup)
        self.assertNotEqual(result.returncode, 0)
        result = self.run_script(self.target, 'restore.sh', 'relative-backup')
        self.assertNotEqual(result.returncode, 0)
        result = self.run_script(self.target, 'restore.sh', '--project-name', '../unsafe', self.backup)
        self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
