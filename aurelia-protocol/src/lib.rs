use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};

pub const TARGET_VERSION: &str = "Beta 1.7.3";
pub const EXPECTED_PROTOCOL_VERSION: i32 = 14;

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    InvalidArgument(String),
    InvalidData(String),
    WrongPacketId { expected: u8, actual: u8 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidArgument(message) | Self::InvalidData(message) => f.write_str(message),
            Self::WrongPacketId { expected, actual } => {
                write!(
                    f,
                    "wrong packet id: expected {expected:#04x}, got {actual:#04x}"
                )
            }
        }
    }
}

impl Error for ProtocolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketDirection {
    ClientToServer,
    ServerToClient,
}

impl PacketDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClientToServer => "C->S",
            Self::ServerToClient => "S->C",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientboundLoginResponseMode {
    Beta173Observed,
    McdevsLegacy,
}

impl ClientboundLoginResponseMode {
    pub const fn cli_value(self) -> &'static str {
        match self {
            Self::Beta173Observed => "beta173-observed",
            Self::McdevsLegacy => "mcdevs-legacy",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "beta173-observed" => Ok(Self::Beta173Observed),
            "mcdevs-legacy" => Ok(Self::McdevsLegacy),
            _ => Err(ProtocolError::InvalidArgument(format!(
                "unknown login response mode: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketFrame {
    packet_id: u8,
    payload: Vec<u8>,
}

impl PacketFrame {
    pub fn new(packet_id: u8, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            packet_id,
            payload: payload.into(),
        }
    }

    pub const fn packet_id(&self) -> u8 {
        self.packet_id
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}

pub struct LegacyPacketFrameCodec;

impl LegacyPacketFrameCodec {
    pub fn read(input: &mut impl Read, payload_length: usize) -> Result<PacketFrame> {
        let packet_id = read_u8(input)?;
        let mut payload = vec![0; payload_length];
        input.read_exact(&mut payload)?;
        Ok(PacketFrame::new(packet_id, payload))
    }

    pub fn write(frame: &PacketFrame, output: &mut impl Write) -> Result<()> {
        write_u8(output, frame.packet_id())?;
        output.write_all(frame.payload())?;
        Ok(())
    }
}

pub trait PacketCodec<P> {
    const PACKET_ID: u8;

    fn decode(input: &mut impl Read) -> Result<P>;
    fn encode(packet: &P, output: &mut impl Write) -> Result<()>;

    fn to_frame(packet: &P) -> Result<PacketFrame> {
        let mut payload = Vec::new();
        Self::encode(packet, &mut payload)?;
        Ok(PacketFrame::new(Self::PACKET_ID, payload))
    }

    fn from_frame(frame: PacketFrame) -> Result<P> {
        if frame.packet_id() != Self::PACKET_ID {
            return Err(ProtocolError::WrongPacketId {
                expected: Self::PACKET_ID,
                actual: frame.packet_id(),
            });
        }

        let mut cursor = io::Cursor::new(frame.into_payload());
        Self::decode(&mut cursor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketMetadata {
    pub direction: PacketDirection,
    pub packet_id: u8,
    pub name: &'static str,
}

impl PacketMetadata {
    pub const fn new(direction: PacketDirection, packet_id: u8, name: &'static str) -> Self {
        Self {
            direction,
            packet_id,
            name,
        }
    }
}

pub struct PacketCodecRegistry {
    packets: HashMap<(PacketDirection, u8), PacketMetadata>,
}

impl PacketCodecRegistry {
    pub fn beta173_defaults() -> Self {
        let mut registry = Self {
            packets: HashMap::new(),
        };
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            KeepAlivePacket::ID,
            "KeepAlive",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundLoginPacket::ID,
            "Login",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            HandshakePacket::ID,
            "Handshake",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ChatPacket::ID,
            "Chat",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            0x0A,
            "Player",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            0x0B,
            "PlayerPosition",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            0x0C,
            "PlayerLook",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            0x0D,
            "PlayerPositionLook",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundPlayerDiggingPacket::ID,
            "PlayerDigging",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundPlayerBlockPlacementPacket::ID,
            "PlayerBlockPlacement",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundHeldItemChangePacket::ID,
            "HeldItemChange",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundAnimationPacket::ID,
            "Animation",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundEntityActionPacket::ID,
            "EntityAction",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundCloseWindowPacket::ID,
            "CloseWindow",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundWindowClickPacket::ID,
            "WindowClick",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            ServerboundConfirmTransactionPacket::ID,
            "ConfirmTransaction",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ClientToServer,
            DisconnectPacket::ID,
            "Disconnect",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            KeepAlivePacket::ID,
            "KeepAlive",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundLoginResponsePacket::ID,
            "LoginResponse",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ChatPacket::ID,
            "Chat",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundBeta173TimeUpdatePacket::ID,
            "TimeUpdate",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundSpawnPositionPacket::ID,
            "SpawnPosition",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundPlayerPositionLookPacket::ID,
            "PlayerPositionLook",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundChunkVisibilityPacket::ID,
            "SetChunkVisibility",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundChunkDataPacket::ID,
            "ChunkData",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundBlockChangePacket::ID,
            "BlockChange",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundSetSlotPacket::ID,
            "SetSlot",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundSetWindowItemsPacket::ID,
            "SetWindowItems",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            ClientboundConfirmTransactionPacket::ID,
            "ConfirmTransaction",
        ));
        registry.register(PacketMetadata::new(
            PacketDirection::ServerToClient,
            DisconnectPacket::ID,
            "Disconnect",
        ));
        registry
    }

    pub fn register(&mut self, metadata: PacketMetadata) {
        self.packets
            .insert((metadata.direction, metadata.packet_id), metadata);
    }

    pub fn contains(&self, packet_id: u8) -> bool {
        self.contains_direction(PacketDirection::ClientToServer, packet_id)
    }

    pub fn contains_direction(&self, direction: PacketDirection, packet_id: u8) -> bool {
        self.packets.contains_key(&(direction, packet_id))
    }

    pub fn find(&self, packet_id: u8) -> Option<u8> {
        self.contains(packet_id).then_some(packet_id)
    }

    pub fn metadata(&self, direction: PacketDirection, packet_id: u8) -> Option<PacketMetadata> {
        self.packets.get(&(direction, packet_id)).copied()
    }

    pub fn packet_name(&self, direction: PacketDirection, packet_id: u8) -> Option<&'static str> {
        self.metadata(direction, packet_id)
            .map(|metadata| metadata.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepAlivePacket {
    pub keep_alive_id: i32,
}

impl KeepAlivePacket {
    pub const ID: u8 = 0x00;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            keep_alive_id: read_i32(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i32(output, self.keep_alive_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakePacket {
    pub username: String,
}

impl HandshakePacket {
    pub const ID: u8 = 0x02;
    pub const USERNAME_MAX_CHARS: usize = 16;

    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
        }
    }
}

pub struct HandshakePacketCodec;

impl PacketCodec<HandshakePacket> for HandshakePacketCodec {
    const PACKET_ID: u8 = HandshakePacket::ID;

    fn decode(input: &mut impl Read) -> Result<HandshakePacket> {
        Ok(HandshakePacket::new(read_legacy_string(
            input,
            HandshakePacket::USERNAME_MAX_CHARS,
        )?))
    }

    fn encode(packet: &HandshakePacket, output: &mut impl Write) -> Result<()> {
        write_legacy_string(
            output,
            &packet.username,
            HandshakePacket::USERNAME_MAX_CHARS,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectPacket {
    pub reason: String,
}

impl DisconnectPacket {
    pub const ID: u8 = 0xFF;
    pub const REASON_MAX_CHARS: usize = 100;

    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

pub struct DisconnectPacketCodec;

impl PacketCodec<DisconnectPacket> for DisconnectPacketCodec {
    const PACKET_ID: u8 = DisconnectPacket::ID;

    fn decode(input: &mut impl Read) -> Result<DisconnectPacket> {
        Ok(DisconnectPacket::new(read_legacy_string(
            input,
            DisconnectPacket::REASON_MAX_CHARS,
        )?))
    }

    fn encode(packet: &DisconnectPacket, output: &mut impl Write) -> Result<()> {
        write_legacy_string(output, &packet.reason, DisconnectPacket::REASON_MAX_CHARS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundLoginPacket {
    pub protocol_version: i32,
    pub username: String,
    pub unused_or_seed: i64,
    pub dimension: i8,
}

impl ServerboundLoginPacket {
    pub const ID: u8 = 0x01;
    pub const USERNAME_MAX_CHARS: usize = 16;
}

pub struct ServerboundLoginPacketCodec;

impl PacketCodec<ServerboundLoginPacket> for ServerboundLoginPacketCodec {
    const PACKET_ID: u8 = ServerboundLoginPacket::ID;

    fn decode(input: &mut impl Read) -> Result<ServerboundLoginPacket> {
        Ok(ServerboundLoginPacket {
            protocol_version: read_i32(input)?,
            username: read_legacy_string(input, ServerboundLoginPacket::USERNAME_MAX_CHARS)?,
            unused_or_seed: read_i64(input)?,
            dimension: read_i8(input)?,
        })
    }

    fn encode(packet: &ServerboundLoginPacket, output: &mut impl Write) -> Result<()> {
        write_i32(output, packet.protocol_version)?;
        write_legacy_string(
            output,
            &packet.username,
            ServerboundLoginPacket::USERNAME_MAX_CHARS,
        )?;
        write_i64(output, packet.unused_or_seed)?;
        write_i8(output, packet.dimension)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPacket {
    pub message: String,
}

impl ChatPacket {
    pub const ID: u8 = 0x03;
    pub const MESSAGE_MAX_CHARS: usize = 100;
    pub const MESSAGE_HARD_MAX_CHARS: usize = 512;

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: truncate_utf16_units(message.into(), Self::MESSAGE_MAX_CHARS),
        }
    }

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self::new(read_legacy_string_truncated(
            input,
            Self::MESSAGE_MAX_CHARS,
            Self::MESSAGE_HARD_MAX_CHARS,
        )?))
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_legacy_string(
            output,
            &truncate_utf16_units(self.message.clone(), Self::MESSAGE_MAX_CHARS),
            Self::MESSAGE_MAX_CHARS,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundLoginResponsePacket {
    pub entity_id: i32,
    pub level_type_or_unused: String,
    pub map_seed: i64,
    pub dimension: i8,
    pub game_mode: i8,
    pub difficulty: i8,
    pub unused: i8,
    pub max_players: i8,
}

impl ClientboundLoginResponsePacket {
    pub const ID: u8 = 0x01;

    pub fn beta173_observed_defaults() -> Self {
        Self {
            entity_id: 1,
            level_type_or_unused: String::new(),
            map_seed: 0,
            dimension: 0,
            game_mode: 0,
            difficulty: 1,
            unused: 0,
            max_players: 8,
        }
    }

    pub fn mcdevs_legacy_defaults() -> Self {
        Self {
            level_type_or_unused: "default".to_string(),
            ..Self::beta173_observed_defaults()
        }
    }
}

pub struct ClientboundLoginResponsePacketCodec {
    mode: ClientboundLoginResponseMode,
}

impl ClientboundLoginResponsePacketCodec {
    pub const LEVEL_TYPE_MAX_CHARS: usize = 32;

    pub const fn new(mode: ClientboundLoginResponseMode) -> Self {
        Self { mode }
    }

    pub const fn mode(&self) -> ClientboundLoginResponseMode {
        self.mode
    }

    pub fn encode(
        &self,
        packet: &ClientboundLoginResponsePacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_i32(output, packet.entity_id)?;
        write_legacy_string(
            output,
            &packet.level_type_or_unused,
            Self::LEVEL_TYPE_MAX_CHARS,
        )?;
        match self.mode {
            ClientboundLoginResponseMode::Beta173Observed => {
                write_i64(output, packet.map_seed)?;
                write_i8(output, packet.dimension)
            }
            ClientboundLoginResponseMode::McdevsLegacy => {
                write_i8(output, packet.game_mode)?;
                write_i8(output, packet.dimension)?;
                write_i8(output, packet.difficulty)?;
                write_i8(output, packet.unused)?;
                write_i8(output, packet.max_players)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundBeta173TimeUpdatePacket {
    pub time: i64,
}

impl ClientboundBeta173TimeUpdatePacket {
    pub const ID: u8 = 0x04;
}

pub struct ClientboundBeta173TimeUpdatePacketCodec;

impl ClientboundBeta173TimeUpdatePacketCodec {
    pub fn encode(
        packet: &ClientboundBeta173TimeUpdatePacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_i64(output, packet.time)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundModernTimeUpdatePacket {
    pub world_age: i64,
    pub time_of_day: i64,
}

impl ClientboundModernTimeUpdatePacket {
    pub const ID: u8 = 0x04;
}

pub struct ClientboundModernTimeUpdatePacketCodec;

impl ClientboundModernTimeUpdatePacketCodec {
    pub fn encode(
        packet: &ClientboundModernTimeUpdatePacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_i64(output, packet.world_age)?;
        write_i64(output, packet.time_of_day)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSpawnPositionPacket {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ClientboundSpawnPositionPacket {
    pub const ID: u8 = 0x06;

    pub const fn default_spawn() -> Self {
        Self { x: 0, y: 65, z: 0 }
    }
}

pub struct ClientboundSpawnPositionPacketCodec;

impl ClientboundSpawnPositionPacketCodec {
    pub fn encode(packet: &ClientboundSpawnPositionPacket, output: &mut impl Write) -> Result<()> {
        write_i32(output, packet.x)?;
        write_i32(output, packet.y)?;
        write_i32(output, packet.z)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerPositionLookPacket {
    pub x: f64,
    pub y: f64,
    pub stance: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl ClientboundPlayerPositionLookPacket {
    pub const ID: u8 = 0x0D;

    pub const fn default_spawn() -> Self {
        Self {
            x: 0.5,
            y: 66.0,
            stance: 67.62,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        }
    }
}

pub struct ClientboundPlayerPositionLookPacketCodec;

impl ClientboundPlayerPositionLookPacketCodec {
    pub fn encode(
        packet: &ClientboundPlayerPositionLookPacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_f64(output, packet.x)?;
        write_f64(output, packet.y)?;
        write_f64(output, packet.stance)?;
        write_f64(output, packet.z)?;
        write_f32(output, packet.yaw)?;
        write_f32(output, packet.pitch)?;
        write_bool(output, packet.on_ground)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundChunkVisibilityPacket {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub load: bool,
}

impl ClientboundChunkVisibilityPacket {
    pub const ID: u8 = 0x32;

    pub const fn load(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            chunk_z,
            load: true,
        }
    }

    pub const fn unload(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            chunk_z,
            load: false,
        }
    }
}

pub struct ClientboundChunkVisibilityPacketCodec;

impl ClientboundChunkVisibilityPacketCodec {
    pub fn encode(
        packet: &ClientboundChunkVisibilityPacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_i32(output, packet.chunk_x)?;
        write_i32(output, packet.chunk_z)?;
        write_bool(output, packet.load)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundChunkDataPacket {
    pub x: i32,
    pub y: i16,
    pub z: i32,
    pub width_minus_one: u8,
    pub height_minus_one: u8,
    pub length_minus_one: u8,
    pub compressed_data: Vec<u8>,
}

impl ClientboundChunkDataPacket {
    pub const ID: u8 = 0x33;
}

pub struct ClientboundChunkDataPacketCodec;

impl ClientboundChunkDataPacketCodec {
    pub fn encode(packet: &ClientboundChunkDataPacket, output: &mut impl Write) -> Result<()> {
        write_i32(output, packet.x)?;
        write_i16(output, packet.y)?;
        write_i32(output, packet.z)?;
        write_u8(output, packet.width_minus_one)?;
        write_u8(output, packet.height_minus_one)?;
        write_u8(output, packet.length_minus_one)?;
        write_i32(output, packet.compressed_data.len() as i32)?;
        output.write_all(&packet.compressed_data)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundBlockChangePacket {
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub block_type: i16,
    pub metadata: i32,
}

impl ClientboundBlockChangePacket {
    pub const ID: u8 = 0x35;
}

pub struct ClientboundBlockChangePacketCodec;

impl ClientboundBlockChangePacketCodec {
    pub fn encode(packet: &ClientboundBlockChangePacket, output: &mut impl Write) -> Result<()> {
        write_i32(output, packet.x)?;
        write_u8(output, packet.y)?;
        write_i32(output, packet.z)?;
        write_i16(output, packet.block_type)?;
        write_i32(output, packet.metadata)
    }
}

pub mod experimental_flat_chunk_data {
    use super::ClientboundChunkDataPacket;

    pub const WIDTH: usize = 16;
    pub const HEIGHT: usize = 128;
    pub const LENGTH: usize = 16;
    pub const BLOCK_BYTES: usize = WIDTH * HEIGHT * LENGTH;
    pub const NIBBLE_ARRAY_BYTES: usize = BLOCK_BYTES / 2;
    pub const UNCOMPRESSED_FULL_CHUNK_BYTES: usize = BLOCK_BYTES + (NIBBLE_ARRAY_BYTES * 3);

    const STONE_BLOCK_ID: u8 = 1;
    const DIRT_BLOCK_ID: u8 = 3;
    const GRASS_BLOCK_ID: u8 = 2;

    pub fn chunk_at(chunk_x: i32, chunk_z: i32) -> ClientboundChunkDataPacket {
        ClientboundChunkDataPacket {
            x: chunk_x * WIDTH as i32,
            y: 0,
            z: chunk_z * LENGTH as i32,
            width_minus_one: (WIDTH - 1) as u8,
            height_minus_one: (HEIGHT - 1) as u8,
            length_minus_one: (LENGTH - 1) as u8,
            compressed_data: zlib_store(&uncompressed_full_chunk()),
        }
    }

    pub fn chunk_from_block_arrays(
        chunk_x: i32,
        chunk_z: i32,
        block_ids: &[u8],
        metadata: &[u8],
    ) -> ClientboundChunkDataPacket {
        assert_eq!(BLOCK_BYTES, block_ids.len());
        assert_eq!(BLOCK_BYTES, metadata.len());

        let mut bytes = Vec::with_capacity(UNCOMPRESSED_FULL_CHUNK_BYTES);
        bytes.extend_from_slice(block_ids);
        bytes.extend_from_slice(&pack_nibbles(metadata));
        bytes.extend(std::iter::repeat(0).take(NIBBLE_ARRAY_BYTES));
        bytes.extend(std::iter::repeat(0xFF).take(NIBBLE_ARRAY_BYTES));

        ClientboundChunkDataPacket {
            x: chunk_x * WIDTH as i32,
            y: 0,
            z: chunk_z * LENGTH as i32,
            width_minus_one: (WIDTH - 1) as u8,
            height_minus_one: (HEIGHT - 1) as u8,
            length_minus_one: (LENGTH - 1) as u8,
            compressed_data: zlib_store(&bytes),
        }
    }

    pub fn uncompressed_full_chunk() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(UNCOMPRESSED_FULL_CHUNK_BYTES);
        let mut block_ids = vec![0; BLOCK_BYTES];
        for x in 0..WIDTH {
            for z in 0..LENGTH {
                for y in 0..HEIGHT {
                    block_ids[index(x, y, z)] = block_id_for_y(y);
                }
            }
        }

        bytes.extend_from_slice(&block_ids);
        bytes.extend(std::iter::repeat(0).take(NIBBLE_ARRAY_BYTES));
        bytes.extend(std::iter::repeat(0).take(NIBBLE_ARRAY_BYTES));
        bytes.extend(std::iter::repeat(0xFF).take(NIBBLE_ARRAY_BYTES));
        bytes
    }

    pub fn block_index(x: usize, y: usize, z: usize) -> usize {
        assert!(
            x < WIDTH && y < HEIGHT && z < LENGTH,
            "coordinates are outside a full Beta chunk"
        );
        index(x, y, z)
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        y + (z * HEIGHT) + (x * HEIGHT * LENGTH)
    }

    fn block_id_for_y(y: usize) -> u8 {
        if y == 63 {
            GRASS_BLOCK_ID
        } else if (59..=62).contains(&y) {
            DIRT_BLOCK_ID
        } else if y < 59 {
            STONE_BLOCK_ID
        } else {
            0
        }
    }

    fn pack_nibbles(values: &[u8]) -> Vec<u8> {
        let mut packed = Vec::with_capacity(values.len() / 2);
        for pair in values.chunks(2) {
            let low = pair[0] & 0x0F;
            let high = pair.get(1).copied().unwrap_or(0) & 0x0F;
            packed.push(low | (high << 4));
        }
        packed
    }

    fn zlib_store(input: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity(input.len() + (input.len() / 65_535) * 5 + 6);
        output.extend_from_slice(&[0x78, 0x01]);

        let mut remaining = input;
        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(65_535);
            let is_final = chunk_len == remaining.len();
            output.push(if is_final { 0x01 } else { 0x00 });
            let len = chunk_len as u16;
            output.extend_from_slice(&len.to_le_bytes());
            output.extend_from_slice(&(!len).to_le_bytes());
            output.extend_from_slice(&remaining[..chunk_len]);
            remaining = &remaining[chunk_len..];
        }

        let checksum = adler32(input);
        output.extend_from_slice(&checksum.to_be_bytes());
        output
    }

    fn adler32(input: &[u8]) -> u32 {
        const MOD_ADLER: u32 = 65_521;
        let mut a = 1u32;
        let mut b = 0u32;
        for byte in input {
            a = (a + u32::from(*byte)) % MOD_ADLER;
            b = (b + a) % MOD_ADLER;
        }
        (b << 16) | a
    }
}

pub fn movement_payload_length(packet_id: u8) -> Option<usize> {
    match packet_id {
        0x0A => Some(1),
        0x0B => Some(33),
        0x0C => Some(9),
        0x0D => Some(41),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerboundPacketKind {
    KeepAlive,
    Login,
    Handshake,
    Chat,
    Player,
    PlayerPosition,
    PlayerLook,
    PlayerPositionLook,
    PlayerDigging,
    PlayerBlockPlacement,
    HeldItemChange,
    Animation,
    EntityAction,
    CloseWindow,
    WindowClick,
    ConfirmTransaction,
    Disconnect,
    Unknown(u8),
}

impl ServerboundPacketKind {
    pub const fn from_id(packet_id: u8) -> Self {
        match packet_id {
            0x00 => Self::KeepAlive,
            0x01 => Self::Login,
            0x02 => Self::Handshake,
            0x03 => Self::Chat,
            0x0A => Self::Player,
            0x0B => Self::PlayerPosition,
            0x0C => Self::PlayerLook,
            0x0D => Self::PlayerPositionLook,
            0x0E => Self::PlayerDigging,
            0x0F => Self::PlayerBlockPlacement,
            0x10 => Self::HeldItemChange,
            0x12 => Self::Animation,
            0x13 => Self::EntityAction,
            0x65 => Self::CloseWindow,
            0x66 => Self::WindowClick,
            0x6A => Self::ConfirmTransaction,
            0xFF => Self::Disconnect,
            _ => Self::Unknown(packet_id),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::KeepAlive => "KeepAlive",
            Self::Login => "Login",
            Self::Handshake => "Handshake",
            Self::Chat => "Chat",
            Self::Player => "Player",
            Self::PlayerPosition => "PlayerPosition",
            Self::PlayerLook => "PlayerLook",
            Self::PlayerPositionLook => "PlayerPositionLook",
            Self::PlayerDigging => "PlayerDigging",
            Self::PlayerBlockPlacement => "PlayerBlockPlacement",
            Self::HeldItemChange => "HeldItemChange",
            Self::Animation => "Animation",
            Self::EntityAction => "EntityAction",
            Self::CloseWindow => "CloseWindow",
            Self::WindowClick => "WindowClick",
            Self::ConfirmTransaction => "ConfirmTransaction",
            Self::Disconnect => "Disconnect",
            Self::Unknown(_) => "Unknown",
        }
    }

    pub const fn fixed_payload_length(self) -> Option<usize> {
        match self {
            Self::KeepAlive => Some(4),
            Self::Player => Some(1),
            Self::PlayerPosition => Some(33),
            Self::PlayerLook => Some(9),
            Self::PlayerPositionLook => Some(41),
            Self::PlayerDigging => Some(11),
            Self::HeldItemChange => Some(2),
            Self::Animation => Some(5),
            Self::EntityAction => Some(5),
            Self::CloseWindow => Some(1),
            Self::ConfirmTransaction => Some(4),
            _ => None,
        }
    }

    pub const fn has_documented_layout(self) -> bool {
        matches!(
            self,
            Self::Login
                | Self::KeepAlive
                | Self::Handshake
                | Self::Chat
                | Self::Player
                | Self::PlayerPosition
                | Self::PlayerLook
                | Self::PlayerPositionLook
                | Self::PlayerDigging
                | Self::PlayerBlockPlacement
                | Self::HeldItemChange
                | Self::Animation
                | Self::EntityAction
                | Self::CloseWindow
                | Self::WindowClick
                | Self::ConfirmTransaction
                | Self::Disconnect
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerboundMovementPacket {
    Player {
        on_ground: bool,
    },
    PlayerPosition {
        x: f64,
        y: f64,
        stance: f64,
        z: f64,
        on_ground: bool,
    },
    PlayerLook {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerPositionLook {
        x: f64,
        y: f64,
        stance: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
}

impl ServerboundMovementPacket {
    pub fn decode(packet_id: u8, input: &mut impl Read) -> Result<Option<Self>> {
        let packet = match packet_id {
            0x0A => Self::Player {
                on_ground: read_bool(input)?,
            },
            0x0B => Self::PlayerPosition {
                x: read_f64(input)?,
                y: read_f64(input)?,
                stance: read_f64(input)?,
                z: read_f64(input)?,
                on_ground: read_bool(input)?,
            },
            0x0C => Self::PlayerLook {
                yaw: read_f32(input)?,
                pitch: read_f32(input)?,
                on_ground: read_bool(input)?,
            },
            0x0D => Self::PlayerPositionLook {
                x: read_f64(input)?,
                y: read_f64(input)?,
                stance: read_f64(input)?,
                z: read_f64(input)?,
                yaw: read_f32(input)?,
                pitch: read_f32(input)?,
                on_ground: read_bool(input)?,
            },
            _ => return Ok(None),
        };
        Ok(Some(packet))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundAnimationPacket {
    pub entity_id: i32,
    pub animation: i8,
}

impl ServerboundAnimationPacket {
    pub const ID: u8 = 0x12;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            entity_id: read_i32(input)?,
            animation: read_i8(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i32(output, self.entity_id)?;
        write_i8(output, self.animation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundEntityActionPacket {
    pub entity_id: i32,
    pub action_id: i8,
}

impl ServerboundEntityActionPacket {
    pub const ID: u8 = 0x13;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            entity_id: read_i32(input)?,
            action_id: read_i8(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i32(output, self.entity_id)?;
        write_i8(output, self.action_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundHeldItemChangePacket {
    pub selected_slot: i16,
}

impl ServerboundHeldItemChangePacket {
    pub const ID: u8 = 0x10;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            selected_slot: read_i16(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i16(output, self.selected_slot)
    }

    pub const fn hotbar_slot(self) -> Option<u8> {
        if self.selected_slot >= 0 && self.selected_slot <= 8 {
            Some(self.selected_slot as u8)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundPlayerDiggingPacket {
    pub status: i8,
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub face: i8,
}

impl ServerboundPlayerDiggingPacket {
    pub const ID: u8 = 0x0E;
    pub const START_DIGGING_STATUS: i8 = 0;
    pub const CANCEL_DIGGING_STATUS: i8 = 1;
    pub const FINISHED_DIGGING_STATUS: i8 = 2;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            status: read_i8(input)?,
            x: read_i32(input)?,
            y: read_u8(input)?,
            z: read_i32(input)?,
            face: read_i8(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i8(output, self.status)?;
        write_i32(output, self.x)?;
        write_u8(output, self.y)?;
        write_i32(output, self.z)?;
        write_i8(output, self.face)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySlotData {
    Empty,
    Present {
        item_id: i16,
        count: u8,
        damage: i16,
    },
}

impl LegacySlotData {
    pub fn decode(input: &mut impl Read) -> Result<Self> {
        let item_id = read_i16(input)?;
        if item_id == -1 {
            return Ok(Self::Empty);
        }
        Ok(Self::Present {
            item_id,
            count: read_u8(input)?,
            damage: read_i16(input)?,
        })
    }

    pub const fn item_id(self) -> Option<i16> {
        match self {
            Self::Empty => None,
            Self::Present { item_id, .. } => Some(item_id),
        }
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        match *self {
            Self::Empty => write_i16(output, -1),
            Self::Present {
                item_id,
                count,
                damage,
            } => {
                write_i16(output, item_id)?;
                write_u8(output, count)?;
                write_i16(output, damage)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundCloseWindowPacket {
    pub window_id: i8,
}

impl ServerboundCloseWindowPacket {
    pub const ID: u8 = 0x65;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            window_id: read_i8(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i8(output, self.window_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundWindowClickPacket {
    pub window_id: i8,
    pub slot: i16,
    pub mouse_button: i8,
    pub action_number: i16,
    pub shift: bool,
    pub clicked_item: LegacySlotData,
}

impl ServerboundWindowClickPacket {
    pub const ID: u8 = 0x66;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            window_id: read_i8(input)?,
            slot: read_i16(input)?,
            mouse_button: read_i8(input)?,
            action_number: read_i16(input)?,
            shift: read_bool(input)?,
            clicked_item: LegacySlotData::decode(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i8(output, self.window_id)?;
        write_i16(output, self.slot)?;
        write_i8(output, self.mouse_button)?;
        write_i16(output, self.action_number)?;
        write_bool(output, self.shift)?;
        self.clicked_item.encode(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundConfirmTransactionPacket {
    pub window_id: i8,
    pub action_number: i16,
    pub accepted: bool,
}

impl ServerboundConfirmTransactionPacket {
    pub const ID: u8 = 0x6A;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            window_id: read_i8(input)?,
            action_number: read_i16(input)?,
            accepted: read_bool(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i8(output, self.window_id)?;
        write_i16(output, self.action_number)?;
        write_bool(output, self.accepted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundSetSlotPacket {
    pub window_id: i8,
    pub slot: i16,
    pub slot_data: LegacySlotData,
}

impl ClientboundSetSlotPacket {
    pub const ID: u8 = 0x67;
}

pub struct ClientboundSetSlotPacketCodec;

impl ClientboundSetSlotPacketCodec {
    pub fn encode(packet: &ClientboundSetSlotPacket, output: &mut impl Write) -> Result<()> {
        write_i8(output, packet.window_id)?;
        write_i16(output, packet.slot)?;
        packet.slot_data.encode(output)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetWindowItemsPacket {
    pub window_id: i8,
    pub slots: Vec<LegacySlotData>,
}

impl ClientboundSetWindowItemsPacket {
    pub const ID: u8 = 0x68;
}

pub struct ClientboundSetWindowItemsPacketCodec;

impl ClientboundSetWindowItemsPacketCodec {
    pub fn encode(packet: &ClientboundSetWindowItemsPacket, output: &mut impl Write) -> Result<()> {
        write_i8(output, packet.window_id)?;
        write_i16(output, packet.slots.len() as i16)?;
        for slot in &packet.slots {
            slot.encode(output)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundConfirmTransactionPacket {
    pub window_id: i8,
    pub action_number: i16,
    pub accepted: bool,
}

impl ClientboundConfirmTransactionPacket {
    pub const ID: u8 = 0x6A;
}

pub struct ClientboundConfirmTransactionPacketCodec;

impl ClientboundConfirmTransactionPacketCodec {
    pub fn encode(
        packet: &ClientboundConfirmTransactionPacket,
        output: &mut impl Write,
    ) -> Result<()> {
        write_i8(output, packet.window_id)?;
        write_i16(output, packet.action_number)?;
        write_bool(output, packet.accepted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerboundPlayerBlockPlacementPacket {
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub direction: i8,
    pub held_item: LegacySlotData,
}

impl ServerboundPlayerBlockPlacementPacket {
    pub const ID: u8 = 0x0F;

    pub fn decode(input: &mut impl Read) -> Result<Self> {
        Ok(Self {
            x: read_i32(input)?,
            y: read_u8(input)?,
            z: read_i32(input)?,
            direction: read_i8(input)?,
            held_item: LegacySlotData::decode(input)?,
        })
    }

    pub fn encode(&self, output: &mut impl Write) -> Result<()> {
        write_i32(output, self.x)?;
        write_u8(output, self.y)?;
        write_i32(output, self.z)?;
        write_i8(output, self.direction)?;
        self.held_item.encode(output)
    }

    pub fn is_special_item_use(self) -> bool {
        self.x == -1 && self.y == u8::MAX && self.z == -1 && self.direction == -1
    }
}

pub fn read_legacy_string(input: &mut impl Read, max_chars: usize) -> Result<String> {
    validate_max_chars(max_chars)?;
    let length = read_u16(input)? as usize;
    if length > max_chars {
        return Err(ProtocolError::InvalidData(format!(
            "legacy string length {length} exceeds max {max_chars}"
        )));
    }

    let mut units = Vec::with_capacity(length);
    for _ in 0..length {
        units.push(read_u16(input)?);
    }
    String::from_utf16(&units).map_err(|error| ProtocolError::InvalidData(error.to_string()))
}

pub fn read_legacy_string_truncated(
    input: &mut impl Read,
    max_chars: usize,
    hard_max_chars: usize,
) -> Result<String> {
    validate_max_chars(max_chars)?;
    validate_max_chars(hard_max_chars)?;
    let length = read_u16(input)? as usize;
    let kept_len = length.min(max_chars);
    let mut kept = Vec::with_capacity(kept_len);
    for index in 0..length {
        let unit = read_u16(input)?;
        if index < kept_len {
            kept.push(unit);
        }
        if index >= hard_max_chars {
            continue;
        }
    }
    Ok(String::from_utf16_lossy(&kept))
}

pub fn write_legacy_string(output: &mut impl Write, value: &str, max_chars: usize) -> Result<()> {
    validate_max_chars(max_chars)?;
    let units: Vec<u16> = value.encode_utf16().collect();
    if units.len() > max_chars {
        return Err(ProtocolError::InvalidData(format!(
            "legacy string length {} exceeds max {max_chars}",
            units.len()
        )));
    }

    write_u16(output, units.len() as u16)?;
    for unit in units {
        write_u16(output, unit)?;
    }
    Ok(())
}

pub fn truncate_utf16_units(value: String, max_chars: usize) -> String {
    if value.encode_utf16().count() <= max_chars {
        return value;
    }

    String::from_utf16_lossy(&value.encode_utf16().take(max_chars).collect::<Vec<u16>>())
}

fn validate_max_chars(max_chars: usize) -> Result<()> {
    if max_chars > i16::MAX as usize {
        return Err(ProtocolError::InvalidArgument(format!(
            "maxChars must be between 0 and {}",
            i16::MAX
        )));
    }
    Ok(())
}

pub fn read_u8(input: &mut impl Read) -> Result<u8> {
    let mut bytes = [0; 1];
    input.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

pub fn read_i8(input: &mut impl Read) -> Result<i8> {
    Ok(read_u8(input)? as i8)
}

pub fn read_u16(input: &mut impl Read) -> Result<u16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(u16::from_be_bytes(bytes))
}

pub fn read_i16(input: &mut impl Read) -> Result<i16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(i16::from_be_bytes(bytes))
}

pub fn read_i32(input: &mut impl Read) -> Result<i32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(i32::from_be_bytes(bytes))
}

pub fn read_i64(input: &mut impl Read) -> Result<i64> {
    let mut bytes = [0; 8];
    input.read_exact(&mut bytes)?;
    Ok(i64::from_be_bytes(bytes))
}

pub fn read_f32(input: &mut impl Read) -> Result<f32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(f32::from_be_bytes(bytes))
}

pub fn read_f64(input: &mut impl Read) -> Result<f64> {
    let mut bytes = [0; 8];
    input.read_exact(&mut bytes)?;
    Ok(f64::from_be_bytes(bytes))
}

pub fn read_bool(input: &mut impl Read) -> Result<bool> {
    Ok(read_u8(input)? != 0)
}

pub fn write_u8(output: &mut impl Write, value: u8) -> Result<()> {
    output.write_all(&[value])?;
    Ok(())
}

pub fn write_i8(output: &mut impl Write, value: i8) -> Result<()> {
    write_u8(output, value as u8)
}

pub fn write_u16(output: &mut impl Write, value: u16) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_i16(output: &mut impl Write, value: i16) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_i32(output: &mut impl Write, value: i32) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_i64(output: &mut impl Write, value: i64) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_f32(output: &mut impl Write, value: f32) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_f64(output: &mut impl Write, value: f64) -> Result<()> {
    output.write_all(&value.to_be_bytes())?;
    Ok(())
}

pub fn write_bool(output: &mut impl Write, value: bool) -> Result<()> {
    write_u8(output, u8::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta173_protocol_version_is_documented_as_expected_value() {
        assert_eq!("Beta 1.7.3", TARGET_VERSION);
        assert_eq!(14, EXPECTED_PROTOCOL_VERSION);
    }

    #[test]
    fn directions_expose_trace_labels() {
        assert_eq!("C->S", PacketDirection::ClientToServer.label());
        assert_eq!("S->C", PacketDirection::ServerToClient.label());
    }

    #[test]
    fn legacy_string_round_trips_ascii_as_utf16_chars() {
        let mut bytes = Vec::new();

        write_legacy_string(&mut bytes, "Alex", 16).unwrap();

        assert_eq!(
            vec![0x00, 0x04, 0x00, 0x41, 0x00, 0x6C, 0x00, 0x65, 0x00, 0x78],
            bytes
        );
        assert_eq!(
            "Alex",
            read_legacy_string(&mut bytes.as_slice(), 16).unwrap()
        );
    }

    #[test]
    fn legacy_string_rejects_length_beyond_limit_before_reading_payload() {
        let error = read_legacy_string(&mut [0x00, 0x11].as_slice(), 16).unwrap_err();
        assert!(matches!(error, ProtocolError::InvalidData(_)));
    }

    #[test]
    fn legacy_string_rejects_truncated_payload() {
        let error = read_legacy_string(&mut [0x00, 0x02, 0x00, 0x41].as_slice(), 16).unwrap_err();
        assert!(matches!(error, ProtocolError::Io(_)));
    }

    #[test]
    fn frame_writes_packet_id_before_payload() {
        let frame = PacketFrame::new(0x02, [0x00, 0x05]);
        let mut bytes = Vec::new();

        LegacyPacketFrameCodec::write(&frame, &mut bytes).unwrap();

        assert_eq!(vec![0x02, 0x00, 0x05], bytes);
    }

    #[test]
    fn frame_reads_packet_id_and_caller_sized_payload() {
        let mut bytes = [0xFF, 0x01, 0x02, 0x03].as_slice();

        let frame = LegacyPacketFrameCodec::read(&mut bytes, 3).unwrap();

        assert_eq!(0xFF, frame.packet_id());
        assert_eq!(&[0x01, 0x02, 0x03], frame.payload());
    }

    #[test]
    fn handshake_round_trips_payload_and_frame() {
        let packet = HandshakePacket::new("Alex");
        let frame = HandshakePacketCodec::to_frame(&packet).unwrap();
        let decoded = HandshakePacketCodec::from_frame(frame.clone()).unwrap();

        assert_eq!(HandshakePacket::ID, frame.packet_id());
        assert_eq!(packet, decoded);
    }

    #[test]
    fn handshake_encodes_expected_clean_room_payload_bytes() {
        let mut bytes = Vec::new();

        HandshakePacketCodec::encode(&HandshakePacket::new("Alex"), &mut bytes).unwrap();

        assert_eq!(
            vec![0x00, 0x04, 0x00, 0x41, 0x00, 0x6C, 0x00, 0x65, 0x00, 0x78],
            bytes
        );
    }

    #[test]
    fn disconnect_round_trips_payload_and_frame() {
        let packet = DisconnectPacket::new("Bye");
        let frame = DisconnectPacketCodec::to_frame(&packet).unwrap();
        let decoded = DisconnectPacketCodec::from_frame(frame.clone()).unwrap();

        assert_eq!(DisconnectPacket::ID, frame.packet_id());
        assert_eq!(packet, decoded);
    }

    #[test]
    fn serverbound_login_encodes_observed_luxorium_payload() {
        let packet = ServerboundLoginPacket {
            protocol_version: EXPECTED_PROTOCOL_VERSION,
            username: "Luxorium".to_string(),
            unused_or_seed: 0,
            dimension: 0,
        };
        let mut bytes = Vec::new();

        ServerboundLoginPacketCodec::encode(&packet, &mut bytes).unwrap();

        assert_eq!(luxorium_login_payload(), bytes);
        assert_eq!(
            packet,
            ServerboundLoginPacketCodec::decode(&mut bytes.as_slice()).unwrap()
        );
    }

    #[test]
    fn registry_contains_default_codecs() {
        let registry = PacketCodecRegistry::beta173_defaults();

        assert!(registry.contains(HandshakePacket::ID));
        assert!(registry.contains(ServerboundLoginPacket::ID));
        assert!(registry.contains(DisconnectPacket::ID));
        assert_eq!(None, registry.find(0x7E));
    }

    #[test]
    fn registry_names_login_by_direction() {
        let registry = PacketCodecRegistry::beta173_defaults();

        assert_eq!(
            Some("Login"),
            registry.packet_name(PacketDirection::ClientToServer, 0x01)
        );
        assert_eq!(
            Some("LoginResponse"),
            registry.packet_name(PacketDirection::ServerToClient, 0x01)
        );
        assert_eq!(
            Some(PacketMetadata::new(
                PacketDirection::ClientToServer,
                0x01,
                "Login"
            )),
            registry.metadata(PacketDirection::ClientToServer, 0x01)
        );
        assert_eq!(
            Some(PacketMetadata::new(
                PacketDirection::ServerToClient,
                0x01,
                "LoginResponse"
            )),
            registry.metadata(PacketDirection::ServerToClient, 0x01)
        );
    }

    #[test]
    fn registry_keeps_unknown_packets_unknown_by_direction() {
        let registry = PacketCodecRegistry::beta173_defaults();

        assert_eq!(
            None,
            registry.packet_name(PacketDirection::ClientToServer, 0x7E)
        );
        assert_eq!(
            None,
            registry.packet_name(PacketDirection::ServerToClient, 0x7E)
        );
    }

    #[test]
    fn reports_known_movement_payload_lengths() {
        assert_eq!(Some(1), movement_payload_length(0x0A));
        assert_eq!(Some(33), movement_payload_length(0x0B));
        assert_eq!(Some(9), movement_payload_length(0x0C));
        assert_eq!(Some(41), movement_payload_length(0x0D));
        assert_eq!(None, movement_payload_length(0x7E));
    }

    #[test]
    fn classifies_serverbound_packet_ids() {
        assert_eq!(
            ServerboundPacketKind::KeepAlive,
            ServerboundPacketKind::from_id(0x00)
        );
        assert_eq!(
            ServerboundPacketKind::Chat,
            ServerboundPacketKind::from_id(0x03)
        );
        assert_eq!(
            ServerboundPacketKind::PlayerDigging,
            ServerboundPacketKind::from_id(0x0E)
        );
        assert_eq!(
            ServerboundPacketKind::PlayerBlockPlacement,
            ServerboundPacketKind::from_id(0x0F)
        );
        assert_eq!(
            ServerboundPacketKind::HeldItemChange,
            ServerboundPacketKind::from_id(0x10)
        );
        assert_eq!(
            ServerboundPacketKind::Animation,
            ServerboundPacketKind::from_id(0x12)
        );
        assert_eq!(
            ServerboundPacketKind::EntityAction,
            ServerboundPacketKind::from_id(0x13)
        );
        assert_eq!(
            ServerboundPacketKind::CloseWindow,
            ServerboundPacketKind::from_id(0x65)
        );
        assert_eq!(
            ServerboundPacketKind::WindowClick,
            ServerboundPacketKind::from_id(0x66)
        );
        assert_eq!(
            ServerboundPacketKind::ConfirmTransaction,
            ServerboundPacketKind::from_id(0x6A)
        );
        assert_eq!(
            ServerboundPacketKind::Unknown(0x7E),
            ServerboundPacketKind::from_id(0x7E)
        );
        assert_eq!(
            Some(41),
            ServerboundPacketKind::PlayerPositionLook.fixed_payload_length()
        );
        assert_eq!(
            Some(4),
            ServerboundPacketKind::KeepAlive.fixed_payload_length()
        );
        assert_eq!(
            Some(11),
            ServerboundPacketKind::PlayerDigging.fixed_payload_length()
        );
        assert_eq!(
            Some(2),
            ServerboundPacketKind::HeldItemChange.fixed_payload_length()
        );
        assert_eq!(
            Some(5),
            ServerboundPacketKind::Animation.fixed_payload_length()
        );
        assert_eq!(
            Some(5),
            ServerboundPacketKind::EntityAction.fixed_payload_length()
        );
        assert_eq!(
            Some(1),
            ServerboundPacketKind::CloseWindow.fixed_payload_length()
        );
        assert_eq!(
            None,
            ServerboundPacketKind::WindowClick.fixed_payload_length()
        );
        assert_eq!(
            Some(4),
            ServerboundPacketKind::ConfirmTransaction.fixed_payload_length()
        );
        assert!(ServerboundPacketKind::PlayerDigging.has_documented_layout());
        assert!(ServerboundPacketKind::PlayerBlockPlacement.has_documented_layout());
        assert!(ServerboundPacketKind::WindowClick.has_documented_layout());
        assert!(ServerboundPacketKind::Chat.has_documented_layout());
    }

    #[test]
    fn decodes_serverbound_movement_packets() {
        let mut position_look = Vec::new();
        write_f64(&mut position_look, 0.5).unwrap();
        write_f64(&mut position_look, 66.0).unwrap();
        write_f64(&mut position_look, 67.62).unwrap();
        write_f64(&mut position_look, -1.5).unwrap();
        write_f32(&mut position_look, 90.0).unwrap();
        write_f32(&mut position_look, 12.5).unwrap();
        write_bool(&mut position_look, true).unwrap();

        assert_eq!(
            Some(ServerboundMovementPacket::PlayerPositionLook {
                x: 0.5,
                y: 66.0,
                stance: 67.62,
                z: -1.5,
                yaw: 90.0,
                pitch: 12.5,
                on_ground: true,
            }),
            ServerboundMovementPacket::decode(0x0D, &mut position_look.as_slice()).unwrap()
        );

        assert_eq!(
            Some(ServerboundMovementPacket::Player { on_ground: false }),
            ServerboundMovementPacket::decode(0x0A, &mut [0].as_slice()).unwrap()
        );
        assert_eq!(
            None,
            ServerboundMovementPacket::decode(0x7E, &mut [].as_slice()).unwrap()
        );
    }

    #[test]
    fn decodes_serverbound_interaction_packets() {
        let keepalive = KeepAlivePacket::decode(&mut [0x00, 0x00, 0x00, 0x2A].as_slice()).unwrap();
        assert_eq!(KeepAlivePacket { keep_alive_id: 42 }, keepalive);
        let mut keepalive_bytes = Vec::new();
        keepalive.encode(&mut keepalive_bytes).unwrap();
        assert_eq!(vec![0x00, 0x00, 0x00, 0x2A], keepalive_bytes);

        let animation =
            ServerboundAnimationPacket::decode(&mut [0x00, 0x00, 0x00, 0x2A, 0x01].as_slice())
                .unwrap();
        assert_eq!(
            ServerboundAnimationPacket {
                entity_id: 42,
                animation: 1
            },
            animation
        );

        let action =
            ServerboundEntityActionPacket::decode(&mut [0x00, 0x00, 0x00, 0x2A, 0x01].as_slice())
                .unwrap();
        assert_eq!(
            ServerboundEntityActionPacket {
                entity_id: 42,
                action_id: 1
            },
            action
        );

        let held = ServerboundHeldItemChangePacket::decode(&mut [0x00, 0x04].as_slice()).unwrap();
        assert_eq!(Some(4), held.hotbar_slot());
        let invalid =
            ServerboundHeldItemChangePacket::decode(&mut [0x00, 0x09].as_slice()).unwrap();
        assert_eq!(None, invalid.hotbar_slot());

        let digging = ServerboundPlayerDiggingPacket::decode(
            &mut [0x02, 0, 0, 0, 1, 63, 0xFF, 0xFF, 0xFF, 0xFE, 0x01].as_slice(),
        )
        .unwrap();
        assert_eq!(
            ServerboundPlayerDiggingPacket {
                status: 2,
                x: 1,
                y: 63,
                z: -2,
                face: 1,
            },
            digging
        );
    }

    #[test]
    fn decodes_legacy_slot_data_and_block_placement() {
        let mut non_empty_payload =
            std::io::Cursor::new([0, 0, 0, 1, 64, 0xFF, 0xFF, 0xFF, 0xFE, 1, 0, 3, 1, 0, 0]);
        let packet = ServerboundPlayerBlockPlacementPacket::decode(&mut non_empty_payload).unwrap();

        assert_eq!(1, packet.x);
        assert_eq!(64, packet.y);
        assert_eq!(-2, packet.z);
        assert_eq!(1, packet.direction);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 1,
                damage: 0
            },
            packet.held_item
        );
        assert_eq!(Some(3), packet.held_item.item_id());
        assert_eq!(15, non_empty_payload.position());

        let mut empty_payload = std::io::Cursor::new([
            0xFF, 0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0D,
        ]);
        let empty = ServerboundPlayerBlockPlacementPacket::decode(&mut empty_payload).unwrap();
        assert!(empty.is_special_item_use());
        assert_eq!(LegacySlotData::Empty, empty.held_item);
        assert_eq!(12, empty_payload.position());
        assert_eq!(0x0D, read_u8(&mut empty_payload).unwrap());
    }

    #[test]
    fn decodes_window_packets_and_consumes_clicked_item_slots_exactly() {
        let mut close = std::io::Cursor::new([0x01, 0x0A]);
        assert_eq!(
            ServerboundCloseWindowPacket { window_id: 1 },
            ServerboundCloseWindowPacket::decode(&mut close).unwrap()
        );
        assert_eq!(1, close.position());
        assert_eq!(0x0A, read_u8(&mut close).unwrap());

        let mut empty_click =
            std::io::Cursor::new([0x00, 0x00, 0x05, 0x00, 0x00, 0x07, 0x00, 0xFF, 0xFF, 0x0A]);
        assert_eq!(
            ServerboundWindowClickPacket {
                window_id: 0,
                slot: 5,
                mouse_button: 0,
                action_number: 7,
                shift: false,
                clicked_item: LegacySlotData::Empty,
            },
            ServerboundWindowClickPacket::decode(&mut empty_click).unwrap()
        );
        assert_eq!(9, empty_click.position());
        assert_eq!(
            Some(ServerboundMovementPacket::Player { on_ground: true }),
            ServerboundMovementPacket::decode(
                read_u8(&mut empty_click).unwrap(),
                &mut [1].as_slice()
            )
            .unwrap()
        );

        let mut non_empty_click = std::io::Cursor::new([
            0x00, 0x00, 0x2A, 0x01, 0x00, 0x08, 0x01, 0x00, 0x03, 0x40, 0x00, 0x05, 0x0A,
        ]);
        assert_eq!(
            ServerboundWindowClickPacket {
                window_id: 0,
                slot: 42,
                mouse_button: 1,
                action_number: 8,
                shift: true,
                clicked_item: LegacySlotData::Present {
                    item_id: 3,
                    count: 64,
                    damage: 5,
                },
            },
            ServerboundWindowClickPacket::decode(&mut non_empty_click).unwrap()
        );
        assert_eq!(12, non_empty_click.position());
        assert_eq!(0x0A, read_u8(&mut non_empty_click).unwrap());

        let confirm =
            ServerboundConfirmTransactionPacket::decode(&mut [0x01, 0x00, 0x08, 0x01].as_slice())
                .unwrap();
        assert_eq!(
            ServerboundConfirmTransactionPacket {
                window_id: 1,
                action_number: 8,
                accepted: true,
            },
            confirm
        );
    }

    #[test]
    fn chat_decode_truncates_and_encode_is_beta_safe() {
        let mut chat = Vec::new();
        write_legacy_string(&mut chat, "hello", ChatPacket::MESSAGE_MAX_CHARS).unwrap();
        assert_eq!(
            "hello",
            ChatPacket::decode(&mut chat.as_slice()).unwrap().message
        );

        let long = "x".repeat(ChatPacket::MESSAGE_MAX_CHARS + 10);
        let packet = ChatPacket::new(long);
        assert_eq!(ChatPacket::MESSAGE_MAX_CHARS, packet.message.len());
        let mut encoded = Vec::new();
        packet.encode(&mut encoded).unwrap();
        assert_eq!(
            ChatPacket::MESSAGE_MAX_CHARS as u16,
            u16::from_be_bytes([encoded[0], encoded[1]])
        );
    }

    #[test]
    fn clientbound_survival_mvp_packets_encode_expected_payloads() {
        let mut time = Vec::new();
        ClientboundBeta173TimeUpdatePacketCodec::encode(
            &ClientboundBeta173TimeUpdatePacket { time: 20 },
            &mut time,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 0, 0, 0, 0, 0, 20], time);

        let mut modern_time = Vec::new();
        ClientboundModernTimeUpdatePacketCodec::encode(
            &ClientboundModernTimeUpdatePacket {
                world_age: 20,
                time_of_day: 30,
            },
            &mut modern_time,
        )
        .unwrap();
        assert_eq!(
            vec![0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 30],
            modern_time
        );

        let mut slot = Vec::new();
        ClientboundSetSlotPacketCodec::encode(
            &ClientboundSetSlotPacket {
                window_id: 0,
                slot: 36,
                slot_data: LegacySlotData::Present {
                    item_id: 3,
                    count: 64,
                    damage: 0,
                },
            },
            &mut slot,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 36, 0, 3, 64, 0, 0], slot);

        let mut items = Vec::new();
        ClientboundSetWindowItemsPacketCodec::encode(
            &ClientboundSetWindowItemsPacket {
                window_id: 0,
                slots: vec![
                    LegacySlotData::Empty,
                    LegacySlotData::Present {
                        item_id: 4,
                        count: 2,
                        damage: 0,
                    },
                ],
            },
            &mut items,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 2, 0xFF, 0xFF, 0, 4, 2, 0, 0], items);

        let mut confirm = Vec::new();
        ClientboundConfirmTransactionPacketCodec::encode(
            &ClientboundConfirmTransactionPacket {
                window_id: 0,
                action_number: 7,
                accepted: true,
            },
            &mut confirm,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 7, 1], confirm);
    }

    #[test]
    fn beta173_time_update_is_one_long_and_preserves_following_packet_boundary() {
        let mut time_payload = Vec::new();
        ClientboundBeta173TimeUpdatePacketCodec::encode(
            &ClientboundBeta173TimeUpdatePacket { time: 0x73 },
            &mut time_payload,
        )
        .unwrap();
        assert_eq!(8, time_payload.len());

        let mut items_payload = Vec::new();
        ClientboundSetWindowItemsPacketCodec::encode(
            &ClientboundSetWindowItemsPacket {
                window_id: 0,
                slots: vec![LegacySlotData::Empty],
            },
            &mut items_payload,
        )
        .unwrap();

        let mut stream = Vec::new();
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundBeta173TimeUpdatePacket::ID, time_payload),
            &mut stream,
        )
        .unwrap();
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundSetWindowItemsPacket::ID, items_payload),
            &mut stream,
        )
        .unwrap();

        assert_eq!(ClientboundBeta173TimeUpdatePacket::ID, stream[0]);
        assert_eq!(ClientboundSetWindowItemsPacket::ID, stream[9]);
        assert_ne!(0x73, stream[9]);
    }

    #[test]
    fn beta173_set_window_items_slot_lengths_are_exact() {
        let mut empty_slot = Vec::new();
        LegacySlotData::Empty.encode(&mut empty_slot).unwrap();
        assert_eq!(vec![0xFF, 0xFF], empty_slot);

        let mut present_slot = Vec::new();
        LegacySlotData::Present {
            item_id: 4,
            count: 2,
            damage: 3,
        }
        .encode(&mut present_slot)
        .unwrap();
        assert_eq!(vec![0, 4, 2, 0, 3], present_slot);

        let mut slots = vec![LegacySlotData::Empty; 45];
        slots[36] = LegacySlotData::Present {
            item_id: 3,
            count: 64,
            damage: 0,
        };
        slots[37] = LegacySlotData::Present {
            item_id: 4,
            count: 64,
            damage: 0,
        };
        slots[38] = LegacySlotData::Present {
            item_id: 5,
            count: 64,
            damage: 0,
        };
        slots[39] = LegacySlotData::Present {
            item_id: 50,
            count: 64,
            damage: 0,
        };
        slots[40] = LegacySlotData::Present {
            item_id: 270,
            count: 1,
            damage: 0,
        };

        let mut payload = Vec::new();
        ClientboundSetWindowItemsPacketCodec::encode(
            &ClientboundSetWindowItemsPacket {
                window_id: 0,
                slots,
            },
            &mut payload,
        )
        .unwrap();
        assert_eq!(108, payload.len());
        assert_eq!(&[0, 0, 45], &payload[..3]);

        let mut stream = Vec::new();
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundSetWindowItemsPacket::ID, payload),
            &mut stream,
        )
        .unwrap();
        stream.push(0x0A);
        assert_eq!(0x0A, stream[109]);
    }

    #[test]
    fn clientbound_experimental_packets_encode_expected_payloads() {
        let mut login = Vec::new();
        ClientboundLoginResponsePacketCodec::new(ClientboundLoginResponseMode::Beta173Observed)
            .encode(
                &ClientboundLoginResponsePacket::beta173_observed_defaults(),
                &mut login,
            )
            .unwrap();
        assert_eq!(vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], login);

        let mut spawn = Vec::new();
        ClientboundSpawnPositionPacketCodec::encode(
            &ClientboundSpawnPositionPacket::default_spawn(),
            &mut spawn,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 0, 0, 0, 0, 0, 65, 0, 0, 0, 0], spawn);

        let mut visibility = Vec::new();
        ClientboundChunkVisibilityPacketCodec::encode(
            &ClientboundChunkVisibilityPacket::load(0, 0),
            &mut visibility,
        )
        .unwrap();
        assert_eq!(vec![0, 0, 0, 0, 0, 0, 0, 0, 1], visibility);

        let mut unload_visibility = Vec::new();
        ClientboundChunkVisibilityPacketCodec::encode(
            &ClientboundChunkVisibilityPacket::unload(-1, 2),
            &mut unload_visibility,
        )
        .unwrap();
        assert_eq!(
            vec![0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 2, 0],
            unload_visibility
        );
    }

    #[test]
    fn clientbound_block_change_encodes_expected_payload() {
        let mut payload = Vec::new();

        ClientboundBlockChangePacketCodec::encode(
            &ClientboundBlockChangePacket {
                x: -1,
                y: 64,
                z: 2,
                block_type: 3,
                metadata: 0,
            },
            &mut payload,
        )
        .unwrap();

        assert_eq!(
            vec![0xFF, 0xFF, 0xFF, 0xFF, 64, 0, 0, 0, 2, 0, 3, 0, 0, 0, 0],
            payload
        );
    }

    #[test]
    fn clientbound_player_position_look_matches_reference_bytes() {
        let mut payload = Vec::new();

        ClientboundPlayerPositionLookPacketCodec::encode(
            &ClientboundPlayerPositionLookPacket::default_spawn(),
            &mut payload,
        )
        .unwrap();

        assert_eq!(41, payload.len());
        assert_eq!(
            vec![
                0x3F, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x50, 0x80, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x40, 0x50, 0xE7, 0xAE, 0x14, 0x7A, 0xE1, 0x48, 0x3F, 0xE0, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ],
            payload
        );
    }

    #[test]
    fn experimental_raw_full_chunk_has_expected_length_and_floor() {
        let raw = experimental_flat_chunk_data::uncompressed_full_chunk();

        assert_eq!(
            experimental_flat_chunk_data::UNCOMPRESSED_FULL_CHUNK_BYTES,
            raw.len()
        );
        assert_eq!(1, raw[experimental_flat_chunk_data::block_index(0, 0, 0)]);
        assert_eq!(3, raw[experimental_flat_chunk_data::block_index(0, 62, 0)]);
        assert_eq!(2, raw[experimental_flat_chunk_data::block_index(0, 63, 0)]);
        assert_eq!(0, raw[experimental_flat_chunk_data::block_index(0, 64, 0)]);
    }

    #[test]
    fn experimental_chunk_data_header_matches_reference_contract() {
        let packet = experimental_flat_chunk_data::chunk_at(0, 0);
        let mut payload = Vec::new();

        ClientboundChunkDataPacketCodec::encode(&packet, &mut payload).unwrap();

        assert_eq!(&[0, 0, 0, 0], &payload[0..4]);
        assert_eq!(&[0, 0], &payload[4..6]);
        assert_eq!(&[0, 0, 0, 0], &payload[6..10]);
        assert_eq!(&[15, 127, 15], &payload[10..13]);
        assert_eq!(
            packet.compressed_data.len() as i32,
            i32::from_be_bytes(payload[13..17].try_into().unwrap())
        );
        assert_eq!(packet.compressed_data.len(), payload[17..].len());
    }

    fn luxorium_login_payload() -> Vec<u8> {
        vec![
            0x00, 0x00, 0x00, 0x0E, 0x00, 0x08, 0x00, 0x4C, 0x00, 0x75, 0x00, 0x78, 0x00, 0x6F,
            0x00, 0x72, 0x00, 0x69, 0x00, 0x75, 0x00, 0x6D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00,
        ]
    }
}
