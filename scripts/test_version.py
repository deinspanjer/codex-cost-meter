import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("version.py")


class VersionToolTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.write_project()

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write_project(
        self, name="fixture", version="0.0.0", unreleased="- Initial release work\n"
    ):
        (self.root / "Cargo.toml").write_text(
            "[package]\n"
            f"name = \"{name}\"\n"
            f"version = \"{version}\"\n"
            "edition = \"2024\"\n"
            "\n[package.metadata.fixture]\n"
            "version = \"metadata-version\"\n",
            encoding="utf-8",
        )
        (self.root / "src").mkdir(exist_ok=True)
        (self.root / "src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        (self.root / "CHANGELOG.md").write_text(
            "# Changelog\n\n## [Unreleased]\n\n" + unreleased,
            encoding="utf-8",
        )
        (self.root / "README.md").write_text("# Fixture\n", encoding="utf-8")
        (self.root / "LICENSE").write_text("MIT\n", encoding="utf-8")

    def run_tool(self, *arguments, success=True):
        result = subprocess.run(
            ["python3", str(SCRIPT), *arguments],
            cwd=self.root,
            text=True,
            capture_output=True,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_bump_calculates_semver_and_zero_minor(self):
        self.run_tool("bump", "minor")
        self.assertEqual(self.run_tool("current").stdout.strip(), "0.1.0")

    def test_bump_rejects_empty_unreleased(self):
        self.write_project(unreleased="")
        result = self.run_tool("bump", "patch", success=False)
        self.assertIn("Unreleased", result.stderr)
        self.assertEqual(self.run_tool("current").stdout.strip(), "0.0.0")

    def test_bump_creates_date_free_heading_and_only_replaces_package_version(self):
        self.run_tool("bump", "0.0.1")
        cargo_toml = (self.root / "Cargo.toml").read_text(encoding="utf-8")
        changelog = (self.root / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertIn('version = "0.0.1"', cargo_toml)
        self.assertIn('version = "metadata-version"', cargo_toml)
        self.assertIn("## [0.0.1]\n\n- Initial release work", changelog)
        self.assertNotIn("## [0.0.1] -", changelog)

    def test_bump_rejects_exact_version_regression(self):
        self.write_project(version="1.2.3")
        result = self.run_tool("bump", "1.2.2", success=False)
        self.assertIn("greater", result.stderr)

    def test_verify_requires_matching_package_and_lock_versions(self):
        (self.root / "Cargo.lock").write_text(
            "version = 4\n\n[[package]]\nname = \"fixture\"\nversion = \"9.9.9\"\n",
            encoding="utf-8",
        )
        self.run_tool("verify", success=False)

    def test_verify_uses_the_unsourced_root_package_entry(self):
        (self.root / "Cargo.lock").write_text(
            "version = 4\n\n"
            "[[package]]\nname = \"fixture\"\nversion = \"0.0.0\"\n"
            "source = \"registry+https://example.invalid/index\"\n\n"
            "[[package]]\nname = \"fixture\"\nversion = \"9.9.9\"\n",
            encoding="utf-8",
        )
        self.run_tool("verify", success=False)

    def test_verify_accepts_matching_unsourced_root_package_entry(self):
        (self.root / "Cargo.lock").write_text(
            "version = 4\n\n[[package]]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
            encoding="utf-8",
        )
        self.run_tool("verify")

    def test_notes_extracts_only_requested_release(self):
        (self.root / "CHANGELOG.md").write_text(
            "# Changelog\n\n## [Unreleased]\n\n- Next\n\n"
            "## [1.2.3]\n\n- Keep this\n\n## [1.2.2]\n\n- Not this\n",
            encoding="utf-8",
        )
        self.assertEqual(self.run_tool("notes", "1.2.3").stdout, "- Keep this\n")

    def test_changed_requires_a_nonzero_untagged_version(self):
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.com")
        self.git("config", "user.name", "Test")
        self.git("add", ".")
        self.git("commit", "-qm", "initial")
        before = self.git("rev-parse", "HEAD").stdout.strip()
        self.assertEqual(self.run_tool("changed", "--before", before).stdout.strip(), "false")
        self.run_tool("bump", "minor")
        self.git("add", ".")
        self.git("commit", "-qm", "release")
        self.assertEqual(self.run_tool("changed", "--before", before).stdout.strip(), "true")
        before_retry = self.git("rev-parse", "HEAD").stdout.strip()
        (self.root / "README.md").write_text("# Retry\n", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "retry")
        self.assertEqual(
            self.run_tool("changed", "--before", before_retry).stdout.strip(), "true"
        )
        self.git("tag", "v0.1.0")
        self.assertEqual(self.run_tool("changed", "--before", before).stdout.strip(), "false")

    def test_package_contains_only_the_release_binary_and_documents(self):
        self.write_project(name="codex-cost-meter")
        binary = self.root / "codex-cost-meter"
        binary.write_bytes(b"fixture binary")
        (self.root / "Cargo.lock").write_text(
            "version = 4\n\n[[package]]\nname = \"codex-cost-meter\"\nversion = \"0.0.0\"\n",
            encoding="utf-8",
        )
        output = self.root / "output"
        self.run_tool("package", "--binary", binary, "--output-dir", output)
        archive = output / "codex-cost-meter-v0.0.0-macos-universal2.tar.gz"
        with tarfile.open(archive, "r:gz") as package:
            self.assertEqual(package.getnames(), ["codex-cost-meter", "README.md", "LICENSE"])

    def git(self, *arguments):
        return subprocess.run(
            ["git", *arguments], cwd=self.root, text=True, capture_output=True, check=True
        )


if __name__ == "__main__":
    unittest.main()
