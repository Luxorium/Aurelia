# Compatibility

Aurelia targets eventual compatibility with original Minecraft Beta 1.7.3 clients.

Current project version: `0.2.0`.

Current milestone: **Vanilla Parity Foundation**.

Aurelia is unofficial and is not affiliated with Mojang, Microsoft, or Minecraft. This document tracks clean-room compatibility evidence and known gaps; it is not a claim of complete server parity.

## Current State

- Blocking TCP networking exists.
- Handshake and observed serverbound login are decoded.
- `--experimental-join --playable-flat-world` sends provisional join packets,
  a flat spawn chunk area, waits for first movement, then starts post-join sync
  conservatively and keeps the socket open.
- Latest manual real-client testing reached clean quit after login, chunks,
  movement, break/place, crouch/entity action, inventory open/close, and many
  `0x66` WindowClick packets.
- `0x00` KeepAlive is sent periodically when enabled. Serverbound KeepAlive
  decode is compatibility-gated by `--keepalive-mode`; the default
  `serverbound-no-payload` does not consume four bytes from the following
  stream until the Beta 1.7.3 serverbound payload shape is verified.
- `0x03` Chat is decoded and echoed to the sender; debug slash commands are
  implemented for `/aurelia`, `/whereami`, `/givebasic`, `/save`, `/setblock`,
  and `/time`.
- `S->C 0x04` TimeUpdate uses the Beta 1.7.3 one-`i64` layout. The default
  compatibility mode sends it once after first movement; `--time-update-mode`
  can switch it off or back to interval sends.
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
- Complete Beta 1.7.3 item and block rule tables drive stack sizes, placeable
  checks, tool categories/tiers, harvest requirements, drops, solidity, and
  light emission for every block id `0..=96` and item id `256..=357` plus both
  music discs. Values from public documentation are isolated and marked
  approximate for later trace correction.
- Technical block forms (fluids, fire, the block forms of doors, signs, beds,
  repeaters, sugar cane, and cake, piston internals, and portal) are classified
  as never placeable from an inventory stack, so a hostile client cannot place
  them by claiming the block id as a held item.
- Placement handles the Beta item-use-in-air sentinel
  `x=-1,y=255,z=-1,face=-1` without mutation or disconnecting, and sends a
  held-slot `SetSlot` correction when inventory sync is enabled.
- Placement and digging trace logs now include compact rejection/drop context
  for manual real-client testing. Rejected placement sends a selected hotbar
  `SetSlot` correction after block corrections and does not consume inventory.
- Player inventory window `0` currently maps hotbar indices `0..=8` to window
  slots `36..=44`; `0x10 HeldItemChange` selects those server-side slots.
- Inventory/window packets `0x65` CloseWindow, `0x66` WindowClick, and `0x6A`
  ConfirmTransaction are handled conservatively for player inventory window `0`.
- `S->C 0x67` SetSlot, `0x68` SetWindowItems, and `0x6A` ConfirmTransaction are
  written for starter hotbar sync and simple click corrections.
- `--no-inventory-sync`, `--no-time-update`, `--time-update-mode
  off|once|interval`, `--no-keepalive`, `--keepalive-mode
  off|serverbound-no-payload|serverbound-int32`, `--defer-inventory-sync`, and
  `--post-join-minimal` are available as narrow compatibility toggles for the
  new clientbound survival-session features.
- Server sends `S->C 0x35 BlockChange` after accepted or rejected block edits.
- Chunk view tracking sends missing nearby chunks when the player changes
  chunks and sends `S->C 0x32` visibility unloads for chunks that leave the
  configured radius.
- A shared game state owns the flat world, entity manager, connected player
  registry, and world tick counter.
- Modified flat-world chunks are saved and reloaded from an Aurelia-native MVP
  format under `<world>/aurelia-flat-v1/`.
- Basic player state is saved and reloaded from an Aurelia-native format under
  `<world>/aurelia-players-v1/`.
- Block get/set/break/place exists in world state with dirty chunk tracking and
  rule-driven inventory drops.
- Server-side health/death state, fall damage, void damage, and a respawn
  helper exist, but client-visible health/death/respawn packet sync is not yet
  verified.
- Survival gameplay is still incomplete: crafting, smelting, chests, workbench
  UI, item entities, visible mobs, and combat are not implemented.
- Placement remains conservative: blocks are placed only into air, with no
  replaceable-block table, collision checks, or full use-on-block item behavior
  yet.

The current repository is an early playable-test foundation, not a complete compatibility claim.

## Manual 0.2.0 Acceptance Checklist

Use a clean Minecraft Beta 1.7.3 client and a local Aurelia server started with
`--experimental-join --playable-flat-world`. Keep packet tracing enabled when
capturing regressions, but do not treat this as a full compatibility claim.

- Join the server.
- Walk around.
- Cross chunk boundaries.
- Break dirt, stone, glass, and ore blocks.
- Verify drops and inventory behavior.
- Place blocks from the hotbar.
- Try placing an invalid or non-placeable item.
- Take damage.
- Die if the current packet flow allows a stable client observation.
- Respawn if a verified packet path has been implemented.
- Run `/save`.
- Restart the server.
- Verify position, inventory, and world edits persist.
- Verify no client crash or disconnect.

## Native Persistence Format

The current persistence format is Aurelia-native and only stores modified
flat-world chunks. Each dirty chunk is written as `c.<chunkX>.<chunkZ>.achunk`
inside `<world>/aurelia-flat-v1/` with:

- magic bytes `AURELIA-CHUNK-1`;
- big-endian `i32` chunk X and Z;
- big-endian `u32` block array length followed by 32768 block IDs;
- big-endian `u32` metadata array length followed by 32768 metadata bytes.

Player data is written as simple text `.aplayer` files under
`<world>/aurelia-players-v1/` with username, position, rotation, health, spawn
position, selected hotbar slot, and player inventory slots.

Unchanged chunks are generated from the flat-world generator. These formats are
not Anvil, McRegion, NBT, or vanilla-compatible save formats.

## Compatibility Principles

- Compatibility claims must be backed by tests or documented observations.
- Behavior notes should be written cleanly and independently.
- Implementation code must stay original.
- Do not include Mojang source code, Minecraft assets, generated jars, decompiled source, copied protocol code, or copied server/modding project implementations.
- Current persistence is Aurelia-native and not vanilla McRegion/NBT.

## TODOs

- Track protocol compatibility by packet.
- Track world compatibility by client-visible behavior.
- Track survival compatibility by gameplay system.
- Add clean manual client test notes as real-client behavior is verified.
