# Beta 1.7.3 Protocol Notes

This document records clean-room protocol assumptions and implementation status.

## Scope

Aurelia will target the original Beta 1.7.3 client protocol. The current code
contains packet model stubs, packet codec interfaces, a small packet frame
helper for the leading packet ID byte plus caller-sized payload bytes, and
typed codecs for handshake, observed serverbound login, provisional clientbound
login response, spawn position, player position/look, experimental chunk data,
and disconnect payloads.
A blocking TCP listener accepts clients and runs a per-connection player
session loop backed by shared game state. With
`--experimental-join --playable-flat-world`, the session can decode handshake
and login, send provisional join packets, send a small flat spawn chunk area,
stream newly needed chunks as the player changes chunks, and continue reading
movement packets. This does not prove full world join compatibility yet.
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
- Common undocumented post-login packet IDs are named in traces, but their
  payloads are not decoded until clean field layouts are captured.

### Observed In Current Aurelia Tests

- A client can connect to the local test listener, send a handshake frame, and
  receive a disconnect frame.
- Trace continuation can send a trace-only handshake response, receive the
  observed serverbound login packet, decode it, and return a clear
  world-join-not-implemented disconnect.
- Unknown, missing, and malformed initial packets receive explicit disconnect
  reasons where possible.
- No test currently exercises a real Beta 1.7.3 client or server.

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
`--experimental-join`.

`S->C 0x32 SetChunkVisibility` writes:

- `int chunkX`.
- `int chunkZ`.
- `boolean load`.

`S->C 0x33 ChunkData` uses the Beta block-region shape:

- `int x`.
- `short y`.
- `int z`.
- `byte widthMinusOne`.
- `byte heightMinusOne`.
- `byte lengthMinusOne`.
- `int compressedSize`.
- `byte[] compressedData`.

Current first attempt:

- Sends `0x32` for chunk `(0,0)` with `load = true`.
- Sends `0x33` for block region `x = 0`, `y = 0`, `z = 0`.
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
- Experimental `S->C 0x0D` player position/look: doubles, floats, boolean.
- Experimental `S->C 0x32` set chunk visibility: two ints and a boolean.
- Experimental `S->C 0x33` chunk data: Beta block-region compressed packet.
- `0xFF` Disconnect: reason string, currently limited to 100 characters.

## Codec Registry

`PacketCodecRegistry::beta173_defaults()` currently maps:

- `0x02` to `HandshakePacketCodec`.
- `0x01` to `ServerboundLoginPacketCodec`.
- `0xFF` to `DisconnectPacketCodec`.

Unknown packet IDs return an empty lookup. The registry is currently for
serverbound dispatch only; direction-aware registries should be added when
clientbound packet codecs are implemented.

## Session Loop

The server listener accepts TCP connections and creates a `PlayerSession` for
each socket. The session tracks these states:

- `Handshaking`.
- `Login`.
- `Joined`.
- `Disconnected`.

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
7. Send `S->C 0x32` set chunk visibility.
8. Send experimental `S->C 0x33` chunk data packets.
9. Keep the connection open and read subsequent client packets.

With `--playable-flat-world`, the initial chunk radius defaults to `1`, sending
the spawn chunk and its eight neighbors. Use `--chunk-radius 0` to send only
chunk `(0,0)`.

The joined loop currently handles:

- `C->S 0x0A Player`.
- `C->S 0x0B PlayerPosition`.
- `C->S 0x0C PlayerLook`.
- `C->S 0x0D PlayerPositionLook`.
- `C->S 0x0E PlayerDigging`.
- `C->S 0x0F PlayerBlockPlacement`.
- `C->S 0x10 HeldItemChange`.
- `C->S 0x12 Animation`.
- `C->S 0x13 EntityAction`.
- `C->S 0xFF Disconnect`.

Movement packets update the server-side player position, look, on-ground flag,
and current chunk. Interaction packets are drained or applied conservatively:
animation is ignored, entity action tracks crouch where possible, held item
change stores slots `0..=8`, digging status `2` breaks reachable visible
non-air blocks, and placement targets the selected face with an MVP block
fallback. Unsupported post-join packet IDs are traced and receive a clear
disconnect instead of panicking.

Common post-login packet IDs currently recognized by name but not decoded:

- `0x00 KeepAlive`.
- `0x03 Chat`.

These are intentionally trace-first until field order and payload length are
documented from clean observations.

## Packet Trace Mode

Packet tracing is disabled by default. When enabled with `--trace-packets`,
Aurelia logs client-to-server packet metadata for packets it reads:

```text
[trace] C->S #1 id=0x02 name=Handshake payloadLength=10 payloadHex=00 04 00 41 00 6C 00 65 00 78
```

Known packet names are direction-aware. Current names include `Handshake`,
`Login` for `C->S 0x01`, `LoginResponse` for `S->C 0x01`, `Player`,
`PlayerPosition`, `PlayerLook`, `PlayerPositionLook`, `SpawnPosition`,
`SetChunkVisibility`, `ChunkData`, `BlockChange`, and `Disconnect`. Unknown
packet IDs are formatted as `Unknown`. Because Beta-era packets are not
length-prefixed here, unknown packet payload length is not inferred.

Experimental post-login tracing reads known C->S movement packets with fixed
payload sizes, plus documented fixed interaction packets:

- `0x0A Player`: 1 byte.
- `0x0B PlayerPosition`: 33 bytes.
- `0x0C PlayerLook`: 9 bytes.
- `0x0D PlayerPositionLook`: 41 bytes.
- `0x0E PlayerDigging`: 11 bytes.
- `0x10 HeldItemChange`: 2 bytes.
- `0x12 Animation`: 5 bytes.
- `0x13 EntityAction`: 5 bytes.

`0x0F PlayerBlockPlacement` is variable length because it carries legacy slot
data. In Beta 1.7.3 Aurelia decodes `x`, `y`, `z`, `direction`, then slot data
only. It must not read cursor bytes after the slot; those bytes belong to the
next packet in observed real-client streams.

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
