# Development

This guide covers the normal local workflow for Aurelia contributors.

## Prerequisites

- Stable Rust toolchain with `cargo` and `rustfmt`.
- Git.
- Optional: GitHub CLI `gh` for repository labels and metadata helpers.
- Optional for manual compatibility testing: a clean Minecraft Beta 1.7.3 client kept outside this repository.

Do not place Minecraft jars, generated sources, assets, or decompiled source in this repository.

## Setup

From the repository root:

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Smoke Tests

Basic server startup smoke test:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
```

Experimental flat-world startup smoke test:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

Use a distinct world name when you want to avoid mixing manual test state:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --world dev-smoke
```

## Real-Client Test Command

Run the experimental path locally:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512
```

Then connect with a clean Beta 1.7.3 client to `127.0.0.1:25565`.

Run against a vanilla Beta 1.7.3-style world folder:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --world ./world --world-format=vanilla-beta173 --chunk-radius 1 --compat-debug
```

This path is for compatibility research and MVP gameplay testing. It is not a full compatibility claim.

## Tracing Flags

Useful flags:

- `--trace-packets` enables packet metadata logging.
- `--trace-packet-limit <n>` controls how many packet payloads are printed.
- `--trace-continue-after-handshake` keeps reading after the initial handshake in trace research mode.
- `--trace-handshake-response <value>` changes the trace-only handshake response string.
- `--compat-debug` enables packet tracing and raises the trace window for compatibility debugging.
- `--login-response-mode beta173-observed` uses the currently recommended provisional login response mode.
- `--login-response-mode mcdevs-legacy` keeps the alternate debug response mode available for comparison.
- `--world <path>` selects the world folder.
- `--world-format auto|aurelia-flat|vanilla-beta173` selects save storage.
  Auto prefers `level.dat` plus `region/*.mcr`, then Aurelia flat storage.
- `--chunk-radius <n>` controls the chunk radius sent around the player in
  playable flat and vanilla world modes.
- `--post-join-minimal` suppresses optional post-join clientbound packets for stream-alignment testing.
- `--no-inventory-sync`, `--no-time-update`, `--time-update-mode off|once|interval`, `--no-keepalive`, and `--keepalive-mode off|serverbound-no-payload|serverbound-int32` isolate specific compatibility surfaces.

Redact secrets before sharing logs. Do not paste decompiled source or proprietary material into traces or reports.

## Compatibility Reports

Use the compatibility trace issue template when reporting real-client behavior. Include:

- Aurelia version or commit.
- Exact command used.
- Client version and whether it is clean.
- Screen state or disconnect message.
- Packet trace lines if available.
- Expected behavior written as black-box observations, not copied source.

Compatibility claims should be supported by public docs, black-box traces, original observations, independent notes, or tests.

## Clippy

CI currently gates formatting, tests, and build. Clippy hardening is future polish and should not be added as a hard-failing CI step until the workspace is kept clean under:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

It is still useful to run clippy locally when touching code.

## Documentation Updates

Update docs when changing:

- Public commands or flags.
- Compatibility status.
- Persistence formats.
- Clean-room policy.
- Roadmap labels or milestone scope.
- User-visible behavior.

Keep README claims concise and link deeper details into `docs/`.
