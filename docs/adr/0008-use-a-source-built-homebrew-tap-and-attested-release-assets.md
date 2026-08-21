# 0008: Use a source-built Homebrew tap and attested release assets

Status: Accepted

Date: 2026-08-21

## Context

The project needs a lower-friction macOS install path without creating a signing, installer, or update service. Windows and Linux already have portable release assets, but no package-manager manifests. Platform trust must not be conflated with file integrity or installation convenience.

### Research basis

The design was checked against Homebrew's official tap, formula, tap-trust, acceptable-formula, and supply-chain guidance. Current `homebrew/core` Rust formulae were inspected for the source-build pattern, and Homebrew 6.0's installed implementation was checked to confirm that `std_cargo_args` supplies `--locked` and a fully qualified third-party formula install keeps trust scoped to that formula. A real temporary tap then passed style, strict audit, source install, and formula test.

### Observed repository inventory

The following was verified at `v0.8.1` and from the public `v0.8.1` release:

| Area | Current state before this decision |
| --- | --- |
| Release trigger | A non-documentation push to `main`; a release is created only when the `Cargo.toml` version has no matching tag. |
| Dependency lock | `Cargo.lock` is committed in version 4 format and its root package version is checked against `Cargo.toml`. Release builds used the lockfile but did not reject an out-of-date lockfile because `cargo build` omitted `--locked`. |
| Build targets | `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-musl`, and `aarch64-unknown-linux-musl`. |
| Archives | One macOS Universal 2 `.tar.gz`, one Windows x64 `.zip`, and Linux musl `.tar.gz` files for x86_64 and aarch64. Each contains the executable, `README.md`, and `LICENSE`. |
| Integrity | Packaging is deterministic and publishes a separate SHA-256 file for each of the four archives. GitHub also reports a digest for each uploaded asset. |
| Provenance | GitHub Actions are pinned by commit. The public `v0.8.1` assets and tag had no GitHub attestations. |
| Publisher identity | The macOS Universal 2 binary is ad-hoc signed only. Windows is unsigned. Linux has no project signing key or distribution-repository signature. |
| Package managers | The repository contained no Homebrew, WinGet, Scoop, Linux repository, or MSIX manifest. A crates.io search returned no `codex-cost-meter` package. External Store listings were not inspected. |

### Separate trust concerns

| Concern | What answers it | What it does not answer |
| --- | --- | --- |
| Artifact integrity and provenance | Pinned source SHA-256, `Cargo.lock`, `cargo ... --locked`, release SHA-256 files, pinned Actions, and GitHub artifact attestations. | Whether macOS or Windows recognizes an identified publisher. |
| Operating-system publisher identity | Apple Developer ID plus notarization on macOS; Authenticode through a trusted provider or Microsoft Store signing on Windows. | Whether installation is convenient or the artifact came through a package manager. |
| Install and first-run friction | Homebrew, Cargo, WinGet, Scoop, or a Store can make acquisition and upgrades easier. A source build also avoids browser-download quarantine on the resulting executable. | A universal promise that platform or enterprise policy will allow execution. |

An ad-hoc macOS signature is an executability and integrity mechanism, not publisher identity. SHA-256 and GitHub attestations establish integrity or provenance, not Gatekeeper, SmartScreen, or Smart App Control trust.

## Decision

### macOS and Homebrew

Use this repository as a custom personal tap and keep one source-building formula under `Formula/`. Users tap it with the explicit repository URL because the source repository is not named `homebrew-codex-cost-meter`:

```text
brew tap deinspanjer/codex-cost-meter https://github.com/deinspanjer/codex-cost-meter
brew install deinspanjer/codex-cost-meter/codex-cost-meter
```

The formula pins a release source archive by SHA-256, declares Rust as a build dependency, and uses Homebrew's `std_cargo_args`, which includes `cargo install --locked`. It installs no service and performs no post-install mutation.

The application resolves the installed executable before writing a native schedule. A Homebrew upgrade changes the resolved Cellar path, so users with a schedule must rerun `codex-cost-meter schedule install` after an upgrade. They must use `schedule remove` followed by `brew uninstall`, not the application's self-uninstall, so Homebrew retains ownership of its Cellar files.

The same formula is intentionally not restricted to macOS and is a candidate Linuxbrew path. Linux support remains unclaimed until it passes the clean-machine checks below.

### Windows and Linux

Document a locked source install as the zero-manifest path on all platforms, including Windows:

```text
cargo install --locked --git https://github.com/deinspanjer/codex-cost-meter --tag v<VERSION> codex-cost-meter
```

Retain the existing direct archives as fallbacks because they avoid requiring Homebrew or a Rust toolchain and already support Windows and static Linux installs. Continue describing macOS direct downloads as ad-hoc signed and Windows downloads as unsigned.

WinGet and Scoop manifests are the next Windows package-manager implementations, but they live in external package repositories and depend on a published release URL and SHA-256. Add them only after clean Windows install, upgrade, schedule-repair, and uninstall verification. Do not add apt/rpm repositories or Flatpak for Linux; the source formula, Cargo install, and existing static archives cover the current requirement with less ongoing key and repository maintenance.

### Release CI

Make these exact release changes:

1. Use `cargo build --locked --release --target ...` in native CI, release CI, and the local macOS package recipe.
2. Keep the existing deterministic four-archive/eight-asset packaging and SHA-256 files.
3. Before publishing the draft release, attest all eight files under `target/release/` with `actions/attest-build-provenance` pinned to commit `977bb373ede98d70efdf65b84cb5f73e068dcc2a` (the resolved `v3` action commit at the decision date).
4. Grant only `attestations: write`, `id-token: write`, and the existing `contents: write` to the publish job. Keep repository-level permissions read-only.
5. Keep source formula updates as an explicit post-release change: update its tag and source SHA-256, run the formula checks, and merge normally. Do not give the release workflow a repository-writing token.

Users can verify a downloaded asset with its SHA-256 file and, independently, with `gh attestation verify <ASSET> --repo deinspanjer/codex-cost-meter`. Neither check supplies OS publisher identity.

## Options not selected

| Option | Decision and revisit condition |
| --- | --- |
| Custom Homebrew bottles | No. Source builds are sufficient. Add bottles only if measured build time or Rust download size materially harms adoption. |
| `homebrew/core` | Not now. Consider after stable public adoption and when the formula meets current core acceptance requirements; core would then own bottles. |
| Separate `homebrew-tap` repository | Not now. The custom tap works from this repository. Split only if tap-only release automation or multiple formulae justify another repository. |
| Microsoft Store MSIX | No. The current scheduler stores an absolute executable path and invokes command tools; prove a Store-compatible, update-stable scheduler before packaging. |
| SignPath Foundation | No current dependency. Reconsider if the project is accepted and signed direct Windows downloads become important. |
| Azure Artifact Signing | No. It adds paid Azure and identity-validation operations and still cannot promise immediate SmartScreen reputation. Reconsider only from observed direct-download friction. |
| Apple Developer ID | No for the current Homebrew-first path. Reconsider when warning-free direct browser downloads are a demonstrated requirement and the annual account plus credential/notarization pipeline is approved. |

## Clean-machine verification matrix

These checks are required before expanding support claims. A successful build on a hosted runner is not a substitute for the first-run checks.

| Environment | Install and integrity | Runtime and lifecycle | Trust evidence to record |
| --- | --- | --- | --- |
| Apple Silicon macOS 14+ | Tap by explicit URL; install the formula; run `brew test`; confirm `which codex-cost-meter`. | Run `--version`, `report --help`, and a privacy-safe report; install/status/remove the schedule; upgrade or reinstall and repair the schedule. | `file`, `codesign -dv`, quarantine attributes, and any Gatekeeper prompt. |
| Intel macOS 14+ | Repeat the formula flow on a clean Intel machine. | Repeat runtime, schedule, upgrade, and uninstall checks. | Same evidence as Apple Silicon; do not infer Intel behavior from Universal 2 release tests. |
| Windows x64 with SmartScreen enabled | Run the tagged `cargo install --locked` flow; separately verify the ZIP SHA-256 and any future WinGet/Scoop manifest hash. | Run `--version`, a privacy-safe report, and schedule install/status/remove; verify package upgrade and uninstall behavior. | Record the exact download route, zone metadata, prompt text, policy, and signature state. |
| Windows x64 with Smart App Control enabled, where a clean evaluation/on VM is available | Test the Cargo-built executable and direct/package-manager artifact separately. | Run only if policy permits; record blocks rather than bypassing policy. | Record SAC mode and verdict. Do not generalize from SmartScreen or from a machine where SAC cannot be re-enabled. |
| Linux x86_64 and aarch64 | Test the source formula where Homebrew is supported; repeat the tagged Cargo install; verify the existing musl archive checksum. | Run `--version`, a privacy-safe report, and systemd user schedule lifecycle in a real logged-in user session. | Record architecture, libc, package path, and whether a working user bus exists. |

## Evidence boundaries

Observed:

- The repository and public `v0.8.1` release have the inventory recorded above.
- Homebrew 6.0's `std_cargo_args` includes `--locked`, and the documented tagged Git Cargo install produced a working `0.8.1` executable.
- A temporary custom tap passed Homebrew style, strict audit, source install, formula test, and immediate `--version` execution on the development Apple Silicon host.
- The formula's `v0.8.1` source archive checksum is `5a9ea4e459eb6b6500d75b6cc29359184346bdd312cc4c2bd89d5387a05bef1e`.

Inferred pending clean-machine verification:

- A Homebrew source build should avoid the conventional browser-download quarantine path and execute immediately on supported macOS hosts.
- The formula should also build on supported Linuxbrew hosts because the Rust crate and formula have no macOS-only install step.

Unknown:

- Actual clean-machine Apple Silicon, Intel, SmartScreen, and Smart App Control outcomes for this formula and future WinGet/Scoop manifests.
- Whether public demand will justify bottles, `homebrew/core`, Developer ID, Windows publisher signing, or Store packaging.
- Whether SignPath Foundation would accept this project or materially improve its direct-download adoption.

## References

- [Homebrew taps](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
- [Homebrew formula cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew tap trust](https://docs.brew.sh/Tap-Trust)
- [Homebrew acceptable formulae](https://docs.brew.sh/Acceptable-Formulae)
- [Homebrew supply-chain security](https://docs.brew.sh/Supply-Chain-Security)
- [GitHub artifact attestations](https://docs.github.com/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
- [Cargo install](https://doc.rust-lang.org/cargo/commands/cargo-install.html)
- [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Windows SmartScreen reputation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)
- [Windows code-signing options](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)
