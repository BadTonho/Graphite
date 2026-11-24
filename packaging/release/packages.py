"""Convert the verified portable distribution into native release packages."""

import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

from metadata import ROOT, release_version


def run(args, **kwargs):
    try:
        return subprocess.run(list(map(str, args)), check=True, **kwargs)
    except subprocess.CalledProcessError as error:
        # Captured output would otherwise vanish with the exception and hide the cause.
        for stream in (error.stdout, error.stderr):
            if stream:
                print(stream, file=sys.stderr)
        raise


def is_linux_x64_glibc_binary(path):
    """Whether `path` is an x86-64 ELF file that a glibc system can link and load.

    Native modules ship prebuilds for other systems (Windows, macOS, ARM, musl). The tool that
    derives package dependencies can only analyze binaries for the system it runs on.
    """
    if "musl" in str(path).lower():
        return False
    try:
        with open(path, "rb") as stream:
            header = stream.read(20)
    except OSError:
        return False
    # ELF magic, 64-bit, little endian, then e_machine 0x3E (x86-64) at offset 18.
    return header[:4] == b"\x7fELF" and header[4] == 2 and header[5] == 1 \
        and int.from_bytes(header[18:20], "little") == 0x3E


def check_tar_members(package, destination):
    """Refuses members that would land outside `destination` or are not files, folders or links."""
    root = destination.resolve()
    for member in package.getmembers():
        target = (root / member.name).resolve()
        if not target.is_relative_to(root):
            raise ValueError("Archive path escapes extraction directory")
        if member.issym() or member.islnk():
            base = target.parent if member.issym() else root
            if not (base / member.linkname).resolve().is_relative_to(root):
                raise ValueError("Archive link escapes extraction directory")
        elif not (member.isfile() or member.isdir()):
            raise ValueError("Archive contains an unsupported entry type")


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
            if getattr(tarfile, "data_filter", None):
                package.extractall(destination, filter="data")
            else:
                # Python before 3.11.4 (the system Python of Debian 12) has no extraction filters.
                check_tar_members(package, destination)
                package.extractall(destination)
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
    for required in ("THIRD_PARTY_NOTICES.txt", "licenses/icon-font/NOTICE.md"):
        if not (bundle / required).is_file():
            raise ValueError(f"Portable archive is missing {required}")
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
    shutil.copy2(bundle / "THIRD_PARTY_NOTICES.txt", licenses)
    shutil.copytree(bundle / "licenses/icon-font", licenses / "icon-font")


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
    elf.extend(path for path in (root / "usr/lib/graphite/code-oss").rglob("*.node")
               if is_linux_x64_glibc_binary(path))
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
        "%license /usr/share/licenses/graphite\n", encoding="utf-8")
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


# The installer compiler opens files through APIs limited to 260 characters (MAX_PATH).
WINDOWS_PATH_LIMIT = 259


def check_path_lengths(bundle, limit=WINDOWS_PATH_LIMIT):
    """Fails early, naming the files, when the installer compiler could not open some of them."""
    too_long = sorted(str(path.relative_to(bundle)) for path in bundle.rglob("*")
                      if len(str(path)) > limit)
    if too_long:
        raise ValueError(f"{len(too_long)} files have paths longer than {limit} characters "
                         f"(first: {too_long[0]})")


def windows_installer(bundle, output, version, work):
    check_path_lengths(bundle)
    components = split_windows_components(bundle, work / "components")
    check_path_lengths(components)
    compiler = shutil.which("makensis") or str(Path(r"C:\Program Files (x86)\NSIS\makensis.exe"))
    source = (ROOT / "packaging/release/windows.nsi").read_text(encoding="utf-8")
    for key, value in {"@BUNDLE@": bundle, "@COMPONENTS@": components,
                       "@OUTPUT@": output, "@VERSION@": version}.items():
        source = source.replace(key, str(value))
    script = work / "installer.nsi"
    script.write_text(source, encoding="utf-8-sig")
    run([compiler, str(script)])


def split_windows_components(bundle, components):
    """Partition the disposable extraction tree, keeping the published ZIP intact."""
    runtime = bundle / "code-oss"
    modules_path = Path("code/node_modules")
    optional_packages = {
        "copilot": ("@github/copilot-sdk", "@github/copilot-sdk-win32-x64"),
        "foundry": ("foundry-local-sdk",),
    }
    for packages in optional_packages.values():
        for package in packages:
            if not (runtime / modules_path / package / "package.json").is_file():
                raise ValueError(f"Windows portable archive is missing the installer component package {package}")
    for component, packages in optional_packages.items():
        for package in packages:
            relative = modules_path / package
            destination = components / component / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            (runtime / relative).rename(destination)
    runtime.rename(components / "vsix")
    return components


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
