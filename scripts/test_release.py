import hashlib
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

from package_release import BUNDLE_FILES

ROOT = Path(__file__).resolve().parents[1]
VERSION = "v1.2.3"
TARGET = "x86_64-unknown-linux-musl"


class ReleasePackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="steward-package-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
        binary = self.root / "target" / TARGET / "release/steward"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"fixture executable bytes")
        for name in BUNDLE_FILES:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture: " + name)
        self.output = self.root / "dist"

    def package(self, version=VERSION):
        return subprocess.run([sys.executable, "-B", str(ROOT / "scripts/package_release.py"),
                               "--version", version, "--target", TARGET, "--output", str(self.output)],
                              cwd=self.root, capture_output=True, timeout=10)

    def test_bundle_is_complete_and_existing_assets_are_immutable(self):
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        archive = self.output / f"steward_{VERSION}_{TARGET}.tar.gz"
        before = archive.read_bytes()
        with tarfile.open(archive, "r:gz") as package:
            self.assertEqual(set(package.getnames()), {"steward", *BUNDLE_FILES})
            self.assertTrue(all(member.isfile() for member in package.getmembers()))
        expected = hashlib.sha256(before).hexdigest()
        self.assertEqual(archive.with_name(archive.name + ".sha256").read_text().split(), [expected, archive.name])
        self.assertNotEqual(self.package().returncode, 0)
        self.assertEqual(archive.read_bytes(), before)

    def test_tag_mismatch_and_orphan_sidecar_do_not_publish_an_archive(self):
        self.assertNotEqual(self.package("v9.9.9").returncode, 0)
        self.assertFalse(self.output.exists())
        self.output.mkdir()
        archive = self.output / f"steward_{VERSION}_{TARGET}.tar.gz"
        sidecar = archive.with_name(archive.name + ".sha256")
        sidecar.write_text("preserve this existing checksum")
        self.assertNotEqual(self.package().returncode, 0)
        self.assertFalse(archive.exists())
        self.assertEqual(sidecar.read_text(), "preserve this existing checksum")

    def test_missing_or_linked_bundle_input_is_rejected_before_packaging(self):
        source = self.root / "README.md"
        source.unlink()
        self.assertNotEqual(self.package().returncode, 0)
        source.symlink_to(self.root / "llms.txt")
        self.assertNotEqual(self.package().returncode, 0)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
