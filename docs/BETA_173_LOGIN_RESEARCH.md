# Beta 1.7.3 Login Research

This document tracks login packet questions that must be answered before Aurelia
implements a login codec or claims real Beta 1.7.3 client compatibility.

## Current Position

Aurelia expects the Beta 1.7.3 protocol version to be `14`. The observed
serverbound login trace confirms the client sent this value. The Rust protocol
crate exposes it as `BETA_173_PROTOCOL_VERSION`.

`ServerboundLoginPacket` is now the observed client-to-server login model.
`PacketCodecRegistry` metadata names `C->S 0x01` as `Login` separately from
`S->C 0x01` `LoginResponse`. Aurelia has an explicit `--experimental-join`
path that can send provisional server-to-client login response candidates,
spawn position,
player position/look, chunk visibility, and chunk data. The `beta173-observed`
mode is now the recommended experimental mode because a real client accepted it
far enough to send movement packets. These packets are still not verified as
supported compatibility.

## Known Flow

Implemented non-playable flow:

1. Client connects.
2. Client may send `0x02` handshake.
3. Aurelia decodes the handshake username.
4. Without `--experimental-join`, Aurelia sends an explicit disconnect after
   the initial handshake/login research path.
5. Aurelia closes the socket cleanly.

Experimental playable prototype flow:

1. Client connects.
2. Client sends handshake.
3. Server returns the trace-only handshake response string.
4. Client sends serverbound login.
5. Server decodes serverbound login.
6. Server sends a provisional clientbound login response.
7. Server sends spawn position.
8. Server sends player position/look.
9. Server sends chunk visibility and chunk data for the configured flat-world
   radius.
10. Server keeps the connection open, tracks player movement, and streams
    missing chunks as the player changes chunks.
11. Common post-login packet IDs with undocumented payloads are named in traces
    and disconnected cleanly instead of being guessed.

## Clean-Room Assumptions

- Protocol version `14` is the expected Beta 1.7.3 value.
- Packet ID `0x01` is the observed serverbound login packet ID.
- Packet ID `0x01` may also be used server-to-client for login response, but its
  field layout is unverified. Aurelia now has experimental encode candidates
  only.
- Server handshake response string `-` may be sufficient to prompt the client
  toward login, but this is only a trace-mode assumption until verified.
- Public protocol notes for nearby versions may be misleading, so packet fields
  must be verified specifically for Beta 1.7.3 before implementation.

## Direction-Aware Login Status

- `C->S 0x01 Login`: implemented from the captured `Luxorium` client trace.
- `S->C 0x01 Login Response`: provisionally encoded only behind
  `--experimental-join`; no layout is verified yet.

Do not assume the server-to-client login response uses the same fields or field
meanings as the client-to-server login packet. Direction-specific evidence is
required before promoting a provisional clientbound codec to supported behavior.

## Experimental Login Response Modes

These modes are intended for real-client testing, not compatibility claims.

Recommended mode:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --experimental-join --playable-flat-world --chunk-radius 1 --compat-debug --trace-packets --trace-packet-limit 512 --login-response-mode beta173-observed
```

This sends `S->C 0x01` as:

- `int entityId = 1`.
- `legacy string levelTypeOrUnused = ""`.
- `long mapSeed = 0`.
- `byte dimension = 0`.

Alternate mode:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 64 --trace-continue-after-handshake --experimental-join --login-response-mode mcdevs-legacy
```

This sends `S->C 0x01` as:

- `int entityId = 1`.
- `legacy string levelType = "default"`.
- `byte gameMode = 0`.
- `byte dimension = 0`.
- `byte difficulty = 1`.
- `byte unused = 0`.
- `byte maxPlayers = 8`.

After either login response, Aurelia sends:

- `S->C 0x06 SpawnPosition`: `(0, 65, 0)`.
- `S->C 0x0D PlayerPositionLook`: `(0.5, 66.0, 67.62, 0.5, 0.0, 0.0, false)`.
- `S->C 0x32 SetChunkVisibility` and `S->C 0x33 ChunkData` for the initial
  flat-world chunk area.

If the client fails, paste the full Aurelia trace output and describe the client
screen state, such as disconnect message, stuck at downloading terrain, or
void/world view.

Latest real-client evidence:

- `beta173-observed` reached C->S `0x0D` movement traffic.
- `mcdevs-legacy` reset/disconnected after its S->C `0x01` payload length `25`.
- A previous trace showed C->S `0x0D` as a 512-byte payload because Aurelia's
  post-login fallback swallowed multiple bytes. Current code now reads known
  movement packets by fixed payload size and updates server-side player state.

## Observed Traces

### Client Handshake, Username Luxorium

Trace:

```text
[trace] C->S #1 id=0x02 name=Handshake payloadLength=18 payloadHex=00 08 00 4C 00 75 00 78 00 6F 00 72 00 69 00 75 00 6D
```

Decoded:

- Packet ID: `0x02`.
- String length: `8`.
- Encoding: unsigned 16-bit big-endian character count followed by UTF-16BE
  character data.
- Username: `Luxorium`.

This confirms the current `LegacyStringIO` format for the observed handshake
packet. It does not yet verify login packet fields.

### Serverbound Login, Username Luxorium

Trace:

```text
[trace] C->S #2 id=0x01 name=Login payloadLength=31 payloadHex=00 00 00 0E 00 08 00 4C 00 75 00 78 00 6F 00 72 00 69 00 75 00 6D 00 00 00 00 00 00 00 00 00
```

Decoded:

- Packet ID: `0x01`.
- `int protocolVersion`: `14`.
- `legacy string username`: `Luxorium`.
- `long unusedOrSeed`: `0`.
- `byte dimension`: `0`.

Observed payload breakdown:

- `00 00 00 0E`: int `14`.
- `00 08`: string length `8`.
- `00 4C 00 75 00 78 00 6F 00 72 00 69 00 75 00 6D`: UTF-16BE `Luxorium`.
- `00 00 00 00 00 00 00 00`: long `0`.
- `00`: byte `0`.

This totals 31 bytes and is covered by golden codec tests.

## Unresolved Questions

- Exact clientbound login response field order for Beta 1.7.3.
- Whether the clientbound login response also uses packet ID `0x01`.
- Meaning of the observed serverbound `unusedOrSeed` long field.
- Exact production server response after `0x02` handshake before login.
- Disconnect behavior for protocol-version mismatch.
- Whether `0x0D` server-to-client position/look field order matches Beta 1.7.3.
- Whether the client requires chunks before accepting the position packet.

## Verification Plan

- Capture clientbound login response bytes from a clean local Beta 1.7.3
  client/server session.
- Record byte streams and packet boundaries without copying source code.
- Convert observations into tests before implementing a codec.
- Keep reference workspaces and generated jars out of this repository.

## Capturing The Server-To-Client Login Response

Use a clean Beta 1.7.3 client and a clean Beta 1.7.3 server outside this
repository. Do not commit jars, assets, generated sources, or decompiled source.

Suggested capture options:

- Wireshark on loopback with a display filter such as `tcp.port == 25565`.
- `tcpdump` on loopback, for example:

  ```bash
  sudo tcpdump -i lo -s 0 -X tcp port 25565
  ```

Capture target:

1. Start the clean Beta 1.7.3 server on `127.0.0.1:25565` if possible.
2. Connect once with a clean Beta 1.7.3 client using username `Luxorium` or note
   the username used.
3. Record packet boundaries, direction, packet IDs, payload lengths, and payload
   hex.
4. Paste the result back in this format:

   ```text
   [trace] S->C #? id=0x01 name=LoginResponse payloadLength=? payloadHex=...
   ```

If the capture tool does not expose packet boundaries directly, paste the raw
TCP hex with direction and timing notes so Aurelia can derive a golden test
carefully.

## How To Capture Clean Packet Traces

Aurelia has a developer-only packet tracing mode. It is disabled by default.

Run Aurelia locally with tracing enabled:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets
```

Then connect with a clean Beta 1.7.3 client pointed at `127.0.0.1:25565`.
Without continuation or experimental join mode, Aurelia sends a disconnect after
the initial research path, so the trace captures only the early handshake/login
bytes.

Example trace line:

```text
[trace] C->S #1 id=0x02 name=Handshake payloadLength=10 payloadHex=00 04 00 41 00 6C 00 65 00 78
```

Copy trace output into this research document or the next Codex prompt. Do not
paste decompiled source. If future traces reveal passwords, session tokens, or
other secrets, redact those values before sharing.

Optional trace limit:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 4
```

The default trace limit is `4`. A larger limit is most useful with trace
continuation or experimental join.

To continue after the handshake and attempt to capture the next client packet:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake
```

If needed, choose the trace-only server handshake response string:

```bash
cargo run -p aurelia-server -- --host 127.0.0.1 --port 25565 --trace-packets --trace-packet-limit 8 --trace-continue-after-handshake --trace-handshake-response -
```

Connect once with a clean Beta 1.7.3 client and copy the trace output into this
document or the next Codex prompt. The continuation path decodes the observed
serverbound `0x01` login packet. Use `--experimental-join` and
`--playable-flat-world` to test the provisional join sequence and chunk
streaming path.
