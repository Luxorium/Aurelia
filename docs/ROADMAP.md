# Roadmap

This roadmap treats Aurelia as a clean-room path toward vanilla Minecraft Beta
1.7.3 behavior from the client/player point of view. The scope remains
conservative: only claim parity for behavior implemented and tested from public
documentation, packet traces, black-box testing, or original reasoning.

## 0.1.x - Real-Client Join And Playable Flat-World MVP

- Cargo workspace, server startup, protocol/world/region scaffolding.
- Experimental real-client login path for protocol 14.
- Flat-world chunk streaming with visibility load/unload packets.
- Movement, chat/debug commands, keepalive/time basics.
- Starter inventory sync, conservative inventory clicks, block break/place MVP.
- Aurelia-native dirty chunk persistence and rejoin support.

Remaining 0.1.x validation:

- Keep current join ordering stable.
- Continue real-client trace checks for packet ordering and chunk behavior.

## 0.2.x - Vanilla Survival Mechanics Foundation

- Vanilla parity matrix.
- Early Beta 1.7.3 item stack/tool metadata.
- Early block material, hardness, harvest, and drop rules.
- Rule-driven survival block breaking and placement.
- Server-side health/death/respawn foundation.
- Aurelia-native player persistence for position, rotation, health, inventory,
  and spawn position.

TODO:

- Verify client-visible health/death/respawn packets before sending them.
- Add tool durability and timed digging.
- Add item entities for overflow drops and pickups.
- Expand block and item rule coverage.
- Add collision and replaceable-block semantics.

## 0.3.x - Containers, Crafting, And Furnaces

- Player 2x2 crafting grid.
- Workbench UI and recipes.
- Chest storage and window synchronization.
- Furnace tile entity, fuel, cook time, and UI.
- More complete inventory transaction behavior.

## 0.4.x - Vanilla Terrain/Worldgen And McRegion/NBT Parity

- Vanilla-style terrain generation from clean-room behavior notes.
- Trees, ores, caves, and biome-adjacent rules as verified.
- McRegion/NBT world persistence.
- Vanilla-compatible player and tile-entity save data.

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
