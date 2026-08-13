# Codex Cost Meter

Estimate the API-list-price equivalent of a Codex task and its descendants, then include that estimate in the persisted root-task title.

This repository preserves the working Python prototype while a small, self-contained Rust replacement is developed for macOS and Windows. The estimates are diagnostic approximations, not ChatGPT billing records.

## Repository status

- [`python-prototype/`](python-prototype/) contains the archived scripts, their embedded self-tests, and the macOS LaunchAgent used during prototyping.
- The Rust port has not been scaffolded yet. One crate is the default unless real boundaries justify more.

## Documentation

- [USERS.md](USERS.md) — run the Python prototype and understand its output and risks.
- [DEVELOPERS.md](DEVELOPERS.md) — architecture, invariants, verification, and documentation rules.
- [TODO.md](TODO.md) — unfinished Rust-port work.
- [Architectural decisions](docs/adr/README.md) — the established read, persistence, high-water, and Rust-port decisions.

## License

Licensed under the [MIT License](LICENSE).
