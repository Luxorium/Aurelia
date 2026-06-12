# Roadmap

This roadmap treats Aurelia as a clean-room path toward Minecraft Beta 1.7.3 behavior from the client/player point of view. The scope is conservative: only claim parity for behavior implemented and tested from public documentation, packet traces, black-box testing, or original reasoning.

## Current Milestone: 0.2.0 - Vanilla Parity Foundation

The current milestone proves an early real-client foundation:

- Experimental Beta 1.7.3 real-client flat-world join path.
- Chunk visibility/data sends and chunk load/unload updates while crossing chunk boundaries.
- Movement, chat/debug commands, keepalive/time basics, and packet tracing.
- Starter hotbar sync, conservative inventory clicks, and block break/place MVP behavior.
- Early Beta 1.7.3 block/item rule tables.
- Server-side health/death/fall/void foundations.
- Aurelia-native dirty chunk and basic player persistence.
- Initial vanilla Beta 1.7.3 `level.dat`, McRegion chunk, and player-file
  load/save foundation.

This milestone is not a full compatibility claim.

## 0.2.x - Foundation Hardening

- Verify production login response semantics from clean clientbound evidence.
- Stabilize current join ordering and chunk streaming behavior.
- Expand block and item rule coverage.
- Add tool durability and timed digging.
- Add replaceable-block and collision semantics for placement.
- Add item entities for overflow drops and pickups.
- Verify client-visible health/death/respawn packets before sending them.
- Harden vanilla save compatibility with more synthetic fixtures and
  black-box import/export checks.
- Improve persistence tests for Aurelia-native chunk and player formats.

## 0.3.x - Containers, Crafting, And Furnaces

- Player 2x2 crafting grid.
- Workbench UI and recipes.
- Chest storage and window synchronization.
- Furnace tile entity, fuel, cook time, and UI.
- More complete inventory transaction behavior.

## 0.4.x - Vanilla Terrain/Worldgen And Deeper Save Parity

- Vanilla-style terrain generation from clean-room behavior notes.
- Trees, ores, caves, and biome-adjacent rules as verified.
- Exact lighting behavior, tile-entity function, entity persistence, Nether
  support, and broader McRegion/NBT parity.

## 0.5.x - Entities, Item Entities, And Mobs

- Item entity spawn, pickup, merge, and despawn behavior.
- Entity spawn/despawn/move packets.
- Passive animals.
- Hostile mobs, AI, drops, and combat.

## Later Parity Work

- Redstone.
- Fluids.
- Weather and sleep.
- Vanilla commands/operator behavior.
- Multiplayer edge cases and permissions.
- Full protocol and gameplay parity audit against clean black-box traces.

## Ongoing Requirements

- Keep clean-room warnings visible.
- Keep compatibility docs aligned with actual real-client evidence.
- Avoid committing Mojang code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project implementations.
