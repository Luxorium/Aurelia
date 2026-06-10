# Aurelia

A clean-room, region-threaded Minecraft Beta 1.7.3 server rewrite in Rust.

Aurelia is the base of an original Minecraft Beta 1.7.3-compatible dedicated
server written from scratch in Rust. The long-term goal is for an original Beta
1.7.3 client to connect, join a world, move, chat, receive chunks, break/place
blocks, and play survival.

## What Aurelia Is

- An original dedicated server implementation.
- A clean-room project targeting Beta 1.7.3 protocol and gameplay behavior.
- A future region-threaded server inspired by Folia-style ownership,
  schedulers, mailboxes, and thread checks.
- A small Cargo workspace intended to grow carefully.

## What Aurelia Is Not

- It is not a fork of Minecraft, Bukkit, Paper, Folia, Fabric, or Forge.
- It does not contain Mojang source code or assets.
- It does not generate, distribute, or require committing a Minecraft jar.
- It does not yet claim full Beta 1.7.3 client compatibility.

## Current Status

Aurelia is at the 0.4.0 playable survival prototype stage. The Rust workspace
builds, starts a blocking TCP listener, accepts sockets, and runs each
connection through a long-lived player session loop backed by shared game
state. It contains packet codecs for handshake, disconnect, the observed
serverbound Beta 1.7.3 login packet, provisional clientbound join packets, and
fixed-size movement and interaction packets. The playable flat-world mode sends
a spawn chunk area, streams newly needed chunks as the player changes chunks,
keeps the socket open while movement packets update server-side player state,
and sends block-change updates for simple break/place actions. This does not
yet prove full real-client compatibility.

## Why Beta 1.7.3

Beta 1.7.3 is a compact, well-understood Minecraft protocol and gameplay target
with classic survival behavior. Its smaller scope makes it a practical version
for building a compatible server from first principles before adding more
complex systems.

## Why Region-Threaded Design

Aurelia is designed around eventual region ownership: chunks, entities, tile
entities, block ticks, and local tasks should belong to a region and mutate only
on that region's owning tick thread. Cross-region work should be posted through
mailboxes. The first implementation uses fixed region sections and loud
wrong-thread assertions.

## External Reference Use

External tools may be used separately as local reference workflows to study Beta
1.7.3 behavior and compare behavior against the original game. They must not be
copied into Aurelia, and decompiled Minecraft source must not be pasted into
this repository.

## Clean-Room Warning

Do not include Mojang code, Minecraft assets, generated Minecraft jars, or
decompiled Minecraft source in Aurelia. Contributors who inspect reference code
should write behavior notes and then implement original code from those notes.

## Build

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## Run

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565
```

The default run command binds to `0.0.0.0:25565` and keeps running until stopped.
For a startup smoke test that exits immediately:

```bash
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
```

For developer packet tracing while researching Beta 1.7.3 login bytes:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets
```

For trace-only continuation after the client handshake:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake
```

For the experimental playable prototype:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --trace-packets --trace-packet-limit 64
```

For real-client debugging with a larger trace window:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512
```

The `beta173-observed` login response mode is the recommended experimental
mode. The `mcdevs-legacy` mode is kept only as an alternate debug path; the
latest client test reset the connection after that response.

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 64 --trace-continue-after-handshake --experimental-join --login-response-mode mcdevs-legacy
```

`--playable-flat-world` currently sends chunk `(0,0)` and its neighbors by
default. Use `--chunk-radius 0` to send only the spawn chunk if a client rejects
the wider initial area.

## What Works Now

- Blocking TCP listener with per-connection player sessions.
- Shared game state with one flat world, entity manager, player registry, and
  basic 20 TPS tick loop.
- Handshake and observed serverbound login decoding.
- Protocol version `14` check.
- Provisional login response, spawn position, player position/look, chunk
  visibility, and chunk data sends.
- Flat test world with grass at Y `63` and spawn air at Y `65`.
- Chunk view tracking that sends missing chunks when a player changes chunks.
- Movement packet reads for `0x0A`, `0x0B`, `0x0C`, and `0x0D`.
- Joined-state interaction packet reads for `0x10` held item change, `0x12`
  animation, `0x13` entity action, `0x0E` digging, and `0x0F` block placement.
- MVP block break/place in visible loaded chunks, with `S->C 0x35 BlockChange`
  responses to update or correct the client.
- Basic server-side player state, survival mode marker, health field, block
  get/set/break/place world APIs, selected hotbar slot, crouch flag, world time
  counter, and entity/mob scaffolding.
- Trace names for common post-login packet IDs whose payload layouts still need
  clean observations.

## Not Yet Implemented

- Verified production login response semantics.
- Chunk unload packets and persistent chunk storage.
- Real inventory, item validation, tool speed, drops, permissions, crafting,
  damage, and full survival loop.
- Persistent chunk storage and chunk reserialization from mutated world state.
- Visible mob spawn packets, AI, pathfinding, combat, and persistence.
- Serverbound chat and keepalive payload decoding. These packet IDs are named
  in traces, but their layouts are not implemented until documented from clean
  observations.

## Roadmap Summary

- 0.1.0: Project base, module scaffold, startup, protocol/world/region shells.
- 0.2.0: Beta 1.7.3 protocol handshake.
- 0.3.0: Flat world join.
- 0.4.0: Basic world interaction.
- 0.5.0: Region-threaded tick prototype.
- 0.6.0: Survival foundation.
- 0.7.0: Tile entities.
- 0.8.0: Entities and mobs.
- 0.9.0: Fluids and redstone.
- 1.0.0: Playable Beta 1.7.3-compatible server.
