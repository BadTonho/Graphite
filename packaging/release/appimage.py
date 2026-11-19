"""AppImage assembly on Ubuntu 22.04; GTK/WebKit subprocesses travel with the app."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import urllib.request

from metadata import ROOT
from packages import is_linux_x64_glibc_binary, run

LINUXDEPLOY_URL = ("https://github.com/linuxdeploy/linuxdeploy/releases/download/"
                   "1-alpha-20251107-1/linuxdeploy-x86_64.AppImage")
LINUXDEPLOY_SHA256 = "c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d"


def checked_download(url, destination, expected):
    with urllib.request.urlopen(url, timeout=120) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)
    with destination.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != expected:
        destination.unlink()
        raise ValueError("AppImage tool checksum mismatch")
    destination.chmod(0o755)


# Packages whose libraries need dependencies that the package itself does not ship. They are
# loaded from the bundled host's own folder, so the deploy tool has nothing to resolve for them.
UNRESOLVABLE_PACKAGES = ("foundry-local-sdk",)


def deployable_libraries(*subtrees):
    """Libraries the deploy tool should analyze: x86-64 glibc ELF files that can actually load."""
    libraries = []
    for subtree in subtrees:
        for library in sorted(subtree.rglob("*")):
            if not library.is_file() or library.is_symlink():
                continue
            if not (".so" in library.name or library.suffix == ".node"):
                continue
            if any(package in library.parts for package in UNRESOLVABLE_PACKAGES):
                continue
            if is_linux_x64_glibc_binary(library):
                libraries.append(library)
    return libraries


def build_appimage(bundle, output, work):
    appdir = work / "Graphite.AppDir"
    binaries = appdir / "usr/bin"
    shutil.copytree(bundle, binaries, symlinks=True)
    desktop = appdir / "graphite.desktop"
    shutil.copy2(ROOT / "packaging/linux/graphite.desktop", desktop)
    icon = appdir / "graphite.png"
    shutil.copy2(ROOT / "assets/branding/graphite.png", icon)
    native_lib = Path("/usr/lib/x86_64-linux-gnu")
    resources = [
        (native_lib / "webkit2gtk-4.1", appdir / "usr/lib/webkit2gtk-4.1"),
        (native_lib / "gtk-3.0", appdir / "usr/lib/gtk-3.0"),
        (native_lib / "gdk-pixbuf-2.0", appdir / "usr/lib/gdk-pixbuf-2.0"),
        (Path("/usr/share/glib-2.0/schemas"), appdir / "usr/share/glib-2.0/schemas"),
        (Path("/usr/share/icons/Adwaita"), appdir / "usr/share/icons/Adwaita"),
        (Path("/usr/share/mime"), appdir / "usr/share/mime"),
    ]
    for source, destination in resources:
        if not source.is_dir():
            raise ValueError(f"Missing AppImage resource: {source}")
        shutil.copytree(source, destination, symlinks=True)
    run(["glib-compile-schemas", appdir / "usr/share/glib-2.0/schemas"])
    launcher = appdir / "AppRun"
    shutil.copy2(ROOT / "packaging/release/AppRun", launcher)
    launcher.chmod(0o755)
    tool = work / "linuxdeploy-x86_64.AppImage"
    checked_download(LINUXDEPLOY_URL, tool, LINUXDEPLOY_SHA256)
    env = dict(os.environ, APPIMAGE_EXTRACT_AND_RUN="1", ARCH="x86_64", OUTPUT=str(output))
    args = [tool, "--appdir", appdir, "--desktop-file", desktop, "--icon-file", icon]
    # Explicitly deploy dependencies of Node addons and GTK/WebKit helpers, not just Graphite.
    helpers = list(binaries.glob("graphite*")) + [binaries / "code", binaries / "code-oss/node"]
    helpers += list((appdir / "usr/lib/webkit2gtk-4.1").glob("*Process"))
    for binary in helpers:
        if binary.is_file():
            args += ["--executable", binary]
    for library in deployable_libraries(binaries / "code-oss", appdir / "usr/lib"):
        args += ["--library", library]
    run([*args, "--output", "appimage"], cwd=work, env=env)
    if not output.is_file():
        raise ValueError("linuxdeploy did not produce the requested AppImage")
    output.chmod(0o755)
    # Extraction needs neither FUSE nor a display. UI validation remains manual.
    inspection = work / "inspect-appimage"
    inspection.mkdir()
    run([output, "--appimage-extract"], cwd=inspection, stdout=subprocess.DEVNULL)
    for relative in ("AppRun", "usr/bin/graphite", "usr/bin/code-oss/commit",
                     "usr/lib/webkit2gtk-4.1/WebKitWebProcess"):
        if not (inspection / "squashfs-root" / relative).is_file():
            raise ValueError(f"Incomplete AppImage: {relative}")
