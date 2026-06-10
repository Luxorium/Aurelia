# Roadmap

## 0.1.0 - Project Base

- Cargo workspace and crates.
- Basic server startup.
- Protocol scaffolding.
- World/chunk scaffolding.
- Region scheduler scaffolding.

TODO:

- Keep APIs small and original.
- Keep all compatibility claims clearly scoped.
- Add focused tests for every new foundational type.

## 0.1.2 - Stable Survival Session MVP

- Real Beta 1.7.3 client can join the experimental flat-world path.
- Chunk loading, movement, block breaking, block placement, crouch/entity
  action, inventory open/close, many window clicks, and clean quit are stable in
  the latest manual trace.
- Starter hotbar sync through `0x68` SetWindowItems.
- Initial world chunks are sent before starter inventory/time sync. Beta
  TimeUpdate uses one `i64` after first movement, and inventory sync waits for
  additional movement packets.
- Inventory-backed placement and simple break drops.
- Conservative window click handling with transaction confirmations and resync.
- Chat/debug commands.
- Periodic keepalive and Beta 1.7.3 one-long time update packets.
- Compatibility toggles: `--no-inventory-sync`, `--no-time-update`,
  `--no-keepalive`, `--defer-inventory-sync`, and `--post-join-minimal`.
- Aurelia-native dirty chunk persistence for modified flat-world chunks.

TODO:

- Validate keepalive cadence and inventory transaction behavior over longer
  real-client sessions.
- Expand inventory rules only after preserving stream stability.
- Keep persistence native until a separate vanilla-format milestone.

## 0.1.3 - Survival Interaction Polish

- Structured compat-debug logs for placement, item-use-in-air, digging,
  held-item changes, window clicks, keepalive, and chat commands.
- Explicit player inventory hotbar mapping: hotbar `0..=8` maps to window
  slots `36..=44`.
- Placement rejects with specific reasons and sends corrective block changes
  for clicked/target blocks without decrementing inventory.
- Special `0x0F` item-use-in-air packets are ignored safely.
- Digging start/cancel/drop/use statuses do not mutate blocks; finish breaks
  only reachable loaded non-bedrock blocks and logs drop inventory results.

TODO:

- Validate placement rejection reasons against longer real-client traces.
- Add replaceable-block and collision semantics after packet stability remains
  clean.
- Expand window/crafting behavior only after conservative inventory clicks stay
  stable.

## 0.2.0 - Beta 1.7.3 Protocol Handshake

- TCP listener.
- Packet framing.
- Handshake.
- Login response.
- Disconnect handling.

TODO:

- Keep the current packet frame helper limited to the packet ID byte and
  caller-sized payloads until packet-specific formats are verified.
- Verify handshake and disconnect string formats from clean observations.
- Use packet trace mode to capture clean client packet bytes before implementing
  more codecs.
- Capture Beta 1.7.3 clientbound login response bytes before treating any
  provisional response codec as supported.
- Keep serverbound and clientbound `0x01` login packets direction-separated.
- Keep `--experimental-join` provisional until a real client confirms the full
  login/spawn/chunk sequence.
- Validate the provisional chunk data packet against a real client before
  calling flat-world join complete.
- Document packet IDs and wire fields from behavior notes before adding more
  packet codecs.
- Add golden packet encode/decode tests using original observations, not source.

## 0.3.0 - Flat World Join

- Original Beta 1.7.3 client joins.
- Spawn position.
- Flat chunk sending.
- Movement packet loop.

TODO:

- Use experimental join traces to determine whether the Beta-format spawn chunk
  is accepted and whether a wider chunk area is required.
- Stabilize chunk unload behavior and wider chunk streaming after more real
  client traces.
- Add join/disconnect lifecycle tests.
- Add chat only after field order is documented.

## 0.4.0 - Basic World Interaction

- Movement handling.
- Block break/place.
- Chunk updates.
- Shared game state.
- Server tick loop.
- MVP joined-state interaction packets.

TODO:

- Define authority rules for player movement and block edits.
- Validate MVP digging, placement, held-item, animation, entity-action, and
  block-change behavior against a real Beta 1.7.3 client.
- Expand inventory-backed item/block validation beyond the MVP starter hotbar.
- Keep common post-login packet IDs traceable, but do not guess unknown payload
  lengths.
- Keep storage format original and documented.

## 0.5.0 - Region-Threaded Tick Prototype

- Fixed region sections.
- Region mailboxes.
- Safe cross-region task scheduling.
- Thread ownership assertions.

TODO:

- Add target-region scheduling APIs.
- Add development-only diagnostics for wrong-thread mutation.
- Delay advanced region merge/split logic until ownership is proven.

## 0.6.0 - Survival Foundation

- Inventory.
- Items.
- Health.
- Damage.
- Crafting basics.

TODO:

- Model item stacks with explicit Beta-era constraints.
- Add deterministic gameplay tests.

## 0.7.0 - Tile Entities

- Chests.
- Furnaces.
- Signs.

TODO:

- Define tile entity ticking ownership.
- Add save/load coverage.

## 0.8.0 - Entities and Mobs

- Dropped items.
- Animals.
- Hostile mobs.
- Basic AI.

TODO:

- Keep entity mutation region-owned.
- Add cross-region entity transfer plan.

## 0.9.0 - Fluids/Redstone

- Water/lava ticking.
- Redstone basics.
- Scheduled block ticks.

TODO:

- Route scheduled block ticks through region queues.
- Validate deterministic update order.

## 1.0.0 - Playable Beta 1.7.3-Compatible Server

- Original Beta 1.7.3 client can play survival on Aurelia.
- Compatibility behavior is documented and tested.
- Region-threaded architecture has stable ownership rules.
