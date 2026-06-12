# Beta 1.7.3 Protocol Notes

This document records clean-room protocol assumptions and implementation status for Aurelia `0.2.0` / Vanilla Parity Foundation.

Protocol notes are not permission to copy protocol code from Mojang or other server/modding projects. Use public documentation, black-box traces, original observations, independent notes, and tests.

## Scope

Aurelia will target the original Beta 1.7.3 client protocol. The current code
contains packet model stubs, packet codec interfaces, a small packet frame
helper for the leading packet ID byte plus caller-sized payload bytes, and
typed codecs for handshake, observed serverbound login, provisional clientbound
login response, spawn position, player position/look, experimental chunk data,
keepalive, chat, time, inventory/window sync, block changes, and disconnect
payloads.
A blocking TCP listener accepts clients and runs a per-connection player
session loop backed by shared game state. With
`--experimental-join --playable-flat-world`, the session can decode handshake
and login, send provisional join packets, send a small flat spawn chunk area,
stream newly needed chunks as the player changes chunks, sync a starter hotbar,
handle movement and basic survival interactions, and persist modified flat-world
chunks. This is still an MVP compatibility path, not complete protocol support.
Developer packet tracing can log incoming and outgoing packet metadata.

Beta-era packets are not treated as length-prefixed frames here. Packet-specific
decoders must know how many bytes to consume from the stream. Until that is
verified per packet, frame reads require the caller to provide the payload
length.

## Legacy String Assumption

Handshake and disconnect codecs currently use this clean-room assumption:

- A string starts with an unsigned 16-bit big-endian character count.
- Each character is encoded as a big-endian UTF-16 code unit.
- Lengths above the packet-specific maximum are invalid.
- Truncated character payloads are invalid.

This is implemented in `LegacyStringIO` and covered by unit tests using explicit
byte streams. The observed `Luxorium` handshake trace confirms this format for
packet `0x02`. The exact Beta 1.7.3 string limits and all packet-specific field
details still need verification against clean observations before claiming real
client compatibility.

## Beta 1.7.3 Login

Detailed login research is tracked in `docs/BETA_173_LOGIN_RESEARCH.md`.

### Direction Split

Older Minecraft protocol traffic may reuse packet ID `0x01` in both directions.
Aurelia treats these as separate protocol surfaces:

- `C->S 0x01 Login`: implemented from the captured clean client trace.
- `S->C 0x01 Login Response`: implemented only as provisional experimental
  encode strategies behind `--experimental-join`.

The clientbound layouts are not verified and must not be treated as
compatibility evidence until tested against a real Beta 1.7.3 client.

### Verified In Aurelia

- Aurelia has a `ServerboundLoginPacket` model with packet ID `0x01`.
- Aurelia has a `ServerboundLoginPacketCodec` for the observed
  client-to-server layout.
- `PacketCodecRegistry::beta173_defaults()` registers the login codec.
- Normal server mode does not send a login response.
- Normal server mode does not advance connections past handshake into login.
- Trace continuation mode can decode a serverbound login packet and then either
  disconnect or enter the experimental playable flat-world path.
- Experimental join mode can send provisional clientbound login response,
  spawn position, player position/look, chunk visibility, and chunk data.
- Playable flat-world mode keeps the socket open after join and updates
  server-side player state from fixed-size movement packets.
- Chunk view tracking sends missing chunks around the current player chunk.
- KeepAlive, chat, inventory/window, digging, placement, and basic block-change
  packets are decoded or written for the stable survival-session MVP.

### Observed In Current Aurelia Tests

- A client can connect to the local test listener, send a handshake frame, and
  receive a disconnect frame.
- Trace continuation can send a trace-only handshake response, receive the
  observed serverbound login packet, decode it, and return a clear
  world-join-not-implemented disconnect.
- Unknown, missing, and malformed initial packets receive explicit disconnect
  reasons where possible.
- The latest manual real-client trace stayed connected through login, chunk
  loading, movement, block breaking, block placement, crouch/entity action,
  inventory open/close, and many `0x66 WindowClick` packets. The final
  disconnect was a clean client quit.

### Clean-Room Assumptions

- Beta 1.7.3 protocol version is expected to be `14`.
- Packet ID `0x01` is used for the observed serverbound login packet.
- Login remains direction-specific; the observed serverbound fields do not prove
  the clientbound login response.
- Trace names are direction-aware: `C->S 0x01` is `Login`, while `S->C 0x01` is
  `LoginResponse`.

### Observed Serverbound Login Layout

Captured clean client payload for username `Luxorium`:

```text
00 00 00 0E 00 08 00 4C 00 75 00 78 00 6F 00 72 00 69 00 75 00 6D 00 00 00 00 00 00 00 00 00
```

Implemented decode order:

- `int protocolVersion`: `14`.
- `legacy string username`: `Luxorium`.
- `long unusedOrSeed`: `0`.
- `byte dimension`: `0`.

### Unresolved Questions

- Exact Beta 1.7.3 clientbound login response field order.
- Exact protocol mismatch behavior.
- Exact production handshake response that should precede login.
- Meaning of the observed serverbound `unusedOrSeed` field.

Because the clientbound response is not yet verified, Aurelia must not claim
login support even though experimental join attempts a provisional sequence.

### Experimental Clientbound Login Response Layouts

These layouts are only sent when `--experimental-join` is enabled.

`--login-response-mode=beta173-observed` writes:

- Packet ID: `0x01`.
- `int entityId`: default `1`.
- `legacy string levelTypeOrUnused`: default empty string.
- `long mapSeed`: default `0`.
- `byte dimension`: default `0`.

`--login-response-mode=mcdevs-legacy` writes:

- Packet ID: `0x01`.
- `int entityId`: default `1`.
- `legacy string levelType`: default `default`.
- `byte gameMode`: default `0`.
- `byte dimension`: default `0`.
- `byte difficulty`: default `1`.
- `byte unused`: default `0`.
- `byte maxPlayers`: default `8`.

Both modes are provisional. If a real client rejects either one, the trace
output should be used to adjust or remove the failed layout.

Latest real-client evidence:

- `beta173-observed` was accepted far enough for the client to send movement
  packets.
- `mcdevs-legacy` reset/disconnected after the longer login response and is
  kept only as an alternate debug mode.

### Experimental Clientbound Spawn Packets

`S->C 0x06 SpawnPosition` writes:

- `int x`: default `0`.
- `int y`: default `65`.
- `int z`: default `0`.

`S->C 0x0D PlayerPositionLook` writes:

- `double x`: default `0.5`.
- `double y`: default `66.0`.
- `double stance`: default `67.62`.
- `double z`: default `0.5`.
- `float yaw`: default `0.0`.
- `float pitch`: default `0.0`.
- `boolean onGround`: default `false`.

The `0x0D` server-to-client field order is experimental. Correct it if the
client rejects the packet or traces show another order.

### Experimental Clientbound Chunk Data

`S->C 0x32 SetChunkVisibility` and `S->C 0x33 ChunkData` are sent only in
`--experimental-join` / `--playable-flat-world` compatibility paths.

`S->C 0x32 SetChunkVisibility` writes:

- `int chunkX`.
- `int chunkZ`.
- `boolean load`; `true` marks a chunk visible before data, and `false`
  unloads a chunk that left the configured view radius.

`S->C 0x33 ChunkData` uses the Beta block-region shape:

- `int x`.
- `short y`.
- `int z`.
- `byte widthMinusOne`.
- `byte heightMinusOne`.
- `byte lengthMinusOne`.
- `int compressedSize`.
- `byte[] compressedData`.

Current conservative send:

- Sends `0x32` with `load = true` before each newly visible chunk.
- Sends `0x32` with `load = false` when a previously visible chunk leaves the
  configured radius.
- Sends `0x33` for each newly visible block region.
- Sets `widthMinusOne = 15`, `heightMinusOne = 127`, `lengthMinusOne = 15`.
- Raw compressed data is a full 16x128x16 region:
  - Blocks: 32768 bytes.
  - Data: 16384 bytes.
  - Block light: 16384 bytes.
  - Sky light: 16384 bytes.
  - Total raw bytes: 81920.
- Block index order is `index = y + (z * 128) + (x * 128 * 16)`.
- Terrain is stone through Y `58`, dirt at Y `59..62`, grass at Y `63`, and
  air from Y `64..127`.
- Metadata and block light are zero. Sky light is full `0xFF` nibbles.

This layout is experimental and may need correction if the client rejects it or
continues to show no terrain.

## Implemented Packet Payload Codecs

- `0x02` Handshake: username string, currently limited to 16 characters.
- `C->S 0x01` Serverbound login: int protocol version, username string, long
  `unusedOrSeed`, byte dimension.
- Experimental `S->C 0x01` login response in two provisional modes.
- Experimental `S->C 0x06` spawn position: three ints.
- `0x00` KeepAlive: int keepalive ID.
- `0x03` Chat: legacy string, truncated to a short Beta-safe limit.
- Experimental Beta 1.7.3 `S->C 0x04` time update: one long time value.
- Experimental `S->C 0x0D` player position/look: doubles, floats, boolean.
- Experimental `S->C 0x32` set chunk visibility: two ints and a boolean.
- Experimental `S->C 0x33` chunk data: Beta block-region compressed packet.
- Experimental `S->C 0x67` set slot: window ID, slot, legacy slot data.
- Experimental `S->C 0x68` set window items: window ID, slot count, slot array.
- Experimental `0x6A` confirm transaction: window ID, action number, accepted.
- `0xFF` Disconnect: reason string, currently limited to 100 characters.

## Codec Registry

`PacketCodecRegistry::beta173_defaults()` currently stores direction-aware
packet metadata for known MVP packet IDs. Shared IDs are named by direction;
for example, `C->S 0x01` is `Login` and `S->C 0x01` is `LoginResponse`.

Unknown packet IDs return an empty lookup. Aurelia does not infer unknown
payload lengths for undocumented packets.

## Session Loop

The server listener accepts TCP connections and creates a `PlayerSession` for
each socket. The session tracks connection states:

- `Handshaking`.
- `Login`.
- `Joined`.
- `Disconnected`.

Experimental join also tracks a finer join phase: `Handshaking`, `Login`,
`SendingInitialWorld`, `AwaitingFirstClientMovement`, and `JoinedReady`.

Current outcomes:

- Normal mode still decodes `0x02` handshake and sends
  `DisconnectPacket("Aurelia received your handshake, but login is not implemented yet.")`.
- Missing initial packet sends
  `DisconnectPacket("Aurelia did not receive an initial packet before disconnecting.")`.
- Unknown pre-login packet ID sends
  `DisconnectPacket("Aurelia does not understand your initial packet yet.")`.
- Malformed or truncated known packet sends
  `DisconnectPacket("Aurelia could not decode your initial packet.")`.
- Protocol versions other than `14` receive a protocol mismatch disconnect.

With `--experimental-join`, the listener instead attempts:

1. Read and decode `C->S 0x02` handshake.
2. Send `S->C 0x02` trace handshake response string, default `-`.
3. Read and decode `C->S 0x01` serverbound login.
4. Send provisional `S->C 0x01` login response using the selected mode.
5. Send `S->C 0x06` spawn position.
6. Send `S->C 0x0D` player position/look.
7. Enter `SendingInitialWorld`.
8. Send `S->C 0x32` set chunk visibility.
9. Send experimental `S->C 0x33` chunk data packets.
10. Enter `AwaitingFirstClientMovement`.
11. After the first `0x0A`, `0x0B`, `0x0C`, or `0x0D` movement packet, enter
    `JoinedReady`.
12. Send deferred `S->C 0x04` time update and start periodic keepalive/time
    updates.
13. After at least three additional movement/player packets, send deferred
    `S->C 0x68` starter inventory/window items.

With `--playable-flat-world`, the initial chunk radius defaults to `1`, sending
the spawn chunk and its eight neighbors. As the player crosses chunk
boundaries, Aurelia sends newly required chunks and unloads chunks outside the
configured radius. Use `--chunk-radius 0` to send only the player's current
chunk.

`--no-inventory-sync`, `--no-time-update`, `--time-update-mode
off|once|interval`, `--no-keepalive`, and `--keepalive-mode
off|serverbound-no-payload|serverbound-int32` control narrow compatibility
surfaces for testing. `--defer-inventory-sync` keeps starter inventory delayed
until after several post-join movements; this is currently the default.
`--post-join-minimal` suppresses TimeUpdate, inventory sync, SetSlot,
ConfirmTransaction, chat responses, and KeepAlive scheduling. These flags do
not disable chunk streaming, break/place, or server-side inventory state.

### Clientbound `0x04` TimeUpdate

In Beta 1.7.3/protocol 14 compatibility mode, Aurelia encodes `S->C 0x04`
TimeUpdate as exactly one big-endian `i64` time value:

- packet id: `0x04`;
- payload: `time: i64`;
- payload length: 8 bytes;
- full packet length: 9 bytes.

The modern two-long layout (`world_age: i64`, `time_of_day: i64`) is kept
separate in code and is intentionally not used for Beta 1.7.3. Sending the
modern layout to a Beta client leaves an extra long in the stream and corrupts
the following packet boundary.

The joined loop currently handles:

- `C->S 0x0A Player`.
- `C->S 0x00 KeepAlive`.
- `C->S 0x03 Chat`.
- `C->S 0x0B PlayerPosition`.
- `C->S 0x0C PlayerLook`.
- `C->S 0x0D PlayerPositionLook`.
- `C->S 0x0E PlayerDigging`.
- `C->S 0x0F PlayerBlockPlacement`.
- `C->S 0x10 HeldItemChange`.
- `C->S 0x12 Animation`.
- `C->S 0x13 EntityAction`.
- `C->S 0x65 CloseWindow`.
- `C->S 0x66 WindowClick`.
- `C->S 0x6A ConfirmTransaction`.
- `C->S 0xFF Disconnect`.

Movement packets update the server-side player position, look, on-ground flag,
and current chunk. KeepAlive responses are accepted through the configured
compatibility mode; the default does not consume serverbound payload bytes.
Chat is echoed or handled as short debug commands. Interaction packets are applied
conservatively: animation is ignored, entity action tracks crouch where
possible, held item change stores hotbar indices `0..=8`, digging status `2`
breaks reachable visible non-bedrock blocks and applies rule-driven harvest
drops, and placement uses the selected server-side hotbar stack. Server-side
health/death state, fall damage, void damage, and a respawn helper exist, but
Aurelia does not yet send unverified health/death/respawn packets. Window clicks handle
basic left/right pick-up and place behavior for window `0`; unsupported click
modes are rejected with transaction failure plus inventory resync instead of
disconnecting.
Unsupported post-join packet IDs are traced and receive a clear disconnect
instead of panicking.

### Inventory Slot Mapping

Aurelia currently treats player inventory window `0` as 45 slots. The hotbar
maps directly to window slots:

- hotbar index `0` -> window slot `36`;
- hotbar index `8` -> window slot `44`.

`C->S 0x10 HeldItemChange` selects hotbar indices `0..=8`; placement,
`S->C 0x67 SetSlot`, and `S->C 0x68 SetWindowItems` all use the same mapping.

### Placement And Digging Limits

`C->S 0x0F PlayerBlockPlacement` with `x=-1`, `y=255`, `z=-1`, and
`direction=-1` is treated as item-use-in-air. Aurelia logs it under packet
trace/compat debug and does not mutate world state or decrement inventory.

Normal placement is accepted only when the selected stack is a placeable block,
the target is valid, loaded, in reach, and currently air, and the clicked block
still exists. Rejections send corrective `0x35 BlockChange` packets for the
clicked and target positions where applicable, then send `0x67 SetSlot` for the
selected hotbar/window slot when inventory sync is enabled. Current rejection
reasons are `no-selected-item`, `selected-not-placeable`, `target-unloaded`,
`target-invalid-y`, `target-not-air`, `clicked-block-missing`, and
`out-of-reach`.

Digging status names used by logs are `start`, `cancel`, `finish`,
`drop-stack`, `drop-item`, and `use-finish`. `start` creates or progresses an
active digging state, repeated starts on the same target are progress, target
changes reset tracking, and `cancel` clears tracking. Only valid completion
mutates blocks. Bedrock is protected, and rule-driven drops are added to
inventory only after a successful finish. Capture clean Beta 1.7.3 traces
before adding clientbound health/death/respawn packet writes.

## Packet Trace Mode

Packet tracing is disabled by default. When enabled with `--trace-packets`,
Aurelia logs client-to-server packet metadata for packets it reads:

```text
[trace] C->S #1 id=0x02 name=Handshake payloadLength=10 payloadHex=00 04 00 41 00 6C 00 65 00 78
```

Known packet names are direction-aware. Current names include `Handshake`,
`Login` for `C->S 0x01`, `LoginResponse` for `S->C 0x01`, `Player`,
`PlayerPosition`, `PlayerLook`, `PlayerPositionLook`, `SpawnPosition`,
`TimeUpdate`, `SetChunkVisibility`, `ChunkData`, `BlockChange`, `SetSlot`,
`SetWindowItems`, `CloseWindow`, `WindowClick`, `ConfirmTransaction`, and
`Disconnect`. Unknown packet IDs are formatted as `Unknown`. Because Beta-era
packets are not length-prefixed here, unknown packet payload length is not
inferred.

Experimental post-login tracing reads known C->S movement packets with fixed
payload sizes, plus documented fixed interaction packets:

- `0x0A Player`: 1 byte.
- `0x00 KeepAlive`: controlled by `--keepalive-mode`; default
  `serverbound-no-payload` consumes no payload, `serverbound-int32` consumes 4
  bytes for trace comparison only.
- `0x0B PlayerPosition`: 33 bytes.
- `0x0C PlayerLook`: 9 bytes.
- `0x0D PlayerPositionLook`: 41 bytes.
- `0x0E PlayerDigging`: 11 bytes.
- `0x10 HeldItemChange`: 2 bytes.
- `0x12 Animation`: 5 bytes.
- `0x13 EntityAction`: 5 bytes.
- `0x65 CloseWindow`: 1 byte.
- `0x6A ConfirmTransaction`: 4 bytes.

`0x0F PlayerBlockPlacement` is variable length because it carries legacy slot
data. In Beta 1.7.3 Aurelia decodes `x`, `y`, `z`, `direction`, then slot data
only. It must not read cursor bytes after the slot; those bytes belong to the
next packet in observed real-client streams.

`0x66 WindowClick` is also variable length. Aurelia decodes `window_id`, `slot`,
`mouse_button`, `action_number`, `shift`, then legacy clicked-item slot data.
Clicked-item slot data is `item_id`; if `item_id == -1`, the slot is empty and
the packet ends. Otherwise Aurelia reads `count` and `damage`. Window `0`
left/right clicks are handled for simple stack pickup/place behavior. Shift
click and unsupported modes are rejected and followed by a full `0x68` resync.

Packet tracing is developer-only. It should be used to capture clean behavior
notes and byte streams, not as a compatibility claim. Redact any credentials,
session identifiers, or secrets if future protocol work exposes them.

Trace continuation mode is also available for login response research:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake
```

When enabled, Aurelia sends a trace-only `0x02` handshake response string after
decoding the client handshake. Packet metadata is logged until the trace limit,
but the session can continue reading packets after tracing stops. The default
response string is `-`; this is a trace-mode assumption, not verified login
response support.

## Initial Packet Areas

- Handshake.
- Login.
- Chat.
- Player position.
- Chunk data.
- Disconnect.

## Milestone TODOs

- Document packet IDs from clean observations.
- Document primitive encodings and string limits.
- Add packet framing.
- Add encode/decode tests based on observed byte streams.
- Verify string encoding and length limits against clean observations.
- Verify clientbound handshake and login response behavior before accepting real
  clients.
- Add packet-specific codecs for chat only after field order is documented.
- Capture clean Beta 1.7.3 clientbound login response bytes before treating
  experimental world join as supported.

All notes must be written without copying decompiled Mojang source.
