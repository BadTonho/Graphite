"""Opt-in smoke test of a real Graphite NSIS installer in a disposable Windows account."""

from itertools import product
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite"
INSTALLER = os.environ.get("GRAPHITE_TEST_WINDOWS_INSTALLER")
COMPONENTS = {
    "/NO-VSIX": ("commit", "bridge.mjs", "node.exe",
                 "code/out/vs/workbench/api/node/extensionHostProcess.js"),
    "/NO-COPILOT": ("code/node_modules/@github/copilot-sdk/package.json",
                    "code/node_modules/@github/copilot-sdk-win32-x64/package.json"),
    "/NO-FOUNDRY": ("code/node_modules/foundry-local-sdk/package.json",),
}


def registered_uninstaller():
    import winreg

    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY, 0,
                             winreg.KEY_READ | winreg.KEY_WOW64_32KEY)
    except FileNotFoundError:
        return None
    with key:
        return winreg.QueryValueEx(key, "UninstallString")[0]


def programs_directory():
    import ctypes

    path = ctypes.create_unicode_buffer(260)
    result = ctypes.windll.shell32.SHGetFolderPathW(None, 0x0002, None, 0, path)
    if result != 0:
        raise OSError(f"Unable to locate the current user's Programs folder: {result:#x}")
    return Path(path.value)


@unittest.skipUnless(os.name == "nt" and INSTALLER, "requires a Windows installer and a disposable account")
class WindowsInstallationTest(unittest.TestCase):
    def test_component_matrix_and_reinstallation(self):
        installer = Path(INSTALLER).resolve(strict=True)
        shortcut_dir = programs_directory() / "Graphite"
        self.assertIsNone(registered_uninstaller(), "Use a clean Windows account")
        self.assertFalse(shortcut_dir.exists(), "Use a clean Windows account")

        with tempfile.TemporaryDirectory(prefix="graphite-install-test-") as temporary:
            root = Path(temporary)
            installed = root / "installed Graphite"
            exe = installed / "graphite.exe"
            uninstaller = installed / "Uninstall.exe"
            shortcut = shortcut_dir / "Graphite.lnk"
            appdata = root / "appdata"
            settings = appdata / "Graphite" / "settings.json"
            settings.parent.mkdir(parents=True)
            settings.write_text('{"fixture": true}', encoding="utf-8")
            env = dict(os.environ, APPDATA=str(appdata), GRAPHITE_LANG="en")

            # NSIS requires /D and _?= to be last and unquoted, even when the path contains spaces.
            def install(omitted):
                args = [str(installer), "/S", *omitted]
                install_command = f"{subprocess.list2cmdline(args)} /D={installed}"
                subprocess.run(install_command, check=True, timeout=120, env=env)
                for path in (exe, installed / "code.exe", uninstaller):
                    self.assertTrue(path.is_file(), str(path))
                self.assertTrue(shortcut.is_file())
                runtime = installed / "code-oss"
                self.assertEqual(runtime.is_dir(), len(omitted) < len(COMPONENTS))
                for option, paths in COMPONENTS.items():
                    for path in paths:
                        self.assertEqual((runtime / path).is_file(), option not in omitted, path)
                self.assertEqual(registered_uninstaller(), f'"{uninstaller}"')

            def uninstall():
                if uninstaller.is_file():
                    command = f"{subprocess.list2cmdline([str(uninstaller), '/S'])} _?={installed}"
                    subprocess.run(command, check=True, timeout=120, env=env)

            for omit in product((False, True), repeat=len(COMPONENTS)):
                omitted = [flag for flag, skip in zip(COMPONENTS, omit) if skip]
                with self.subTest(omitted=omitted):
                    try:
                        install(omitted)
                    finally:
                        uninstall()
                    for path in (exe, installed / "code.exe", installed / "code-oss", shortcut):
                        self.assertFalse(path.exists(), str(path))
                    self.assertIsNone(registered_uninstaller())

            try:
                install([])
                install(["/NO-COPILOT", "/NO-FOUNDRY"])
                install(["/NO-VSIX"])
                install(list(COMPONENTS))
                subprocess.run([exe, "plugin", "create", "first-launch"], cwd=root, env=env,
                               check=True, timeout=30)
                self.assertTrue((root / "first-launch" / "graphite-plugin.json").is_file())
                install([])
            finally:
                uninstall()
            self.assertEqual(settings.read_text(encoding="utf-8"), '{"fixture": true}')
