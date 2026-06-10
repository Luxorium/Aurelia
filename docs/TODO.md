# TODO

## Current Milestone

0.2.0 - Vanilla Parity Foundation.

The project base compiles and has Rust workspace scaffolding. A blocking TCP
listener accepts clients and creates a long-lived `PlayerSession`. Shared game
state owns the flat world, entity manager, connected player registry, and tick
counter. Normal mode still disconnects after handshake.
`--experimental-join --playable-flat-world` can decode handshake/login, verify
protocol version `14`, send provisional join packets and a flat spawn chunk
area, wait for first client movement before post-join sync, stream missing
chunks and unload out-of-range chunks as the player moves between chunks, sync
a starter hotbar, keepalive/time update the client, process chat/debug
commands, handle basic inventory clicks, log placement/digging decisions with
clear reasons, apply early survival item/block rules, persist modified flat
chunks and basic player state, then cleanly handle client quit.

## Completed This Run

- Replaced the incorrect newer-style `0x33` bitmap chunk packet with the Beta
  1.7.3 block-region shape.
- Added experimental S->C `0x32` set chunk visibility before chunk data.
- Changed experimental `0x33` to write `int x`, `short y`, `int z`, byte
  dimensions, compressed size, and compressed data.
- Replaced the one-section chunk generator with a full 16x128x16 raw region.
- Generated raw chunk data as Blocks, Data, BlockLight, and SkyLight arrays,
  totaling 81920 bytes before compression.
- Added a simple terrain column: stone, dirt, grass at Y `63`, then air.
- Updated socket-level experimental join tests to expect `0x01`, `0x06`,
  `0x0D`, `0x32`, and `0x33`.
- Added a player session loop with `Handshaking`, `Login`, `Joined`, and
  `Disconnected` states.
- Added movement packet decoding and server-side player position updates.
- Added `--playable-flat-world` and configurable `--chunk-radius`.
- Added world time, survival marker, health field, and entity/mob scaffolding.
- Added shared `GameServerState`, player registration/unregistration, and a
  basic 20 TPS tick loop.
- Added chunk view tracking with duplicate-send prevention.
- Added trace names for common undocumented post-login packet IDs.
- Added serverbound decode for held item change, animation, entity action,
  digging, and block placement.
- Added MVP break/place behavior in visible loaded chunks with `S->C 0x35`
  block-change responses.
- Added legacy slot-data parsing for placement packets and safe empty-slot
  handling.
- Fixed Beta 1.7.3 `0x0F PlayerBlockPlacement` decoding to stop after slot
  data instead of consuming non-existent cursor bytes from the next packet.
- Added conservative joined-state decoding for `0x65` CloseWindow, `0x66`
  WindowClick, and `0x6A` ConfirmTransaction so inventory clicks are drained
  without disconnecting; this has now been extended with basic inventory state.
- Added `0x00` KeepAlive send/decode and lenient received-ID tracking.
- Added `0x03` Chat decode/write plus `/aurelia`, `/whereami`, `/givebasic`,
  `/save`, `/setblock`, and `/time` debug commands.
- Added `0x04` TimeUpdate writes from world time.
- Added `0x67` SetSlot, `0x68` SetWindowItems, and clientbound `0x6A`
  ConfirmTransaction writers.
- Added `PlayerInventory` with starter hotbar stacks, cursor stack, selected
  hotbar tracking, simple left/right window-click behavior, and resync on
  unsupported clicks.
- Replaced normal placement fallback with server-side inventory-backed block
  placement and selected-stack decrement.
- Added basic digging drops to inventory and bedrock break rejection.
- Added Aurelia-native dirty chunk persistence and chunk streaming from stored
  world data instead of static generated chunk bytes.
- Restored real-client login stability by keeping the initial burst to login,
  spawn, position, chunk visibility, and chunk data.
- Fixed Beta 1.7.3 `0x04` TimeUpdate to encode one `i64` payload
  (`payloadLength=8`) instead of the modern two-long layout.
- Deferred `0x68` SetWindowItems until after at least three additional
  movement/player packets following JoinedReady.
- Added `--no-inventory-sync`, `--no-time-update`, `--no-keepalive`,
  `--defer-inventory-sync`, and `--post-join-minimal` compatibility toggles for
  isolating clientbound survival-session features.
- Added explicit hotbar/window-slot mapping helpers: hotbar `0..=8` maps to
  window slots `36..=44`.
- Added structured compat-debug placement logs with click, face, target,
  hotbar index, window slot, item stack, block IDs, result, and reject reason.
- Treated `0x0F` `x=-1,y=255,z=-1,face=-1` as item-use-in-air without world
  mutation or inventory decrement.
- Placement rejects now send clicked/target block corrections where applicable
  and avoid decrementing stacks unless a block was actually placed.
- Added digging status names, bedrock protection coverage, and drop-to-inventory
  result logging.
- Added WindowClick decision logs with accepted/rejected state and changed
  slots.
- Added command bad-argument tests for short Beta-safe usage responses.
- Added a reusable deterministic `ChunkView` abstraction with radius and diff
  tests.
- Added `S->C 0x32` chunk visibility unload support and movement-driven
  unloads for chunks outside the configured radius.
- Added direction-aware packet metadata so `C->S 0x01 Login` and
  `S->C 0x01 LoginResponse` are named separately.
- Added protocol/session regression tests for chunk visibility load/unload
  encoding, chunk view diffs, movement-driven loads/unloads, no unloads for
  chunks still in range, no duplicate unloads, and direction-aware packet
  names.
- Added `docs/VANILLA_PARITY_MATRIX.md` to track vanilla expectations,
  Aurelia status, missing behavior, test strategy, and priority by system.
- Added Beta 1.7.3 item rules for early survival stack limits, placeable
  blocks, tool categories, and tool tiers.
- Added Beta 1.7.3 block rules for early survival materials, approximate
  hardness, preferred/required tools, solidity/transparency, light placeholders,
  and drops.
- Replaced simple break drops with rule-driven harvest behavior. Stone requires
  a pickaxe for cobblestone, glass drops nothing, ore drops follow the current
  rule table, and full inventories do not duplicate drops.
- Tightened placement around valid selected inventory items, placeable item
  rules, loaded chunks, reach, occupied solid targets, stack decrement, and
  rejected-placement correction.
- Added server-side player health/death state, fall/void damage foundation, and
  a respawn helper. Client-visible health/death packets remain unverified.
- Added Aurelia-native player persistence for username, position, rotation,
  health, inventory, and spawn position. Vanilla NBT player save parity remains
  incomplete.

## Next Recommended Tasks

- Run the full manual 0.2.0 acceptance pass against a clean Beta 1.7.3 client:
  - join with `--experimental-join --playable-flat-world`;
  - walk around;
  - cross chunk boundaries;
  - break dirt, stone, glass, and ore blocks;
  - verify drops and inventory behavior;
  - place blocks from the hotbar;
  - try placing invalid/non-placeable items;
  - take damage;
  - die if the current packet flow allows a stable client observation;
  - respawn if a verified packet path is implemented;
  - run `/save`;
  - restart the server;
  - verify position, inventory, and world edits persist;
  - verify no client crash or disconnect.
- Decide protocol-version mismatch behavior from clean observations.
- Expand direction-aware codec registries only when additional clientbound
  codecs are introduced.
- Validate `0x0E`, `0x0F`, `0x10`, `0x12`, `0x13`, and `0x35` behavior against
  a real Beta 1.7.3 client and adjust only from clean observations.
- Validate `0x65`, `0x66`, and `0x6A` inventory/window behavior against a real
  Beta 1.7.3 client and capture transaction response expectations before
  expanding inventory state.
- Add replaceable block/collision semantics for placement after current
  conservative placement stays stable in real-client traces.
- Add item entities for inventory-full drops and pickups.
- Add crafting/workbench/chest/smelting UI support.

## Known Blockers

- Handshake and disconnect string formats are implemented from documented
  clean-room assumptions, not yet verified against captured Beta 1.7.3 traffic.
- Clientbound login response still needs clean-room confirmation.
- Digging and placement are intentionally minimal: no tool speed, no
  permissions, and simplified drops.
- Window/inventory packets support only conservative player-inventory stack
  movement. Shift-click, crafting, armor semantics, workbench, chests, and
  smelting are not implemented.
- Item entities are not implemented; inventory-full drops are ignored.
- Health/death/respawn is server-side only; client-visible packets still need
  clean verification before use.
- Damage sources beyond fall/void placeholders and combat are not implemented.
- The `beta173-observed` login response is only provisionally accepted; it is
  not yet a supported compatibility claim.
- The `mcdevs-legacy` response reset/disconnected the latest tested client.
- The Beta-format experimental chunk shape works in the latest manual trace but
  still needs longer compatibility testing.
- The default playable chunk radius sends a 3x3 area around the current player
  chunk and unloads chunks outside that radius, but the correct production
  client-visible chunk strategy is still unverified.
- The `unusedOrSeed` field name is intentionally conservative until its exact
  semantics are verified.
- Packet metadata is direction-aware for known MVP packets, but full
  clientbound codec registry design is intentionally deferred.
- Persistence is Aurelia-native for modified flat-world chunks and basic player
  data; it is not vanilla McRegion/NBT.

## Testing Notes

- `cargo fmt --all` should leave the workspace formatted.
- `cargo test --workspace` should compile all crates and run unit tests.
- `cargo build --workspace` should build all crates.
- `cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0`
  starts the server on an ephemeral local port and shuts it down immediately.
- `cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --trace-packets --trace-packet-limit 4`
  verifies trace mode can be enabled during startup.
- `cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake`
  verifies trace continuation can be enabled during startup.
- `cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --trace-packets --trace-packet-limit 64 --trace-continue-after-handshake --experimental-join --login-response-mode beta173-observed`
  verifies experimental join can be enabled during startup.
- `cargo run -p aurelia-server -- --smoke-test --host 127.0.0.1 --port 0 --experimental-join --playable-flat-world`
  verifies playable flat-world mode can be enabled during startup.
- `cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512`
  is the recommended real-client debug command.
