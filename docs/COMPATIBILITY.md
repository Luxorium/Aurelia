# Compatibility

Aurelia targets eventual compatibility with original Minecraft Beta 1.7.3
clients.

## Current State

- Blocking TCP networking exists.
- Handshake and observed serverbound login are decoded.
- `--experimental-join --playable-flat-world` sends provisional join packets,
  a flat spawn chunk area, and keeps the socket open.
- Movement packets `0x0A`, `0x0B`, `0x0C`, and `0x0D` update server-side player
  state.
- Held item change `0x10`, animation `0x12`, and entity action `0x13` are
  decoded and drained without disconnecting.
- Digging `0x0E` and placement `0x0F` are decoded for MVP break/place behavior
  in visible loaded chunks.
- Server sends `S->C 0x35 BlockChange` after accepted or rejected block edits.
- Chunk view tracking sends missing nearby chunks when the player changes
  chunks.
- A shared game state owns the flat world, entity manager, connected player
  registry, and world tick counter.
- Chunk serialization is still experimental and limited to a generated flat
  test area.
- Block get/set/break/place exists in world state, but inventory validation,
  drops, tool speed, and persistence are not implemented.
- Survival gameplay is scaffolded only; inventory, damage, crafting, item
  drops, visible mobs, and combat are not implemented.

The current repository is a playable-test foundation, not a compatibility
claim.

## Compatibility Principles

- Compatibility claims must be backed by tests or documented observations.
- Behavior notes should be written cleanly and independently.
- Implementation code must stay original.

## TODOs

- Track protocol compatibility by packet.
- Track world compatibility by client-visible behavior.
- Track survival compatibility by gameplay system.
- Add manual client test notes once networking exists.
