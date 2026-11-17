"""Convert the verified portable distribution into native release packages."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import zipfile

from metadata import ROOT, release_version


def run(args, **kwargs):
    return subprocess.run(list(map(str, args)), check=True, **kwargs)


def unpack(archive, destination):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            for name in package.namelist():
                target = (destination / name).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise ValueError("Archive path escapes extraction directory")
            package.extractall(destination)
    else:
        with tarfile.open(archive) as package:
            package.extractall(destination, filter="data")
    directories = list(destination.iterdir())
    if len(directories) != 1 or not directories[0].is_dir():
        raise ValueError("Portable archive must contain exactly one directory")
    bundle = directories[0]
    windows = archive.suffix == ".zip"
    for name in ("graphite", "code", "graphite-extension-host"):
        if not (bundle / (name + (".exe" if windows else ""))).is_file():
            raise ValueError(f"Portable archive is missing {name}")
    if not (bundle / "code-oss/commit").is_file():
        raise ValueError("Portable archive is missing the Code OSS runtime")
    shutil.copy2(ROOT / "LICENSE", bundle / "LICENSE")
    return bundle


def native_version(version, kind):
    main, separator, prerelease = version.partition("-")
    return main + (("" if kind == "arch" else "~") + prerelease.replace("-", ".")
                   if separator else "")


def linux_tree(bundle, root):
    destination = root / "usr/lib/graphite"
    shutil.copytree(bundle, destination, symlinks=True)
    binaries = root / "usr/bin"
    binaries.mkdir(parents=True)
    for command, executable in (("graphite", "graphite"), ("graphite-code", "code")):
        path = binaries / command
        path.write_text(f'#!/bin/sh\nexec /usr/lib/graphite/{executable} "$@"\n', encoding="utf-8")
        path.chmod(0o755)
    applications = root / "usr/share/applications"
    applications.mkdir(parents=True)
    shutil.copy2(ROOT / "packaging/linux/graphite.desktop", applications)
    icons = root / "usr/share/pixmaps"
    icons.mkdir(parents=True)
    shutil.copy2(ROOT / "assets/branding/graphite.png", icons / "graphite.png")
    licenses = root / "usr/share/licenses/graphite"
    licenses.mkdir(parents=True)
    shutil.copy2(ROOT / "LICENSE", licenses)


def deb(root, output, version, work):
    # Derive ABI/version requirements on the actual Debian/Ubuntu builder.
    control_dir = work / "debian"
    control_dir.mkdir()
    (control_dir / "control").write_text(
        "Source: graphite\nSection: editors\nPriority: optional\n"
        "Maintainer: Graphite maintainers\n\nPackage: graphite\nArchitecture: amd64\n"
        "Description: Native code editor\n", encoding="utf-8")
    elf = [root / "usr/lib/graphite/graphite", root / "usr/lib/graphite/code",
           root / "usr/lib/graphite/graphite-extension-host", root / "usr/lib/graphite/code-oss/node"]
    elf.extend((root / "usr/lib/graphite/code-oss").rglob("*.node"))
    result = run(["dpkg-shlibdeps", "-O", "--ignore-missing-info",
                  *[f"-e{p}" for p in elf]], cwd=work, capture_output=True, text=True)
    dependencies = next(line.split("=", 1)[1] for line in result.stdout.splitlines()
                        if line.startswith("shlibs:Depends="))
    metadata = root / "DEBIAN"
    metadata.mkdir()
    (metadata / "control").write_text(
        f"Package: graphite\nVersion: {native_version(version, 'deb')}\nArchitecture: amd64\n"
        "Section: editors\nPriority: optional\nMaintainer: Graphite maintainers\n"
        f"Depends: {dependencies}, libgtk-3-0, libwebkit2gtk-4.1-0, libxkbcommon0\n"
        "Recommends: xdg-utils\nDescription: Native Rust code editor with bundled extension host\n",
        encoding="utf-8")
    run(["dpkg-deb", "--root-owner-group", "--build", root, output])
    run(["dpkg-deb", "--info", output])


def rpm(root, output, version, work):
    spec = work / "graphite.spec"
    spec.write_text(
        f"Name: graphite\nVersion: {native_version(version, 'rpm')}\nRelease: 1\n"
        "Summary: Native Rust code editor\nLicense: MIT\nBuildArch: x86_64\n"
        "Requires: gtk3, webkit2gtk4.1, libxkbcommon\n"
        "%global debug_package %{nil}\n%global __os_install_post %{nil}\n"
        "%description\nNative editor with bundled Node and Code OSS extension host.\n"
        "%install\nmkdir -p \"%{buildroot}\"\n"
        f'cp -a "{root}/." "%{{buildroot}}/"\n'
        "%files\n/usr/lib/graphite\n/usr/bin/graphite\n/usr/bin/graphite-code\n"
        "/usr/share/applications/graphite.desktop\n"
        "/usr/share/pixmaps/graphite.png\n"
        "%license /usr/share/licenses/graphite/LICENSE\n", encoding="utf-8")
    run(["rpmbuild", "-bb", "--define", f"_topdir {work / 'rpmbuild'}", spec])
    packages = list((work / "rpmbuild/RPMS").rglob("*.rpm"))
    if len(packages) != 1:
        raise ValueError("Expected one RPM")
    shutil.copy2(packages[0], output)
    run(["rpm", "-qip", output])


def arch(root, output, version, work):
    (work / "PKGBUILD").write_text(
        f"pkgname=graphite\npkgver={native_version(version, 'arch')}\npkgrel=1\n"
        "pkgdesc='Native Rust code editor with bundled extension host'\n"
        "arch=('x86_64')\nlicense=('MIT')\n"
        "depends=('gtk3' 'webkit2gtk-4.1' 'libxkbcommon' 'gcc-libs' 'glibc')\n"
        "options=('!strip' '!debug')\n"
        'package() { cp -a "$startdir/payload/." "$pkgdir/"; }\n', encoding="utf-8")
    # makepkg deliberately refuses root. Only the disposable staging tree changes owner.
    root.rename(work / "payload")
    run(["chown", "-R", "builder:builder", work])
    run(["runuser", "-u", "builder", "--", "makepkg", "--nodeps", "--noconfirm"], cwd=work)
    packages = list(work.glob("*.pkg.tar.zst"))
    if len(packages) != 1:
        raise ValueError("Expected one Arch package")
    shutil.copy2(packages[0], output)
    run(["bsdtar", "-xOf", output, ".PKGINFO"])


def windows_installer(bundle, output, version, work):
    compiler = shutil.which("makensis") or str(Path(r"C:\Program Files (x86)\NSIS\makensis.exe"))
    source = (ROOT / "packaging/release/windows.nsi").read_text(encoding="utf-8")
    for key, value in {"@BUNDLE@": str(bundle), "@OUTPUT@": str(output), "@VERSION@": version}.items():
        source = source.replace(key, value)
    script = work / "installer.nsi"
    script.write_text(source, encoding="utf-8")
    run([compiler, str(script)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--target", required=True,
                        choices=("windows", "debian12", "ubuntu22.04", "fedora44", "arch"))
    parser.add_argument("--portable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    version = release_version(args.tag)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="graphite-package-") as temporary:
        work = Path(temporary)
        extraction = work / "extracted"
        extraction.mkdir()
        bundle = unpack(args.portable.resolve(), extraction)
        if args.target in ("windows", "ubuntu22.04"):
            platform = "windows" if args.target == "windows" else "linux"
            suffix = ".zip" if platform == "windows" else ".tar.gz"
            shutil.copy2(args.portable, output / f"graphite-{args.tag}-{platform}-x64{suffix}")
            with zipfile.ZipFile(output / f"graphite-update-{args.tag}-{platform}-x64.zip",
                                 "w", zipfile.ZIP_DEFLATED) as archive:
                binary = "graphite.exe" if platform == "windows" else "graphite"
                archive.write(bundle / binary, binary)
        if args.target == "windows":
            windows_installer(bundle, output / f"graphite-{args.tag}-windows-x64-setup.exe", version, work)
            return
        root = work / "root"
        linux_tree(bundle, root)
        if args.target in ("debian12", "ubuntu22.04"):
            deb(root, output / f"graphite-{args.tag}-linux-{args.target}-x64.deb", version, work)
        elif args.target == "fedora44":
            rpm(root, output / f"graphite-{args.tag}-linux-fedora44-x64.rpm", version, work)
        else:
            arch(root, output / f"graphite-{args.tag}-linux-arch-x64.pkg.tar.zst", version, work)
        if args.target == "ubuntu22.04":
            from appimage import build_appimage
            build_appimage(bundle, output / f"graphite-{args.tag}-linux-x64.AppImage", work)


if __name__ == "__main__":
    main()
