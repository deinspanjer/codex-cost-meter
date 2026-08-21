fmt:
    cargo fmt --check

fmt-fix:
    cargo fmt

test:
    cargo test

test-filter FILTER:
    cargo test {{FILTER}}

test-version:
    python3 -m unittest scripts/test_version.py -v

version-verify:
    python3 scripts/version.py verify

bump SELECTOR:
    python3 scripts/version.py bump {{SELECTOR}}

release-notes VERSION:
    python3 scripts/version.py notes {{VERSION}}

version-changed BEFORE:
    python3 scripts/version.py changed --before {{BEFORE}}

package:
    RUSTC="$(rustup which rustc)" cargo build --locked --release --target aarch64-apple-darwin
    RUSTC="$(rustup which rustc)" cargo build --locked --release --target x86_64-apple-darwin
    mkdir -p target/universal2/release target/release
    lipo -create -output target/universal2/release/codex-cost-meter target/aarch64-apple-darwin/release/codex-cost-meter target/x86_64-apple-darwin/release/codex-cost-meter
    test "$(lipo -archs target/universal2/release/codex-cost-meter | tr ' ' '\n' | sort | tr '\n' ' ')" = "arm64 x86_64 "
    codesign --force --sign - target/universal2/release/codex-cost-meter
    codesign --verify --strict target/universal2/release/codex-cost-meter
    python3 scripts/version.py package --binary target/universal2/release/codex-cost-meter --output-dir target/release

check: fmt test test-version
    cargo clippy --all-targets -- -D warnings
