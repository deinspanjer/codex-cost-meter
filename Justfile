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

check: fmt test test-version
    cargo clippy --all-targets -- -D warnings
