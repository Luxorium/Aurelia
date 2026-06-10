use aurelia_common::{BlockPos, ChunkPos};
use aurelia_protocol::{
    experimental_flat_chunk_data, read_u8, ClientboundBlockChangePacket,
    ClientboundBlockChangePacketCodec, ClientboundChunkDataPacket, ClientboundChunkDataPacketCodec,
    ClientboundChunkVisibilityPacket, ClientboundChunkVisibilityPacketCodec,
    ClientboundLoginResponseMode, ClientboundLoginResponsePacket,
    ClientboundLoginResponsePacketCodec, ClientboundPlayerPositionLookPacket,
    ClientboundPlayerPositionLookPacketCodec, ClientboundSpawnPositionPacket,
    ClientboundSpawnPositionPacketCodec, DisconnectPacket, DisconnectPacketCodec, HandshakePacket,
    HandshakePacketCodec, LegacyPacketFrameCodec, PacketCodec, PacketDirection, PacketFrame,
    ProtocolError, ServerboundAnimationPacket, ServerboundEntityActionPacket,
    ServerboundHeldItemChangePacket, ServerboundLoginPacket, ServerboundLoginPacketCodec,
    ServerboundMovementPacket, ServerboundPacketKind, ServerboundPlayerBlockPlacementPacket,
    ServerboundPlayerDiggingPacket, EXPECTED_PROTOCOL_VERSION, TARGET_VERSION,
};
use aurelia_region::RegionScheduler;
use aurelia_world::{
    BlockState, EntityId, EntityKind, EntityManager, FlatWorldGenerator, InMemoryWorldStorage,
    World,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const VERSION: &str = "0.1.0-SNAPSHOT";
pub const HANDSHAKE_RECEIVED_DISCONNECT: &str =
    "Aurelia received your handshake, but login is not implemented yet.";
pub const MISSING_PACKET_DISCONNECT: &str =
    "Aurelia did not receive an initial packet before disconnecting.";
pub const MALFORMED_PACKET_DISCONNECT: &str = "Aurelia could not decode your initial packet.";
pub const UNKNOWN_PACKET_DISCONNECT: &str = "Aurelia does not understand your initial packet yet.";
pub const EXPECTED_HANDSHAKE_DISCONNECT: &str = "Aurelia expected a handshake packet first.";
pub const LOGIN_RECEIVED_DISCONNECT: &str =
    "Aurelia received your login packet, but world join is not implemented yet.";
pub const EXPECTED_LOGIN_DISCONNECT: &str = "Aurelia expected a login packet after handshake.";
pub const PROTOCOL_MISMATCH_DISCONNECT: &str =
    "Aurelia only supports Minecraft Beta 1.7.3 protocol version 14.";
pub const POST_JOIN_PROTOCOL_DISCONNECT: &str = "Aurelia received an unsupported post-join packet.";
pub const UNDOCUMENTED_PACKET_DISCONNECT: &str =
    "Aurelia received a packet whose Beta 1.7.3 layout is not documented yet.";

pub mod trace {
    use aurelia_protocol::{
        movement_payload_length, ClientboundBlockChangePacket, ClientboundChunkDataPacket,
        ClientboundChunkVisibilityPacket, ClientboundPlayerPositionLookPacket,
        ClientboundSpawnPositionPacket, DisconnectPacket, HandshakePacket, PacketDirection,
        ServerboundLoginPacket,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PacketTraceEntry {
        pub packet_index: usize,
        pub packet_id: u8,
        pub payload_length: usize,
        pub payload_hex: String,
        pub direction: PacketDirection,
        pub decoded_packet_name: Option<String>,
    }

    impl PacketTraceEntry {
        pub fn new(
            packet_index: usize,
            packet_id: u8,
            payload_length: usize,
            payload_hex: impl Into<String>,
            direction: PacketDirection,
            decoded_packet_name: Option<String>,
        ) -> super::Result<Self> {
            if packet_index < 1 {
                return Err(super::ServerError::InvalidConfig(
                    "packetIndex must be positive".to_string(),
                ));
            }

            Ok(Self {
                packet_index,
                packet_id,
                payload_length,
                payload_hex: payload_hex.into(),
                direction,
                decoded_packet_name,
            })
        }
    }

    pub fn format_trace_entry(entry: &PacketTraceEntry) -> String {
        let name = entry.decoded_packet_name.as_deref().unwrap_or("Unknown");
        format!(
            "[trace] {} #{} id=0x{:02X} name={} payloadLength={} payloadHex={}",
            entry.direction.label(),
            entry.packet_index,
            entry.packet_id,
            name,
            entry.payload_length,
            entry.payload_hex
        )
    }

    pub fn format_payload_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn packet_trace_name(direction: PacketDirection, packet_id: u8) -> Option<&'static str> {
        if packet_id == ServerboundLoginPacket::ID {
            return match direction {
                PacketDirection::ClientToServer => Some("Login"),
                PacketDirection::ServerToClient => Some("LoginResponse"),
            };
        }

        match packet_id {
            0x00 => Some("KeepAlive"),
            0x03 => Some("Chat"),
            id if id == ClientboundSpawnPositionPacket::ID => Some("SpawnPosition"),
            id if id == ClientboundPlayerPositionLookPacket::ID => Some("PlayerPositionLook"),
            id if id == ClientboundChunkVisibilityPacket::ID => Some("SetChunkVisibility"),
            id if id == ClientboundChunkDataPacket::ID => Some("ChunkData"),
            id if id == ClientboundBlockChangePacket::ID => Some("BlockChange"),
            id if id == HandshakePacket::ID => Some("Handshake"),
            id if id == DisconnectPacket::ID => Some("Disconnect"),
            0x0E => Some("PlayerDigging"),
            0x0F => Some("PlayerBlockPlacement"),
            0x10 => Some("HeldItemChange"),
            0x12 => Some("Animation"),
            0x13 => Some("EntityAction"),
            id if movement_payload_length(id).is_some() => match id {
                0x0A => Some("Player"),
                0x0B => Some("PlayerPosition"),
                0x0C => Some("PlayerLook"),
                0x0D => Some("PlayerPositionLook"),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ServerError {
    InvalidConfig(String),
    Protocol(ProtocolError),
    Io(io::Error),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => f.write_str(message),
            Self::Protocol(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtocolError> for ServerError {
    fn from(value: ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<io::Error> for ServerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub world_name: String,
    pub packet_tracing_enabled: bool,
    pub packet_trace_limit: usize,
    pub trace_continue_after_handshake: bool,
    pub trace_handshake_response: String,
    pub experimental_join_enabled: bool,
    pub login_response_mode: ClientboundLoginResponseMode,
    pub playable_flat_world: bool,
    pub initial_chunk_radius: i32,
}

impl ServerConfig {
    pub const DEFAULT_PACKET_TRACE_LIMIT: usize = 4;
    pub const DEFAULT_TRACE_HANDSHAKE_RESPONSE: &'static str = "-";
    pub const DEFAULT_INITIAL_CHUNK_RADIUS: i32 = 1;

    pub fn new(host: impl Into<String>, port: u16, world_name: impl Into<String>) -> Result<Self> {
        Self::with_options(
            host,
            port,
            world_name,
            false,
            Self::DEFAULT_PACKET_TRACE_LIMIT,
            false,
            Self::DEFAULT_TRACE_HANDSHAKE_RESPONSE,
            false,
            ClientboundLoginResponseMode::Beta173Observed,
            false,
            Self::DEFAULT_INITIAL_CHUNK_RADIUS,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        host: impl Into<String>,
        port: u16,
        world_name: impl Into<String>,
        packet_tracing_enabled: bool,
        packet_trace_limit: usize,
        trace_continue_after_handshake: bool,
        trace_handshake_response: impl Into<String>,
        experimental_join_enabled: bool,
        login_response_mode: ClientboundLoginResponseMode,
        playable_flat_world: bool,
        initial_chunk_radius: i32,
    ) -> Result<Self> {
        let config = Self {
            host: host.into(),
            port,
            world_name: world_name.into(),
            packet_tracing_enabled,
            packet_trace_limit,
            trace_continue_after_handshake,
            trace_handshake_response: trace_handshake_response.into(),
            experimental_join_enabled,
            login_response_mode,
            playable_flat_world,
            initial_chunk_radius,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn default_config() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 25565,
            world_name: "world".to_string(),
            packet_tracing_enabled: false,
            packet_trace_limit: Self::DEFAULT_PACKET_TRACE_LIMIT,
            trace_continue_after_handshake: false,
            trace_handshake_response: Self::DEFAULT_TRACE_HANDSHAKE_RESPONSE.to_string(),
            experimental_join_enabled: false,
            login_response_mode: ClientboundLoginResponseMode::Beta173Observed,
            playable_flat_world: false,
            initial_chunk_radius: Self::DEFAULT_INITIAL_CHUNK_RADIUS,
        }
    }

    fn validate(&self) -> Result<()> {
        if self.host.trim().is_empty() {
            return Err(ServerError::InvalidConfig(
                "host must not be blank".to_string(),
            ));
        }
        if self.world_name.trim().is_empty() {
            return Err(ServerError::InvalidConfig(
                "worldName must not be blank".to_string(),
            ));
        }
        if self.packet_trace_limit < 1 {
            return Err(ServerError::InvalidConfig(
                "packetTraceLimit must be positive".to_string(),
            ));
        }
        if self.initial_chunk_radius < 0 || self.initial_chunk_radius > 8 {
            return Err(ServerError::InvalidConfig(
                "initialChunkRadius must be between 0 and 8".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn parse_config(args: &[impl AsRef<str>]) -> Result<ServerConfig> {
    let defaults = ServerConfig::default_config();
    let mut host = defaults.host;
    let mut port = defaults.port;
    let mut world_name = defaults.world_name;
    let mut packet_tracing_enabled = defaults.packet_tracing_enabled;
    let mut packet_trace_limit = defaults.packet_trace_limit;
    let mut trace_continue_after_handshake = defaults.trace_continue_after_handshake;
    let mut trace_handshake_response = defaults.trace_handshake_response;
    let mut experimental_join_enabled = defaults.experimental_join_enabled;
    let mut login_response_mode = defaults.login_response_mode;
    let mut playable_flat_world = defaults.playable_flat_world;
    let mut initial_chunk_radius = defaults.initial_chunk_radius;

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_ref();
        if let Some(value) = arg.strip_prefix("--host=") {
            host = value.to_string();
        } else if arg == "--host" {
            index += 1;
            host = required_arg_value(args, index, "--host")?.to_string();
        } else if let Some(value) = arg.strip_prefix("--port=") {
            port = parse_port(value)?;
        } else if arg == "--port" {
            index += 1;
            port = parse_port(required_arg_value(args, index, "--port")?)?;
        } else if let Some(value) = arg.strip_prefix("--world=") {
            world_name = value.to_string();
        } else if arg == "--world" {
            index += 1;
            world_name = required_arg_value(args, index, "--world")?.to_string();
        } else if arg == "--trace-packets" {
            packet_tracing_enabled = true;
        } else if arg == "--compat-debug" {
            packet_tracing_enabled = true;
            packet_trace_limit = packet_trace_limit.max(512);
        } else if let Some(value) = arg.strip_prefix("--trace-packet-limit=") {
            packet_trace_limit = parse_packet_trace_limit(value)?;
        } else if arg == "--trace-packet-limit" {
            index += 1;
            packet_trace_limit =
                parse_packet_trace_limit(required_arg_value(args, index, "--trace-packet-limit")?)?;
        } else if arg == "--trace-continue-after-handshake" {
            trace_continue_after_handshake = true;
        } else if let Some(value) = arg.strip_prefix("--trace-handshake-response=") {
            trace_handshake_response = value.to_string();
        } else if arg == "--trace-handshake-response" {
            index += 1;
            trace_handshake_response =
                required_arg_value(args, index, "--trace-handshake-response")?.to_string();
        } else if arg == "--experimental-join" {
            experimental_join_enabled = true;
        } else if arg == "--playable-flat-world" {
            playable_flat_world = true;
        } else if let Some(value) = arg.strip_prefix("--login-response-mode=") {
            login_response_mode = ClientboundLoginResponseMode::parse(value)?;
        } else if arg == "--login-response-mode" {
            index += 1;
            login_response_mode = ClientboundLoginResponseMode::parse(required_arg_value(
                args,
                index,
                "--login-response-mode",
            )?)?;
        } else if let Some(value) = arg.strip_prefix("--chunk-radius=") {
            initial_chunk_radius = parse_chunk_radius(value)?;
        } else if arg == "--chunk-radius" {
            index += 1;
            initial_chunk_radius =
                parse_chunk_radius(required_arg_value(args, index, "--chunk-radius")?)?;
        }
        index += 1;
    }

    ServerConfig::with_options(
        host,
        port,
        world_name,
        packet_tracing_enabled,
        packet_trace_limit,
        trace_continue_after_handshake,
        trace_handshake_response,
        experimental_join_enabled,
        login_response_mode,
        playable_flat_world,
        initial_chunk_radius,
    )
}

fn required_arg_value<'a>(
    args: &'a [impl AsRef<str>],
    index: usize,
    flag: &str,
) -> Result<&'a str> {
    args.get(index)
        .map(AsRef::as_ref)
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| ServerError::InvalidConfig(format!("missing value for {flag}")))
}

fn parse_port(value: &str) -> Result<u16> {
    let parsed: u32 = value
        .parse()
        .map_err(|_| ServerError::InvalidConfig(format!("invalid port: {value}")))?;
    if parsed > u16::MAX as u32 {
        return Err(ServerError::InvalidConfig(
            "port must be between 0 and 65535".to_string(),
        ));
    }
    Ok(parsed as u16)
}

fn parse_packet_trace_limit(value: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| ServerError::InvalidConfig(format!("invalid packet trace limit: {value}")))
}

fn parse_chunk_radius(value: &str) -> Result<i32> {
    value
        .parse()
        .map_err(|_| ServerError::InvalidConfig(format!("invalid chunk radius: {value}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking,
    Login,
    Joined,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Survival,
}

pub type SharedGameServerState = Arc<Mutex<GameServerState>>;

#[derive(Debug)]
pub struct GameServerState {
    world: World<InMemoryWorldStorage>,
    entities: EntityManager,
    players: HashMap<String, EntityId>,
}

impl Default for GameServerState {
    fn default() -> Self {
        Self::new_flat()
    }
}

impl GameServerState {
    pub fn new_flat() -> Self {
        Self {
            world: World::new(InMemoryWorldStorage::default(), FlatWorldGenerator),
            entities: EntityManager::default(),
            players: HashMap::new(),
        }
    }

    pub fn shared_flat() -> SharedGameServerState {
        Arc::new(Mutex::new(Self::new_flat()))
    }

    pub fn tick(&mut self) {
        self.world.tick();
    }

    pub fn world_time(&self) -> u64 {
        self.world.time()
    }

    pub fn block_at(&mut self, pos: BlockPos) -> BlockState {
        self.world.block_at(pos)
    }

    pub fn get_block(&mut self, x: i32, y: i32, z: i32) -> BlockState {
        self.world.get_block(x, y, z)
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u8, metadata: u8) -> bool {
        self.world.set_block_id(x, y, z, block_id, metadata)
    }

    pub fn ensure_chunk_loaded(&mut self, pos: ChunkPos) {
        self.world.ensure_chunk_loaded(pos);
    }

    pub fn is_chunk_loaded(&self, pos: ChunkPos) -> bool {
        self.world.is_chunk_loaded(pos)
    }

    pub fn is_valid_block_pos(pos: BlockPos) -> bool {
        World::<InMemoryWorldStorage>::is_valid_block_pos(pos)
    }

    pub fn break_block(&mut self, pos: BlockPos) -> bool {
        self.world.break_block(pos)
    }

    pub fn place_block(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.world.place_block(pos, state)
    }

    pub fn register_player(&mut self, username: impl Into<String>) -> EntityId {
        let username = username.into();
        if let Some(id) = self.players.get(&username) {
            return *id;
        }
        let id = self.entities.spawn(EntityKind::Player, 0.5, 66.0, 0.5);
        self.players.insert(username, id);
        id
    }

    pub fn unregister_player(&mut self, username: &str) -> Option<EntityId> {
        let id = self.players.remove(username)?;
        self.entities.despawn(id);
        Some(id)
    }

    pub fn player_entity(&self, username: &str) -> Option<EntityId> {
        self.players.get(username).copied()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn spawn_passive_mobs_near_spawn(&mut self) -> Vec<EntityId> {
        vec![
            self.entities.spawn(EntityKind::Pig, 4.5, 65.0, 4.5),
            self.entities.spawn(EntityKind::Cow, -4.5, 65.0, 4.5),
        ]
    }
}

#[derive(Debug)]
pub struct ServerTickLoop {
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ServerTickLoop {
    pub fn start(state: SharedGameServerState) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let worker = thread::spawn(move || {
            while worker_running.load(Ordering::Acquire) {
                Self::tick_once(&state);
                thread::sleep(Duration::from_millis(aurelia_common::MILLIS_PER_TICK));
            }
        });
        Self {
            running,
            worker: Some(worker),
        }
    }

    pub fn tick_once(state: &SharedGameServerState) {
        if let Ok(mut state) = state.lock() {
            state.tick();
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| ServerError::InvalidConfig("tick loop panicked".to_string()))?;
        }
        Ok(())
    }
}

impl Drop for ServerTickLoop {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub username: String,
    pub entity_id: EntityId,
    pub game_mode: GameMode,
    pub health: i32,
    pub x: f64,
    pub y: f64,
    pub stance: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub current_chunk: ChunkPos,
    pub selected_hotbar_slot: u8,
    pub crouching: bool,
}

impl PlayerState {
    pub fn new(username: impl Into<String>, entity_id: EntityId) -> Self {
        let x = 0.5;
        let y = 66.0;
        let z = 0.5;
        Self {
            username: username.into(),
            entity_id,
            game_mode: GameMode::Survival,
            health: 20,
            x,
            y,
            stance: 67.62,
            z,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            current_chunk: ChunkPos::from_block(x.floor() as i32, z.floor() as i32),
            selected_hotbar_slot: 0,
            crouching: false,
        }
    }

    pub fn apply_movement(&mut self, movement: ServerboundMovementPacket) {
        match movement {
            ServerboundMovementPacket::Player { on_ground } => {
                self.on_ground = on_ground;
            }
            ServerboundMovementPacket::PlayerPosition {
                x,
                y,
                stance,
                z,
                on_ground,
            } => {
                self.x = x;
                self.y = y;
                self.stance = stance;
                self.z = z;
                self.on_ground = on_ground;
            }
            ServerboundMovementPacket::PlayerLook {
                yaw,
                pitch,
                on_ground,
            } => {
                self.yaw = yaw;
                self.pitch = pitch;
                self.on_ground = on_ground;
            }
            ServerboundMovementPacket::PlayerPositionLook {
                x,
                y,
                stance,
                z,
                yaw,
                pitch,
                on_ground,
            } => {
                self.x = x;
                self.y = y;
                self.stance = stance;
                self.z = z;
                self.yaw = yaw;
                self.pitch = pitch;
                self.on_ground = on_ground;
            }
        }
        self.current_chunk = ChunkPos::from_block(self.x.floor() as i32, self.z.floor() as i32);
    }

    pub fn set_hotbar_slot(&mut self, slot: u8) {
        if slot <= 8 {
            self.selected_hotbar_slot = slot;
        }
    }

    pub fn apply_entity_action(&mut self, action_id: i8) {
        match action_id {
            1 => self.crouching = true,
            2 => self.crouching = false,
            _ => {}
        }
    }

    pub fn can_reach(&self, pos: BlockPos) -> bool {
        const MAX_REACH_SQUARED: f64 = 36.0;
        let dx = (pos.x as f64 + 0.5) - self.x;
        let dy = (pos.y as f64 + 0.5) - self.y;
        let dz = (pos.z as f64 + 0.5) - self.z;
        (dx * dx) + (dy * dy) + (dz * dz) <= MAX_REACH_SQUARED
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionExit {
    Disconnected(String),
    ClientClosed,
}

#[derive(Debug, Clone)]
pub struct PlayerSession {
    config: ServerConfig,
    state_ref: SharedGameServerState,
    state: ConnectionState,
    player: Option<PlayerState>,
    registered_username: Option<String>,
    sent_chunks: HashSet<ChunkPos>,
    last_packet_id: Option<u8>,
    trace_index: usize,
}

impl PlayerSession {
    pub fn new(config: ServerConfig) -> Self {
        Self::with_state(config, GameServerState::shared_flat())
    }

    pub fn with_state(config: ServerConfig, state_ref: SharedGameServerState) -> Self {
        Self {
            config,
            state_ref,
            state: ConnectionState::Handshaking,
            player: None,
            registered_username: None,
            sent_chunks: HashSet::new(),
            last_packet_id: None,
            trace_index: 0,
        }
    }

    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn player(&self) -> Option<&PlayerState> {
        self.player.as_ref()
    }

    pub fn run(&mut self, connection: &mut (impl Read + Write)) -> Result<SessionExit> {
        if let Some(exit) = self.expect_handshake(connection)? {
            return Ok(exit);
        }
        if let Some(exit) = self.expect_login_or_disconnect(connection)? {
            return Ok(exit);
        }

        if self.state == ConnectionState::Joined {
            self.run_joined_loop(connection)
        } else {
            Ok(SessionExit::ClientClosed)
        }
    }

    fn expect_handshake(
        &mut self,
        connection: &mut (impl Read + Write),
    ) -> Result<Option<SessionExit>> {
        self.state = ConnectionState::Handshaking;
        let packet_id = match read_u8(connection) {
            Ok(packet_id) => packet_id,
            Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                return self
                    .disconnect(connection, MISSING_PACKET_DISCONNECT)
                    .map(Some);
            }
            Err(error) => return Err(error.into()),
        };
        self.last_packet_id = Some(packet_id);

        if packet_id != HandshakePacket::ID {
            self.trace_packet(PacketDirection::ClientToServer, packet_id, &[]);
            return self
                .disconnect(connection, EXPECTED_HANDSHAKE_DISCONNECT)
                .map(Some);
        }

        match HandshakePacketCodec::decode(connection) {
            Ok(handshake) => {
                let payload = HandshakePacketCodec::to_frame(&handshake)?.into_payload();
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                self.state = ConnectionState::Login;
                if self.config.trace_continue_after_handshake
                    || self.config.experimental_join_enabled
                    || self.config.playable_flat_world
                {
                    send_codec_frame::<HandshakePacketCodec, _>(
                        connection,
                        &HandshakePacket::new(self.config.trace_handshake_response.as_str()),
                    )?;
                } else {
                    return self
                        .disconnect(connection, HANDSHAKE_RECEIVED_DISCONNECT)
                        .map(Some);
                }
            }
            Err(_) => {
                return self
                    .disconnect(connection, MALFORMED_PACKET_DISCONNECT)
                    .map(Some);
            }
        }
        Ok(None)
    }

    fn expect_login_or_disconnect(
        &mut self,
        connection: &mut (impl Read + Write),
    ) -> Result<Option<SessionExit>> {
        if self.state != ConnectionState::Login {
            return Ok(None);
        }

        let packet_id = match read_u8(connection) {
            Ok(packet_id) => packet_id,
            Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                return self
                    .disconnect(connection, MISSING_PACKET_DISCONNECT)
                    .map(Some);
            }
            Err(error) => return Err(error.into()),
        };
        self.last_packet_id = Some(packet_id);

        if packet_id != ServerboundLoginPacket::ID {
            self.trace_packet(PacketDirection::ClientToServer, packet_id, &[]);
            return self
                .disconnect(connection, EXPECTED_LOGIN_DISCONNECT)
                .map(Some);
        }

        let login = match ServerboundLoginPacketCodec::decode(connection) {
            Ok(login) => login,
            Err(_) => {
                return self
                    .disconnect(connection, MALFORMED_PACKET_DISCONNECT)
                    .map(Some);
            }
        };
        let payload = ServerboundLoginPacketCodec::to_frame(&login)?.into_payload();
        self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);

        if login.protocol_version != EXPECTED_PROTOCOL_VERSION {
            return self
                .disconnect(connection, PROTOCOL_MISMATCH_DISCONNECT)
                .map(Some);
        }

        if !(self.config.experimental_join_enabled || self.config.playable_flat_world) {
            return self
                .disconnect(connection, LOGIN_RECEIVED_DISCONNECT)
                .map(Some);
        }

        let username = login.username;
        let entity_id = {
            let mut state = self
                .state_ref
                .lock()
                .map_err(|_| ServerError::InvalidConfig("game state lock poisoned".to_string()))?;
            state.register_player(username.clone())
        };
        self.registered_username = Some(username.clone());
        self.player = Some(PlayerState::new(username, entity_id));
        self.send_join_sequence(connection)?;
        self.state = ConnectionState::Joined;
        Ok(None)
    }

    fn run_joined_loop(&mut self, connection: &mut (impl Read + Write)) -> Result<SessionExit> {
        loop {
            let packet_id = match read_u8(connection) {
                Ok(packet_id) => packet_id,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    self.unregister_player();
                    self.log_session_close("client closed connection");
                    return Ok(SessionExit::ClientClosed);
                }
                Err(error) => return Err(error.into()),
            };
            self.last_packet_id = Some(packet_id);

            let packet_kind = ServerboundPacketKind::from_id(packet_id);
            if let Some(payload_len) = packet_kind.fixed_payload_length() {
                let mut payload = vec![0; payload_len];
                connection.read_exact(&mut payload)?;
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                match packet_kind {
                    ServerboundPacketKind::Player
                    | ServerboundPacketKind::PlayerPosition
                    | ServerboundPacketKind::PlayerLook
                    | ServerboundPacketKind::PlayerPositionLook => {
                        if let Some(movement) =
                            ServerboundMovementPacket::decode(packet_id, &mut payload.as_slice())?
                        {
                            if let Some(player) = self.player.as_mut() {
                                player.apply_movement(movement);
                            }
                            if self.config.packet_tracing_enabled {
                                if let Some(player) = self.player.as_ref() {
                                    eprintln!(
                                        "[session] {} moved to {:.3},{:.3},{:.3} chunk={},{}",
                                        player.username,
                                        player.x,
                                        player.y,
                                        player.z,
                                        player.current_chunk.x,
                                        player.current_chunk.z
                                    );
                                }
                            }
                            self.stream_chunks_for_player(connection)?;
                        }
                    }
                    ServerboundPacketKind::PlayerDigging => {
                        let packet =
                            ServerboundPlayerDiggingPacket::decode(&mut payload.as_slice())?;
                        self.handle_player_digging(packet, connection)?;
                    }
                    ServerboundPacketKind::HeldItemChange => {
                        let packet =
                            ServerboundHeldItemChangePacket::decode(&mut payload.as_slice())?;
                        if let (Some(player), Some(slot)) =
                            (self.player.as_mut(), packet.hotbar_slot())
                        {
                            player.set_hotbar_slot(slot);
                        }
                    }
                    ServerboundPacketKind::Animation => {
                        let _ = ServerboundAnimationPacket::decode(&mut payload.as_slice())?;
                    }
                    ServerboundPacketKind::EntityAction => {
                        let packet =
                            ServerboundEntityActionPacket::decode(&mut payload.as_slice())?;
                        if let Some(player) = self.player.as_mut() {
                            player.apply_entity_action(packet.action_id);
                        }
                    }
                    _ => {}
                }
                continue;
            }

            if packet_id == ServerboundPlayerBlockPlacementPacket::ID {
                let packet = ServerboundPlayerBlockPlacementPacket::decode(connection)?;
                let mut payload = Vec::new();
                packet.encode(&mut payload)?;
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                self.handle_player_block_placement(packet, connection)?;
                continue;
            }

            if packet_id == DisconnectPacket::ID {
                let packet = DisconnectPacketCodec::decode(connection)?;
                self.state = ConnectionState::Disconnected;
                self.unregister_player();
                self.log_session_close(&format!("client disconnected: {}", packet.reason));
                return Ok(SessionExit::Disconnected(packet.reason));
            }

            if !packet_kind.has_documented_layout() {
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &[]);
                self.log_undocumented_packet(packet_id, packet_kind);
                return self.disconnect(connection, UNDOCUMENTED_PACKET_DISCONNECT);
            }

            self.trace_packet(PacketDirection::ClientToServer, packet_id, &[]);
            self.log_unsupported_packet(packet_id);
            return self.disconnect(connection, POST_JOIN_PROTOCOL_DISCONNECT);
        }
    }

    fn send_join_sequence(&mut self, connection: &mut impl Write) -> Result<()> {
        let entity_id = self
            .player
            .as_ref()
            .map(|player| player.entity_id.raw() as i32)
            .unwrap_or(1);
        let mut login_response = match self.config.login_response_mode {
            ClientboundLoginResponseMode::Beta173Observed => {
                ClientboundLoginResponsePacket::beta173_observed_defaults()
            }
            ClientboundLoginResponseMode::McdevsLegacy => {
                ClientboundLoginResponsePacket::mcdevs_legacy_defaults()
            }
        };
        login_response.entity_id = entity_id;

        self.write_clientbound_login_response(connection, &login_response)?;
        self.write_clientbound_frame(connection, ClientboundSpawnPositionPacket::ID, |payload| {
            ClientboundSpawnPositionPacketCodec::encode(
                &ClientboundSpawnPositionPacket::default_spawn(),
                payload,
            )
        })?;
        self.write_clientbound_frame(
            connection,
            ClientboundPlayerPositionLookPacket::ID,
            |payload| {
                ClientboundPlayerPositionLookPacketCodec::encode(
                    &ClientboundPlayerPositionLookPacket::default_spawn(),
                    payload,
                )
            },
        )?;

        self.stream_chunks_for_player(connection)?;
        Ok(())
    }

    fn stream_chunks_for_player(&mut self, connection: &mut impl Write) -> Result<()> {
        let Some(player) = self.player.as_ref() else {
            return Ok(());
        };
        let center = player.current_chunk;
        let needed = chunks_in_radius(center, self.chunk_radius());
        for pos in needed {
            if self.sent_chunks.insert(pos) {
                self.write_chunk_pair(connection, pos)?;
            }
        }
        Ok(())
    }

    fn chunk_radius(&self) -> i32 {
        if self.config.playable_flat_world {
            self.config.initial_chunk_radius
        } else {
            0
        }
    }

    fn write_clientbound_login_response(
        &mut self,
        connection: &mut impl Write,
        packet: &ClientboundLoginResponsePacket,
    ) -> Result<()> {
        let mut payload = Vec::new();
        ClientboundLoginResponsePacketCodec::new(self.config.login_response_mode)
            .encode(packet, &mut payload)?;
        self.trace_packet(
            PacketDirection::ServerToClient,
            ClientboundLoginResponsePacket::ID,
            &payload,
        );
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundLoginResponsePacket::ID, payload),
            connection,
        )?;
        Ok(())
    }

    fn write_clientbound_frame(
        &mut self,
        connection: &mut impl Write,
        packet_id: u8,
        encode: impl FnOnce(&mut Vec<u8>) -> aurelia_protocol::Result<()>,
    ) -> Result<()> {
        let mut payload = Vec::new();
        encode(&mut payload)?;
        self.trace_packet(PacketDirection::ServerToClient, packet_id, &payload);
        LegacyPacketFrameCodec::write(&PacketFrame::new(packet_id, payload), connection)?;
        Ok(())
    }

    fn write_chunk_pair(&mut self, connection: &mut impl Write, pos: ChunkPos) -> Result<()> {
        self.with_game_state(|state| {
            state.ensure_chunk_loaded(pos);
            Ok(())
        })?;
        self.write_clientbound_frame(
            connection,
            ClientboundChunkVisibilityPacket::ID,
            |payload| {
                ClientboundChunkVisibilityPacketCodec::encode(
                    &ClientboundChunkVisibilityPacket::load(pos.x, pos.z),
                    payload,
                )
            },
        )?;
        self.write_clientbound_frame(connection, ClientboundChunkDataPacket::ID, |payload| {
            ClientboundChunkDataPacketCodec::encode(
                &experimental_flat_chunk_data::chunk_at(pos.x, pos.z),
                payload,
            )
        })?;
        Ok(())
    }

    fn handle_player_digging(
        &mut self,
        packet: ServerboundPlayerDiggingPacket,
        connection: &mut impl Write,
    ) -> Result<()> {
        let pos = BlockPos::new(packet.x, i32::from(packet.y), packet.z);
        let should_break = packet.status == ServerboundPlayerDiggingPacket::FINISHED_DIGGING_STATUS
            && self.is_block_loaded_for_player(pos)
            && self.player_can_reach(pos);

        let state = self.with_game_state(|state| {
            let current = state.block_at(pos);
            if should_break && current != BlockState::AIR {
                state.break_block(pos);
                Ok(BlockState::AIR)
            } else {
                Ok(current)
            }
        })?;
        self.write_block_change(connection, pos, state)
    }

    fn handle_player_block_placement(
        &mut self,
        packet: ServerboundPlayerBlockPlacementPacket,
        connection: &mut impl Write,
    ) -> Result<()> {
        if packet.is_special_item_use() {
            return Ok(());
        }

        let Some(target) = placement_target_pos(
            BlockPos::new(packet.x, i32::from(packet.y), packet.z),
            packet.direction,
        ) else {
            let pos = BlockPos::new(packet.x, i32::from(packet.y), packet.z);
            let state = self.with_game_state(|state| Ok(state.block_at(pos)))?;
            return self.write_block_change(connection, pos, state);
        };

        let desired = placement_block_state(packet.held_item.item_id());
        let can_place = self.is_block_loaded_for_player(target) && self.player_can_reach(target);
        let state = self.with_game_state(|state| {
            let current = state.block_at(target);
            if can_place && current == BlockState::AIR {
                state.place_block(target, desired);
                Ok(desired)
            } else {
                Ok(current)
            }
        })?;
        self.write_block_change(connection, target, state)
    }

    fn write_block_change(
        &mut self,
        connection: &mut impl Write,
        pos: BlockPos,
        state: BlockState,
    ) -> Result<()> {
        if !GameServerState::is_valid_block_pos(pos) {
            return Ok(());
        }
        self.write_clientbound_frame(connection, ClientboundBlockChangePacket::ID, |payload| {
            ClientboundBlockChangePacketCodec::encode(
                &ClientboundBlockChangePacket {
                    x: pos.x,
                    y: pos.y as u8,
                    z: pos.z,
                    block_type: i16::from(state.id),
                    metadata: i32::from(state.metadata),
                },
                payload,
            )
        })
    }

    fn is_block_loaded_for_player(&self, pos: BlockPos) -> bool {
        GameServerState::is_valid_block_pos(pos)
            && self
                .sent_chunks
                .contains(&ChunkPos::from_block(pos.x, pos.z))
            && self
                .state_ref
                .lock()
                .map(|state| state.is_chunk_loaded(ChunkPos::from_block(pos.x, pos.z)))
                .unwrap_or(false)
    }

    fn player_can_reach(&self, pos: BlockPos) -> bool {
        self.player
            .as_ref()
            .map(|player| player.can_reach(pos))
            .unwrap_or(false)
    }

    fn with_game_state<T>(
        &self,
        action: impl FnOnce(&mut GameServerState) -> Result<T>,
    ) -> Result<T> {
        let mut state = self
            .state_ref
            .lock()
            .map_err(|_| ServerError::InvalidConfig("game state lock poisoned".to_string()))?;
        action(&mut state)
    }

    fn disconnect(
        &mut self,
        connection: &mut impl Write,
        reason: impl Into<String>,
    ) -> Result<SessionExit> {
        let reason = reason.into();
        send_codec_frame::<DisconnectPacketCodec, _>(
            connection,
            &DisconnectPacket::new(reason.as_str()),
        )?;
        self.state = ConnectionState::Disconnected;
        self.unregister_player();
        self.log_session_close(&format!("server disconnected: {reason}"));
        Ok(SessionExit::Disconnected(reason))
    }

    fn trace_packet(&mut self, direction: PacketDirection, packet_id: u8, payload: &[u8]) {
        if !self.config.packet_tracing_enabled || self.trace_index >= self.config.packet_trace_limit
        {
            return;
        }
        self.trace_index += 1;
        let name = trace::packet_trace_name(direction, packet_id).map(str::to_string);
        let entry = trace::PacketTraceEntry::new(
            self.trace_index,
            packet_id,
            payload.len(),
            self.trace_payload_hex(payload),
            direction,
            name,
        );
        if let Ok(entry) = entry {
            eprintln!("{}", trace::format_trace_entry(&entry));
        }
    }

    fn trace_payload_hex(&self, payload: &[u8]) -> String {
        const TRACE_HEX_BYTES: usize = 96;
        if payload.len() <= TRACE_HEX_BYTES {
            return trace::format_payload_hex(payload);
        }
        format!(
            "{} ...(+{} bytes)",
            trace::format_payload_hex(&payload[..TRACE_HEX_BYTES]),
            payload.len() - TRACE_HEX_BYTES
        )
    }

    fn unregister_player(&mut self) {
        if let Some(username) = self.registered_username.take() {
            if let Ok(mut state) = self.state_ref.lock() {
                state.unregister_player(&username);
            }
        }
    }

    fn log_undocumented_packet(&self, packet_id: u8, kind: ServerboundPacketKind) {
        eprintln!(
            "[session] unsupported undocumented packet id=0x{packet_id:02X} kind={} state={:?} player={} chunk={}",
            kind.name(),
            self.state,
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );
    }

    fn log_unsupported_packet(&self, packet_id: u8) {
        eprintln!(
            "[session] unsupported packet id=0x{packet_id:02X} state={:?} player={} chunk={}",
            self.state,
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );
    }

    fn log_session_close(&self, reason: &str) {
        eprintln!(
            "[session] closed state={:?} lastPacket={} player={} chunk={} reason={}",
            self.state,
            self.last_packet_id
                .map(|id| format!("0x{id:02X}"))
                .unwrap_or_else(|| "none".to_string()),
            self.player_name_for_log(),
            self.player_chunk_for_log(),
            reason
        );
    }

    fn player_name_for_log(&self) -> &str {
        self.player
            .as_ref()
            .map(|player| player.username.as_str())
            .unwrap_or("<none>")
    }

    fn player_chunk_for_log(&self) -> String {
        self.player
            .as_ref()
            .map(|player| format!("{},{}", player.current_chunk.x, player.current_chunk.z))
            .unwrap_or_else(|| "<none>".to_string())
    }
}

pub fn chunks_in_radius(center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
    let radius = radius.max(0);
    let mut chunks = Vec::new();
    for x in center.x - radius..=center.x + radius {
        for z in center.z - radius..=center.z + radius {
            chunks.push(ChunkPos::new(x, z));
        }
    }
    chunks
}

pub fn placement_face_offset(direction: i8) -> Option<(i32, i32, i32)> {
    match direction {
        0 => Some((0, -1, 0)),
        1 => Some((0, 1, 0)),
        2 => Some((0, 0, -1)),
        3 => Some((0, 0, 1)),
        4 => Some((-1, 0, 0)),
        5 => Some((1, 0, 0)),
        _ => None,
    }
}

pub fn placement_target_pos(against: BlockPos, direction: i8) -> Option<BlockPos> {
    let (dx, dy, dz) = placement_face_offset(direction)?;
    Some(against.offset(dx, dy, dz))
}

fn placement_block_state(item_id: Option<i16>) -> BlockState {
    match item_id {
        Some(1) => BlockState::STONE,
        Some(2) => BlockState::GRASS,
        Some(3) => BlockState::DIRT,
        Some(id) if (1..=255).contains(&id) => BlockState::new_unchecked(id as u8, 0),
        _ => BlockState::DIRT,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    Disconnected(String),
    ExperimentalJoinStarted,
}

#[derive(Debug, Clone)]
pub struct FirstPacketDispatcher {
    trace_continue_after_handshake: bool,
    trace_handshake_response: String,
    experimental_join_enabled: bool,
    login_response_mode: ClientboundLoginResponseMode,
}

impl FirstPacketDispatcher {
    pub fn new(config: &ServerConfig) -> Self {
        Self {
            trace_continue_after_handshake: config.trace_continue_after_handshake,
            trace_handshake_response: config.trace_handshake_response.clone(),
            experimental_join_enabled: config.experimental_join_enabled,
            login_response_mode: config.login_response_mode,
        }
    }

    pub fn handle(&self, connection: &mut (impl Read + Write)) -> Result<DispatchOutcome> {
        let initial = self.read_initial(connection)?;
        match initial {
            InitialPacket::Missing => self.disconnect(connection, MISSING_PACKET_DISCONNECT),
            InitialPacket::Malformed => self.disconnect(connection, MALFORMED_PACKET_DISCONNECT),
            InitialPacket::Unknown => self.disconnect(connection, UNKNOWN_PACKET_DISCONNECT),
            InitialPacket::Login(_) => self.disconnect(connection, EXPECTED_HANDSHAKE_DISCONNECT),
            InitialPacket::Handshake(_) if !self.trace_continue_after_handshake => {
                self.disconnect(connection, HANDSHAKE_RECEIVED_DISCONNECT)
            }
            InitialPacket::Handshake(_) => self.handle_after_handshake(connection),
        }
    }

    fn handle_after_handshake(
        &self,
        connection: &mut (impl Read + Write),
    ) -> Result<DispatchOutcome> {
        let response = HandshakePacket::new(self.trace_handshake_response.clone());
        send_codec_frame::<HandshakePacketCodec, _>(connection, &response)?;

        let login = self.read_initial(connection)?;
        match login {
            InitialPacket::Missing => self.disconnect(connection, MISSING_PACKET_DISCONNECT),
            InitialPacket::Malformed => self.disconnect(connection, MALFORMED_PACKET_DISCONNECT),
            InitialPacket::Unknown => self.disconnect(connection, UNKNOWN_PACKET_DISCONNECT),
            InitialPacket::Handshake(_) => self.disconnect(connection, EXPECTED_LOGIN_DISCONNECT),
            InitialPacket::Login(_) if !self.experimental_join_enabled => {
                self.disconnect(connection, LOGIN_RECEIVED_DISCONNECT)
            }
            InitialPacket::Login(_) => {
                self.send_experimental_join_sequence(connection)?;
                Ok(DispatchOutcome::ExperimentalJoinStarted)
            }
        }
    }

    fn read_initial(&self, connection: &mut impl Read) -> Result<InitialPacket> {
        let packet_id = match read_u8(connection) {
            Ok(packet_id) => packet_id,
            Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                return Ok(InitialPacket::Missing);
            }
            Err(error) => return Err(error.into()),
        };

        match packet_id {
            HandshakePacket::ID => HandshakePacketCodec::decode(connection)
                .map(InitialPacket::Handshake)
                .or_else(|error| match error {
                    ProtocolError::Io(io_error) if io_error.kind() == ErrorKind::UnexpectedEof => {
                        Ok(InitialPacket::Malformed)
                    }
                    ProtocolError::InvalidData(_) => Ok(InitialPacket::Malformed),
                    other => Err(other),
                })
                .map_err(ServerError::from),
            ServerboundLoginPacket::ID => ServerboundLoginPacketCodec::decode(connection)
                .map(InitialPacket::Login)
                .or_else(|error| match error {
                    ProtocolError::Io(io_error) if io_error.kind() == ErrorKind::UnexpectedEof => {
                        Ok(InitialPacket::Malformed)
                    }
                    ProtocolError::InvalidData(_) => Ok(InitialPacket::Malformed),
                    other => Err(other),
                })
                .map_err(ServerError::from),
            _ => Ok(InitialPacket::Unknown),
        }
    }

    fn send_experimental_join_sequence(&self, connection: &mut impl Write) -> Result<()> {
        let login_response = match self.login_response_mode {
            ClientboundLoginResponseMode::Beta173Observed => {
                ClientboundLoginResponsePacket::beta173_observed_defaults()
            }
            ClientboundLoginResponseMode::McdevsLegacy => {
                ClientboundLoginResponsePacket::mcdevs_legacy_defaults()
            }
        };

        let mut payload = Vec::new();
        ClientboundLoginResponsePacketCodec::new(self.login_response_mode)
            .encode(&login_response, &mut payload)?;
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundLoginResponsePacket::ID, payload),
            connection,
        )?;

        let spawn = ClientboundSpawnPositionPacket::default_spawn();
        let mut payload = Vec::new();
        ClientboundSpawnPositionPacketCodec::encode(&spawn, &mut payload)?;
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundSpawnPositionPacket::ID, payload),
            connection,
        )?;

        let position = ClientboundPlayerPositionLookPacket::default_spawn();
        let mut payload = Vec::new();
        ClientboundPlayerPositionLookPacketCodec::encode(&position, &mut payload)?;
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundPlayerPositionLookPacket::ID, payload),
            connection,
        )?;

        let visibility = ClientboundChunkVisibilityPacket::load(0, 0);
        let mut payload = Vec::new();
        ClientboundChunkVisibilityPacketCodec::encode(&visibility, &mut payload)?;
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundChunkVisibilityPacket::ID, payload),
            connection,
        )?;

        let chunk = experimental_flat_chunk_data::chunk_at(0, 0);
        let mut payload = Vec::new();
        ClientboundChunkDataPacketCodec::encode(&chunk, &mut payload)?;
        LegacyPacketFrameCodec::write(
            &PacketFrame::new(ClientboundChunkDataPacket::ID, payload),
            connection,
        )?;
        Ok(())
    }

    fn disconnect(
        &self,
        connection: &mut impl Write,
        reason: impl Into<String>,
    ) -> Result<DispatchOutcome> {
        let reason = reason.into();
        send_codec_frame::<DisconnectPacketCodec, _>(
            connection,
            &DisconnectPacket::new(reason.as_str()),
        )?;
        Ok(DispatchOutcome::Disconnected(reason))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InitialPacket {
    Missing,
    Malformed,
    Unknown,
    Handshake(HandshakePacket),
    Login(ServerboundLoginPacket),
}

fn send_codec_frame<C, P>(connection: &mut impl Write, packet: &P) -> Result<()>
where
    C: PacketCodec<P>,
{
    let frame = C::to_frame(packet)?;
    LegacyPacketFrameCodec::write(&frame, connection)?;
    Ok(())
}

pub struct ServerBootstrap {
    config: ServerConfig,
}

impl ServerBootstrap {
    pub const fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub fn start(self) -> Result<RunningServer> {
        let state = GameServerState::shared_flat();
        {
            let mut state = state
                .lock()
                .map_err(|_| ServerError::InvalidConfig("game state lock poisoned".to_string()))?;
            state.spawn_passive_mobs_near_spawn();
        }
        let _regions = RegionScheduler::default();
        eprintln!("Starting Aurelia {VERSION}");
        eprintln!("Target compatibility: {TARGET_VERSION}");
        eprintln!("World: {}", self.config.world_name);
        eprintln!("Bind address: {}:{}", self.config.host, self.config.port);

        let listener = TcpListener::bind((self.config.host.as_str(), self.config.port))?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let listener = Arc::new(listener);
        let running = Arc::new(AtomicBool::new(true));
        let tick_loop = ServerTickLoop::start(Arc::clone(&state));
        let worker = spawn_accept_loop(
            Arc::clone(&listener),
            Arc::clone(&running),
            self.config.clone(),
            Arc::clone(&state),
        );

        Ok(RunningServer {
            listener,
            local_addr,
            running,
            tick_loop: Some(tick_loop),
            worker: Some(worker),
            state,
        })
    }
}

fn spawn_accept_loop(
    listener: Arc<TcpListener>,
    running: Arc<AtomicBool>,
    config: ServerConfig,
    state: SharedGameServerState,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let config = config.clone();
                    let state = Arc::clone(&state);
                    thread::spawn(move || {
                        let mut session = PlayerSession::with_state(config, state);
                        if let Err(error) = session.run(&mut stream) {
                            eprintln!("connection handling failed: {error}");
                        }
                        let _ = stream.shutdown(Shutdown::Both);
                    });
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) if running.load(Ordering::Acquire) => {
                    eprintln!("accept failed: {error}");
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    })
}

#[derive(Debug)]
pub struct RunningServer {
    listener: Arc<TcpListener>,
    local_addr: SocketAddr,
    running: Arc<AtomicBool>,
    tick_loop: Option<ServerTickLoop>,
    worker: Option<JoinHandle<()>>,
    state: SharedGameServerState,
}

impl RunningServer {
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn listener(&self) -> Arc<TcpListener> {
        Arc::clone(&self.listener)
    }

    pub fn state(&self) -> SharedGameServerState {
        Arc::clone(&self.state)
    }

    pub fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| ServerError::InvalidConfig("accept loop panicked".to_string()))?;
        }
        if let Some(mut tick_loop) = self.tick_loop.take() {
            tick_loop.stop()?;
        }
        Ok(())
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurelia_protocol::{PacketCodec, PacketDirection};
    use std::io::{Cursor, Read, Write};

    #[test]
    fn default_config_uses_dedicated_server_port() {
        let config = ServerConfig::default_config();

        assert_eq!("0.0.0.0", config.host);
        assert_eq!(25565, config.port);
        assert_eq!("world", config.world_name);
        assert!(!config.packet_tracing_enabled);
        assert_eq!(
            ServerConfig::DEFAULT_PACKET_TRACE_LIMIT,
            config.packet_trace_limit
        );
        assert!(!config.trace_continue_after_handshake);
        assert_eq!(
            ServerConfig::DEFAULT_TRACE_HANDSHAKE_RESPONSE,
            config.trace_handshake_response
        );
        assert!(!config.experimental_join_enabled);
        assert_eq!(
            ClientboundLoginResponseMode::Beta173Observed,
            config.login_response_mode
        );
        assert!(!config.playable_flat_world);
        assert_eq!(
            ServerConfig::DEFAULT_INITIAL_CHUNK_RADIUS,
            config.initial_chunk_radius
        );
    }

    #[test]
    fn allows_ephemeral_port_for_tests() {
        let config = ServerConfig::new("127.0.0.1", 0, "test-world").unwrap();

        assert_eq!(0, config.port);
    }

    #[test]
    fn rejects_invalid_config_values() {
        assert!(ServerConfig::new("", 25565, "world").is_err());
        assert!(ServerConfig::new("127.0.0.1", 25565, "").is_err());
        assert!(ServerConfig::with_options(
            "127.0.0.1",
            25565,
            "world",
            true,
            0,
            false,
            "-",
            false,
            ClientboundLoginResponseMode::Beta173Observed,
            false,
            ServerConfig::DEFAULT_INITIAL_CHUNK_RADIUS,
        )
        .is_err());
        assert!(ServerConfig::with_options(
            "127.0.0.1",
            25565,
            "world",
            true,
            4,
            false,
            "-",
            false,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            -1,
        )
        .is_err());
        assert!(parse_config(&["--port=65536"]).is_err());
    }

    #[test]
    fn parses_trace_continuation_flags() {
        let config = parse_config(&[
            "--host=127.0.0.1",
            "--port=0",
            "--trace-packets",
            "--trace-packet-limit=8",
            "--trace-continue-after-handshake",
            "--trace-handshake-response=-",
        ])
        .unwrap();

        assert_eq!("127.0.0.1", config.host);
        assert_eq!(0, config.port);
        assert!(config.packet_tracing_enabled);
        assert_eq!(8, config.packet_trace_limit);
        assert!(config.trace_continue_after_handshake);
        assert_eq!("-", config.trace_handshake_response);
    }

    #[test]
    fn compat_debug_enables_large_packet_trace_window() {
        let config = parse_config(&["--compat-debug"]).unwrap();

        assert!(config.packet_tracing_enabled);
        assert_eq!(512, config.packet_trace_limit);
    }

    #[test]
    fn parses_playable_flat_world_flags() {
        let config = parse_config(&[
            "--experimental-join",
            "--playable-flat-world",
            "--chunk-radius",
            "0",
        ])
        .unwrap();

        assert!(config.experimental_join_enabled);
        assert!(config.playable_flat_world);
        assert_eq!(0, config.initial_chunk_radius);
    }

    #[test]
    fn parses_login_response_modes() {
        let beta = parse_config(&[
            "--experimental-join",
            "--login-response-mode=beta173-observed",
        ])
        .unwrap();
        let legacy =
            parse_config(&["--experimental-join", "--login-response-mode=mcdevs-legacy"]).unwrap();

        assert!(beta.experimental_join_enabled);
        assert_eq!(
            ClientboundLoginResponseMode::Beta173Observed,
            beta.login_response_mode
        );
        assert_eq!(
            ClientboundLoginResponseMode::McdevsLegacy,
            legacy.login_response_mode
        );
        assert!(parse_config(&["--login-response-mode=nope"]).is_err());
    }

    #[test]
    fn bootstrap_binds_tcp_listener() {
        let config = ServerConfig::new("127.0.0.1", 0, "test-world").unwrap();
        let started = ServerBootstrap::new(config).start();

        match started {
            Ok(server) => {
                assert_eq!("127.0.0.1", server.local_addr().ip().to_string());
                assert_ne!(0, server.local_addr().port());
            }
            Err(ServerError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied => {}
            Err(error) => panic!("unexpected bootstrap failure: {error}"),
        }
    }

    #[test]
    fn formats_known_packet_trace_line() {
        let entry = trace::PacketTraceEntry::new(
            1,
            0x02,
            10,
            "00 04 00 41 00 6C 00 65 00 78",
            PacketDirection::ClientToServer,
            Some("Handshake".to_string()),
        )
        .unwrap();

        assert_eq!(
            "[trace] C->S #1 id=0x02 name=Handshake payloadLength=10 payloadHex=00 04 00 41 00 6C 00 65 00 78",
            trace::format_trace_entry(&entry)
        );
    }

    #[test]
    fn formats_unknown_packet_safely() {
        let entry =
            trace::PacketTraceEntry::new(2, 0x7E, 0, "", PacketDirection::ClientToServer, None)
                .unwrap();

        assert_eq!(
            "[trace] C->S #2 id=0x7E name=Unknown payloadLength=0 payloadHex=",
            trace::format_trace_entry(&entry)
        );
    }

    #[test]
    fn known_packet_ids_have_trace_names() {
        assert_eq!(
            Some("Handshake"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x02)
        );
        assert_eq!(
            Some("Login"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x01)
        );
        assert_eq!(
            Some("LoginResponse"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x01)
        );
        assert_eq!(
            Some("Player"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x0A)
        );
        assert_eq!(
            Some("PlayerPosition"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x0B)
        );
        assert_eq!(
            Some("PlayerLook"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x0C)
        );
        assert_eq!(
            Some("SpawnPosition"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x06)
        );
        assert_eq!(
            Some("PlayerPositionLook"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x0D)
        );
        assert_eq!(
            Some("SetChunkVisibility"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x32)
        );
        assert_eq!(
            Some("ChunkData"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x33)
        );
        assert_eq!(
            Some("Disconnect"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0xFF)
        );
        assert_eq!(
            Some("Chat"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x03)
        );
        assert_eq!(
            Some("PlayerDigging"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x0E)
        );
        assert_eq!(
            Some("PlayerBlockPlacement"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x0F)
        );
        assert_eq!(
            Some("HeldItemChange"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x10)
        );
        assert_eq!(
            Some("Animation"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x12)
        );
        assert_eq!(
            Some("EntityAction"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x13)
        );
        assert_eq!(
            Some("BlockChange"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x35)
        );
        assert_eq!(
            None,
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x7E)
        );
    }

    #[test]
    fn formats_payload_bytes_as_uppercase_hex() {
        assert_eq!(
            "00 0F 7E FF",
            trace::format_payload_hex(&[0x00, 0x0F, 0x7E, 0xFF])
        );
    }

    #[test]
    fn dispatcher_disconnects_after_plain_handshake() {
        let config = ServerConfig::new("127.0.0.1", 0, "test-world").unwrap();
        let dispatcher = FirstPacketDispatcher::new(&config);
        let mut stream = Duplex::new(encoded_handshake("Alex"));

        let outcome = dispatcher.handle(&mut stream).unwrap();

        assert_eq!(
            DispatchOutcome::Disconnected(HANDSHAKE_RECEIVED_DISCONNECT.to_string()),
            outcome
        );
        assert_eq!(
            HANDSHAKE_RECEIVED_DISCONNECT,
            decode_disconnect(&stream.written)
        );
    }

    #[test]
    fn dispatcher_reports_malformed_handshake() {
        let config = ServerConfig::new("127.0.0.1", 0, "test-world").unwrap();
        let dispatcher = FirstPacketDispatcher::new(&config);
        let mut stream = Duplex::new(vec![HandshakePacket::ID, 0x00, 0x04, 0x00, 0x41]);

        let outcome = dispatcher.handle(&mut stream).unwrap();

        assert_eq!(
            DispatchOutcome::Disconnected(MALFORMED_PACKET_DISCONNECT.to_string()),
            outcome
        );
        assert_eq!(
            MALFORMED_PACKET_DISCONNECT,
            decode_disconnect(&stream.written)
        );
    }

    #[test]
    fn dispatcher_trace_continuation_sends_handshake_response_and_reads_login() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            true,
            8,
            true,
            "-",
            false,
            ClientboundLoginResponseMode::Beta173Observed,
            false,
            ServerConfig::DEFAULT_INITIAL_CHUNK_RADIUS,
        )
        .unwrap();
        let dispatcher = FirstPacketDispatcher::new(&config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        let mut stream = Duplex::new(input);

        let outcome = dispatcher.handle(&mut stream).unwrap();

        assert_eq!(
            DispatchOutcome::Disconnected(LOGIN_RECEIVED_DISCONNECT.to_string()),
            outcome
        );
        let mut output = stream.written.as_slice();
        let handshake = LegacyPacketFrameCodec::read(&mut output, 4).unwrap();
        assert_eq!(HandshakePacket::ID, handshake.packet_id());
        assert_eq!(
            HandshakePacket::new("-"),
            HandshakePacketCodec::from_frame(handshake).unwrap()
        );
        assert_eq!(LOGIN_RECEIVED_DISCONNECT, decode_disconnect(output));
    }

    #[test]
    fn dispatcher_experimental_join_emits_provisional_packet_sequence() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            true,
            32,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            false,
            ServerConfig::DEFAULT_INITIAL_CHUNK_RADIUS,
        )
        .unwrap();
        let dispatcher = FirstPacketDispatcher::new(&config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        let mut stream = Duplex::new(input);

        let outcome = dispatcher.handle(&mut stream).unwrap();

        assert_eq!(DispatchOutcome::ExperimentalJoinStarted, outcome);
        let mut output = stream.written.as_slice();
        assert_eq!(
            HandshakePacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 4)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundLoginResponsePacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 15)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundSpawnPositionPacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 12)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundPlayerPositionLookPacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 41)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundChunkVisibilityPacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 9)
                .unwrap()
                .packet_id()
        );
        assert_eq!(ClientboundChunkDataPacket::ID, output[0]);
        assert!(!output[1..].is_empty());
    }

    #[test]
    fn player_session_rejects_protocol_mismatch() {
        let mut config = ServerConfig::default_config();
        config.trace_continue_after_handshake = true;
        config.experimental_join_enabled = true;
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login_with_protocol("Luxorium", 13));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(
            SessionExit::Disconnected(PROTOCOL_MISMATCH_DISCONNECT.to_string()),
            outcome
        );
        assert_eq!(ConnectionState::Disconnected, session.state());
    }

    #[test]
    fn playable_session_sends_spawn_area_and_updates_movement() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            true,
            64,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            1,
        )
        .unwrap();
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_position_look(
            17.5, 66.0, 67.62, -1.5, 90.0, 12.5, true,
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        let player = session.player().unwrap();
        assert_eq!("Luxorium", player.username);
        assert_eq!(17.5, player.x);
        assert_eq!(-1.5, player.z);
        assert_eq!(90.0, player.yaw);
        assert!(player.on_ground);
        assert_eq!(ChunkPos::new(1, -1), player.current_chunk);

        let mut output = stream.written.as_slice();
        assert_eq!(
            HandshakePacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 4)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundLoginResponsePacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 15)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundSpawnPositionPacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 12)
                .unwrap()
                .packet_id()
        );
        assert_eq!(
            ClientboundPlayerPositionLookPacket::ID,
            LegacyPacketFrameCodec::read(&mut output, 41)
                .unwrap()
                .packet_id()
        );

        let mut chunk_visibility_frames = 0;
        let mut chunk_data_frames = 0;
        while !output.is_empty() {
            let packet_id = output[0];
            if packet_id == ClientboundChunkVisibilityPacket::ID {
                let _ = LegacyPacketFrameCodec::read(&mut output, 9).unwrap();
                chunk_visibility_frames += 1;
            } else if packet_id == ClientboundChunkDataPacket::ID {
                output = skip_chunk_data_frame(output);
                chunk_data_frames += 1;
            } else {
                panic!("unexpected packet id {packet_id:#04x}");
            }
        }
        assert_eq!(14, chunk_visibility_frames);
        assert_eq!(14, chunk_data_frames);
    }

    #[test]
    fn playable_session_chunk_radius_zero_sends_only_spawn_chunk() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            false,
            64,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            0,
        )
        .unwrap();
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        let mut output = stream.written.as_slice();
        let _ = LegacyPacketFrameCodec::read(&mut output, 4).unwrap();
        let _ = LegacyPacketFrameCodec::read(&mut output, 15).unwrap();
        let _ = LegacyPacketFrameCodec::read(&mut output, 12).unwrap();
        let _ = LegacyPacketFrameCodec::read(&mut output, 41).unwrap();
        let _ = LegacyPacketFrameCodec::read(&mut output, 9).unwrap();
        output = skip_chunk_data_frame(output);
        assert!(output.is_empty());
    }

    #[test]
    fn chunks_in_radius_handles_radius_zero_one_and_negative_centers() {
        assert_eq!(
            vec![ChunkPos::new(0, 0)],
            chunks_in_radius(ChunkPos::new(0, 0), 0)
        );
        let chunks = chunks_in_radius(ChunkPos::new(-1, 2), 1);

        assert_eq!(9, chunks.len());
        assert!(chunks.contains(&ChunkPos::new(-2, 1)));
        assert!(chunks.contains(&ChunkPos::new(-1, 2)));
        assert!(chunks.contains(&ChunkPos::new(0, 3)));
    }

    #[test]
    fn placement_face_offsets_match_beta_direction_ids() {
        assert_eq!(Some((0, -1, 0)), placement_face_offset(0));
        assert_eq!(Some((0, 1, 0)), placement_face_offset(1));
        assert_eq!(Some((0, 0, -1)), placement_face_offset(2));
        assert_eq!(Some((0, 0, 1)), placement_face_offset(3));
        assert_eq!(Some((-1, 0, 0)), placement_face_offset(4));
        assert_eq!(Some((1, 0, 0)), placement_face_offset(5));
        assert_eq!(None, placement_face_offset(-1));
        assert_eq!(
            Some(BlockPos::new(10, 65, -4)),
            placement_target_pos(BlockPos::new(10, 64, -4), 1)
        );
    }

    #[test]
    fn movement_within_same_chunk_does_not_resend_chunks() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            false,
            64,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            1,
        )
        .unwrap();
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_position_look(
            1.5, 66.0, 67.62, 1.5, 0.0, 0.0, true,
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        let (_, chunk_data_frames) = count_chunk_frames(&stream.written);
        assert_eq!(9, chunk_data_frames);
    }

    #[test]
    fn negative_chunk_movement_streams_missing_chunks() {
        let config = ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            false,
            64,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            1,
        )
        .unwrap();
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_position_look(
            -17.5, 66.0, 67.62, -1.5, 0.0, 0.0, true,
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            ChunkPos::new(-2, -1),
            session.player().unwrap().current_chunk
        );
        let (_, chunk_data_frames) = count_chunk_frames(&stream.written);
        assert_eq!(16, chunk_data_frames);
    }

    #[test]
    fn joined_animation_entity_action_and_hotbar_do_not_disconnect() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_held_item_change(4));
        input.extend(encoded_animation(1, 1));
        input.extend(encoded_entity_action(1, 1));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        let player = session.player().unwrap();
        assert_eq!(4, player.selected_hotbar_slot);
        assert!(player.crouching);
    }

    #[test]
    fn player_digging_finished_breaks_visible_block_and_sends_block_change() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_digging(2, 0, 63, 0, 1));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            BlockState::AIR,
            state.lock().unwrap().block_at(BlockPos::new(0, 63, 0))
        );
        assert!(
            block_changes(&stream.written).contains(&ClientboundBlockChangePacket {
                x: 0,
                y: 63,
                z: 0,
                block_type: 0,
                metadata: 0,
            })
        );
    }

    #[test]
    fn player_block_placement_places_block_on_target_face_and_sends_block_change() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_block_placement(0, 63, 0, 1, Some((3, 1, 0))));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            BlockState::DIRT,
            state.lock().unwrap().block_at(BlockPos::new(0, 64, 0))
        );
        assert!(
            block_changes(&stream.written).contains(&ClientboundBlockChangePacket {
                x: 0,
                y: 64,
                z: 0,
                block_type: 3,
                metadata: 0,
            })
        );
    }

    #[test]
    fn special_item_use_placement_is_ignored_without_disconnect() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_block_placement(-1, 255, -1, -1, None));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert!(block_changes(&stream.written).is_empty());
    }

    #[test]
    fn empty_placement_does_not_consume_following_position_look_packet_id() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_block_placement(-1, 255, -1, -1, None));
        input.extend(encoded_player_position_look(
            2.5, 66.0, 67.62, 3.5, 45.0, 10.0, true,
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        let player = session.player().unwrap();
        assert_eq!(2.5, player.x);
        assert_eq!(3.5, player.z);
        assert_eq!(45.0, player.yaw);
        assert!(player.on_ground);
    }

    #[test]
    fn two_sessions_allocate_unique_entity_ids_and_unregister_on_close() {
        let state = GameServerState::shared_flat();
        let mut first = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut second = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut first_input = encoded_handshake("Alex");
        first_input.extend(encoded_login("Alex"));
        let mut second_input = encoded_handshake("Steve");
        second_input.extend(encoded_login("Steve"));

        assert_eq!(
            SessionExit::ClientClosed,
            first.run(&mut Duplex::new(first_input)).unwrap()
        );
        assert_eq!(
            SessionExit::ClientClosed,
            second.run(&mut Duplex::new(second_input)).unwrap()
        );

        assert_ne!(
            first.player().unwrap().entity_id,
            second.player().unwrap().entity_id
        );
        let state = state.lock().unwrap();
        assert_eq!(0, state.player_count());
        assert_eq!(0, state.entity_count());
    }

    #[test]
    fn shared_state_world_mutation_persists() {
        let state = GameServerState::shared_flat();
        let pos = BlockPos::new(-1, 64, -1);
        {
            let mut state = state.lock().unwrap();
            assert!(state.place_block(pos, BlockState::DIRT));
        }
        {
            let mut state = state.lock().unwrap();
            assert_eq!(BlockState::DIRT, state.block_at(pos));
            assert!(state.break_block(pos));
            assert_eq!(BlockState::AIR, state.block_at(pos));
        }
    }

    #[test]
    fn passive_mob_scaffold_allocates_server_side_entities() {
        let mut state = GameServerState::new_flat();

        let spawned = state.spawn_passive_mobs_near_spawn();

        assert_eq!(2, spawned.len());
        assert_eq!(2, state.entity_count());
        assert_ne!(spawned[0], spawned[1]);
    }

    #[test]
    fn tick_once_advances_shared_world_time() {
        let state = GameServerState::shared_flat();

        ServerTickLoop::tick_once(&state);
        ServerTickLoop::tick_once(&state);

        assert_eq!(2, state.lock().unwrap().world_time());
    }

    #[test]
    fn tick_loop_starts_and_stops() {
        let state = GameServerState::shared_flat();
        let mut tick_loop = ServerTickLoop::start(Arc::clone(&state));

        std::thread::sleep(Duration::from_millis(70));
        tick_loop.stop().unwrap();

        assert!(state.lock().unwrap().world_time() > 0);
    }

    #[test]
    fn undocumented_chat_packet_disconnects_cleanly_without_panic() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.push(0x03);
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(
            SessionExit::Disconnected(UNDOCUMENTED_PACKET_DISCONNECT.to_string()),
            outcome
        );
        assert_eq!(ConnectionState::Disconnected, session.state());
    }

    struct Duplex {
        read: Cursor<Vec<u8>>,
        written: Vec<u8>,
    }

    impl Duplex {
        fn new(read: Vec<u8>) -> Self {
            Self {
                read: Cursor::new(read),
                written: Vec::new(),
            }
        }
    }

    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.read.read(buf)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn encoded_handshake(username: &str) -> Vec<u8> {
        let frame = HandshakePacketCodec::to_frame(&HandshakePacket::new(username)).unwrap();
        let mut bytes = Vec::new();
        LegacyPacketFrameCodec::write(&frame, &mut bytes).unwrap();
        bytes
    }

    fn encoded_login(username: &str) -> Vec<u8> {
        encoded_login_with_protocol(username, aurelia_protocol::EXPECTED_PROTOCOL_VERSION)
    }

    fn encoded_login_with_protocol(username: &str, protocol_version: i32) -> Vec<u8> {
        let packet = ServerboundLoginPacket {
            protocol_version,
            username: username.to_string(),
            unused_or_seed: 0,
            dimension: 0,
        };
        let frame = ServerboundLoginPacketCodec::to_frame(&packet).unwrap();
        let mut bytes = Vec::new();
        LegacyPacketFrameCodec::write(&frame, &mut bytes).unwrap();
        bytes
    }

    fn playable_config(chunk_radius: i32) -> ServerConfig {
        ServerConfig::with_options(
            "127.0.0.1",
            0,
            "test-world",
            false,
            64,
            true,
            "-",
            true,
            ClientboundLoginResponseMode::Beta173Observed,
            true,
            chunk_radius,
        )
        .unwrap()
    }

    fn encoded_player_position_look(
        x: f64,
        y: f64,
        stance: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> Vec<u8> {
        let mut bytes = vec![0x0D];
        aurelia_protocol::write_f64(&mut bytes, x).unwrap();
        aurelia_protocol::write_f64(&mut bytes, y).unwrap();
        aurelia_protocol::write_f64(&mut bytes, stance).unwrap();
        aurelia_protocol::write_f64(&mut bytes, z).unwrap();
        aurelia_protocol::write_f32(&mut bytes, yaw).unwrap();
        aurelia_protocol::write_f32(&mut bytes, pitch).unwrap();
        aurelia_protocol::write_bool(&mut bytes, on_ground).unwrap();
        bytes
    }

    fn encoded_held_item_change(slot: i16) -> Vec<u8> {
        let mut bytes = vec![ServerboundHeldItemChangePacket::ID];
        aurelia_protocol::write_i16(&mut bytes, slot).unwrap();
        bytes
    }

    fn encoded_animation(entity_id: i32, animation: i8) -> Vec<u8> {
        let mut bytes = vec![ServerboundAnimationPacket::ID];
        aurelia_protocol::write_i32(&mut bytes, entity_id).unwrap();
        aurelia_protocol::write_i8(&mut bytes, animation).unwrap();
        bytes
    }

    fn encoded_entity_action(entity_id: i32, action_id: i8) -> Vec<u8> {
        let mut bytes = vec![ServerboundEntityActionPacket::ID];
        aurelia_protocol::write_i32(&mut bytes, entity_id).unwrap();
        aurelia_protocol::write_i8(&mut bytes, action_id).unwrap();
        bytes
    }

    fn encoded_player_digging(status: i8, x: i32, y: u8, z: i32, face: i8) -> Vec<u8> {
        let mut bytes = vec![ServerboundPlayerDiggingPacket::ID];
        aurelia_protocol::write_i8(&mut bytes, status).unwrap();
        aurelia_protocol::write_i32(&mut bytes, x).unwrap();
        aurelia_protocol::write_u8(&mut bytes, y).unwrap();
        aurelia_protocol::write_i32(&mut bytes, z).unwrap();
        aurelia_protocol::write_i8(&mut bytes, face).unwrap();
        bytes
    }

    fn encoded_player_block_placement(
        x: i32,
        y: u8,
        z: i32,
        direction: i8,
        held_item: Option<(i16, u8, i16)>,
    ) -> Vec<u8> {
        let mut bytes = vec![ServerboundPlayerBlockPlacementPacket::ID];
        aurelia_protocol::write_i32(&mut bytes, x).unwrap();
        aurelia_protocol::write_u8(&mut bytes, y).unwrap();
        aurelia_protocol::write_i32(&mut bytes, z).unwrap();
        aurelia_protocol::write_i8(&mut bytes, direction).unwrap();
        if let Some((item_id, count, damage)) = held_item {
            aurelia_protocol::write_i16(&mut bytes, item_id).unwrap();
            aurelia_protocol::write_u8(&mut bytes, count).unwrap();
            aurelia_protocol::write_i16(&mut bytes, damage).unwrap();
        } else {
            aurelia_protocol::write_i16(&mut bytes, -1).unwrap();
        }
        bytes
    }

    fn skip_chunk_data_frame(mut output: &[u8]) -> &[u8] {
        assert_eq!(ClientboundChunkDataPacket::ID, output[0]);
        output = &output[1..];
        assert!(output.len() >= 17);
        let compressed_size = i32::from_be_bytes(output[13..17].try_into().unwrap()) as usize;
        &output[17 + compressed_size..]
    }

    fn count_chunk_frames(bytes: &[u8]) -> (usize, usize) {
        let mut output = bytes;
        let mut visibility = 0;
        let mut data = 0;
        while !output.is_empty() {
            match output[0] {
                HandshakePacket::ID => {
                    output = &output[1 + 4..];
                }
                ClientboundLoginResponsePacket::ID => {
                    output = &output[1 + 15..];
                }
                ClientboundSpawnPositionPacket::ID => {
                    output = &output[1 + 12..];
                }
                ClientboundPlayerPositionLookPacket::ID => {
                    output = &output[1 + 41..];
                }
                ClientboundChunkVisibilityPacket::ID => {
                    visibility += 1;
                    output = &output[1 + 9..];
                }
                ClientboundChunkDataPacket::ID => {
                    data += 1;
                    output = skip_chunk_data_frame(output);
                }
                packet_id => panic!("unexpected packet id {packet_id:#04x}"),
            }
        }
        (visibility, data)
    }

    fn block_changes(bytes: &[u8]) -> Vec<ClientboundBlockChangePacket> {
        let mut output = bytes;
        let mut changes = Vec::new();
        while !output.is_empty() {
            match output[0] {
                HandshakePacket::ID => {
                    output = &output[1 + 4..];
                }
                ClientboundLoginResponsePacket::ID => {
                    output = &output[1 + 15..];
                }
                ClientboundSpawnPositionPacket::ID => {
                    output = &output[1 + 12..];
                }
                ClientboundPlayerPositionLookPacket::ID => {
                    output = &output[1 + 41..];
                }
                ClientboundChunkVisibilityPacket::ID => {
                    output = &output[1 + 9..];
                }
                ClientboundChunkDataPacket::ID => {
                    output = skip_chunk_data_frame(output);
                }
                ClientboundBlockChangePacket::ID => {
                    let payload = &output[1..16];
                    changes.push(ClientboundBlockChangePacket {
                        x: i32::from_be_bytes(payload[0..4].try_into().unwrap()),
                        y: payload[4],
                        z: i32::from_be_bytes(payload[5..9].try_into().unwrap()),
                        block_type: i16::from_be_bytes(payload[9..11].try_into().unwrap()),
                        metadata: i32::from_be_bytes(payload[11..15].try_into().unwrap()),
                    });
                    output = &output[16..];
                }
                packet_id => panic!("unexpected packet id {packet_id:#04x}"),
            }
        }
        changes
    }

    fn decode_disconnect(bytes: &[u8]) -> String {
        let mut input = bytes;
        assert_eq!(DisconnectPacket::ID, read_u8(&mut input).unwrap());
        DisconnectPacketCodec::decode(&mut input).unwrap().reason
    }
}
