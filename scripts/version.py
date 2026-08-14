import argparse
from pathlib import Path
import re
import subprocess


VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")


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


def changed(root, before):
    _, version = read_package(root)
    if tagged(root, version):
        return False
    result = subprocess.run(
        ["git", "show", f"{before}:Cargo.toml"],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        return True
    try:
        _, previous = package_metadata(result.stdout)
    except ValueError:
        return True
    return previous != version


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
        else:
            verify(root)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
