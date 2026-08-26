# Direct release trust and remaining unsigned paths

## Run a release or build it yourself

Homebrew is the preferred macOS install path because it builds pinned source through the package manager. A tagged `cargo install --locked` is available on macOS, Windows, and Linux. Before acting on a warning for a direct archive, verify its checksum and confirm that it came from this project's GitHub Release.

* **macOS:** Binaries produced by the current release workflow use Developer ID signing and Apple notarization. Users do not install publisher certificates manually. Follow organizational Gatekeeper policy rather than changing trust settings.
* **Windows:** Microsoft describes the standard **More info** then **Run anyway** flow for a known download in [its Windows publishing guidance](https://learn.microsoft.com/windows/apps/package-and-deploy/publish-first-app#step-6-handle-smartscreen-for-new-apps). An organization can disable that bypass; follow its application-control policy rather than weakening Windows security.
* **Build from source on macOS, Windows, or Linux:** use the repository's [build and test guide](../DEVELOPERS.md#build-and-test). It documents the required Rust version, the macOS Universal 2 targets, and the supported package command.

## Release workflow state

Direct GitHub archives have platform-specific trust properties:

* The macOS Universal 2 executable is **Developer ID signed**, uses hardened runtime and a secure timestamp, and is notarized before packaging. Gatekeeper can establish its publisher and retrieve the notarization ticket from Apple. Apple does not support stapling a ticket to a standalone executable, so first execution may require network access.
* The Windows executable is **unsigned**, so SmartScreen and organizational application-control policies can warn or block it.
* Linux archives use the normal archive-and-checksum distribution model. There is no single Linux desktop trust service equivalent to Gatekeeper or SmartScreen.

Existing releases retain the signature state they had when published; production signing does not retroactively change their assets.

Checksums detect a changed download only when the checksum itself was obtained from a trusted project release. GitHub artifact attestations add verifiable build provenance to new release assets. These mechanisms remain separate from macOS Developer ID publisher identity and do not identify a Windows publisher.

## Why some paths remain unsigned

The remaining production solutions add a material external account, cost, or packaging commitment:

| Platform or path | What would improve | Current hurdle |
| --- | --- | --- |
| Windows direct download | A public-CA Authenticode signature identifies the publisher. | Conventional organization-validated certificates require identity validation, protected key storage, and recurring certificate cost. SmartScreen reputation can still warn on early releases. |
| Windows Artifact Signing | Managed CI signing without a hardware token. | Requires a Microsoft Entra tenant, a **paid** Azure subscription, identity validation, eligible geography, and ongoing service cost. |
| Microsoft Store MSIX | Microsoft signs the submitted package and gives users the lowest-friction Windows install/update path. | It is not a drop-in wrapper: Store certification and MSIX packaging are required. The current command-tool scheduler and stored absolute executable path need a Store-compatible, update-safe design first. |
| GitHub Release MSIX | GitHub can host a direct MSIX download. | Direct MSIX still needs a public-CA signature. GitHub does not confer Microsoft Store signing or Windows publisher trust. |
| Linux packages | A distribution package manager can provide native install/update UX. | Supporting signed repositories or distro packages adds ongoing release and support work; Flatpak sandboxing conflicts with the tool's normal access to local Codex data and user scheduling. |

The complete Windows evidence and source links are in [Windows code-signing options without Azure](research/windows-code-signing-options.md). Deferred Windows work should be reconsidered when there is an approved signing budget, an accepted publisher identity, or evidence that direct-download friction warrants the packaging work.

## GitHub protections that do not replace platform signing

GitHub Release checksums, [artifact attestations](https://docs.github.com/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations), and [immutable releases](https://docs.github.com/code-security/concepts/supply-chain-security/immutable-releases) can improve provenance and tamper detection. New release assets are attested by the pinned release workflow and can be checked with `gh attestation verify <ASSET> --repo deinspanjer/codex-cost-meter`. These controls complement the macOS Developer ID signature but do not establish Windows publisher identity or remove SmartScreen friction.
