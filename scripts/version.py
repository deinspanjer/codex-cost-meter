import argparse
import gzip
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile


VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def parse_version(value):
    match = VERSION.fullmatch(value)
    if not match:
        raise ValueError(f"invalid version: {value}")
    return tuple(int(part) for part in match.groups())


def package_block(cargo_toml):
    match = re.search(r"(?ms)^\[package\]\s*$.*?(?=^\[|\Z)", cargo_toml)
    if not match:
        raise ValueError("Cargo.toml has no [package] section")
    return match


def package_metadata(cargo_toml):
    block = package_block(cargo_toml).group()
    name = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', block)
    version = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', block)
    if not name or not version:
        raise ValueError("[package] requires name and version")
    parse_version(version.group(1))
    return name.group(1), version.group(1)


def read_package(root):
    return package_metadata((root / "Cargo.toml").read_text(encoding="utf-8"))


def next_version(current, selector):
    major, minor, patch = parse_version(current)
    if selector == "patch":
        target = (major, minor, patch + 1)
    elif selector == "minor":
        target = (major, minor + 1, 0)
    elif selector == "major":
        target = (major + 1, 0, 0)
    else:
        target = parse_version(selector)
    if target <= (major, minor, patch):
        raise ValueError("new version must be greater than current version")
    return ".".join(str(part) for part in target)


def unreleased(changelog):
    match = re.search(r"(?ms)^## \[Unreleased\]\s*$\n(.*?)(?=^## |\Z)", changelog)
    if not match:
        raise ValueError("CHANGELOG.md has no [Unreleased] section")
    body = match.group(1).strip()
    if not body:
        raise ValueError("[Unreleased] section must not be empty")
    return match, body


def replace_package_version(cargo_toml, target):
    block = package_block(cargo_toml)
    replacement, count = re.subn(
        r'(?m)^version\s*=\s*"[^"]+"\s*$',
        f'version = "{target}"',
        block.group(),
        count=1,
    )
    if count != 1:
        raise ValueError("[package] requires a version")
    return cargo_toml[: block.start()] + replacement + cargo_toml[block.end() :]


def rotate_changelog(changelog, target):
    match, body = unreleased(changelog)
    replacement = f"## [Unreleased]\n\n## [{target}]\n\n{body}\n\n"
    return changelog[: match.start()] + replacement + changelog[match.end() :]


def lock_matches(root, name, version):
    lock = root / "Cargo.lock"
    if not lock.exists():
        return False
    for block in re.findall(r"(?ms)^\[\[package\]\]\s*$.*?(?=^\[\[package\]\]|\Z)", lock.read_text(encoding="utf-8")):
        lock_name = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', block)
        lock_version = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', block)
        if (
            lock_name
            and lock_version
            and lock_name.group(1) == name
            and not re.search(r"(?m)^source\s*=", block)
        ):
            return lock_version.group(1) == version
    return False


def verify(root):
    name, version = read_package(root)
    if not lock_matches(root, name, version):
        raise ValueError("Cargo.lock package version does not match Cargo.toml")


def bump(root, selector):
    cargo_path = root / "Cargo.toml"
    changelog_path = root / "CHANGELOG.md"
    cargo_toml = cargo_path.read_text(encoding="utf-8")
    changelog = changelog_path.read_text(encoding="utf-8")
    _, current = package_metadata(cargo_toml)
    target = next_version(current, selector)
    unreleased(changelog)
    cargo_path.write_text(replace_package_version(cargo_toml, target), encoding="utf-8")
    changelog_path.write_text(rotate_changelog(changelog, target), encoding="utf-8")
    subprocess.run(["cargo", "check"], cwd=root, check=True)
    verify(root)
    return target


def notes(root, version):
    parse_version(version)
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    match = re.search(
        rf"(?ms)^## \[{re.escape(version)}\]\s*$\n(.*?)(?=^## |\Z)", changelog
    )
    if not match or not match.group(1).strip():
        raise ValueError(f"no release notes for {version}")
    return match.group(1).strip() + "\n"


def tagged(root, version):
    result = subprocess.run(
        ["git", "tag", "--list", f"v{version}", version],
        cwd=root,
        text=True,
        capture_output=True,
    )
    return result.returncode == 0 and bool(result.stdout.strip())


def changed(root, _before):
    _, version = read_package(root)
    return version != "0.0.0" and not tagged(root, version)


def package(root, binary, output_dir, platform="macos-universal2"):
    name, version = read_package(root)
    verify(root)
    binary = binary.resolve()
    if binary.name != name:
        raise ValueError(f"binary must be named {name}")
    if not binary.is_file():
        raise ValueError(f"binary does not exist: {binary}")

    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / f"{name}-v{version}-{platform}.tar.gz"
    checksum = archive.with_suffix(archive.suffix + ".sha256")
    with tempfile.TemporaryDirectory() as temporary_directory:
        staging = Path(temporary_directory)
        files = (
            (binary, name, 0o755),
            (root / "README.md", "README.md", 0o644),
            (root / "LICENSE", "LICENSE", 0o644),
        )
        for source, member, _ in files:
            if not source.is_file():
                raise ValueError(f"package input does not exist: {source}")
            shutil.copyfile(source, staging / member)
        with archive.open("wb") as raw_archive:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw_archive, mtime=0) as compressed:
                with tarfile.open(mode="w", fileobj=compressed) as tar:
                    for _, member, mode in files:
                        staged = staging / member
                        info = tarfile.TarInfo(member)
                        info.size = staged.stat().st_size
                        info.mode = mode
                        info.mtime = info.uid = info.gid = 0
                        info.uname = info.gname = ""
                        with staged.open("rb") as source:
                            tar.addfile(info, source)
    checksum.write_text(
        f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n",
        encoding="utf-8",
    )
    return archive


def package_linux(root, binary, architecture, output_dir):
    if architecture not in {"x86_64", "aarch64"}:
        raise ValueError("architecture must be x86_64 or aarch64")
    return package(root, binary, output_dir, f"linux-{architecture}-musl")


def package_windows(root, binary, output_dir):
    name, version = read_package(root)
    verify(root)
    binary = binary.resolve()
    if binary.name != f"{name}.exe":
        raise ValueError(f"binary must be named {name}.exe")
    if not binary.is_file():
        raise ValueError(f"binary does not exist: {binary}")

    files = (
        (binary, f"{name}.exe", 0o755),
        (root / "README.md", "README.md", 0o644),
        (root / "LICENSE", "LICENSE", 0o644),
    )
    for source, _, _ in files:
        if not source.is_file():
            raise ValueError(f"package input does not exist: {source}")

    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / f"{name}-v{version}-windows-x64.zip"
    checksum = archive.with_suffix(archive.suffix + ".sha256")
    with zipfile.ZipFile(archive, "w") as package_file:
        for source, member, mode in files:
            info = zipfile.ZipInfo(member, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (0o100000 | mode) << 16
            package_file.writestr(
                info,
                source.read_bytes(),
                compress_type=zipfile.ZIP_DEFLATED,
                compresslevel=9,
            )
    checksum.write_text(
        f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n",
        encoding="utf-8",
    )
    return archive


def update_formula(root, checksum):
    if not SHA256.fullmatch(checksum):
        raise ValueError("SHA-256 must be 64 lowercase hexadecimal characters")
    name, version = read_package(root)
    formula_path = root / "Formula" / f"{name}.rb"
    formula = formula_path.read_text(encoding="utf-8")
    formula, url_count = re.subn(
        r'(?m)^(  url "https://github\.com/[^\"]+/archive/refs/tags/)v\d+\.\d+\.\d+(\.tar\.gz")$',
        rf"\g<1>v{version}\g<2>",
        formula,
    )
    formula, checksum_count = re.subn(
        r'(?m)^(  sha256 ")[0-9a-f]{64}(")$',
        rf"\g<1>{checksum}\g<2>",
        formula,
    )
    if url_count != 1 or checksum_count != 1:
        raise ValueError("formula requires one tagged source URL and SHA-256")
    formula_path.write_text(formula, encoding="utf-8")
    return formula_path


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    bump_parser = commands.add_parser("bump")
    bump_parser.add_argument("selector")
    commands.add_parser("current")
    notes_parser = commands.add_parser("notes")
    notes_parser.add_argument("version")
    changed_parser = commands.add_parser("changed")
    changed_parser.add_argument("--before", required=True)
    package_parser = commands.add_parser("package")
    package_parser.add_argument("--binary", type=Path, required=True)
    package_parser.add_argument("--output-dir", type=Path, required=True)
    package_linux_parser = commands.add_parser("package-linux")
    package_linux_parser.add_argument("--architecture", required=True)
    package_linux_parser.add_argument("--binary", type=Path, required=True)
    package_linux_parser.add_argument("--output-dir", type=Path, required=True)
    package_windows_parser = commands.add_parser("package-windows")
    package_windows_parser.add_argument("--binary", type=Path, required=True)
    package_windows_parser.add_argument("--output-dir", type=Path, required=True)
    update_formula_parser = commands.add_parser("update-formula")
    update_formula_parser.add_argument("--sha256", required=True)
    commands.add_parser("verify")
    arguments = parser.parse_args()
    root = Path.cwd()
    try:
        if arguments.command == "bump":
            print(bump(root, arguments.selector))
        elif arguments.command == "current":
            print(read_package(root)[1])
        elif arguments.command == "notes":
            print(notes(root, arguments.version), end="")
        elif arguments.command == "changed":
            print(str(changed(root, arguments.before)).lower())
        elif arguments.command == "package":
            print(package(root, arguments.binary, arguments.output_dir))
        elif arguments.command == "package-linux":
            print(
                package_linux(
                    root, arguments.binary, arguments.architecture, arguments.output_dir
                )
            )
        elif arguments.command == "package-windows":
            print(package_windows(root, arguments.binary, arguments.output_dir))
        elif arguments.command == "update-formula":
            print(update_formula(root, arguments.sha256))
        else:
            verify(root)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
