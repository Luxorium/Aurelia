# TODO

## Current Milestone

0.4.0 - Basic world interaction.

The project base compiles and has Rust workspace scaffolding. A blocking TCP
listener accepts clients and creates a long-lived `PlayerSession`. Shared game
state owns the flat world, entity manager, connected player registry, and tick
counter. Normal mode still disconnects after handshake.
`--experimental-join --playable-flat-world` can decode handshake/login, verify
protocol version `14`, send provisional join packets and a flat spawn chunk
area, stream missing chunks as the player moves between chunks, then keep
reading movement and basic interaction packets.

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

## Next Recommended Tasks

- Test the Beta-format `0x32`/`0x33` chunk path against a clean Beta 1.7.3
  client and report whether terrain appears, downloading terrain hangs, or the
  client disconnects.
- Paste S->C `0x32` and `0x33` traces and the first C->S movement traces after
  chunk send.
- If the client still shows no terrain, send a wider chunk area around spawn or
  adjust the raw block-region lighting/data layout.
- Decide protocol-version mismatch behavior from clean observations.
- Add direction-aware codec registries when clientbound codecs are introduced.
- Capture clean payload layouts for chat and keepalive.
- Validate `0x0E`, `0x0F`, `0x10`, `0x12`, `0x13`, and `0x35` behavior against
  a real Beta 1.7.3 client and adjust only from clean observations.
- Replace MVP placement fallback with real inventory and item/block validation.

## Known Blockers

- Handshake and disconnect string formats are implemented from documented
  clean-room assumptions, not yet verified against captured Beta 1.7.3 traffic.
- Clientbound login response and chat packet field order still need clean-room
  confirmation.
- Keepalive and chat packet layouts are not implemented yet; those IDs are
  trace-named only.
- Digging and placement are intentionally minimal: no inventory checks, no tool
  speed, no drops, no permissions, and no persistence.
- The `beta173-observed` login response is only provisionally accepted; it is
  not yet a supported compatibility claim.
- The `mcdevs-legacy` response reset/disconnected the latest tested client.
- The Beta-format experimental chunk shape is still unverified against a real
  client and may be rejected or ignored.
- The default playable chunk radius sends a 3x3 area around spawn, but the
  correct client-visible chunk strategy is still unverified.
- The `unusedOrSeed` field name is intentionally conservative until its exact
  semantics are verified.
- The current codec registry is serverbound-only in practice; clientbound
  registry design is intentionally deferred.
- No real client compatibility testing should be claimed until handshake, login,
  and disconnect behavior are verified against clean observations.

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
