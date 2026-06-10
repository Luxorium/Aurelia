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

Aurelia is at the 0.2.0 Vanilla Parity Foundation stage. A real Beta 1.7.3
client can join the experimental flat-world path, receive chunks, move, cross
chunk boundaries with load/unload visibility updates, break and place blocks
from a starter hotbar, run short chat/debug commands, click inventory slots
conservatively, save dirty flat-world edits and basic player state, quit
cleanly, and rejoin with saved edits. This is still an early parity foundation,
not a complete Beta 1.7.3 server.

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

For the stable survival-session MVP:

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
default. Use `--chunk-radius 0` to send only the spawn chunk if needed. Dirty
flat-world chunks are saved under `<world>/aurelia-flat-v1/`. For login
regression debugging, `--no-inventory-sync`, `--no-time-update`,
`--time-update-mode off|once|interval`, `--no-keepalive`, and
`--keepalive-mode off|serverbound-no-payload|serverbound-int32` isolate those
compatibility surfaces. The default serverbound KeepAlive mode does not consume
payload bytes until the Beta 1.7.3 return shape is verified. `--defer-inventory-sync`
keeps starter inventory delayed until after several post-join movement packets
(the current default), and `--post-join-minimal` suppresses optional post-join
clientbound packets for core stream-alignment testing.

## What Works Now

- Blocking TCP listener with per-connection player sessions.
- Shared game state with one flat world, entity manager, player registry, and
  basic 20 TPS tick loop.
- Handshake and observed serverbound login decoding.
- Protocol version `14` check.
- Provisional login response, spawn position, player position/look, chunk
  visibility, and chunk data sends.
- Initial join now sends chunk visibility/data before inventory sync, time
  update, keepalive, or chat responses; Beta 1.7.3 time update is deferred
  until the first client movement packet and starter inventory is delayed until
  three additional movement packets.
- Flat test world with grass at Y `63` and spawn air at Y `65`.
- Chunk view tracking that sends missing chunks and `S->C 0x32` unload
  visibility packets when a player changes chunks.
- Periodic clientbound `0x00` keepalive while joined, with configurable
  serverbound KeepAlive decoding. Beta 1.7.3 `0x04` TimeUpdate is sent once by
  default and can be set to interval mode; Aurelia encodes it as one `i64`, not
  the modern two-long layout.
- Movement packet reads for `0x0A`, `0x0B`, `0x0C`, and `0x0D`.
- Joined-state interaction packet reads for `0x10` held item change, `0x12`
  animation, `0x13` entity action, `0x0E` digging, `0x0F` block placement,
  `0x65` close window, `0x66` window click, and `0x6A` confirm transaction.
- Serverbound chat `0x03`, clientbound chat responses, and debug commands:
  `/aurelia`, `/whereami`, `/givebasic`, `/save`, `/setblock`, and `/time`.
- Starter hotbar sync through `S->C 0x68 SetWindowItems`, slot corrections
  through `S->C 0x67 SetSlot`, and conservative window-click confirmation.
- Player inventory window `0` maps hotbar indices `0..=8` to window slots
  `36..=44`.
- Beta 1.7.3 item metadata for early survival stack limits, placeable blocks,
  and basic tool categories.
- Beta 1.7.3 block metadata for early survival materials, approximate
  hardness, preferred/required tools, light placeholders, and drops.
- Inventory-backed block placement in visible loaded chunks. Successful
  placement decrements the selected server-side hotbar stack; rejected
  placement sends block corrections plus selected-slot `SetSlot` and does not
  consume items.
- Rule-driven block breaking uses an active digging state, prevents bedrock
  edits, sends `S->C 0x35 BlockChange`, applies held-tool harvest rules, and
  adds drops to inventory when space is available without duplicating
  full-inventory drops.
- Dirty modified chunks are saved and reloaded in an Aurelia-native MVP format.
- Basic server-side player state, survival mode marker, health/death state,
  fall/void damage foundation, respawn helper, block get/set/break/place world
  APIs, selected hotbar slot, crouch flag, world time counter, and entity/mob
  scaffolding.
- Aurelia-native player persistence for username, position, rotation, health,
  inventory, and spawn position.
- A vanilla parity matrix in `docs/VANILLA_PARITY_MATRIX.md`.

## Not Yet Implemented

- Verified production login response semantics.
- Full production chunk streaming policy beyond the conservative radius-based
  MVP.
- Crafting, smelting, chests, workbench UI, and full inventory rules.
- Item entities for overflow drops and pickups.
- Placement remains conservative: no replaceable-block table beyond air
  targets, no collision checks, no tool-speed timing, and no full interaction
  semantics for use-on-block items yet.
- Client-visible health/death/respawn packet sync is not verified; health is
  currently server-side foundation only.
- Tool durability, exact break timing, permissions, and full survival loop.
- Visible mob spawn packets, AI, pathfinding, combat, and mob persistence.
- Vanilla McRegion/NBT world and player save parity. Current persistence is
  Aurelia-native.

## Roadmap Summary

- 0.1.x: Real-client join and playable flat-world MVP.
- 0.2.x: Vanilla survival mechanics foundation.
- 0.3.x: Containers, crafting, and furnaces.
- 0.4.x: Vanilla terrain/worldgen plus McRegion/NBT parity.
- 0.5.x: Entities, item entities, and mobs.
- Later: redstone, fluids, weather, commands, multiplayer edge cases, and full
  vanilla parity.
