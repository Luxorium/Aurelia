# Contributing To Aurelia

Thanks for helping improve Aurelia. This project is a clean-room Rust rewrite of a Minecraft Beta 1.7.3-compatible dedicated server, so correctness and provenance matter as much as code quality.

## Project Ground Rules

- Keep implementation code original.
- Do not commit Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied code from Bukkit, Paper, Folia, Fabric, Forge, or similar projects.
- Support compatibility claims with public documentation, black-box traces, original observations, independently written behavior notes, or tests.
- Avoid broad rewrites unless they are needed for the issue at hand.
- Keep public wording honest: Aurelia is at `0.2.0` / Vanilla Parity Foundation and is not a complete Beta 1.7.3 server.

Read [docs/CLEAN_ROOM_POLICY.md](docs/CLEAN_ROOM_POLICY.md) before contributing protocol, gameplay, world, or compatibility work.

## Local Workflow

Run the standard checks before opening a pull request:

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

For real-client testing, use a clean Beta 1.7.3 client and keep traces limited to behavior evidence:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512
```

## Good First Contributions

- Documentation cleanup and consistency fixes.
- Focused unit tests for protocol codecs, inventory rules, block/item rules, and persistence.
- Clean-room compatibility traces with clear reproduction steps.
- Narrow world or gameplay behavior improvements backed by tests.
- Issue triage using labels such as `needs-trace`, `compatibility`, `protocol`, and `legal-clean-room`.

## Pull Request Checklist

- The change is scoped and described clearly.
- New behavior has tests where practical.
- Documentation is updated when commands, flags, compatibility status, or roadmap wording changes.
- No forbidden reference material is included.
- Compatibility claims are supported by clean-room evidence.
- The standard local commands pass or failures are explained in the PR.

## Reporting Compatibility Issues

Use the compatibility trace issue template when a real Beta 1.7.3 client behaves differently than expected. Include:

- Aurelia commit or release.
- Exact server command.
- Client version and whether it is a clean client.
- What happened on screen.
- Packet trace output if available.
- Any black-box observations, written in your own words.

Do not paste decompiled source, proprietary assets, generated jars, session tokens, or secrets.
