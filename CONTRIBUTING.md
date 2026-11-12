# Contributing

[`AGENTS.md`](AGENTS.md) is the single source of contributor rules: the
authoritative documents, the ADR process, the skills, the required workflow,
the quality gates, and the definition of done. This file only covers local
setup.

## Setup

1. Install [rustup](https://rustup.rs). The pinned toolchain in
   `rust-toolchain.toml` is installed automatically on the first `cargo` call.
2. Install the supply-chain tools at the pinned versions with `make tools`.
3. Run `make check`, which runs the five quality gates of `AGENTS.md` §7.
   All of them must pass before a change is complete. Hosted CI is disabled
   until Go-Live (ADR-0013), so this local run is the gate.

`make` lists every developer task; the README describes them.

## Tests

- `cargo test --locked` needs no network access and no provider credentials.
- Integration tests run the real binary inside `tests/support::Sandbox`,
  which clears the environment and isolates `HOME`, `VLAB_CONFIG_DIR`, and
  `VLAB_DATA_DIR` in a temporary directory.
- `tests/snapshots/help.txt` is the reviewed help output. When the CLI
  contract changes on purpose, regenerate it with `make snapshot` and review
  the diff.

See [`.docs/CI.md`](.docs/CI.md) for the CI configuration that Go-Live
activates.
