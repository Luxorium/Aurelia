# Architecture

Aurelia is a small Rust workspace that separates protocol, world state, future region ownership, and the server runtime. The current implementation is intentionally conservative: it proves a clean-room Beta 1.7.3 real-client foundation before expanding into full vanilla parity.

## Current Milestone

Version `0.2.0` is the Vanilla Parity Foundation milestone. The experimental path can admit a real Beta 1.7.3 client into a flat world, stream chunks, track movement, handle basic break/place and inventory interactions, and persist Aurelia-native flat-world edits and player state.

It is not a complete Beta 1.7.3 server.

## Workspace Map

### `aurelia-common`

Shared domain types and helpers used across crates:

- `BlockPos`, `ChunkPos`, and chunk-view utilities.
- Early Beta 1.7.3 item metadata such as stack limits, placeable blocks, and tool categories.
- Small reusable logic that should not depend on networking or storage.

### `aurelia-protocol`

Clean-room protocol models, codecs, and trace metadata:

- Beta 1.7.3 protocol constants, including protocol version `14`.
- Legacy string and slot-data encoding helpers.
- Direction-aware packet metadata for packet tracing.
- Implemented codecs for handshake, observed serverbound login, provisional clientbound login responses, movement-adjacent packets, chunk visibility/data, chat, time, inventory/window sync, block changes, keepalive, and disconnects.

Protocol code should stay byte-oriented, testable, and evidence-driven. Do not copy protocol implementations from Mojang or server/modding projects.

### `aurelia-world`

World data and gameplay-adjacent rules:

- Flat-world chunk generation.
- Block get/set/break/place APIs.
- Early Beta 1.7.3 block metadata, harvest rules, drops, and placement checks.
- Dirty chunk tracking.
- Entity ID and mob scaffolding.
- Aurelia-native flat-world persistence.

This crate does not currently implement vanilla McRegion/NBT persistence or vanilla terrain generation.

### `aurelia-region`

Future region ownership and scheduling foundation:

- Region scheduler scaffolding.
- The intended ownership model for chunks, entities, tile entities, scheduled block ticks, and local tasks.
- A place to grow thread ownership, mailbox, and diagnostics work without mixing it into protocol parsing.

The production region-threaded architecture is not complete yet. See [REGION_THREADING.md](REGION_THREADING.md).

### `aurelia-server`

Runtime integration:

- Command-line configuration.
- Blocking TCP listener.
- Per-connection `PlayerSession` loop.
- Shared game server state and 20 TPS tick loop.
- Experimental real-client join path.
- Chunk load/unload visibility tracking.
- Chat/debug commands.
- Conservative inventory clicks and break/place behavior.
- Aurelia-native player persistence integration.

## Runtime Flow

1. `aurelia-server` accepts a TCP connection and creates a player session.
2. `aurelia-protocol` decodes handshake and observed serverbound login packets.
3. With `--experimental-join --playable-flat-world`, the server sends provisional login/spawn/position packets and flat-world chunk visibility/data.
4. The joined loop decodes known movement, chat, inventory, digging, placement, and disconnect packets.
5. World mutations go through `aurelia-world` APIs and are persisted in Aurelia-native files when dirty.
6. Chunk view changes send load/unload visibility updates as the player crosses chunk boundaries.

Unknown or unverified protocol surfaces should fail conservatively instead of guessing through the stream.

## Persistence

Current persistence is Aurelia-native:

- Flat-world chunk edits are stored under `<world>/aurelia-flat-v1/`.
- Player state is stored under `<world>/aurelia-players-v1/`.

These formats are not Anvil, McRegion, NBT, or vanilla-compatible save formats. Vanilla save parity is planned for a later milestone.

## Clean-Room Boundaries

Aurelia must not include Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project implementations. Architecture decisions should be justified by public documentation, black-box traces, original observations, independent behavior notes, or tests.
