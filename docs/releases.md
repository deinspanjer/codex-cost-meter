# Packaging and releases

[Developer guide](../DEVELOPERS.md)

## Packaging and platform checks

macOS packaging requires Xcode Command Line Tools (`lipo` and `codesign`). Before `just package`, install both supported macOS targets:

```text
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

`just package` builds both macOS slices, creates a Universal 2 binary, verifies its architectures and ad-hoc signature, and writes a deterministic archive plus checksum under `target/release/`. Native Windows CI runs the corresponding checks and release build on `x86_64-pc-windows-msvc`, then creates and inspects the deterministic Windows ZIP and checksum. Native Linux CI runs the Linux lifecycle fake-runner and CLI contracts, builds static musl binaries for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, then uses `python3 scripts/version.py package-linux --architecture x86_64|aarch64` to create and inspect each deterministic archive and checksum. It deliberately does not install a real systemd unit: hosted runners can lack a user bus, while the fake-runner tests cover the exact lifecycle command plans. Before release, inspect all archive member lists and checksums, the Universal 2 architectures and strict ad-hoc signature, and the Windows x64 fixture.

## Homebrew

The source-building Homebrew formula lives at `Formula/codex-cost-meter.rb`. It pins a tagged source archive and uses Homebrew's locked Cargo install arguments. Validate it from a temporary or installed tap with `brew style <TAP>`, `brew audit --strict <TAP>/codex-cost-meter`, `brew install --build-from-source <TAP>/codex-cost-meter`, `brew test <TAP>/codex-cost-meter`, and `brew uninstall codex-cost-meter`. Homebrew rejects formula paths outside a tap. Do not install a real schedule during the formula test.

`completions bash|zsh` uses `clap_complete` to generate scripts from the same Clap command definition as argument parsing. The formula uses Homebrew's `generate_completions_from_executable` helper to install both scripts, gated to version 1.3.0 or newer so existing source pins remain installable. The release workflow's version/checksum update enables completion installation when it advances the pin. See [Homebrew's completion helper](https://docs.brew.sh/Formula-Cookbook#shell-completion) and [clap_complete's generator](https://docs.rs/clap_complete/latest/clap_complete/aot/fn.generate.html).

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates locked native macOS, Windows, and Linux builds, packages and checksums one macOS archive, one Windows archive, and two Linux musl archives, Developer ID-signs and notarizes the Universal 2 binary, creates GitHub provenance attestations for all eight assets, tags the merge, and publishes exactly eight assets; an unchanged version only validates.

After a release is public, the release workflow uses a repository-scoped GitHub App to calculate the tagged source SHA-256 and open a normal formula PR from an `automation/homebrew-v<VERSION>` branch. It never writes directly to protected `main` or enables auto-merge; repeat the [formula validation](#homebrew) before merging. A release task is not complete until the GitHub release is public and this generated formula PR has passed CI and been merged. When asked to publish or ensure a release, own that full sequence unless the user limits its scope; update a local installation only when explicitly requested. The App must be installed only on this repository with `Contents: read and write` and `Pull requests: read and write`. Run `scripts/setup-homebrew-pr-app.sh` to create or rotate its credentials: the Client ID is repository variable `HOMEBREW_PR_APP_CLIENT_ID`, while the private key is stored as both a 1Password API Credential and repository secret `HOMEBREW_PR_APP_PRIVATE_KEY`. Verify an attested asset with `gh attestation verify <ASSET> --repo deinspanjer/codex-cost-meter`. The full distribution decision and clean-machine matrix are in [ADR 0008](adr/0008-use-a-source-built-homebrew-tap-and-attested-release-assets.md).

The macOS publish job imports the Developer ID identity into an ephemeral keychain, signs only after creating the Universal 2 binary, requires Apple notarization acceptance, and deletes temporary signing material even when a later step fails. It requires repository secrets `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD`, `APPLE_NOTARY_APPLE_ID`, and `APPLE_NOTARY_PASSWORD`, plus repository variable `APPLE_TEAM_ID`. Keep the recoverable `.p12` and its distinct export password in 1Password; never commit signing material.

Before a phase or release closes, require self, task, and final review; focused and full validation; durable documentation uplift; and accounting. The maintainability review checks requirement traceability, focused tests, proportionate module/dependency/test growth, current consumers for abstractions, and explicit bounded follow-ups. Stop for owner review under the program-stop conditions in [ADR 0004](adr/0004-preserve-the-python-prototype-and-port-to-rust.md).
