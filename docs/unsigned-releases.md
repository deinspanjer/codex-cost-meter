# Why releases are unsigned for now

## Run a release or build it yourself

Before bypassing a platform warning, verify the release checksum and confirm that the download came from this project's GitHub Release.

* **macOS:** Apple documents the one-release exception in [Safely open apps on your Mac](https://support.apple.com/102445). Do not change Gatekeeper globally.
* **Windows:** Microsoft describes the standard **More info** then **Run anyway** flow for a known download in [its Windows publishing guidance](https://learn.microsoft.com/windows/apps/package-and-deploy/publish-first-app#step-6-handle-smartscreen-for-new-apps). An organization can disable that bypass; follow its application-control policy rather than weakening Windows security.
* **Build from source on macOS, Windows, or Linux:** use the repository's [build and test guide](../DEVELOPERS.md#build-and-test). It documents the required Rust version, the macOS Universal 2 targets, and the supported package command.

## Current release state

Releases are intentionally distributed as direct GitHub archives while production code signing is deferred:

* The macOS Universal 2 executable is **ad-hoc signed**, not Developer ID signed or notarized. Gatekeeper therefore cannot establish an identified publisher.
* The Windows executable is **unsigned**, so SmartScreen and organizational application-control policies can warn or block it.
* Linux archives use the normal archive-and-checksum distribution model. There is no single Linux desktop trust service equivalent to Gatekeeper or SmartScreen.

Checksums detect a changed download only when the checksum itself was obtained from a trusted project release. They do not identify the publisher.

## Why this remains unsigned

This is not a claim that signing is unnecessary. Each production solution currently adds a material external account, cost, or packaging commitment:

| Platform or path | What would improve | Current hurdle |
| --- | --- | --- |
| macOS direct download | Developer ID signing and notarization remove the unidentified-developer barrier for the existing Universal 2 archive. | Apple Developer Program membership costs $99/year, and releases need a protected signing identity plus notarization in the release workflow. |
| Windows direct download | A public-CA Authenticode signature identifies the publisher. | Conventional organization-validated certificates require identity validation, protected key storage, and recurring certificate cost. SmartScreen reputation can still warn on early releases. |
| Windows Artifact Signing | Managed CI signing without a hardware token. | Requires a Microsoft Entra tenant, a **paid** Azure subscription, identity validation, eligible geography, and ongoing service cost. |
| Microsoft Store MSIX | Microsoft signs the submitted package and gives users the lowest-friction Windows install/update path. | It is not a drop-in wrapper: Store certification and MSIX packaging are required. The current command-tool scheduler and stored absolute executable path need a Store-compatible, update-safe design first. |
| GitHub Release MSIX | GitHub can host a direct MSIX download. | Direct MSIX still needs a public-CA signature. GitHub does not confer Microsoft Store signing or Windows publisher trust. |
| Linux packages | A distribution package manager can provide native install/update UX. | Supporting signed repositories or distro packages adds ongoing release and support work; Flatpak sandboxing conflicts with the tool's normal access to local Codex data and user scheduling. |

The complete evidence and source links are in [Windows code-signing options without Azure](research/windows-code-signing-options.md). The deferred work should be reconsidered when there is an approved signing budget, an accepted publisher identity, or evidence that direct-download friction warrants the packaging work.

## GitHub protections that do not replace platform signing

GitHub Release checksums, [artifact attestations](https://docs.github.com/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations), and [immutable releases](https://docs.github.com/code-security/concepts/supply-chain-security/immutable-releases) can improve provenance and tamper detection. Windows and macOS do not treat them as an identified-publisher signature, so they do not remove Gatekeeper or SmartScreen friction.
