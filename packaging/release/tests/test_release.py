"""Offline behavioral tests: no GitHub account, native package tool or personal data."""

import hashlib
import io
import json
import os
from contextlib import redirect_stderr
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import portable
import appimage
import assets
import github_release
import licenses
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

    ENGLISH = "# Known limitations (0.1.0)\n\n## Platforms\n\n- x86-64 only.\n"
    PORTUGUESE = "# Limitações conhecidas (0.1.0)\n\n## Plataformas\n\n- Só x86-64.\n"

    def limitations(self, english=None, portuguese=None):
        english = self.ENGLISH if english is None else english
        portuguese = self.PORTUGUESE if portuguese is None else portuguese
        for language, text in (("en", english), ("pt-BR", portuguese)):
            directory = self.root / "docs" / language / "roadmap"
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "known-limitations.md").write_text(text, encoding="utf-8")

    def test_initial_version_and_notes_need_no_changelog(self):
        self.limitations()
        self.assertEqual(metadata.release_version("v0.1.0", self.root), "0.1.0")
        self.assertIn("Initial release", metadata.release_notes("v0.1.0", self.root))

    def test_initial_notes_list_the_known_limitations_in_both_languages(self):
        self.limitations()
        notes = metadata.release_notes("v0.1.0", self.root)
        self.assertIn("## Known limitations", notes)
        self.assertIn("x86-64 only", notes)
        self.assertIn("## Limitações conhecidas", notes)
        self.assertIn("Só x86-64", notes)
        # The page titles are dropped and its sections move one level down.
        self.assertNotIn("# Known limitations (0.1.0)", notes)
        self.assertIn("### Platforms", notes)

    def test_an_initial_release_without_the_limitations_pages_is_refused(self):
        with self.assertRaisesRegex(ValueError, "known-limitations"):
            metadata.release_notes("v0.1.0", self.root)
        self.limitations(portuguese="   \n")
        with self.assertRaisesRegex(ValueError, "pt-BR"):
            metadata.release_notes("v0.1.0", self.root)

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


def crate(identifier, name, directory, **fields):
    return {"id": identifier, "name": name, "version": "1.0.0",
            "manifest_path": str(directory / "Cargo.toml"), **fields}


class LicenseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def make_crate(self, name, files):
        directory = self.root / "crates" / name
        directory.mkdir(parents=True)
        for filename, text in files.items():
            (directory / filename).write_text(text, encoding="utf-8")
        return directory

    def test_only_crates_linked_into_the_program_are_listed(self):
        metadata_json = {
            "workspace_members": ["app"],
            "packages": [{"id": name, "name": name, "version": "1.0.0", "manifest_path": "x"}
                         for name in ("app", "runtime", "nested", "dev-only", "build-only")],
            "resolve": {"nodes": [
                {"id": "app", "deps": [
                    {"pkg": "runtime", "dep_kinds": [{"kind": None}]},
                    {"pkg": "dev-only", "dep_kinds": [{"kind": "dev"}]},
                    {"pkg": "build-only", "dep_kinds": [{"kind": "build"}]}]},
                {"id": "runtime", "deps": [{"pkg": "nested", "dep_kinds": [{"kind": None}]}]},
                {"id": "nested", "deps": []},
                {"id": "dev-only", "deps": []},
                {"id": "build-only", "deps": []}]},
        }
        names = [package["name"] for package in licenses.shipped_packages(metadata_json)]
        self.assertEqual(names, ["nested", "runtime"])

    def test_a_crate_used_in_normal_and_dev_mode_is_still_shipped(self):
        metadata_json = {
            "workspace_members": ["app"],
            "packages": [{"id": "app", "name": "app", "version": "1", "manifest_path": "x"},
                         {"id": "both", "name": "both", "version": "1", "manifest_path": "x"}],
            "resolve": {"nodes": [
                {"id": "app", "deps": [{"pkg": "both", "dep_kinds": [{"kind": "dev"}, {"kind": None}]}]},
                {"id": "both", "deps": []}]},
        }
        self.assertEqual([p["name"] for p in licenses.shipped_packages(metadata_json)], ["both"])

    def test_license_files_are_read_whatever_their_spelling(self):
        directory = self.make_crate("sample", {
            "LICENSE-MIT": "mit text", "LICENSE-APACHE": "apache text", "COPYING": "copying",
            "NOTICE": "notice", "README.md": "not a license", "Cargo.toml": ""})
        names = [name for name, _ in licenses.license_texts(directory)]
        self.assertEqual(names, ["COPYING", "LICENSE-APACHE", "LICENSE-MIT", "NOTICE"])
        self.assertEqual(licenses.license_texts(self.root / "missing"), [])

    def test_the_notices_name_each_crate_with_its_license_and_text(self):
        first = self.make_crate("alpha", {"LICENSE": "Copyright Alpha"})
        second = self.make_crate("beta", {"LICENSE": "Copyright Beta"})
        entries = licenses.entries_for([
            crate("a", "alpha", first, license="MIT", repository="https://example.test/alpha"),
            crate("b", "beta", second, license="Apache-2.0")])
        text = licenses.render(entries)
        self.assertIn("* alpha 1.0.0 - MIT (https://example.test/alpha)", text)
        self.assertIn("* beta 1.0.0 - Apache-2.0", text)
        self.assertIn("Copyright Alpha", text)
        self.assertIn("Copyright Beta", text)
        self.assertIn("MIT license", text.split("Rust crates")[0])

    def test_crates_with_identical_text_share_one_copy(self):
        shared = "Permission is hereby granted..."
        entries = licenses.entries_for([
            crate(name, name, self.make_crate(name, {"LICENSE": shared}), license="MIT")
            for name in ("one", "two", "three")])
        text = licenses.render(entries)
        self.assertEqual(text.count(shared), 1)
        for name in ("one", "two", "three"):
            self.assertIn(f"* {name} 1.0.0 - MIT", text)

    def test_a_crate_that_declares_nothing_and_ships_nothing_blocks_the_bundle(self):
        entries = licenses.entries_for([crate("x", "mystery", self.make_crate("mystery", {}))])
        self.assertEqual([e["name"] for e in licenses.without_license_files(entries)], ["mystery"])
        destination = self.root / "package"
        destination.mkdir()
        with self.assertRaisesRegex(ValueError, "mystery"):
            licenses.write_bundle(destination, root=self.root, entries=entries)
        self.assertFalse((destination / "THIRD_PARTY_NOTICES.txt").exists())

    def test_a_declared_license_without_a_file_is_noted_not_dropped(self):
        entries = licenses.entries_for([
            crate("x", "bare", self.make_crate("bare", {}), license="MIT")])
        self.assertEqual(licenses.without_license_files(entries), [])
        self.assertIn("ships no license file", licenses.render(entries))

    def test_the_bundle_carries_the_notices_and_the_icon_font_licenses(self):
        source = self.root / "crates/graphite-render/assets"
        source.mkdir(parents=True)
        for name in ("NOTICE.md", "LICENSE-Codicons.txt", "graphite-icons.ttf", "README.md"):
            (source / name).write_text(name, encoding="utf-8")
        entries = licenses.entries_for([
            crate("a", "alpha", self.make_crate("alpha", {"LICENSE": "Copyright Alpha"}), license="MIT")])
        destination = self.root / "package"
        destination.mkdir()
        self.assertEqual(licenses.write_bundle(destination, root=self.root, entries=entries), 1)
        self.assertIn("alpha 1.0.0", (destination / "THIRD_PARTY_NOTICES.txt").read_text(encoding="utf-8"))
        copied = sorted(path.name for path in (destination / "licenses/icon-font").iterdir())
        # The font itself and the readme are not licenses and are not copied.
        self.assertEqual(copied, ["LICENSE-Codicons.txt", "NOTICE.md"])

    def test_vendored_javascript_is_listed_with_its_own_license(self):
        directory = self.root / "vendor/lib"
        directory.mkdir(parents=True)
        (directory / "package.json").write_text(
            json.dumps({"name": "lib", "version": "2.0.0", "license": "MIT",
                        "repository": {"url": "https://example.test/lib"}}), encoding="utf-8")
        (directory / "LICENSE").write_text("Copyright Lib authors", encoding="utf-8")
        entries = licenses.vendored_entries(self.root, (Path("vendor/lib"),))
        self.assertEqual([(e["name"], e["version"]) for e in entries],
                         [("lib (bundled JavaScript)", "2.0.0")])
        text = licenses.render(entries)
        self.assertIn("* lib (bundled JavaScript) 2.0.0 - MIT (https://example.test/lib)", text)
        self.assertIn("Copyright Lib authors", text)

    def test_the_embedded_emmet_bundle_is_covered_by_the_notices(self):
        entries = licenses.vendored_entries(metadata.ROOT)
        self.assertEqual([e["name"] for e in entries], ["emmet (bundled JavaScript)"])
        self.assertTrue(entries[0]["texts"], "the repository copy of Emmet must keep its license")
        self.assertEqual(licenses.without_license_files(entries), [])

    def test_the_real_icon_font_licenses_exist_in_the_repository(self):
        assets_dir = metadata.ROOT / licenses.ICON_FONT_ASSETS
        self.assertTrue((assets_dir / "NOTICE.md").is_file())
        self.assertTrue(list(assets_dir.glob("LICENSE-*.txt")))


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
        (bundle / "THIRD_PARTY_NOTICES.txt").write_text("notices fixture", encoding="utf-8")
        (bundle / "licenses/icon-font").mkdir(parents=True)
        (bundle / "licenses/icon-font/NOTICE.md").write_text("font fixture", encoding="utf-8")
        stage = self.root / "stage"
        packages.linux_tree(bundle, stage)
        self.assertEqual((stage / "usr/lib/graphite/graphite").read_bytes(), b"\x7fELF fixture")
        self.assertIn('exec /usr/lib/graphite/code "$@"',
                      (stage / "usr/bin/graphite-code").read_text(encoding="utf-8"))
        self.assertTrue((stage / "usr/share/applications/graphite.desktop").is_file())
        self.assertTrue((stage / "usr/share/licenses/graphite/LICENSE").is_file())
        self.assertEqual((stage / "usr/share/licenses/graphite/THIRD_PARTY_NOTICES.txt").read_text(
            encoding="utf-8"), "notices fixture")
        self.assertTrue((stage / "usr/share/licenses/graphite/icon-font/NOTICE.md").is_file())

    def test_a_failed_command_shows_its_captured_output(self):
        script = "import sys; sys.stderr.write('the real reason'); sys.exit(2)"
        captured = io.StringIO()
        with redirect_stderr(captured), self.assertRaises(subprocess.CalledProcessError):
            packages.run([sys.executable, "-c", script], capture_output=True, text=True)
        self.assertIn("the real reason", captured.getvalue())

    def test_only_x86_64_glibc_binaries_are_analyzed_for_dependencies(self):
        def elf(name, machine=0x3E, bits=2, order=1):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            header = b"ELF" + bytes([bits, order]) + bytes(12) + machine.to_bytes(2, "little")
            path.write_bytes(header + bytes(8))
            return path
        self.assertTrue(packages.is_linux_x64_glibc_binary(elf("linux-x64/pty.node")))
        self.assertFalse(packages.is_linux_x64_glibc_binary(elf("linux-arm64/pty.node", machine=0xB7)))
        self.assertFalse(packages.is_linux_x64_glibc_binary(elf("koffi/musl_x64/koffi.node")))
        windows = self.root / "win32-x64/pty.node"
        windows.parent.mkdir()
        windows.write_bytes(b"MZ" + bytes(30))
        self.assertFalse(packages.is_linux_x64_glibc_binary(windows))
        self.assertFalse(packages.is_linux_x64_glibc_binary(self.root / "missing.node"))

    def test_a_rebuilt_native_module_keeps_only_its_compiled_files(self):
        module = self.root / "spdlog"
        (module / "build/Release/obj/deep/state.tlog").mkdir(parents=True)
        (module / "build/Release/obj/deep/state.tlog/x.lastbuildstate").write_text("x")
        (module / "build/node_modules/node-addon-api").mkdir(parents=True)
        (module / "build/Release/spdlog.node").write_bytes(b"binary")
        (module / "build/Release/spdlog.pdb").write_bytes(b"symbols")
        (module / "package.json").write_text("{}")
        portable.prune_native_build(module)
        self.assertEqual(sorted(str(p.relative_to(module)).replace("\\", "/")
                                for p in module.rglob("*") if p.is_file()),
                         ["build/Release/spdlog.node", "package.json"])
        self.assertEqual((module / "build/Release/spdlog.node").read_bytes(), b"binary")
        portable.prune_native_build(self.root / "without-build")  # nothing to do, no error

    def test_tar_archives_extract_safely_without_python_extraction_filters(self):
        # Debian 12 ships a Python whose tarfile has no `filter` argument.
        def tar_with(path, name, kind=tarfile.REGTYPE, link=""):
            with tarfile.open(path, "w:gz") as output:
                entry = tarfile.TarInfo(name)
                entry.type, entry.linkname = kind, link
                entry.size = 1 if kind == tarfile.REGTYPE else 0
                output.addfile(entry, io.BytesIO(b"x") if kind == tarfile.REGTYPE else None)
        with patch.object(tarfile, "data_filter", None):
            for name, kind, link in (("../escaped.txt", tarfile.REGTYPE, ""),
                                     ("bundle/link", tarfile.SYMTYPE, "../../outside"),
                                     ("bundle/device", tarfile.CHRTYPE, "")):
                archive = self.root / "unsafe.tar.gz"
                tar_with(archive, name, kind, link)
                extraction = self.root / "no-filter-unsafe"
                extraction.mkdir(exist_ok=True)
                with self.assertRaises(ValueError, msg=name):
                    packages.unpack(archive, extraction)
                with self.assertRaises(ValueError, msg=name):
                    if kind == tarfile.REGTYPE:
                        portable.extract_regular_files(archive, extraction)
                    else:
                        raise ValueError("only path traversal applies to the source extractor")
            safe = self.root / "safe.tar.gz"
            with tarfile.open(safe, "w:gz") as output:
                for name, data in (("bundle/graphite", b"binary"), ("bundle/code", b"c"),
                                   ("bundle/graphite-extension-host", b"h"),
                                   ("bundle/code-oss/commit", b"c"),
                                   ("bundle/THIRD_PARTY_NOTICES.txt", b"n"),
                                   ("bundle/licenses/icon-font/NOTICE.md", b"f")):
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    output.addfile(entry, io.BytesIO(data))
            extraction = self.root / "no-filter-safe"
            extraction.mkdir()
            bundle = packages.unpack(safe, extraction)
            self.assertEqual((bundle / "graphite").read_bytes(), b"binary")
            sources = self.root / "sources"
            sources.mkdir()
            portable.extract_regular_files(safe, sources)
            self.assertEqual((sources / "bundle/code-oss/commit").read_bytes(), b"c")

    def test_every_node_gyp_module_is_pruned_and_other_modules_are_left_alone(self):
        modules = self.root / "node_modules"
        for name in ("@vscode/windows-process-tree", "plain-module"):
            module = modules / name
            (module / "build/Release/obj").mkdir(parents=True)
            (module / "build/Release/obj/x.tlog").write_text("x")
            (module / "build/Release/addon.node").write_bytes(b"binary")
            (module / "build/Release/addon.pdb").write_bytes(b"symbols")
        (modules / "@vscode/windows-process-tree/build/binding.sln").write_text("{}")
        portable.prune_native_builds(modules)
        pruned = modules / "@vscode/windows-process-tree/build"
        self.assertEqual([p.name for p in pruned.rglob("*") if p.is_file()], ["addon.node"])
        # A `build` folder that is not node-gyp output (no config.gypi or binding.sln) is left alone.
        self.assertTrue((modules / "plain-module/build/Release/addon.pdb").is_file())

    def test_windows_installer_inputs_with_paths_past_the_limit_are_named_before_building(self):
        bundle = self.root / "bundle"
        deep = bundle / ("d" * 40) / ("e" * 40)
        deep.mkdir(parents=True)
        (deep / "ok.txt").write_text("x")
        packages.check_path_lengths(bundle, limit=len(str(deep / "ok.txt")))
        with self.assertRaises(ValueError) as caught:
            packages.check_path_lengths(bundle, limit=len(str(deep / "ok.txt")) - 1)
        self.assertIn("ok.txt", str(caught.exception))

    def test_the_appimage_tool_only_analyzes_libraries_that_can_load_and_resolve(self):
        def elf(relative, machine=0x3E):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"ELF" + bytes([2, 1]) + bytes(12) + machine.to_bytes(2, "little") + bytes(8))
            return path
        wanted = elf("host/node_modules/node-pty/prebuilds/linux-x64/pty.node")
        elf("host/node_modules/node-pty/prebuilds/linux-arm64/pty.node", machine=0xB7)
        elf("host/node_modules/@koromix/koffi-linux-x64/musl_x64/koffi.node")
        elf("host/node_modules/foundry-local-sdk/prebuilds/linux-x64/libfoundry_local.so")
        (self.root / "host/node_modules/other").mkdir(parents=True)
        (self.root / "host/node_modules/other/win.node").write_bytes(b"MZ" + bytes(30))
        (self.root / "host/node_modules/other/readme.txt").write_text("not a library")
        self.assertEqual(appimage.deployable_libraries(self.root / "host", self.root / "missing"),
                         [wanted])

    def test_complete_portable_archive_preserves_helpers_and_installs_license(self):
        archive = self.root / "complete.zip"
        with zipfile.ZipFile(archive, "w") as output:
            for name in ("graphite.exe", "code.exe", "graphite-extension-host.exe", "code-oss/commit",
                         "THIRD_PARTY_NOTICES.txt", "licenses/icon-font/NOTICE.md"):
                output.writestr("bundle/" + name, "synthetic fixture")
        extraction = self.root / "complete"
        extraction.mkdir()
        bundle = packages.unpack(archive, extraction)
        self.assertEqual((bundle / "code.exe").read_text(), "synthetic fixture")
        self.assertEqual((bundle / "LICENSE").read_bytes(), (metadata.ROOT / "LICENSE").read_bytes())

    def test_a_portable_archive_without_the_third_party_notices_is_refused(self):
        base = ["graphite.exe", "code.exe", "graphite-extension-host.exe", "code-oss/commit"]
        for missing in ("THIRD_PARTY_NOTICES.txt", "licenses/icon-font/NOTICE.md"):
            with self.subTest(missing=missing):
                archive = self.root / "incomplete.zip"
                with zipfile.ZipFile(archive, "w") as output:
                    for name in base + ["THIRD_PARTY_NOTICES.txt", "licenses/icon-font/NOTICE.md"]:
                        if name != missing:
                            output.writestr("bundle/" + name, "synthetic fixture")
                extraction = self.root / ("incomplete-" + missing.replace("/", "-"))
                extraction.mkdir()
                with self.assertRaisesRegex(ValueError, "THIRD_PARTY_NOTICES|NOTICE.md"):
                    packages.unpack(archive, extraction)

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
