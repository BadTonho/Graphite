"""Build the portable Graphite distribution on its target operating system."""

import argparse
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parents[1]
COMMIT = "c4453f8ea430795f941c1b2ff6c9321135a74a02"
NODE = "24.22.1"
SOURCE_HASH = "4b271369c048cdf098937af73f2542c83ac2f5d1027e946f8d35d8aeced58fdb"
NODE_HASHES = {
    "win-x64.zip": "d2046d28c2c3dcd211ac5145c0bf30e2047ef9dfb306693c8c6e2eccd8b95fa5",
    "linux-x64.tar.xz": "6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6",
}


def fetch(url, path, checksum):
    if not path.exists():
        partial = path.with_suffix(".part")
        with urllib.request.urlopen(url, timeout=120) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        partial.replace(path)
    with path.open("rb") as archive:
        actual = hashlib.file_digest(archive, "sha256").hexdigest()
    if actual != checksum:
        raise ValueError(f"SHA-256 mismatch for {path}")
    return path


def run(args, directory, env=None):
    subprocess.run(args, cwd=directory, env=env, check=True)


def extract_regular_files(archive, destination):
    """Extracts only the regular files and directories of a tar archive into `destination`."""
    with tarfile.open(archive) as source:
        members = [entry for entry in source if entry.isfile() or entry.isdir()]
        if getattr(tarfile, "data_filter", None):
            source.extractall(destination, members=members, filter="data")
            return
        # Python before 3.11.4 (the system Python of Debian 12) has no extraction filters.
        root = Path(destination).resolve()
        for entry in members:
            if not (root / entry.name).resolve().is_relative_to(root):
                raise ValueError("Archive path escapes extraction directory")
        source.extractall(destination, members=members)


def prune_native_build(module):
    """Keeps only the compiled `build/Release/*.node` files of a rebuilt native module.

    The rest of `build/` is compiler output (object files, `.tlog` state, a nested copy of
    node-addon-api). It is never loaded, and its paths can pass the 260 characters that the
    Windows installer compiler can open.
    """
    build_directory = module / "build"
    if not build_directory.is_dir():
        return
    release = build_directory / "Release"
    kept = {path.name: path.read_bytes() for path in release.glob("*.node")} if release.is_dir() else {}
    shutil.rmtree(build_directory)
    if kept:
        release.mkdir(parents=True)
        for name, content in kept.items():
            (release / name).write_bytes(content)


def prune_native_builds(node_modules):
    """Applies `prune_native_build` to every module that was built with node-gyp.

    node-gyp leaves `config.gypi` or `binding.sln` in the `build` folder; which one depends on
    the module and the platform.
    """
    markers = list(node_modules.rglob("config.gypi")) + list(node_modules.rglob("binding.sln"))
    for marker in markers:
        if marker.parent.name == "build" and marker.parent.is_dir():
            prune_native_build(marker.parent.parent)


def build(cache, runtime, work, windows):
    node_suffix = "win-x64.zip" if windows else "linux-x64.tar.xz"
    node_name = f"node-v{NODE}-{node_suffix}"
    node_archive = fetch(
        f"https://nodejs.org/dist/v{NODE}/{node_name}", cache / node_name, NODE_HASHES[node_suffix]
    )
    source_archive = fetch(
        f"https://codeload.github.com/microsoft/vscode/tar.gz/{COMMIT}",
        cache / f"vscode-{COMMIT}.tar.gz",
        SOURCE_HASH,
    )
    shutil.unpack_archive(node_archive, work)
    extract_regular_files(source_archive, work)
    node_home = work / f"node-v{NODE}-{node_suffix.split('.')[0]}"
    node_bin = node_home if windows else node_home / "bin"
    node = node_bin / ("node.exe" if windows else "node")
    npm = node_home / ("node_modules/npm/bin/npm-cli.js" if windows else "lib/node_modules/npm/bin/npm-cli.js")
    source = work / f"vscode-{COMMIT}"
    env = dict(os.environ, PATH=f"{node_bin}{os.pathsep}{os.environ['PATH']}",
               npm_config_cache=str(cache / "npm-cache"),
               npm_config_devdir=str(cache / "node-gyp"),
               npm_config_runtime="node", npm_config_target=NODE,
               npm_config_disturl="https://nodejs.org/dist")
    npm_flags = ["--ignore-scripts", "--no-audit", "--no-fund"]
    run([node, npm, "ci", *npm_flags], source, env)
    run([node, npm, "ci", *npm_flags], source / "build", env)
    run([node, "build/next/index.ts", "transpile", "--exclude-tests"], source, env)
    run([node, npm, "prune", "--omit=dev", *npm_flags], source, env)
    run([node, npm, "rebuild", "@vscode/native-watchdog", "@vscode/spdlog",
         "--no-audit", "--no-fund"], source, env)
    prune_native_builds(source / "node_modules")

    runtime.mkdir()
    shutil.copy2(node, runtime)
    shutil.copy2(node_home / "LICENSE", runtime / "NODE-LICENSE")
    shutil.copy2(ROOT / "packaging/code-oss/bridge.mjs", runtime)
    code = runtime / "code"
    code.mkdir()
    for name in ("out", "node_modules"):
        shutil.move(source / name, code / name)
    for name in ("package.json", "product.json", "LICENSE.txt", "ThirdPartyNotices.txt"):
        shutil.copy2(source / name, code)
    (runtime / "commit").write_text(COMMIT + "\n", encoding="ascii")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=ROOT / "target/phase5-inputs")
    parser.add_argument("--output", type=Path, default=ROOT / "target/portable")
    args = parser.parse_args()
    windows = platform.system() == "Windows"
    if platform.system() not in ("Windows", "Linux") or platform.machine().lower() not in ("x86_64", "amd64"):
        parser.error("Build this package on Windows or Linux x86-64")
    cache, output = args.cache.resolve(), args.output.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    run(["cargo", "build", "--workspace", "--release", "--locked", "--target-dir",
         ROOT / "target"], ROOT)
    name = "graphite-windows-x64" if windows else "graphite-linux-x64"
    with tempfile.TemporaryDirectory(prefix="build-", dir=output) as temporary:
        work = Path(temporary)
        package = work / name
        package.mkdir()
        build(cache, package / "code-oss", work, windows)
        for binary in ("graphite", "code", "graphite-extension-host"):
            binary += ".exe" if windows else ""
            shutil.copy2(ROOT / "target/release" / binary, package)
        shutil.copy2(ROOT / "LICENSE", package / "LICENSE")
        run([sys.executable, ROOT / "packaging/release/licenses.py", "--output-dir", package], ROOT)
        archive = shutil.make_archive(str(output / name), "zip" if windows else "gztar", work, name)
        print(archive)


if __name__ == "__main__":
    main()
