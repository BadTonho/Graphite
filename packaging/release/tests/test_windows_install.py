"""Opt-in smoke test of a real Graphite NSIS installer in a disposable Windows account."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite"
INSTALLER = os.environ.get("GRAPHITE_TEST_WINDOWS_INSTALLER")


def registered_uninstaller():
    import winreg

    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY)
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
    def test_installer_registers_application_and_uninstaller_removes_it(self):
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
            installed_files = (exe, installed / "code.exe", installed / "code-oss" / "commit")
            # NSIS requires /D and _?= to be last and unquoted, even when the path contains spaces.
            install_command = f"{subprocess.list2cmdline([str(installer), '/S'])} /D={installed}"
            try:
                subprocess.run(install_command, check=True, timeout=120)
                for path in (*installed_files, uninstaller):
                    self.assertTrue(path.is_file(), str(path))
                self.assertTrue(shortcut.is_file())
                self.assertEqual(registered_uninstaller(), f'"{uninstaller}"')

                env = dict(os.environ, APPDATA=str(root / "profile"), GRAPHITE_LANG="en")
                subprocess.run([exe, "plugin", "create", "first-launch"], cwd=root, env=env,
                               check=True, timeout=30)
                self.assertTrue((root / "first-launch" / "graphite-plugin.json").is_file())
            finally:
                if uninstaller.is_file():
                    uninstall_command = f"{subprocess.list2cmdline([str(uninstaller), '/S'])} _?={installed}"
                    subprocess.run(uninstall_command, check=True, timeout=120)

            for path in (*installed_files, shortcut):
                self.assertFalse(path.exists(), str(path))
            self.assertIsNone(registered_uninstaller())
