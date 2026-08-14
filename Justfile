fmt:
    cargo fmt --check

fmt-fix:
    cargo fmt

test:
    cargo test

test-filter FILTER:
    cargo test {{FILTER}}

check: fmt test
