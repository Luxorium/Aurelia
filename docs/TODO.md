# TODO

## Current Milestone

`0.2.0 - Vanilla Parity Foundation`

Aurelia currently provides an early clean-room compatibility foundation for Minecraft Beta 1.7.3. A real client can use the experimental flat-world path to join, receive chunks, move, cross chunk boundaries with visibility load/unload updates, break/place blocks from a starter hotbar, use chat/debug commands, perform conservative inventory clicks, persist dirty flat-world edits and basic player state, quit cleanly, and rejoin with saved edits.

This is still experimental and incomplete. Do not describe Aurelia as a complete Beta 1.7.3 server.

## Next Recommended Tasks

- Run the full manual `0.2.0` acceptance pass against a clean Beta 1.7.3 client:
  - join with `--experimental-join --playable-flat-world`;
  - walk around;
  - cross chunk boundaries;
  - break dirt, stone, glass, and ore blocks;
  - verify drops and inventory behavior;
  - place blocks from the hotbar;
  - try placing invalid or non-placeable items;
  - take damage;
  - die only if the current packet flow allows a stable client observation;
  - respawn only after a verified packet path has been implemented;
  - run `/save`;
  - restart the server;
  - verify position, inventory, and world edits persist;
  - verify no client crash or unexpected disconnect.
- Capture clean clientbound login response evidence before promoting login semantics from provisional to supported.
- Validate `0x0E`, `0x0F`, `0x10`, `0x12`, `0x13`, and `0x35` behavior against real-client traces.
- Validate `0x65`, `0x66`, and `0x6A` inventory/window behavior before expanding inventory state.
- Add replaceable-block and collision semantics after current conservative placement remains stable.
- Add item entities for inventory-full drops and pickups.
- Add crafting/workbench/chest/smelting UI support.
- Verify approximate block/item rule values (hardness, light, drop counts, ambiguous stack limits) from clean traces; the full id range `0..=96` and `256..=357` is now covered.
- Add tool durability and timed digging.
- Verify health/death/respawn packet sync before making it client-visible.

## Known Blockers

- Clientbound login response semantics are still provisional.
- The `beta173-observed` login response is accepted far enough for movement in current manual testing, but it is not yet a production compatibility claim.
- The `mcdevs-legacy` response reset/disconnected the latest tested client and is kept only as an alternate debug path.
- Handshake and disconnect string formats are implemented from clean-room assumptions and observed traces, but exact production behavior still needs more evidence.
- The current chunk strategy is a conservative radius-based MVP, not a verified production policy.
- Digging and placement are intentionally minimal: no exact tool speed, no durability, no permissions, and simplified drops.
- Window/inventory packets support conservative player-inventory stack movement only. Shift-click, crafting, armor semantics, workbench, chests, and smelting are not implemented.
- Item entities are not implemented; inventory-full drops are ignored rather than duplicated.
- Health/death/respawn is server-side foundation only until client-visible packets are verified.
- Damage sources beyond fall/void placeholders and combat are not implemented.
- Two persistence backends exist: Aurelia-native flat storage (default) and vanilla Beta 1.7.3 McRegion/NBT (`level.dat`, `region/*.mcr`, `players/*.dat`). Missing vanilla chunks return empty air; lighting is placeholdered; tile entity and entity behavior is not implemented.

## Testing Notes

```bash
cargo fmt --all
cargo test --workspace
cargo build --workspace
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0
cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world
```

Recommended real-client debug command:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512
```

Trace-only login research command:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake
```

## Clean-Room Guardrails

- Do not commit Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project implementations.
- Keep reference workspaces outside this repository.
- Convert observations into original notes and tests.
- Keep compatibility claims backed by clean evidence.
