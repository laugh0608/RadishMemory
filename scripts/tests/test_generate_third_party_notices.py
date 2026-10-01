from __future__ import annotations

import importlib.util
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "generate-third-party-notices.py"
SPEC = importlib.util.spec_from_file_location("third_party_notices", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load third-party notice generator")
NOTICES = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = NOTICES
SPEC.loader.exec_module(NOTICES)


class ReviewedVendorChecks(unittest.TestCase):
    def copy_vendor(self, root: Path) -> Path:
        destination = root / NOTICES.VENDOR_PATH
        shutil.copytree(NOTICES.REPO_ROOT / NOTICES.VENDOR_PATH, destination)
        return destination

    def test_reviewed_upstream_patch_passes(self) -> None:
        NOTICES.check_reviewed_vendor(NOTICES.REPO_ROOT)

    def test_changed_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            vendor = self.copy_vendor(root)
            (vendor / "src/cred.rs").write_bytes(b"unreviewed source\n")
            with self.assertRaisesRegex(RuntimeError, "unreviewed vendor file or content"):
                NOTICES.check_reviewed_vendor(root)

    def test_missing_and_additional_sources_are_rejected(self) -> None:
        for change in ("missing", "additional"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                vendor = self.copy_vendor(root)
                if change == "missing":
                    (vendor / "src/cred.rs").unlink()
                else:
                    (vendor / "src/extra.rs").write_bytes(b"unreviewed source\n")
                with self.assertRaises(RuntimeError):
                    NOTICES.check_reviewed_vendor(root)

    def test_provenance_cannot_authorize_its_own_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            vendor = self.copy_vendor(root)
            (vendor / "provenance.json").write_bytes(b"{}\n")
            with self.assertRaisesRegex(RuntimeError, "provenance changed without review"):
                NOTICES.check_reviewed_vendor(root)

    def test_symlinked_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            vendor = self.copy_vendor(root)
            source = vendor / "src/cred.rs"
            retained = root / "retained.rs"
            source.rename(retained)
            try:
                source.symlink_to(retained)
            except OSError as error:
                self.skipTest(f"symlink privilege unavailable: {error.errno}")
            with self.assertRaisesRegex(RuntimeError, "must not contain symlinks"):
                NOTICES.check_reviewed_vendor(root)

    def metadata(self, dependency: dict[str, object]) -> dict[str, object]:
        roots = [{"id": name, "name": name} for name in NOTICES.ROOT_PACKAGES]
        return {
            "packages": [*roots, dependency],
            "workspace_members": [root["id"] for root in roots],
            "resolve": {"nodes": [
                *[{"id": root["id"], "deps": [{
                    "pkg": dependency["id"], "dep_kinds": [{"kind": None}],
                }]} for root in roots],
                {"id": dependency["id"], "deps": []},
            ]},
        }

    def test_vendor_is_in_inventory_with_both_source_digests(self) -> None:
        dependency = {
            "id": "synthetic-vendor", "name": "windows-native-keyring-store",
            "version": "1.1.0", "source": None, "license": "MIT OR Apache-2.0",
            "manifest_path": str(NOTICES.REPO_ROOT / NOTICES.VENDOR_PATH / "Cargo.toml"),
        }
        with patch.object(NOTICES, "cargo_metadata", return_value=self.metadata(dependency)):
            packages = NOTICES.collect_packages()
        self.assertEqual(len(packages), 1)
        self.assertEqual(packages[0].basis, "MIT")
        self.assertIn(NOTICES.VENDOR_ARCHIVE_SHA256, packages[0].checksum)
        self.assertIn(NOTICES.VENDOR_PROVENANCE_SHA256, packages[0].checksum)

    def test_unknown_third_party_path_is_not_silently_excluded(self) -> None:
        dependency = {"id": "synthetic-unreviewed", "name": "unreviewed", "source": None}
        with patch.object(NOTICES, "cargo_metadata", return_value=self.metadata(dependency)):
            with self.assertRaisesRegex(RuntimeError, "unreviewed path dependency"):
                NOTICES.collect_packages()


if __name__ == "__main__":
    unittest.main()
