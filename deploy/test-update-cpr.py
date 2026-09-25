"""离线验证手动升级脚本；不执行 --apply，不接触部署或 systemd。"""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('update-cpr.sh')
ELF = b'\x7fELF\x02\x01' + bytes(12) + b'\x3e\x00'


class ManualUpdaterTests(unittest.TestCase):
    def check_package(self, change=None, corrupt_checksum=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'codex-proxy-rs-linux-amd64.tar.gz'
            files = {
                'codex-proxy-rs': ELF,
                'web/dist/index.html': b'web',
                'VERSION': b'3.13.1-Flod-fork.1\n',
                'REVISION': b'a' * 40 + b'\n',
                'plugins/official/plugin-release-manifest.json': json.dumps({
                    'schema_version': 1,
                    'sealed': True,
                    'gateway_version': '3.13.1-Flod-fork.1',
                    'gateway_git_sha': 'a' * 40,
                    'plugin_host': {},
                    'plugins': [],
                }).encode(),
            }
            if change:
                change(files)
            with tarfile.open(package, 'w:gz') as archive:
                for name, content in files.items():
                    member = tarfile.TarInfo(name)
                    member.mode = 0o755
                    if content is None:
                        member.type = tarfile.SYMTYPE
                        member.linkname = '/etc/passwd'
                        archive.addfile(member)
                    else:
                        member.size = len(content)
                        archive.addfile(member, io.BytesIO(content))
            digest = hashlib.sha256(package.read_bytes()).hexdigest()
            sums = root / 'SHA256SUMS'
            sums.write_text(f'{"0" * 64 if corrupt_checksum else digest}  {package.name}\n')
            before = package.read_bytes()
            result = subprocess.run(
                ['bash', str(SCRIPT), '--check', str(package), str(sums)],
                capture_output=True, text=True,
            )
            self.assertEqual(before, package.read_bytes())
            return result

    def test_valid_package_is_read_only(self):
        result = self.check_package()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('仅校验', result.stdout)

    def test_bad_checksum_is_rejected(self):
        self.assertNotEqual(self.check_package(corrupt_checksum=True).returncode, 0)

    def test_traversal_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update({'../escape': b'x'})).returncode, 0)

    def test_symlink_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update({'link': None})).returncode, 0)

    def test_missing_revision_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.pop('REVISION')).returncode, 0)

    def test_missing_plugin_manifest_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.pop('plugins/official/plugin-release-manifest.json')).returncode, 0)

    def test_plugin_manifest_identity_mismatch_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update({
            'plugins/official/plugin-release-manifest.json': json.dumps({
                'schema_version': 1,
                'sealed': True,
                'gateway_version': '3.13.1-Flod-fork.2',
                'gateway_git_sha': 'a' * 40,
            }).encode(),
        })).returncode, 0)

    def test_official_version_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update(VERSION=b'3.12.1')).returncode, 0)

    def test_legacy_fork_version_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update(VERSION=b'3.12.1-fork.20')).returncode, 0)

    def test_wrong_architecture_is_rejected(self):
        self.assertNotEqual(self.check_package(lambda files: files.update({'codex-proxy-rs': b'not ELF'})).returncode, 0)

    def test_no_arguments_do_not_apply(self):
        result = subprocess.run(['bash', str(SCRIPT)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)


if __name__ == '__main__':
    unittest.main()
