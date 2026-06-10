# Compatibility

Aurelia targets eventual compatibility with original Minecraft Beta 1.7.3
clients.

## Current State

- Blocking TCP networking exists.
- Handshake and observed serverbound login are decoded.
- `--experimental-join --playable-flat-world` sends provisional join packets,
  a flat spawn chunk area, waits for first movement, then starts post-join sync
  conservatively and keeps the socket open.
- Latest manual real-client testing reached clean quit after login, chunks,
  movement, break/place, crouch/entity action, inventory open/close, and many
  `0x66` WindowClick packets.
- `0x00` KeepAlive is sent periodically and decoded leniently when returned.
- `0x03` Chat is decoded and echoed to the sender; debug slash commands are
  implemented for `/aurelia`, `/whereami`, `/givebasic`, `/save`, `/setblock`,
  and `/time`.
- `S->C 0x04` TimeUpdate is sent periodically from world time using the Beta
  1.7.3 one-`i64` layout. The modern two-long layout is intentionally not used
  in protocol 14 compatibility mode.
- Initial login deliberately sends chunk visibility/data before TimeUpdate,
  SetWindowItems, SetSlot, KeepAlive, or chat responses. TimeUpdate is deferred
  until the first movement packet; starter inventory is delayed until at least
  three additional movement/player packets.
- Movement packets `0x0A`, `0x0B`, `0x0C`, and `0x0D` update server-side player
  state.
- Held item change `0x10`, animation `0x12`, and entity action `0x13` are
  decoded and drained without disconnecting.
- Digging `0x0E` and placement `0x0F` are decoded for inventory-backed MVP
  break/place behavior in visible loaded chunks.
- Placement handles the Beta item-use-in-air sentinel
  `x=-1,y=255,z=-1,face=-1` without mutation or disconnecting.
- Placement and digging trace logs now include compact rejection/drop context
  for manual real-client testing.
- Player inventory window `0` currently maps hotbar indices `0..=8` to window
  slots `36..=44`; `0x10 HeldItemChange` selects those server-side slots.
- Inventory/window packets `0x65` CloseWindow, `0x66` WindowClick, and `0x6A`
  ConfirmTransaction are handled conservatively for player inventory window `0`.
- `S->C 0x67` SetSlot, `0x68` SetWindowItems, and `0x6A` ConfirmTransaction are
  written for starter hotbar sync and simple click corrections.
- `--no-inventory-sync`, `--no-time-update`, `--no-keepalive`,
  `--defer-inventory-sync`, and `--post-join-minimal` are available as narrow
  compatibility toggles for the new clientbound survival-session features.
- Server sends `S->C 0x35 BlockChange` after accepted or rejected block edits.
- Chunk view tracking sends missing nearby chunks when the player changes
  chunks.
- A shared game state owns the flat world, entity manager, connected player
  registry, and world tick counter.
- Modified flat-world chunks are saved and reloaded from an Aurelia-native MVP
  format under `<world>/aurelia-flat-v1/`.
- Block get/set/break/place exists in world state with dirty chunk tracking and
  simple inventory drops.
- Survival gameplay is still incomplete: crafting, smelting, chests, workbench
  UI, item entities, real health/damage/death, visible mobs, and combat are not
  implemented.
- Placement remains conservative: blocks are placed only into air, with no
  replaceable-block table, collision checks, or full use-on-block item behavior
  yet.

The current repository is a playable-test foundation, not a compatibility
claim.

## Native Persistence Format

The current persistence format is Aurelia-native and only stores modified
flat-world chunks. Each dirty chunk is written as `c.<chunkX>.<chunkZ>.achunk`
inside `<world>/aurelia-flat-v1/` with:

- magic bytes `AURELIA-CHUNK-1`;
- big-endian `i32` chunk X and Z;
- big-endian `u32` block array length followed by 32768 block IDs;
- big-endian `u32` metadata array length followed by 32768 metadata bytes.

Unchanged chunks are generated from the flat-world generator. This is not
Anvil, McRegion, or a vanilla-compatible save format.

## Compatibility Principles

- Compatibility claims must be backed by tests or documented observations.
- Behavior notes should be written cleanly and independently.
- Implementation code must stay original.

## TODOs

- Track protocol compatibility by packet.
- Track world compatibility by client-visible behavior.
- Track survival compatibility by gameplay system.
- Add manual client test notes once networking exists.
