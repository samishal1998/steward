import hashlib
import io
import os
import pathlib
import subprocess
import tarfile
import tempfile
import unittest

from prepare_installer import TARGETS, prepare

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = "v0.1.0-alpha.2"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="steward-installer-")
        self.addCleanup(self.tmp.cleanup)
        self.root = pathlib.Path(self.tmp.name)
        self.assets = self.root / "assets"
        self.tools = self.root / "tools"
        self.destination = self.root / "destination with spaces"
        for folder in [self.assets, self.tools, self.destination]:
            folder.mkdir()
        self.rows = []
        for pattern, target in TARGETS.items():
            name = f"steward_{VERSION}_{target}.tar.gz"
            program = f"#!/bin/sh\nprintf 'steward {VERSION[1:]}\\n'\n".encode()
            with tarfile.open(self.assets / name, "w:gz") as tar:
                entry = tarfile.TarInfo("steward")
                entry.mode = 0o755
                entry.size = len(program)
                tar.addfile(entry, io.BytesIO(program))
            digest = hashlib.sha256((self.assets / name).read_bytes()).hexdigest()
            self.rows.append((pattern, name, digest))
        self.generated = "#!/bin/sh\n# samishal1998/steward " + VERSION + "\n"
        for pattern, name, digest in self.rows:
            self.generated += f"  {pattern})\n    url='https://github.com/samishal1998/steward/releases/download/{VERSION}/{name}'\n    file='{name}'\n    sum='{digest}'\n    ;;\n"
        self.pinned = prepare(self.generated, VERSION)
        (self.assets / "install.sh").write_text(self.pinned)
        digest = hashlib.sha256(self.pinned.encode()).hexdigest()
        (self.assets / "SHA256SUMS").write_text(f"{digest}  install.sh\n")
        self.executable("uname", '#!/bin/sh\nif [ "$1" = -s ]; then printf "%s\\n" "$TEST_OS"; else printf "%s\\n" "$TEST_ARCH"; fi\n')
        # Local fixtures deliberately intercept curl. No Internet or user install paths.
        self.executable("curl", '''#!/usr/bin/env python3
import os,pathlib,sys
a=sys.argv[1:]; url=a[-1]
if not url.startswith('https://github.com/samishal1998/steward/releases/'):
    sys.exit(2)
if url.endswith('/latest'):
    if os.environ.get('TEST_NO_STABLE')=='1':
        print('200|https://github.com/samishal1998/steward/releases',end='')
    elif os.environ.get('TEST_NO_STABLE')=='2':
        print('404|https://github.com/samishal1998/steward/releases/latest',end='')
    else:
        print('200|https://github.com/samishal1998/steward/releases/tag/'+os.environ['TEST_VERSION'],end='')
    sys.exit(0)
if '/download/'+os.environ['TEST_VERSION']+'/' not in url:
    sys.exit(22)
file=pathlib.Path(os.environ['TEST_ASSETS'])/url.rsplit('/',1)[-1]
if not file.is_file(): sys.exit(22)
dest=pathlib.Path(a[a.index('--output')+1]); dest.write_bytes(file.read_bytes())
''')
        self.environment = {**os.environ, "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
                             "TEST_ASSETS": str(self.assets), "TEST_OS": "Linux", "TEST_ARCH": "x86_64", "TEST_VERSION": VERSION}

    def executable(self, name, body):
        file = self.tools / name
        file.write_text(body)
        file.chmod(0o755)

    def run_script(self, script, args=(), extra=None):
        return subprocess.run(["sh", "-s", "--", "--dir", str(self.destination), *args], input=script,
                              text=True, capture_output=True, timeout=15, env={**self.environment, **(extra or {})})

    def test_two_stage_latest_pinned_and_alpha_fallback(self):
        launcher = (ROOT / "install.sh").read_text()
        for args, env in [((), {}), (("--version", VERSION), {}), ((), {"TEST_NO_STABLE": "1"}), ((), {"TEST_NO_STABLE": "2"})]:
            result = self.run_script(launcher, args, env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(subprocess.check_output([str(self.destination / "steward"), "--version"], text=True).strip(), "steward " + VERSION[1:])

    def test_all_platform_rows(self):
        for system, arch in [("Linux", "x86_64"), ("Linux", "aarch64"), ("Linux", "arm64"), ("Darwin", "x86_64"), ("Darwin", "arm64")]:
            result = self.run_script(self.pinned, extra={"TEST_OS": system, "TEST_ARCH": arch})
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_binary_checksum_failure_preserves_existing_install(self):
        self.assertEqual(self.run_script(self.pinned).returncode, 0)
        before = (self.destination / "steward").read_bytes()
        file = self.assets / self.rows[0][1]
        file.write_bytes(file.read_bytes() + b"tampered")
        result = self.run_script(self.pinned)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Checksum mismatch", result.stderr)
        self.assertEqual((self.destination / "steward").read_bytes(), before)

    def test_installer_checksum_failure_does_not_execute_download(self):
        (self.assets / "install.sh").write_text('exit 42\n')
        result = self.run_script((ROOT / "install.sh").read_text(), ("--version", VERSION))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Installer checksum mismatch", result.stderr)
        self.assertFalse((self.destination / "steward").exists())

    def test_valid_archive_with_wrong_binary_version_preserves_existing_install(self):
        self.assertEqual(self.run_script(self.pinned).returncode, 0)
        before = (self.destination / "steward").read_bytes()
        _, name, original = self.rows[0]
        program = b"#!/bin/sh\nprintf 'steward 9.9.9\\n'\n"
        with tarfile.open(self.assets / name, "w:gz") as tar:
            entry = tarfile.TarInfo("steward")
            entry.mode = 0o755
            entry.size = len(program)
            tar.addfile(entry, io.BytesIO(program))
        digest = hashlib.sha256((self.assets / name).read_bytes()).hexdigest()
        script = prepare(self.generated.replace(original, digest), VERSION)
        result = self.run_script(script)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected product/version", result.stderr)
        self.assertEqual((self.destination / "steward").read_bytes(), before)

    def test_wrong_version_and_destination_symlink_are_rejected(self):
        self.assertNotEqual(self.run_script((ROOT / "install.sh").read_text(), ("--version", "../../bad")).returncode, 0)
        self.assertNotEqual(self.run_script(self.pinned, extra={"TEST_ARCH": "armv7l"}).returncode, 0)
        target = self.root / "external"
        target.write_text("Keep this")
        (self.destination / "steward").symlink_to(target)
        self.assertNotEqual(self.run_script(self.pinned).returncode, 0)
        self.assertEqual(target.read_text(), "Keep this")

    def test_missing_checksum_or_wrong_generator_source_is_rejected(self):
        for raw in [self.generated.replace("sum='", "missing='", 1),
                    self.generated.replace("samishal1998/steward", "other/steward"),
                    self.generated.replace("https://github.com/", "http://github.com/", 1)]:
            with self.assertRaises(ValueError):
                prepare(raw, VERSION)

    def test_truncated_piped_installer_has_no_effect(self):
        result = self.run_script(self.pinned.rsplit('main "$@"', 1)[0][:-20])
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.destination / "steward").exists())

    def test_help_needs_no_download_and_fallback_matches_cargo(self):
        import tomllib
        self.assertEqual("v" + tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"], VERSION)
        result = subprocess.run(["sh", str(ROOT / "install.sh"), "--help"], text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage:", result.stdout)


if __name__ == "__main__":
    unittest.main()
