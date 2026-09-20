import importlib.util
from pathlib import Path
import unittest
import yaml
import subprocess
import sys
import tempfile
import stat

spec = importlib.util.spec_from_file_location('migration', Path(__file__).with_name('migrate-cpr-config.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class MigrationTests(unittest.TestCase):
    def test_apply_backup_permissions_and_idempotence(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'config.yaml'
            original = b'openai:\n  wire_profile:\n    residency: us\n'
            path.write_bytes(original)
            path.chmod(0o640)
            command = [sys.executable, str(Path(module.__file__))]
            subprocess.run(command + ['--check', str(path)], check=True, capture_output=True)
            self.assertEqual(path.read_bytes(), original)
            self.assertEqual(len(list(path.parent.iterdir())), 1)
            subprocess.run(command + ['--apply', str(path)], check=True, capture_output=True)
            backups = list(path.parent.glob('config.yaml.before-residency-*'))
            self.assertEqual(len(backups), 1)
            self.assertEqual(backups[0].read_bytes(), original)
            self.assertEqual(stat.S_IMODE(backups[0].stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o640)
            migrated = path.read_bytes()
            subprocess.run(command + ['--apply', str(path)], check=True, capture_output=True)
            self.assertEqual(path.read_bytes(), migrated)
            self.assertEqual(len(list(path.parent.iterdir())), 2)

    def test_preserves_comments_values_and_old_compatibility(self):
        text = '# secret comment\nopenai:\n  wire_profile:\n    residency: "us" # keep\n  api:\n    base_url: https://example.invalid\n'
        result = module.migrate(text)
        self.assertIn('# secret comment', result)
        self.assertIn('residency: "us" # keep', result)
        self.assertEqual(yaml.safe_load(result)['openai']['residency'], 'us')
        self.assertEqual(module.migrate(result), result)

    def test_conflict(self):
        with self.assertRaises(ValueError):
            module.migrate('openai:\n  residency: eu\n  wire_profile:\n    residency: us\n')

    def test_duplicate(self):
        with self.assertRaises(ValueError):
            module.migrate('openai: {}\nopenai: {}\n')

    def test_alias(self):
        with self.assertRaises(ValueError):
            module.migrate('openai: &x {}\nother: *x\n')

    def test_noop(self):
        text = 'openai:\n  residency: us\n'
        self.assertEqual(module.migrate(text), text)


if __name__ == '__main__':
    unittest.main()
