# Aurelia

[![CI](https://github.com/Luxorium/Aurelia/actions/workflows/ci.yml/badge.svg)](https://github.com/Luxorium/Aurelia/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Aurelia is a clean-room Minecraft Beta 1.7.3-compatible dedicated server rewrite in Rust, built toward a future region-threaded architecture.

## Current Status

Aurelia is currently at version `0.2.0`, the **Vanilla Parity Foundation** milestone.

A real Beta 1.7.3 client can join the experimental world path, receive chunks, move, cross chunk boundaries with chunk load/unload visibility updates, break and place blocks from a starter hotbar, use basic chat/debug commands, perform conservative inventory clicks, save dirty world edits and basic player state, quit cleanly, and rejoin with saved edits. Aurelia can also load and save an initial clean-room subset of vanilla Beta 1.7.3 McRegion/NBT worlds.

This is still an early compatibility foundation. Aurelia is not a complete Beta 1.7.3 server, and compatibility claims need clean-room evidence from tests, traces, public documentation, or independent observations.

## What Works Today

- Blocking TCP listener with per-connection player sessions.
- Clean-room Beta 1.7.3 protocol `14` handshake and observed serverbound login decoding.
- Experimental real-client join path behind `--experimental-join --playable-flat-world`.
- Flat chunk generation and chunk visibility load/unload updates while crossing chunk boundaries.
- Movement tracking, chat echo, and short debug commands.
- Starter hotbar sync, held-slot tracking, and conservative player inventory window clicks.
- Clean-room rule tables covering every Beta 1.7.3 block and item id, driving stack sizes, harvest requirements, and drops for survival break/place testing.
- Dirty flat-world chunk persistence in an Aurelia-native format.
- Basic player persistence for username, position, rotation, health, inventory, selected hotbar slot, and spawn position.
- Initial vanilla Beta 1.7.3 world save foundation: `level.dat`, `region/*.mcr` chunk block IDs/metadata, and `players/<username>.dat` position/rotation/health/inventory.
- Server-side health/death/fall/void foundations without unverified client-visible health/death packet claims.
- Unit and socket-level regression tests across the workspace.

## Not Yet Implemented

- Verified production login response semantics.
- Full production chunk streaming policy.
- Vanilla terrain generation, caves, ores, trees, biomes, and structures.
- Full vanilla McRegion/NBT parity beyond the initial load/save foundation.
- Crafting, workbenches, chests, furnaces, and full inventory transaction behavior.
- Item entities, pickups, overflow drops, mobs, AI, combat, and visible entity packets.
- Exact digging timing, tool durability, collision, replaceable-block rules, permissions, and full survival loop.
- Redstone, fluids, weather, sleep, vanilla commands, operators, and multiplayer edge cases.

Known vanilla save limitations: missing McRegion chunks are sent as air instead of generated with vanilla terrain; tile entities and entities are preserved as NBT where possible but are not functionally implemented; lighting arrays are preserved when present and placeholder-filled for new edited chunks, but lighting recalculation is not exact; Nether/DIM-1 is not supported; multiplayer edge cases are incomplete.

## Quick Start

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

Run a startup smoke test that binds to an ephemeral local port and exits:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
```

Run the experimental real-client flat-world path:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512
```

Run against an existing vanilla Beta 1.7.3-style world folder:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --world ./world --world-format=vanilla-beta173 --chunk-radius 1 --compat-debug
```

For a smoke test of the experimental path without keeping the server running:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for setup, tracing flags, compatibility-report guidance, and local testing notes.

## Why Beta 1.7.3?

Beta 1.7.3 is a compact and historically important Minecraft version with a smaller protocol and gameplay surface than modern releases. That makes it a realistic target for clean-room compatibility work: behavior can be studied, documented, tested, and implemented carefully before expanding into more complex systems.

## Why Rust?

Rust gives Aurelia memory safety, explicit ownership boundaries, strong testability, and predictable performance without a garbage-collected runtime. Those properties fit a long-running server where networking, world state, persistence, and eventual region ownership need to stay understandable under load.

## Why Region-Threaded?

The long-term architecture is region-threaded: chunks, entities, tile entities, scheduled block ticks, and local tasks should be owned by a region and mutated only by that region's tick thread. Cross-region work should go through mailboxes.

The current implementation is still an early foundation. The `aurelia-region` crate exists to grow the ownership model deliberately while the protocol and world layers become useful enough to test with real clients.

## Workspace Crates

- [`aurelia-common`](aurelia-common) - shared coordinate types, chunk-view helpers, and early Beta 1.7.3 item rules.
- [`aurelia-protocol`](aurelia-protocol) - clean-room packet models, codecs, trace metadata, and Beta 1.7.3 protocol constants.
- [`aurelia-world`](aurelia-world) - chunk/block models, block rules, world APIs, entity scaffolding, Aurelia-native persistence, and initial vanilla Beta 1.7.3 NBT/McRegion storage.
- [`aurelia-region`](aurelia-region) - future region ownership and scheduling foundation.
- [`aurelia-server`](aurelia-server) - server configuration, TCP listener, player session loop, experimental join path, commands, inventory handling, and persistence integration.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for a fuller overview.

## Compatibility

- [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) tracks the current client-visible compatibility surface.
- [docs/VANILLA_PARITY_MATRIX.md](docs/VANILLA_PARITY_MATRIX.md) tracks areas of vanilla behavior, current status, gaps, and test strategy.

## Roadmap Summary

- `0.2.x`: harden the Vanilla Parity Foundation, verify login semantics, expand block/item rules, improve vanilla save compatibility, and add item entities.
- `0.3.x`: containers, crafting, workbench UI, furnaces, and fuller inventory behavior.
- `0.4.x`: vanilla-style terrain/worldgen and deeper McRegion/NBT parity.
- `0.5.x`: item entities, visible entity packets, passive animals, hostile mobs, AI, drops, and combat.
- Later: redstone, fluids, weather, sleep, commands, permissions, multiplayer edge cases, and broader parity audits.

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full roadmap.

## Clean-Room Policy

Aurelia is an original implementation. Do not commit Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied implementations from Minecraft server/modding projects.

Allowed references include public documentation, black-box packet traces, original observations, independently written behavior notes, and tests written from clean evidence. Read [docs/CLEAN_ROOM_POLICY.md](docs/CLEAN_ROOM_POLICY.md) before contributing compatibility work.

## Contributing

Contributions are welcome when they keep the project honest, testable, and legally clean. Start with [CONTRIBUTING.md](CONTRIBUTING.md), then check [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for local workflow details.

Good early contribution areas include documentation, compatibility traces, focused protocol tests, block/item rule coverage, world persistence tests, and narrow survival-system work that does not overclaim vanilla parity.

## Sponsorship And Support

Sponsorship helps fund clean-room protocol research, compatibility testing, documentation, hosting/build infrastructure, and long-term development time. See [docs/SPONSORSHIP.md](docs/SPONSORSHIP.md) for details.

## Legal And Trademark Notice

Aurelia is unofficial and is not affiliated with Mojang, Microsoft, or Minecraft. Minecraft is a trademark of its respective owners.

This repository does not contain Mojang source code, Minecraft assets, generated Minecraft jars, decompiled Minecraft source, or copied server/modding project code. Compatibility work must remain clean-room and independently implemented.
