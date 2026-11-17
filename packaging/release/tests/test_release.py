"""Offline behavioral tests: no GitHub account, native package tool or personal data."""

import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import appimage
import assets
import github_release
import metadata
import packages


class VersionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.workspace("0.1.0")

    def workspace(self, version):
        (self.root / "Cargo.toml").write_text(
            f'[workspace.package]\nversion = "{version}"\n', encoding="utf-8")

    def test_initial_version_and_notes_need_no_changelog(self):
        self.assertEqual(metadata.release_version("v0.1.0", self.root), "0.1.0")
        self.assertIn("Initial release", metadata.release_notes("v0.1.0", self.root))

    def test_invalid_tag_and_workspace_mismatch_are_rejected(self):
        for tag in ("0.1.0", "v01.1.0", "v0.1", "v0.1.0;echo", "v0.1.0/../x",
                    "v0.1.0-01", "v0.1.0-rc.01", "v0.2.0", "v0.1.0+build", "v0.1.0\n"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                metadata.release_version(tag, self.root)

    def test_subsequent_versions_require_both_nonempty_changelogs(self):
        self.workspace("0.2.0-rc.1")
        directory = self.root / "changelog/0.2.0-rc.1"
        directory.mkdir(parents=True)
        for missing in (None, "en", "pt-BR"):
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                metadata.release_version("v0.2.0-rc.1", self.root)
            if missing is None:
                (directory / "en.md").write_text("New features", encoding="utf-8")
            elif missing == "en":
                (directory / "pt-BR.md").write_text("", encoding="utf-8")
        (directory / "pt-BR.md").write_text("Novas funcionalidades", encoding="utf-8")
        notes = metadata.release_notes("v0.2.0-rc.1", self.root)
        self.assertIn("New features", notes)
        self.assertIn("Novas funcionalidades", notes)
        self.assertEqual(packages.native_version("0.2.0-rc.1", "deb"), "0.2.0~rc.1")
        self.assertEqual(packages.native_version("0.2.0-rc.1", "arch"), "0.2.0rc.1")
        self.assertEqual(packages.native_version("0.2.0-beta-test.1", "rpm"), "0.2.0~beta.test.1")


class DraftTests(unittest.TestCase):
    def setUp(self):
        # Release-tool tests must keep working after the workspace version changes.
        versions = patch.object(github_release, "release_version", return_value="0.1.0")
        notes = patch.object(github_release, "release_notes", return_value="Fixture notes")
        versions.start()
        notes.start()
        self.addCleanup(versions.stop)
        self.addCleanup(notes.stop)

    def fake(self, releases):
        calls = []
        def command(*args):
            calls.append(args)
            if args[0] == "api":
                return json.dumps([releases])
            return ""
        return command, calls

    def test_create_always_uses_draft_and_verifies_tag(self):
        command, calls = self.fake([])
        github_release.ensure_draft("v0.1.0", command)
        create = calls[-1]
        self.assertEqual(create[:3], ("release", "create", "v0.1.0"))
        self.assertIn("--draft", create)
        self.assertIn("--verify-tag", create)
        self.assertNotIn("--generate-notes", create)

    def test_rerun_preserves_existing_draft_and_published_release_is_refused(self):
        for draft in (True, False):
            command, calls = self.fake([{"tag_name": "v0.1.0", "draft": draft}])
            if draft:
                github_release.ensure_draft("v0.1.0", command)
            else:
                with self.assertRaises(ValueError):
                    github_release.ensure_draft("v0.1.0", command)
            self.assertEqual(len(calls), 1)

    def test_prerelease_remains_draft_and_is_marked_prerelease(self):
        command, calls = self.fake([])
        with patch.object(github_release, "release_version", return_value="0.2.0-rc.1"), \
                patch.object(github_release, "release_notes", return_value="Fixture notes"):
            github_release.ensure_draft("v0.2.0-rc.1", command)
        self.assertIn("--draft", calls[-1])
        self.assertIn("--prerelease", calls[-1])

    def test_api_failure_does_not_create_a_release(self):
        calls = []
        def forbidden(*args):
            calls.append(args)
            raise subprocess.CalledProcessError(1, "gh")
        with self.assertRaises(subprocess.CalledProcessError):
            github_release.ensure_draft("v0.1.0", forbidden)
        self.assertEqual(len(calls), 1)

    def test_upload_stops_if_draft_is_published_during_upload(self):
        queries = 0
        uploads = []
        def command(*args):
            nonlocal queries
            if args[0] == "api":
                queries += 1
                return json.dumps([[{"tag_name": "v0.1.0", "draft": queries < 3}]])
            uploads.append(args)
            return ""
        with patch.object(assets, "verify_assets", return_value=[Path("one.zip"), Path("two.zip")]):
            with self.assertRaises(ValueError):
                github_release.upload("v0.1.0", Path("unused"), command)
        self.assertEqual(len(uploads), 1)
        self.assertEqual(uploads[0][:3], ("release", "upload", "v0.1.0"))


class AssetTests(unittest.TestCase):
    def setUp(self):
        versions = patch.object(assets, "release_version", return_value="0.1.0")
        versions.start()
        self.addCleanup(versions.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name in assets.expected_assets("v0.1.0"):
            (self.root / name).write_bytes(b"synthetic package")

    def finalize(self):
        with patch.dict(os.environ, {"GRAPHITE_UPDATE_SIGNING_KEY": ""}):
            assets.finalize(self.root, "v0.1.0")

    def test_complete_set_receives_valid_sidecars(self):
        self.finalize()
        self.assertEqual(len(assets.verify_assets(self.root, "v0.1.0")), 20)

    def test_missing_unexpected_and_sensitive_assets_are_refused(self):
        for extra in ("unexpected.zip", ".env", "signing.key"):
            (self.root / extra).write_text("synthetic fixture", encoding="utf-8")
            with self.assertRaises(ValueError):
                self.finalize()
            (self.root / extra).unlink()
        next(self.root.iterdir()).unlink()
        with self.assertRaises(ValueError):
            self.finalize()

    def test_tampering_and_partial_signatures_block_upload(self):
        self.finalize()
        (self.root / "graphite-v0.1.0-windows-x64.zip.sig").write_text("fixture", encoding="ascii")
        with self.assertRaises(ValueError):
            assets.verify_assets(self.root, "v0.1.0")
        (self.root / "graphite-v0.1.0-windows-x64.zip.sig").unlink()
        (self.root / "graphite-v0.1.0-windows-x64.zip").write_bytes(b"tampered")
        with self.assertRaises(ValueError):
            assets.verify_assets(self.root, "v0.1.0")

    def test_signing_key_is_temporary_and_removed_from_subprocess_environment(self):
        seen = []
        def sign(command, **kwargs):
            private = Path(command[-3])
            self.assertEqual(private.read_text(encoding="ascii"), "synthetic-private-fixture")
            self.assertNotIn("GRAPHITE_UPDATE_SIGNING_KEY", kwargs["env"])
            self.assertFalse(private.is_relative_to(self.root))
            seen.append(private)
            Path(command[-1] + ".sig").write_text("synthetic signature", encoding="ascii")
        with patch.dict(os.environ, {"GRAPHITE_UPDATE_SIGNING_KEY": "synthetic-private-fixture"}), \
                patch.object(assets.subprocess, "run", side_effect=sign):
            assets.finalize(self.root, "v0.1.0")
        self.assertEqual(len(seen), 10)
        self.assertFalse(seen[0].exists())
        self.assertEqual(len(assets.verify_assets(self.root, "v0.1.0")), 30)

    def test_failed_signing_also_removes_the_private_key(self):
        seen = []
        def fail(command, **kwargs):
            seen.append(Path(command[-3]))
            raise subprocess.CalledProcessError(1, "synthetic-signer")
        with patch.dict(os.environ, {"GRAPHITE_UPDATE_SIGNING_KEY": "synthetic-private-fixture"}), \
                patch.object(assets.subprocess, "run", side_effect=fail):
            with self.assertRaises(subprocess.CalledProcessError):
                assets.finalize(self.root, "v0.1.0")
        self.assertEqual(len(seen), 1)
        self.assertFalse(seen[0].exists())
        self.assertFalse(any(self.root.glob("*.key")))


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_archive_path_traversal_is_rejected(self):
        extraction = self.root / "extracted"
        extraction.mkdir()
        archive = self.root / "unsafe.zip"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("../escaped.txt", "unsafe fixture")
        with self.assertRaises(ValueError):
            packages.unpack(archive, extraction)
        self.assertFalse((self.root / "escaped.txt").exists())
        archive = self.root / "unsafe.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            entry = tarfile.TarInfo("../escaped.txt")
            entry.size = 1
            output.addfile(entry, io.BytesIO(b"x"))
        with self.assertRaises(tarfile.FilterError):
            packages.unpack(archive, extraction)
        self.assertFalse((self.root / "escaped.txt").exists())

    def test_bundle_layout_missing_runtime_and_native_launchers(self):
        archive = self.root / "bundle.zip"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("bundle/graphite.exe", b"MZ fixture")
        extraction = self.root / "extracted"
        extraction.mkdir()
        with self.assertRaises(ValueError):
            packages.unpack(archive, extraction)
        bundle = self.root / "linux-bundle"
        (bundle / "code-oss").mkdir(parents=True)
        (bundle / "graphite").write_bytes(b"\x7fELF fixture")
        (bundle / "code-oss/commit").write_text("fixture", encoding="ascii")
        stage = self.root / "stage"
        packages.linux_tree(bundle, stage)
        self.assertEqual((stage / "usr/lib/graphite/graphite").read_bytes(), b"\x7fELF fixture")
        self.assertIn('exec /usr/lib/graphite/code "$@"',
                      (stage / "usr/bin/graphite-code").read_text(encoding="utf-8"))
        self.assertTrue((stage / "usr/share/applications/graphite.desktop").is_file())
        self.assertTrue((stage / "usr/share/licenses/graphite/LICENSE").is_file())

    def test_complete_portable_archive_preserves_helpers_and_installs_license(self):
        archive = self.root / "complete.zip"
        with zipfile.ZipFile(archive, "w") as output:
            for name in ("graphite.exe", "code.exe", "graphite-extension-host.exe", "code-oss/commit"):
                output.writestr("bundle/" + name, "synthetic fixture")
        extraction = self.root / "complete"
        extraction.mkdir()
        bundle = packages.unpack(archive, extraction)
        self.assertEqual((bundle / "code.exe").read_text(), "synthetic fixture")
        self.assertEqual((bundle / "LICENSE").read_bytes(), (metadata.ROOT / "LICENSE").read_bytes())

    def test_appimage_tool_checksum_is_enforced_before_execution(self):
        tool = self.root / "tool"
        with patch.object(appimage.urllib.request, "urlopen", return_value=io.BytesIO(b"bad")):
            with self.assertRaises(ValueError):
                appimage.checked_download("https://example.invalid/tool", tool, "0" * 64)
        self.assertFalse(tool.exists())
        expected = hashlib.sha256(b"fixture").hexdigest()
        with patch.object(appimage.urllib.request, "urlopen", return_value=io.BytesIO(b"fixture")):
            appimage.checked_download("https://example.invalid/tool", tool, expected)
        self.assertEqual(tool.read_bytes(), b"fixture")


if __name__ == "__main__":
    unittest.main()
