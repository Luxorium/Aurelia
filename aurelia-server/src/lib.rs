use aurelia_common::{BlockPos, ChunkPos};
use aurelia_protocol::{
    experimental_flat_chunk_data, read_u8, ChatPacket, ClientboundBeta173TimeUpdatePacket,
    ClientboundBeta173TimeUpdatePacketCodec, ClientboundBlockChangePacket,
    ClientboundBlockChangePacketCodec, ClientboundChunkDataPacket, ClientboundChunkDataPacketCodec,
    ClientboundChunkVisibilityPacket, ClientboundChunkVisibilityPacketCodec,
    ClientboundConfirmTransactionPacket, ClientboundConfirmTransactionPacketCodec,
    ClientboundLoginResponseMode, ClientboundLoginResponsePacket,
    ClientboundLoginResponsePacketCodec, ClientboundPlayerPositionLookPacket,
    ClientboundPlayerPositionLookPacketCodec, ClientboundSetSlotPacket,
    ClientboundSetSlotPacketCodec, ClientboundSetWindowItemsPacket,
    ClientboundSetWindowItemsPacketCodec, ClientboundSpawnPositionPacket,
    ClientboundSpawnPositionPacketCodec, DisconnectPacket, DisconnectPacketCodec, HandshakePacket,
    HandshakePacketCodec, KeepAlivePacket, LegacyPacketFrameCodec, LegacySlotData, PacketCodec,
    PacketDirection, PacketFrame, ProtocolError, ServerboundAnimationPacket,
    ServerboundCloseWindowPacket, ServerboundConfirmTransactionPacket,
    ServerboundEntityActionPacket, ServerboundHeldItemChangePacket, ServerboundLoginPacket,
    ServerboundLoginPacketCodec, ServerboundMovementPacket, ServerboundPacketKind,
    ServerboundPlayerBlockPlacementPacket, ServerboundPlayerDiggingPacket,
    ServerboundWindowClickPacket, EXPECTED_PROTOCOL_VERSION, TARGET_VERSION,
};
use aurelia_region::RegionScheduler;
use aurelia_world::{
    BlockState, Chunk, EntityId, EntityKind, EntityManager, FlatWorldGenerator,
    InMemoryWorldStorage, World,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const VERSION: &str = "0.1.3-SNAPSHOT";
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
            0x04 => Some("TimeUpdate"),
            id if id == ClientboundSpawnPositionPacket::ID => Some("SpawnPosition"),
            id if id == ClientboundPlayerPositionLookPacket::ID => Some("PlayerPositionLook"),
            id if id == ClientboundChunkVisibilityPacket::ID => Some("SetChunkVisibility"),
            id if id == ClientboundChunkDataPacket::ID => Some("ChunkData"),
            id if id == ClientboundBlockChangePacket::ID => Some("BlockChange"),
            0x67 => Some("SetSlot"),
            0x68 => Some("SetWindowItems"),
            id if id == HandshakePacket::ID => Some("Handshake"),
            id if id == DisconnectPacket::ID => Some("Disconnect"),
            0x0E => Some("PlayerDigging"),
            0x0F => Some("PlayerBlockPlacement"),
            0x10 => Some("HeldItemChange"),
            0x12 => Some("Animation"),
            0x13 => Some("EntityAction"),
            0x65 => Some("CloseWindow"),
            0x66 => Some("WindowClick"),
            0x6A => Some("ConfirmTransaction"),
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
    pub inventory_sync_enabled: bool,
    pub time_update_enabled: bool,
    pub keepalive_enabled: bool,
    pub defer_inventory_sync: bool,
    pub post_join_minimal: bool,
}

impl ServerConfig {
    pub const DEFAULT_PACKET_TRACE_LIMIT: usize = 4;
    pub const DEFAULT_TRACE_HANDSHAKE_RESPONSE: &'static str = "-";
    pub const DEFAULT_INITIAL_CHUNK_RADIUS: i32 = 1;
    pub const DEFAULT_DEFERRED_INVENTORY_MOVEMENTS: u32 = 3;

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
        Self::with_options_and_features(
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
            true,
            true,
            true,
            true,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options_and_features(
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
        inventory_sync_enabled: bool,
        time_update_enabled: bool,
        keepalive_enabled: bool,
        defer_inventory_sync: bool,
        post_join_minimal: bool,
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
            inventory_sync_enabled,
            time_update_enabled,
            keepalive_enabled,
            defer_inventory_sync,
            post_join_minimal,
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
            inventory_sync_enabled: true,
            time_update_enabled: true,
            keepalive_enabled: true,
            defer_inventory_sync: true,
            post_join_minimal: false,
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
    let mut inventory_sync_enabled = defaults.inventory_sync_enabled;
    let mut time_update_enabled = defaults.time_update_enabled;
    let mut keepalive_enabled = defaults.keepalive_enabled;
    let mut defer_inventory_sync = defaults.defer_inventory_sync;
    let mut post_join_minimal = defaults.post_join_minimal;

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
        } else if arg == "--no-inventory-sync" {
            inventory_sync_enabled = false;
        } else if arg == "--no-time-update" {
            time_update_enabled = false;
        } else if arg == "--no-keepalive" {
            keepalive_enabled = false;
        } else if arg == "--defer-inventory-sync" {
            defer_inventory_sync = true;
        } else if arg == "--post-join-minimal" {
            post_join_minimal = true;
        }
        index += 1;
    }

    ServerConfig::with_options_and_features(
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
        inventory_sync_enabled,
        time_update_enabled,
        keepalive_enabled,
        defer_inventory_sync,
        post_join_minimal,
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

fn world_save_dir(config: &ServerConfig) -> PathBuf {
    Path::new(&config.world_name).join("aurelia-flat-v1")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking,
    Login,
    Joined,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinPhase {
    Handshaking,
    Login,
    SendingInitialWorld,
    AwaitingFirstClientMovement,
    JoinedReady,
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
    world_save_dir: Option<PathBuf>,
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
            world_save_dir: None,
        }
    }

    pub fn shared_flat() -> SharedGameServerState {
        Arc::new(Mutex::new(Self::new_flat()))
    }

    pub fn new_flat_persistent(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let storage = InMemoryWorldStorage::load_from_dir(&path)?;
        Ok(Self {
            world: World::new(storage, FlatWorldGenerator),
            entities: EntityManager::default(),
            players: HashMap::new(),
            world_save_dir: Some(path),
        })
    }

    pub fn shared_flat_persistent(path: impl Into<PathBuf>) -> Result<SharedGameServerState> {
        Ok(Arc::new(Mutex::new(Self::new_flat_persistent(path)?)))
    }

    pub fn tick(&mut self) {
        self.world.tick();
    }

    pub fn world_time(&self) -> u64 {
        self.world.time()
    }

    pub fn set_world_time(&mut self, time: u64) {
        self.world.set_time(time);
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

    pub fn chunk_snapshot(&mut self, pos: ChunkPos) -> Chunk {
        self.world.chunk_snapshot(pos)
    }

    pub fn dirty_chunk_count(&self) -> usize {
        self.world.dirty_chunk_count()
    }

    pub fn save_dirty_chunks(&mut self) -> Result<usize> {
        let Some(path) = self.world_save_dir.as_ref() else {
            return Ok(0);
        };
        Ok(self.world.save_dirty_chunks(path)?)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInventory {
    slots: Vec<LegacySlotData>,
    cursor: LegacySlotData,
    selected_hotbar_slot: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowClickUpdate {
    pub accepted: bool,
    pub changed_slots: Vec<i16>,
    pub cursor_changed: bool,
}

impl PlayerInventory {
    pub const WINDOW_ID: i8 = 0;
    pub const WINDOW_SLOT_COUNT: usize = 45;
    pub const HOTBAR_START_SLOT: usize = 36;
    pub const HOTBAR_LEN: usize = 9;
    pub const CURSOR_WINDOW_ID: i8 = -1;
    pub const CURSOR_SLOT: i16 = -1;
    pub const DROP_SLOT: i16 = -999;
    const MAX_STACK_SIZE: u8 = 64;

    pub fn starter() -> Self {
        let mut inventory = Self {
            slots: vec![LegacySlotData::Empty; Self::WINDOW_SLOT_COUNT],
            cursor: LegacySlotData::Empty,
            selected_hotbar_slot: 0,
        };
        inventory.set_hotbar_stack(0, stack(3, 64, 0));
        inventory.set_hotbar_stack(1, stack(4, 64, 0));
        inventory.set_hotbar_stack(2, stack(5, 64, 0));
        inventory.set_hotbar_stack(3, stack(50, 64, 0));
        inventory.set_hotbar_stack(4, stack(270, 1, 0));
        inventory
    }

    pub fn slots(&self) -> &[LegacySlotData] {
        &self.slots
    }

    pub const fn cursor(&self) -> LegacySlotData {
        self.cursor
    }

    pub fn set_selected_hotbar_slot(&mut self, slot: u8) {
        if slot < Self::HOTBAR_LEN as u8 {
            self.selected_hotbar_slot = slot;
        }
    }

    pub const fn selected_hotbar_slot(&self) -> u8 {
        self.selected_hotbar_slot
    }

    pub fn selected_window_slot(&self) -> i16 {
        hotbar_index_to_window_slot(self.selected_hotbar_slot).unwrap_or(0)
    }

    pub fn selected_stack(&self) -> LegacySlotData {
        self.slots[self.selected_window_slot() as usize]
    }

    pub fn set_hotbar_stack(&mut self, hotbar_slot: u8, stack: LegacySlotData) {
        if let Some(slot) = hotbar_index_to_window_slot(hotbar_slot) {
            self.slots[slot as usize] = stack;
        }
    }

    pub fn placeable_selected_block(&self) -> Option<BlockState> {
        let LegacySlotData::Present {
            item_id,
            count,
            damage,
        } = self.selected_stack()
        else {
            return None;
        };
        if count == 0 || !is_placeable_block_id(item_id) {
            return None;
        }
        Some(BlockState::new_unchecked(
            item_id as u8,
            (damage & 0x0F) as u8,
        ))
    }

    pub fn decrement_selected_stack(&mut self) -> Option<i16> {
        let slot = self.selected_window_slot();
        decrement_slot(&mut self.slots[slot as usize], 1).then_some(slot)
    }

    pub fn add_drop(&mut self, item_id: i16, count: u8, damage: i16) -> Vec<i16> {
        if count == 0 {
            return Vec::new();
        }

        let mut remaining = count;
        let mut changed = Vec::new();
        for (index, slot) in self.slots.iter_mut().enumerate().skip(9) {
            if remaining == 0 {
                break;
            }
            let LegacySlotData::Present {
                item_id: slot_item,
                count: slot_count,
                damage: slot_damage,
            } = slot
            else {
                continue;
            };
            if *slot_item == item_id && *slot_damage == damage && *slot_count < Self::MAX_STACK_SIZE
            {
                let added = (Self::MAX_STACK_SIZE - *slot_count).min(remaining);
                *slot_count += added;
                remaining -= added;
                changed.push(index as i16);
            }
        }

        for (index, slot) in self.slots.iter_mut().enumerate().skip(9) {
            if remaining == 0 {
                break;
            }
            if *slot == LegacySlotData::Empty {
                let added = Self::MAX_STACK_SIZE.min(remaining);
                *slot = stack(item_id, added, damage);
                remaining -= added;
                changed.push(index as i16);
            }
        }
        changed
    }

    pub fn handle_window_click(
        &mut self,
        packet: ServerboundWindowClickPacket,
    ) -> WindowClickUpdate {
        if packet.window_id != Self::WINDOW_ID || packet.shift {
            return WindowClickUpdate::rejected();
        }
        if packet.slot == Self::DROP_SLOT {
            let changed = self.cursor != LegacySlotData::Empty;
            self.cursor = LegacySlotData::Empty;
            return WindowClickUpdate {
                accepted: true,
                changed_slots: Vec::new(),
                cursor_changed: changed,
            };
        }
        if packet.slot < 0 || packet.slot as usize >= self.slots.len() {
            return WindowClickUpdate::rejected();
        }

        let slot = packet.slot as usize;
        match packet.mouse_button {
            0 => self.left_click(slot),
            1 => self.right_click(slot),
            _ => WindowClickUpdate::rejected(),
        }
    }

    fn left_click(&mut self, slot: usize) -> WindowClickUpdate {
        let before_slot = self.slots[slot];
        let before_cursor = self.cursor;
        match (self.cursor, self.slots[slot]) {
            (LegacySlotData::Empty, slot_stack) => {
                self.cursor = slot_stack;
                self.slots[slot] = LegacySlotData::Empty;
            }
            (cursor_stack, LegacySlotData::Empty) => {
                self.slots[slot] = cursor_stack;
                self.cursor = LegacySlotData::Empty;
            }
            (
                LegacySlotData::Present {
                    item_id,
                    count,
                    damage,
                },
                LegacySlotData::Present {
                    item_id: slot_item_id,
                    count: slot_count,
                    damage: slot_damage,
                },
            ) if item_id == slot_item_id && damage == slot_damage => {
                let space = Self::MAX_STACK_SIZE.saturating_sub(slot_count);
                let moved = space.min(count);
                self.slots[slot] = stack(slot_item_id, slot_count + moved, slot_damage);
                self.cursor = if moved == count {
                    LegacySlotData::Empty
                } else {
                    stack(item_id, count - moved, damage)
                };
            }
            _ => {
                std::mem::swap(&mut self.cursor, &mut self.slots[slot]);
            }
        }
        self.click_update(slot, before_slot, before_cursor)
    }

    fn right_click(&mut self, slot: usize) -> WindowClickUpdate {
        let before_slot = self.slots[slot];
        let before_cursor = self.cursor;
        match (self.cursor, self.slots[slot]) {
            (
                LegacySlotData::Empty,
                LegacySlotData::Present {
                    item_id,
                    count,
                    damage,
                },
            ) => {
                let taken = (count + 1) / 2;
                self.cursor = stack(item_id, taken, damage);
                self.slots[slot] = if taken == count {
                    LegacySlotData::Empty
                } else {
                    stack(item_id, count - taken, damage)
                };
            }
            (
                LegacySlotData::Present {
                    item_id,
                    count,
                    damage,
                },
                LegacySlotData::Empty,
            ) => {
                self.slots[slot] = stack(item_id, 1, damage);
                self.cursor = if count == 1 {
                    LegacySlotData::Empty
                } else {
                    stack(item_id, count - 1, damage)
                };
            }
            (
                LegacySlotData::Present {
                    item_id,
                    count,
                    damage,
                },
                LegacySlotData::Present {
                    item_id: slot_item_id,
                    count: slot_count,
                    damage: slot_damage,
                },
            ) if item_id == slot_item_id
                && damage == slot_damage
                && slot_count < Self::MAX_STACK_SIZE =>
            {
                self.slots[slot] = stack(slot_item_id, slot_count + 1, slot_damage);
                self.cursor = if count == 1 {
                    LegacySlotData::Empty
                } else {
                    stack(item_id, count - 1, damage)
                };
            }
            _ => {
                std::mem::swap(&mut self.cursor, &mut self.slots[slot]);
            }
        }
        self.click_update(slot, before_slot, before_cursor)
    }

    fn click_update(
        &self,
        slot: usize,
        before_slot: LegacySlotData,
        before_cursor: LegacySlotData,
    ) -> WindowClickUpdate {
        let mut changed_slots = Vec::new();
        if self.slots[slot] != before_slot {
            changed_slots.push(slot as i16);
        }
        WindowClickUpdate {
            accepted: true,
            changed_slots,
            cursor_changed: self.cursor != before_cursor,
        }
    }
}

impl WindowClickUpdate {
    pub fn rejected() -> Self {
        Self {
            accepted: false,
            changed_slots: Vec::new(),
            cursor_changed: false,
        }
    }
}

fn stack(item_id: i16, count: u8, damage: i16) -> LegacySlotData {
    if count == 0 {
        LegacySlotData::Empty
    } else {
        LegacySlotData::Present {
            item_id,
            count,
            damage,
        }
    }
}

fn decrement_slot(slot: &mut LegacySlotData, amount: u8) -> bool {
    let LegacySlotData::Present {
        item_id,
        count,
        damage,
    } = *slot
    else {
        return false;
    };
    if count <= amount {
        *slot = LegacySlotData::Empty;
    } else {
        *slot = stack(item_id, count - amount, damage);
    }
    true
}

pub fn hotbar_index_to_window_slot(index: u8) -> Option<i16> {
    (index < PlayerInventory::HOTBAR_LEN as u8)
        .then_some((PlayerInventory::HOTBAR_START_SLOT + index as usize) as i16)
}

pub fn window_slot_to_hotbar_index(slot: i16) -> Option<u8> {
    if slot < 0 {
        return None;
    }
    let slot = slot as usize;
    (PlayerInventory::HOTBAR_START_SLOT
        ..PlayerInventory::HOTBAR_START_SLOT + PlayerInventory::HOTBAR_LEN)
        .contains(&slot)
        .then(|| (slot - PlayerInventory::HOTBAR_START_SLOT) as u8)
}

fn slot_data_summary(slot: LegacySlotData) -> String {
    match slot {
        LegacySlotData::Empty => "empty".to_string(),
        LegacySlotData::Present {
            item_id,
            count,
            damage,
        } => {
            if damage == 0 {
                format!("{item_id}x{count}")
            } else {
                format!("{item_id}x{count}:{damage}")
            }
        }
    }
}

fn selected_placeable_block(inventory: PlacementInventorySnapshot) -> BlockState {
    let LegacySlotData::Present {
        item_id, damage, ..
    } = inventory.selected_stack
    else {
        return BlockState::AIR;
    };
    BlockState::new_unchecked(item_id as u8, (damage & 0x0F) as u8)
}

fn is_placeable_block_id(item_id: i16) -> bool {
    matches!(
        item_id,
        1..=6 | 12..=24 | 35 | 41..=50 | 53..=58 | 61..=67 | 79..=91
    )
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
    pub inventory: PlayerInventory,
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
            inventory: PlayerInventory::starter(),
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
            self.inventory.set_selected_hotbar_slot(slot);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlacementInventorySnapshot {
    hotbar_index: u8,
    window_slot: i16,
    selected_stack: LegacySlotData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlacementRejection {
    NoSelectedItem,
    SelectedNotPlaceable,
    TargetUnloaded,
    TargetInvalidY,
    TargetNotAir,
    ClickedBlockMissing,
    OutOfReach,
}

impl PlacementRejection {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NoSelectedItem => "no-selected-item",
            Self::SelectedNotPlaceable => "selected-not-placeable",
            Self::TargetUnloaded => "target-unloaded",
            Self::TargetInvalidY => "target-invalid-y",
            Self::TargetNotAir => "target-not-air",
            Self::ClickedBlockMissing => "clicked-block-missing",
            Self::OutOfReach => "out-of-reach",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSession {
    config: ServerConfig,
    state_ref: SharedGameServerState,
    state: ConnectionState,
    join_phase: JoinPhase,
    player: Option<PlayerState>,
    registered_username: Option<String>,
    sent_chunks: HashSet<ChunkPos>,
    last_packet_id: Option<u8>,
    last_clientbound_packet_id: Option<u8>,
    last_clientbound_payload_len: usize,
    trace_index: usize,
    last_packet_at: Instant,
    last_keepalive_sent_at: Instant,
    last_time_sent_at: Instant,
    joined_ready_movement_packets: u32,
    inventory_sync_sent: bool,
    next_keepalive_id: i32,
    pending_keepalive_id: Option<i32>,
}

impl PlayerSession {
    const READ_POLL_INTERVAL: Duration = Duration::from_millis(250);
    const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
    const TIME_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
    const CLIENT_TIMEOUT: Duration = Duration::from_secs(180);

    pub fn new(config: ServerConfig) -> Self {
        Self::with_state(config, GameServerState::shared_flat())
    }

    pub fn with_state(config: ServerConfig, state_ref: SharedGameServerState) -> Self {
        let now = Instant::now();
        Self {
            config,
            state_ref,
            state: ConnectionState::Handshaking,
            join_phase: JoinPhase::Handshaking,
            player: None,
            registered_username: None,
            sent_chunks: HashSet::new(),
            last_packet_id: None,
            last_clientbound_packet_id: None,
            last_clientbound_payload_len: 0,
            trace_index: 0,
            last_packet_at: now,
            last_keepalive_sent_at: now,
            last_time_sent_at: now,
            joined_ready_movement_packets: 0,
            inventory_sync_sent: false,
            next_keepalive_id: 1,
            pending_keepalive_id: None,
        }
    }

    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    pub const fn join_phase(&self) -> JoinPhase {
        self.join_phase
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
        self.join_phase = JoinPhase::Handshaking;
        let packet_id = loop {
            match read_u8(connection) {
                Ok(packet_id) => break packet_id,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    return self
                        .disconnect(connection, MISSING_PACKET_DISCONNECT)
                        .map(Some);
                }
                Err(ProtocolError::Io(error)) if is_read_timeout_error(&error) => {
                    if Instant::now().duration_since(self.last_packet_at) > Self::CLIENT_TIMEOUT {
                        return self
                            .disconnect(connection, MISSING_PACKET_DISCONNECT)
                            .map(Some);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            }
        };
        self.last_packet_id = Some(packet_id);
        self.last_packet_at = Instant::now();

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
                self.join_phase = JoinPhase::Login;
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

        let packet_id = loop {
            match read_u8(connection) {
                Ok(packet_id) => break packet_id,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    return self
                        .disconnect(connection, MISSING_PACKET_DISCONNECT)
                        .map(Some);
                }
                Err(ProtocolError::Io(error)) if is_read_timeout_error(&error) => {
                    if Instant::now().duration_since(self.last_packet_at) > Self::CLIENT_TIMEOUT {
                        return self
                            .disconnect(connection, MISSING_PACKET_DISCONNECT)
                            .map(Some);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            }
        };
        self.last_packet_id = Some(packet_id);
        self.last_packet_at = Instant::now();

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
            self.send_due_periodic_packets(connection)?;
            let packet_id = match read_u8(connection) {
                Ok(packet_id) => packet_id,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    self.save_dirty_chunks_for_shutdown();
                    self.unregister_player();
                    self.log_session_close("client closed connection");
                    return Ok(SessionExit::ClientClosed);
                }
                Err(ProtocolError::Io(error)) if is_read_timeout_error(&error) => {
                    if let Some(exit) = self.handle_idle_read(connection)? {
                        return Ok(exit);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            self.last_packet_id = Some(packet_id);
            self.last_packet_at = Instant::now();

            let packet_kind = ServerboundPacketKind::from_id(packet_id);
            if let Some(payload_len) = packet_kind.fixed_payload_length() {
                let mut payload = vec![0; payload_len];
                connection.read_exact(&mut payload)?;
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                match packet_kind {
                    ServerboundPacketKind::KeepAlive => {
                        let packet = KeepAlivePacket::decode(&mut payload.as_slice())?;
                        self.handle_keepalive(packet);
                    }
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
                            if self.join_phase == JoinPhase::AwaitingFirstClientMovement {
                                self.mark_joined_ready(connection)?;
                            } else if self.join_phase == JoinPhase::JoinedReady {
                                self.joined_ready_movement_packets =
                                    self.joined_ready_movement_packets.saturating_add(1);
                                self.send_deferred_inventory_if_due(connection)?;
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
                            if self.config.packet_tracing_enabled {
                                if let Some(snapshot) = self.placement_inventory_snapshot() {
                                    eprintln!(
                                        "[session] held-item-change hotbar={} windowSlot={} item={} player={}",
                                        snapshot.hotbar_index,
                                        snapshot.window_slot,
                                        slot_data_summary(snapshot.selected_stack),
                                        self.player_name_for_log()
                                    );
                                }
                            }
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
                    ServerboundPacketKind::CloseWindow => {
                        let packet = ServerboundCloseWindowPacket::decode(&mut payload.as_slice())?;
                        self.log_close_window(packet);
                    }
                    ServerboundPacketKind::ConfirmTransaction => {
                        let packet =
                            ServerboundConfirmTransactionPacket::decode(&mut payload.as_slice())?;
                        self.log_confirm_transaction(packet);
                    }
                    _ => {}
                }
                continue;
            }

            if packet_id == ChatPacket::ID {
                let packet = ChatPacket::decode(connection)?;
                let mut payload = Vec::new();
                packet.encode(&mut payload)?;
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                self.handle_chat(packet, connection)?;
                continue;
            }

            if packet_id == ServerboundWindowClickPacket::ID {
                let packet = ServerboundWindowClickPacket::decode(connection)?;
                let mut payload = Vec::new();
                packet.encode(&mut payload)?;
                self.trace_packet(PacketDirection::ClientToServer, packet_id, &payload);
                self.handle_window_click(packet, connection)?;
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
                self.save_dirty_chunks_for_shutdown();
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

        self.join_phase = JoinPhase::SendingInitialWorld;
        eprintln!(
            "[session] join phase: sending initial chunks player={} entity={} chunkRadius={}",
            self.player_name_for_log(),
            entity_id,
            self.chunk_radius()
        );
        self.stream_chunks_for_player(connection)?;
        eprintln!(
            "[session] join phase: initial chunks complete player={} sentChunks={}",
            self.player_name_for_log(),
            self.sent_chunks.len()
        );
        self.join_phase = JoinPhase::AwaitingFirstClientMovement;
        eprintln!(
            "[session] join phase: waiting for first movement player={}",
            self.player_name_for_log()
        );
        eprintln!(
            "[session] join complete player={} entity={} chunkRadius={}",
            self.player_name_for_log(),
            entity_id,
            self.chunk_radius()
        );
        Ok(())
    }

    fn mark_joined_ready(&mut self, connection: &mut impl Write) -> Result<()> {
        if self.join_phase != JoinPhase::AwaitingFirstClientMovement {
            return Ok(());
        }
        self.join_phase = JoinPhase::JoinedReady;
        self.joined_ready_movement_packets = 0;
        self.inventory_sync_sent = false;
        eprintln!(
            "[session] join phase: joined ready player={} chunk={}",
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );

        if self.should_send_time_update() {
            self.send_time_update(connection)?;
            eprintln!(
                "[session] deferred time update sent player={} payloadLength={}",
                self.player_name_for_log(),
                self.last_clientbound_payload_len
            );
        }
        self.send_deferred_inventory_if_due(connection)?;
        if self.config.keepalive_enabled && !self.config.post_join_minimal {
            self.last_keepalive_sent_at = Instant::now();
            eprintln!(
                "[session] keepalive scheduler started player={}",
                self.player_name_for_log()
            );
        }
        Ok(())
    }

    fn send_deferred_inventory_if_due(&mut self, connection: &mut impl Write) -> Result<()> {
        if self.inventory_sync_sent || !self.should_send_inventory_sync() {
            return Ok(());
        }
        if self.config.defer_inventory_sync
            && self.joined_ready_movement_packets
                < ServerConfig::DEFAULT_DEFERRED_INVENTORY_MOVEMENTS
        {
            return Ok(());
        }
        self.send_inventory_window(connection)?;
        self.inventory_sync_sent = true;
        eprintln!(
            "[session] deferred inventory sync sent player={} payloadLength={} movementPackets={}",
            self.player_name_for_log(),
            self.last_clientbound_payload_len,
            self.joined_ready_movement_packets
        );
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
        self.last_clientbound_packet_id = Some(ClientboundLoginResponsePacket::ID);
        self.last_clientbound_payload_len = payload.len();
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
        self.last_clientbound_packet_id = Some(packet_id);
        self.last_clientbound_payload_len = payload.len();
        LegacyPacketFrameCodec::write(&PacketFrame::new(packet_id, payload), connection)?;
        Ok(())
    }

    fn write_chunk_pair(&mut self, connection: &mut impl Write, pos: ChunkPos) -> Result<()> {
        let chunk = self.with_game_state(|state| {
            state.ensure_chunk_loaded(pos);
            Ok(state.chunk_snapshot(pos))
        })?;
        let packet = chunk_data_packet_from_chunk(&chunk);
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
            ClientboundChunkDataPacketCodec::encode(&packet, payload)
        })?;
        Ok(())
    }

    fn handle_player_digging(
        &mut self,
        packet: ServerboundPlayerDiggingPacket,
        connection: &mut impl Write,
    ) -> Result<()> {
        let pos = BlockPos::new(packet.x, i32::from(packet.y), packet.z);
        let status = digging_status_name(packet.status);
        let valid = GameServerState::is_valid_block_pos(pos);
        let loaded = self.is_block_loaded_for_player(pos);
        let reachable = self.player_can_reach(pos);
        let current = valid
            .then(|| self.with_game_state(|state| Ok(state.block_at(pos))))
            .transpose()?
            .unwrap_or(BlockState::AIR);

        if packet.status != ServerboundPlayerDiggingPacket::FINISHED_DIGGING_STATUS {
            if valid && (!loaded || !reachable) {
                self.write_block_change(connection, pos, current)?;
            }
            self.log_digging(
                packet,
                pos,
                current,
                status,
                if valid && (!loaded || !reachable) {
                    "corrected"
                } else {
                    "ignored"
                },
                None,
            );
            return Ok(());
        }

        let reject_reason = if !valid {
            Some("invalid-y")
        } else if !loaded {
            Some("target-unloaded")
        } else if !reachable {
            Some("out-of-reach")
        } else if current == BlockState::AIR {
            Some("air")
        } else if current == BlockState::BEDROCK {
            Some("bedrock")
        } else {
            None
        };

        if let Some(reason) = reject_reason {
            if valid {
                self.write_block_change(connection, pos, current)?;
            }
            self.log_digging(packet, pos, current, status, "rejected", Some(reason));
            return Ok(());
        }

        let drop = drop_for_block(current);
        self.with_game_state(|state| {
            state.break_block(pos);
            Ok(())
        })?;
        self.write_block_change(connection, pos, BlockState::AIR)?;
        let mut drop_result = "none".to_string();
        if let (Some((item_id, count, damage)), Some(player)) = (drop, self.player.as_mut()) {
            let changed = player.inventory.add_drop(item_id, count, damage);
            for slot in changed.iter().copied() {
                self.write_inventory_slot(connection, PlayerInventory::WINDOW_ID, slot)?;
            }
            drop_result = if let Some(slot) = changed.first() {
                format!("{item_id}x{count} inventorySlot={slot}")
            } else {
                format!("{item_id}x{count} inventory-full")
            };
        }
        self.log_digging(
            packet,
            pos,
            current,
            status,
            "broken",
            Some(drop_result.as_str()),
        );
        Ok(())
    }

    fn handle_player_block_placement(
        &mut self,
        packet: ServerboundPlayerBlockPlacementPacket,
        connection: &mut impl Write,
    ) -> Result<()> {
        if packet.is_special_item_use() {
            self.log_item_use_air();
            return Ok(());
        }

        let clicked = BlockPos::new(packet.x, i32::from(packet.y), packet.z);
        let Some(target) = placement_target_pos(clicked, packet.direction) else {
            let clicked_state = self.with_game_state(|state| Ok(state.block_at(clicked)))?;
            self.write_block_change(connection, clicked, clicked_state)?;
            self.log_placement(
                packet,
                clicked,
                None,
                self.placement_inventory_snapshot(),
                clicked_state,
                None,
                "rejected",
                Some("invalid-face"),
            );
            return Ok(());
        };

        let inventory = self.placement_inventory_snapshot();
        let target_valid = GameServerState::is_valid_block_pos(target);
        let clicked_valid = GameServerState::is_valid_block_pos(clicked);
        let (clicked_state, target_state) = self.with_game_state(|state| {
            Ok((
                clicked_valid.then(|| state.block_at(clicked)),
                target_valid.then(|| state.block_at(target)),
            ))
        })?;
        let reason =
            self.placement_rejection_reason(inventory, target, clicked_state, target_state);

        if let Some(reason) = reason {
            if let Some(clicked_state) = clicked_state {
                self.write_block_change(connection, clicked, clicked_state)?;
            }
            if let Some(target_state) = target_state {
                self.write_block_change(connection, target, target_state)?;
            }
            self.log_placement(
                packet,
                clicked,
                Some(target),
                inventory,
                clicked_state.unwrap_or(BlockState::AIR),
                target_state,
                "rejected",
                Some(reason.as_str()),
            );
            return Ok(());
        }

        let desired = selected_placeable_block(inventory.expect("validated inventory"));
        let placed = self.with_game_state(|state| Ok(state.place_block(target, desired)))?;
        let actual = if placed {
            desired
        } else {
            self.with_game_state(|state| Ok(state.block_at(target)))?
        };
        self.write_block_change(connection, target, actual)?;
        if placed {
            if let Some(player) = self.player.as_mut() {
                if let Some(slot) = player.inventory.decrement_selected_stack() {
                    self.write_inventory_slot(connection, PlayerInventory::WINDOW_ID, slot)?;
                }
            }
            self.log_placement(
                packet,
                clicked,
                Some(target),
                inventory,
                clicked_state.unwrap_or(BlockState::AIR),
                Some(actual),
                "placed",
                None,
            );
        } else {
            self.log_placement(
                packet,
                clicked,
                Some(target),
                inventory,
                clicked_state.unwrap_or(BlockState::AIR),
                Some(actual),
                "rejected",
                Some("target-not-air"),
            );
        }
        Ok(())
    }

    fn placement_inventory_snapshot(&self) -> Option<PlacementInventorySnapshot> {
        let player = self.player.as_ref()?;
        Some(PlacementInventorySnapshot {
            hotbar_index: player.inventory.selected_hotbar_slot(),
            window_slot: player.inventory.selected_window_slot(),
            selected_stack: player.inventory.selected_stack(),
        })
    }

    fn placement_rejection_reason(
        &self,
        inventory: Option<PlacementInventorySnapshot>,
        target: BlockPos,
        clicked_state: Option<BlockState>,
        target_state: Option<BlockState>,
    ) -> Option<PlacementRejection> {
        let Some(inventory) = inventory else {
            return Some(PlacementRejection::NoSelectedItem);
        };
        let LegacySlotData::Present { item_id, count, .. } = inventory.selected_stack else {
            return Some(PlacementRejection::NoSelectedItem);
        };
        if count == 0 {
            return Some(PlacementRejection::NoSelectedItem);
        }
        if !is_placeable_block_id(item_id) {
            return Some(PlacementRejection::SelectedNotPlaceable);
        }
        if !GameServerState::is_valid_block_pos(target) {
            return Some(PlacementRejection::TargetInvalidY);
        }
        if !self.is_block_loaded_for_player(target) {
            return Some(PlacementRejection::TargetUnloaded);
        }
        if !self.player_can_reach(target) {
            return Some(PlacementRejection::OutOfReach);
        }
        if clicked_state.is_none() || clicked_state == Some(BlockState::AIR) {
            return Some(PlacementRejection::ClickedBlockMissing);
        }
        if target_state != Some(BlockState::AIR) {
            return Some(PlacementRejection::TargetNotAir);
        }
        None
    }

    fn log_item_use_air(&self) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        let inventory = self.placement_inventory_snapshot();
        eprintln!(
            "[session] item-use-air hotbar={} windowSlot={} item={} player={}",
            inventory
                .map(|snapshot| snapshot.hotbar_index.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            inventory
                .map(|snapshot| snapshot.window_slot.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            inventory
                .map(|snapshot| slot_data_summary(snapshot.selected_stack))
                .unwrap_or_else(|| "empty".to_string()),
            self.player_name_for_log()
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn log_placement(
        &self,
        packet: ServerboundPlayerBlockPlacementPacket,
        clicked: BlockPos,
        target: Option<BlockPos>,
        inventory: Option<PlacementInventorySnapshot>,
        clicked_state: BlockState,
        target_state: Option<BlockState>,
        result: &str,
        reason: Option<&str>,
    ) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        let target_text = target
            .map(|target| format!("{},{},{}", target.x, target.y, target.z))
            .unwrap_or_else(|| "<none>".to_string());
        let hotbar = inventory
            .map(|snapshot| snapshot.hotbar_index.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let window_slot = inventory
            .map(|snapshot| snapshot.window_slot.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let item = inventory
            .map(|snapshot| slot_data_summary(snapshot.selected_stack))
            .unwrap_or_else(|| "empty".to_string());
        let target_block = target_state
            .map(|state| state.id.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        if let Some(reason) = reason {
            eprintln!(
                "[session] placement click={},{},{} face={} target={} hotbar={} windowSlot={} item={} clickedBlock={} targetBlock={} result={} reason={} player={}",
                clicked.x,
                clicked.y,
                clicked.z,
                packet.direction,
                target_text,
                hotbar,
                window_slot,
                item,
                clicked_state.id,
                target_block,
                result,
                reason,
                self.player_name_for_log()
            );
        } else {
            eprintln!(
                "[session] placement click={},{},{} face={} target={} hotbar={} windowSlot={} item={} clickedBlock={} targetBlock={} result={} player={}",
                clicked.x,
                clicked.y,
                clicked.z,
                packet.direction,
                target_text,
                hotbar,
                window_slot,
                item,
                clicked_state.id,
                target_block,
                result,
                self.player_name_for_log()
            );
        }
    }

    fn log_digging(
        &self,
        packet: ServerboundPlayerDiggingPacket,
        pos: BlockPos,
        block: BlockState,
        status: &str,
        result: &str,
        detail: Option<&str>,
    ) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        if let Some(detail) = detail {
            eprintln!(
                "[session] digging status={} pos={},{},{} face={} block={} result={} detail={} player={}",
                status,
                pos.x,
                pos.y,
                pos.z,
                packet.face,
                block.id,
                result,
                detail,
                self.player_name_for_log()
            );
        } else {
            eprintln!(
                "[session] digging status={} pos={},{},{} face={} block={} result={} player={}",
                status,
                pos.x,
                pos.y,
                pos.z,
                packet.face,
                block.id,
                result,
                self.player_name_for_log()
            );
        }
    }

    fn handle_window_click(
        &mut self,
        packet: ServerboundWindowClickPacket,
        connection: &mut impl Write,
    ) -> Result<()> {
        let Some(player) = self.player.as_mut() else {
            return Ok(());
        };
        let update = player.inventory.handle_window_click(packet);
        self.write_confirm_transaction(
            connection,
            packet.window_id,
            packet.action_number,
            update.accepted,
        )?;
        if update.accepted {
            for slot in update.changed_slots.iter().copied() {
                self.write_inventory_slot(connection, PlayerInventory::WINDOW_ID, slot)?;
            }
            if update.cursor_changed {
                self.write_cursor_slot(connection)?;
            }
            self.log_window_click_result(packet, &update);
        } else {
            self.send_inventory_window(connection)?;
            self.log_window_click_result(packet, &update);
        }
        Ok(())
    }

    fn handle_chat(&mut self, packet: ChatPacket, connection: &mut impl Write) -> Result<()> {
        let message = packet.message.trim();
        if message.starts_with('/') {
            let response = self.handle_command(message, connection)?;
            self.send_chat(connection, response.as_str())?;
            eprintln!(
                "[session] chat command handled player={} command={}",
                self.player_name_for_log(),
                message.split_whitespace().next().unwrap_or("/")
            );
        } else if !message.is_empty() {
            let response = format!("{}: {}", self.player_name_for_log(), message);
            self.send_chat(connection, response.as_str())?;
        }
        Ok(())
    }

    fn handle_command(&mut self, command: &str, connection: &mut impl Write) -> Result<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let Some(name) = parts.first().copied() else {
            return Ok("Empty command.".to_string());
        };
        match name {
            "/aurelia" => Ok(format!("Aurelia {VERSION} survival polish")),
            "/whereami" => {
                let Some(player) = self.player.as_ref() else {
                    return Ok("No player state.".to_string());
                };
                Ok(format!(
                    "pos {:.1} {:.1} {:.1} chunk {} {}",
                    player.x, player.y, player.z, player.current_chunk.x, player.current_chunk.z
                ))
            }
            "/givebasic" => {
                if let Some(player) = self.player.as_mut() {
                    player.inventory = PlayerInventory::starter();
                    player.selected_hotbar_slot = 0;
                }
                self.send_inventory_window(connection)?;
                Ok("Starter hotbar restored.".to_string())
            }
            "/save" => {
                let saved = self.save_dirty_chunks()?;
                Ok(format!("Saved {saved} dirty chunks."))
            }
            "/setblock" => self.handle_setblock_command(&parts, connection),
            "/time" => self.handle_time_command(&parts, connection),
            _ => Ok("Unknown Aurelia command.".to_string()),
        }
    }

    fn handle_setblock_command(
        &mut self,
        parts: &[&str],
        connection: &mut impl Write,
    ) -> Result<String> {
        if parts.len() != 5 && parts.len() != 6 {
            return Ok("Usage: /setblock x y z id [meta]".to_string());
        }
        let Some(x) = parse_i32_arg(parts[1]) else {
            return Ok("Invalid x.".to_string());
        };
        let Some(y) = parse_i32_arg(parts[2]) else {
            return Ok("Invalid y.".to_string());
        };
        let Some(z) = parse_i32_arg(parts[3]) else {
            return Ok("Invalid z.".to_string());
        };
        let Some(id) = parse_u8_arg(parts[4]) else {
            return Ok("Invalid block id.".to_string());
        };
        let meta = if parts.len() == 6 {
            let Some(meta) = parse_u8_arg(parts[5]) else {
                return Ok("Invalid metadata.".to_string());
            };
            meta
        } else {
            0
        };
        let pos = BlockPos::new(x, y, z);
        let changed = self.with_game_state(|state| Ok(state.set_block(x, y, z, id, meta)))?;
        if changed {
            self.write_block_change(connection, pos, BlockState::new_unchecked(id, meta & 0x0F))?;
            Ok(format!("Set block {x} {y} {z} to {id}:{meta}."))
        } else {
            Ok("Set block rejected.".to_string())
        }
    }

    fn handle_time_command(
        &mut self,
        parts: &[&str],
        connection: &mut impl Write,
    ) -> Result<String> {
        if parts.len() == 1 {
            let time = self.with_game_state(|state| Ok(state.world_time()))?;
            return Ok(format!("Time is {time}."));
        }
        if parts.len() != 2 {
            return Ok("Usage: /time [value]".to_string());
        }
        let Some(value) = parts.get(1).and_then(|value| value.parse::<u64>().ok()) else {
            return Ok("Usage: /time [value]".to_string());
        };
        self.with_game_state(|state| {
            state.set_world_time(value);
            Ok(())
        })?;
        self.send_time_update(connection)?;
        Ok(format!("Time set to {value}."))
    }

    fn should_send_inventory_sync(&self) -> bool {
        self.config.inventory_sync_enabled
            && !self.config.post_join_minimal
            && self.join_phase == JoinPhase::JoinedReady
    }

    fn should_send_time_update(&self) -> bool {
        self.config.time_update_enabled
            && !self.config.post_join_minimal
            && self.join_phase == JoinPhase::JoinedReady
    }

    fn send_inventory_window(&mut self, connection: &mut impl Write) -> Result<()> {
        if !self.should_send_inventory_sync() {
            return Ok(());
        }
        let slots = self
            .player
            .as_ref()
            .map(|player| player.inventory.slots().to_vec())
            .unwrap_or_else(|| vec![LegacySlotData::Empty; PlayerInventory::WINDOW_SLOT_COUNT]);
        self.write_clientbound_frame(connection, ClientboundSetWindowItemsPacket::ID, |payload| {
            ClientboundSetWindowItemsPacketCodec::encode(
                &ClientboundSetWindowItemsPacket {
                    window_id: PlayerInventory::WINDOW_ID,
                    slots,
                },
                payload,
            )
        })?;
        eprintln!(
            "[session] inventory initialized player={} slots={}",
            self.player_name_for_log(),
            PlayerInventory::WINDOW_SLOT_COUNT
        );
        Ok(())
    }

    fn write_inventory_slot(
        &mut self,
        connection: &mut impl Write,
        window_id: i8,
        slot: i16,
    ) -> Result<()> {
        if !self.should_send_inventory_sync() {
            return Ok(());
        }
        let slot_data = self
            .player
            .as_ref()
            .and_then(|player| player.inventory.slots().get(slot as usize).copied())
            .unwrap_or(LegacySlotData::Empty);
        self.write_set_slot(connection, window_id, slot, slot_data)
    }

    fn write_cursor_slot(&mut self, connection: &mut impl Write) -> Result<()> {
        if !self.should_send_inventory_sync() {
            return Ok(());
        }
        let slot_data = self
            .player
            .as_ref()
            .map(|player| player.inventory.cursor())
            .unwrap_or(LegacySlotData::Empty);
        self.write_set_slot(
            connection,
            PlayerInventory::CURSOR_WINDOW_ID,
            PlayerInventory::CURSOR_SLOT,
            slot_data,
        )
    }

    fn write_set_slot(
        &mut self,
        connection: &mut impl Write,
        window_id: i8,
        slot: i16,
        slot_data: LegacySlotData,
    ) -> Result<()> {
        if self.config.post_join_minimal || self.join_phase != JoinPhase::JoinedReady {
            return Ok(());
        }
        self.write_clientbound_frame(connection, ClientboundSetSlotPacket::ID, |payload| {
            ClientboundSetSlotPacketCodec::encode(
                &ClientboundSetSlotPacket {
                    window_id,
                    slot,
                    slot_data,
                },
                payload,
            )
        })
    }

    fn write_confirm_transaction(
        &mut self,
        connection: &mut impl Write,
        window_id: i8,
        action_number: i16,
        accepted: bool,
    ) -> Result<()> {
        if self.config.post_join_minimal || self.join_phase != JoinPhase::JoinedReady {
            return Ok(());
        }
        self.write_clientbound_frame(
            connection,
            ClientboundConfirmTransactionPacket::ID,
            |payload| {
                ClientboundConfirmTransactionPacketCodec::encode(
                    &ClientboundConfirmTransactionPacket {
                        window_id,
                        action_number,
                        accepted,
                    },
                    payload,
                )
            },
        )
    }

    fn send_chat(&mut self, connection: &mut impl Write, message: &str) -> Result<()> {
        if self.config.post_join_minimal || self.join_phase != JoinPhase::JoinedReady {
            return Ok(());
        }
        self.write_clientbound_frame(connection, ChatPacket::ID, |payload| {
            ChatPacket::new(message).encode(payload)
        })
    }

    fn send_time_update(&mut self, connection: &mut impl Write) -> Result<()> {
        if !self.should_send_time_update() {
            return Ok(());
        }
        let time = self.with_game_state(|state| Ok(state.world_time()))?;
        self.write_clientbound_frame(
            connection,
            ClientboundBeta173TimeUpdatePacket::ID,
            |payload| {
                ClientboundBeta173TimeUpdatePacketCodec::encode(
                    &ClientboundBeta173TimeUpdatePacket { time: time as i64 },
                    payload,
                )
            },
        )?;
        self.last_time_sent_at = Instant::now();
        Ok(())
    }

    fn send_keepalive(&mut self, connection: &mut impl Write) -> Result<()> {
        if !self.config.keepalive_enabled
            || self.config.post_join_minimal
            || self.join_phase != JoinPhase::JoinedReady
        {
            return Ok(());
        }
        let id = self.next_keepalive_id;
        self.next_keepalive_id = self.next_keepalive_id.wrapping_add(1).max(1);
        self.pending_keepalive_id = Some(id);
        self.write_clientbound_frame(connection, KeepAlivePacket::ID, |payload| {
            KeepAlivePacket { keep_alive_id: id }.encode(payload)
        })?;
        self.last_keepalive_sent_at = Instant::now();
        eprintln!(
            "[session] sent keepalive id={id} player={}",
            self.player_name_for_log()
        );
        Ok(())
    }

    fn send_due_periodic_packets(&mut self, connection: &mut impl Write) -> Result<()> {
        if self.join_phase != JoinPhase::JoinedReady {
            return Ok(());
        }
        let now = Instant::now();
        if self.config.time_update_enabled
            && !self.config.post_join_minimal
            && now.duration_since(self.last_time_sent_at) >= Self::TIME_UPDATE_INTERVAL
        {
            self.send_time_update(connection)?;
        }
        if self.config.keepalive_enabled
            && !self.config.post_join_minimal
            && now.duration_since(self.last_keepalive_sent_at) >= Self::KEEPALIVE_INTERVAL
        {
            self.send_keepalive(connection)?;
        }
        Ok(())
    }

    fn handle_idle_read(&mut self, connection: &mut impl Write) -> Result<Option<SessionExit>> {
        self.send_due_periodic_packets(connection)?;
        if Instant::now().duration_since(self.last_packet_at) > Self::CLIENT_TIMEOUT {
            return self
                .disconnect(connection, "Aurelia timed out waiting for client packets.")
                .map(Some);
        }
        Ok(None)
    }

    fn handle_keepalive(&mut self, packet: KeepAlivePacket) {
        if self.pending_keepalive_id == Some(packet.keep_alive_id) {
            self.pending_keepalive_id = None;
        }
        eprintln!(
            "[session] received keepalive id={} player={}",
            packet.keep_alive_id,
            self.player_name_for_log()
        );
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

    fn save_dirty_chunks(&self) -> Result<usize> {
        let mut state = self
            .state_ref
            .lock()
            .map_err(|_| ServerError::InvalidConfig("game state lock poisoned".to_string()))?;
        let dirty = state.dirty_chunk_count();
        let saved = state.save_dirty_chunks()?;
        eprintln!("[session] save completed dirtyChunks={dirty} saved={saved}");
        Ok(saved)
    }

    fn save_dirty_chunks_for_shutdown(&self) {
        if let Err(error) = self.save_dirty_chunks() {
            eprintln!("[session] save failed: {error}");
        }
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
        self.save_dirty_chunks_for_shutdown();
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

    fn log_close_window(&self, packet: ServerboundCloseWindowPacket) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        eprintln!(
            "[session] close-window windowId={} state={:?} player={} chunk={}",
            packet.window_id,
            self.state,
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );
    }

    fn log_window_click_result(
        &self,
        packet: ServerboundWindowClickPacket,
        update: &WindowClickUpdate,
    ) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        let changed_slots = if update.changed_slots.is_empty() {
            "none".to_string()
        } else {
            update
                .changed_slots
                .iter()
                .map(i16::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        eprintln!(
            "[session] window-click windowId={} slot={} button={} action={} shift={} clickedItem={} result={} changedSlots={} cursorChanged={} player={} chunk={}",
            packet.window_id,
            packet.slot,
            packet.mouse_button,
            packet.action_number,
            packet.shift,
            slot_data_summary(packet.clicked_item),
            if update.accepted { "accepted" } else { "rejected" },
            changed_slots,
            update.cursor_changed,
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );
    }

    fn log_confirm_transaction(&self, packet: ServerboundConfirmTransactionPacket) {
        if !self.config.packet_tracing_enabled {
            return;
        }
        eprintln!(
            "[session] confirm-transaction windowId={} action={} accepted={} state={:?} player={} chunk={}",
            packet.window_id,
            packet.action_number,
            packet.accepted,
            self.state,
            self.player_name_for_log(),
            self.player_chunk_for_log()
        );
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

    fn disconnect_correlation_for_log(&self) -> String {
        let serverbound = self
            .last_packet_id
            .map(|packet_id| {
                format!(
                    "{}(0x{packet_id:02X})",
                    trace::packet_trace_name(PacketDirection::ClientToServer, packet_id)
                        .unwrap_or("Unknown")
                )
            })
            .unwrap_or_else(|| "<none>".to_string());
        let clientbound = self
            .last_clientbound_packet_id
            .map(|packet_id| {
                format!(
                    "{}(0x{packet_id:02X}, payloadLength={})",
                    trace::packet_trace_name(PacketDirection::ServerToClient, packet_id)
                        .unwrap_or("Unknown"),
                    self.last_clientbound_payload_len
                )
            })
            .unwrap_or_else(|| "<none>".to_string());
        format!(
            "phase={:?} lastServerbound={} lastClientbound={}",
            self.join_phase, serverbound, clientbound
        )
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

fn chunk_data_packet_from_chunk(chunk: &Chunk) -> ClientboundChunkDataPacket {
    let mut block_ids = vec![0; experimental_flat_chunk_data::BLOCK_BYTES];
    let mut metadata = vec![0; experimental_flat_chunk_data::BLOCK_BYTES];
    for x in 0..experimental_flat_chunk_data::WIDTH {
        for z in 0..experimental_flat_chunk_data::LENGTH {
            for y in 0..experimental_flat_chunk_data::HEIGHT {
                let state = chunk.block_at(x, y, z);
                let index = experimental_flat_chunk_data::block_index(x, y, z);
                block_ids[index] = state.id;
                metadata[index] = state.metadata;
            }
        }
    }
    experimental_flat_chunk_data::chunk_from_block_arrays(
        chunk.pos().x,
        chunk.pos().z,
        &block_ids,
        &metadata,
    )
}

fn drop_for_block(state: BlockState) -> Option<(i16, u8, i16)> {
    match state.id {
        0 | 7 => None,
        1 => Some((4, 1, 0)),
        2 | 3 => Some((3, 1, 0)),
        id => Some((i16::from(id), 1, i16::from(state.metadata))),
    }
}

fn digging_status_name(status: i8) -> &'static str {
    match status {
        0 => "start",
        1 => "cancel",
        2 => "finish",
        3 => "drop-stack",
        4 => "drop-item",
        5 => "use-finish",
        _ => "unknown",
    }
}

fn parse_i32_arg(value: &str) -> Option<i32> {
    value.parse().ok()
}

fn parse_u8_arg(value: &str) -> Option<u8> {
    value.parse().ok()
}

fn is_read_timeout_error(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn client_disconnect_during_send(error: &ServerError) -> Option<&io::Error> {
    let io_error = match error {
        ServerError::Io(error) => error,
        ServerError::Protocol(ProtocolError::Io(error)) => error,
        ServerError::InvalidConfig(_) | ServerError::Protocol(_) => return None,
    };
    matches!(
        io_error.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
    )
    .then_some(io_error)
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
        let state = GameServerState::shared_flat_persistent(world_save_dir(&self.config))?;
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
                    if let Err(error) =
                        stream.set_read_timeout(Some(PlayerSession::READ_POLL_INTERVAL))
                    {
                        eprintln!("failed to set client read timeout: {error}");
                    }
                    let config = config.clone();
                    let state = Arc::clone(&state);
                    thread::spawn(move || {
                        let mut session = PlayerSession::with_state(config, state);
                        if let Err(error) = session.run(&mut stream) {
                            if let Some(io_error) = client_disconnect_during_send(&error) {
                                eprintln!(
                                    "client disconnected during join/send: {io_error}; {}",
                                    session.disconnect_correlation_for_log()
                                );
                            } else {
                                eprintln!("connection handling failed: {error}");
                            }
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
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ServerError::InvalidConfig("game state lock poisoned".to_string()))?;
            let dirty = state.dirty_chunk_count();
            let saved = state.save_dirty_chunks()?;
            eprintln!("[server] save completed dirtyChunks={dirty} saved={saved}");
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
        assert!(config.inventory_sync_enabled);
        assert!(config.time_update_enabled);
        assert!(config.keepalive_enabled);
        assert!(config.defer_inventory_sync);
        assert!(!config.post_join_minimal);
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
    fn parses_join_feature_disable_flags() {
        let config = parse_config(&[
            "--experimental-join",
            "--playable-flat-world",
            "--no-inventory-sync",
            "--no-time-update",
            "--no-keepalive",
            "--defer-inventory-sync",
            "--post-join-minimal",
        ])
        .unwrap();

        assert!(!config.inventory_sync_enabled);
        assert!(!config.time_update_enabled);
        assert!(!config.keepalive_enabled);
        assert!(config.defer_inventory_sync);
        assert!(config.post_join_minimal);
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
            Some("TimeUpdate"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x04)
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
            Some("CloseWindow"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x65)
        );
        assert_eq!(
            Some("WindowClick"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x66)
        );
        assert_eq!(
            Some("ConfirmTransaction"),
            trace::packet_trace_name(PacketDirection::ClientToServer, 0x6A)
        );
        assert_eq!(
            Some("BlockChange"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x35)
        );
        assert_eq!(
            Some("SetSlot"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x67)
        );
        assert_eq!(
            Some("SetWindowItems"),
            trace::packet_trace_name(PacketDirection::ServerToClient, 0x68)
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
    fn login_sequence_defers_inventory_and_time_until_first_movement() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        assert_eq!(JoinPhase::AwaitingFirstClientMovement, session.join_phase());
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
        );
        assert_eq!(
            1,
            packet_positions(&stream.written, ClientboundChunkDataPacket::ID).len()
        );
    }

    #[test]
    fn time_update_is_sent_after_first_movement_but_inventory_waits() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());
        let chunk_data = first_packet_position(&stream.written, ClientboundChunkDataPacket::ID)
            .expect("chunk data should be sent during initial world");
        let time = first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
            .expect("deferred time update should be sent");
        assert!(time > chunk_data);
        assert_eq!(
            Some(8),
            first_packet_payload_length(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
    }

    #[test]
    fn deferred_inventory_is_sent_after_three_additional_movements() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());
        assert_eq!(3, session.joined_ready_movement_packets);
        let time = first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
            .expect("deferred time update should be sent");
        let inventory = first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
            .expect("deferred inventory sync should be sent");
        assert!(inventory > time);
    }

    #[test]
    fn no_inventory_sync_suppresses_deferred_set_window_items() {
        let mut config = playable_config(0);
        config.inventory_sync_enabled = false;
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
        assert!(
            first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
                .is_some()
        );
    }

    #[test]
    fn no_time_update_suppresses_deferred_time_update() {
        let mut config = playable_config(0);
        config.time_update_enabled = false;
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
    }

    #[test]
    fn post_join_minimal_suppresses_optional_clientbound_packets() {
        let mut config = playable_config(0);
        config.post_join_minimal = true;
        let mut session = PlayerSession::new(config);
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_chat("/aurelia"));
        input.extend(encoded_window_click(
            0,
            36,
            0,
            7,
            false,
            LegacySlotData::Empty,
        ));
        input.extend(encoded_keepalive(42));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetSlotPacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundConfirmTransactionPacket::ID)
        );
        assert_eq!(None, first_packet_position(&stream.written, ChatPacket::ID));
        assert_eq!(
            None,
            first_packet_position(&stream.written, KeepAlivePacket::ID)
        );
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
        assert_eq!(JoinPhase::JoinedReady, session.join_phase());

        let chunk_data_positions =
            packet_positions(&stream.written, ClientboundChunkDataPacket::ID);
        let initial_chunk_data = chunk_data_positions
            .get(8)
            .copied()
            .expect("radius 1 login should send 9 initial chunk data frames");
        let time = first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
            .expect("deferred time update should be sent");
        assert!(time > initial_chunk_data);
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );

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
            } else if let Some(next) = skip_known_clientbound_frame(output) {
                output = next;
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
        let (_, chunk_data_frames) = count_chunk_frames(output);
        assert_eq!(1, chunk_data_frames);
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundSetWindowItemsPacket::ID)
        );
        assert_eq!(
            None,
            first_packet_position(&stream.written, ClientboundBeta173TimeUpdatePacket::ID)
        );
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
    fn hotbar_window_slot_mapping_matches_beta_player_inventory() {
        assert_eq!(Some(36), hotbar_index_to_window_slot(0));
        assert_eq!(Some(44), hotbar_index_to_window_slot(8));
        assert_eq!(None, hotbar_index_to_window_slot(9));
        assert_eq!(Some(0), window_slot_to_hotbar_index(36));
        assert_eq!(Some(8), window_slot_to_hotbar_index(44));
        assert_eq!(None, window_slot_to_hotbar_index(35));
        assert_eq!(None, window_slot_to_hotbar_index(45));
    }

    #[test]
    fn player_inventory_starter_contents_match_survival_mvp_hotbar() {
        let inventory = PlayerInventory::starter();

        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 64,
                damage: 0
            },
            inventory.slots()[36]
        );
        assert_eq!(
            LegacySlotData::Present {
                item_id: 4,
                count: 64,
                damage: 0
            },
            inventory.slots()[37]
        );
        assert_eq!(
            LegacySlotData::Present {
                item_id: 5,
                count: 64,
                damage: 0
            },
            inventory.slots()[38]
        );
    }

    #[test]
    fn held_item_change_maps_hotbar_to_expected_inventory_slot() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_held_item_change(8));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        let player = session.player().unwrap();
        assert_eq!(8, player.selected_hotbar_slot);
        assert_eq!(44, player.inventory.selected_window_slot());
        assert_eq!(Some(8), window_slot_to_hotbar_index(44));
    }

    #[test]
    fn window_click_moves_empty_and_non_empty_stacks() {
        let mut inventory = PlayerInventory::starter();

        let pickup = inventory.handle_window_click(ServerboundWindowClickPacket {
            window_id: 0,
            slot: 36,
            mouse_button: 0,
            action_number: 1,
            shift: false,
            clicked_item: LegacySlotData::Empty,
        });
        assert!(pickup.accepted);
        assert_eq!(LegacySlotData::Empty, inventory.slots()[36]);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 64,
                damage: 0
            },
            inventory.cursor()
        );

        let place = inventory.handle_window_click(ServerboundWindowClickPacket {
            window_id: 0,
            slot: 9,
            mouse_button: 0,
            action_number: 2,
            shift: false,
            clicked_item: LegacySlotData::Empty,
        });
        assert!(place.accepted);
        assert_eq!(LegacySlotData::Empty, inventory.cursor());
        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 64,
                damage: 0
            },
            inventory.slots()[9]
        );

        let split = inventory.handle_window_click(ServerboundWindowClickPacket {
            window_id: 0,
            slot: 37,
            mouse_button: 1,
            action_number: 3,
            shift: false,
            clicked_item: LegacySlotData::Empty,
        });
        assert!(split.accepted);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 4,
                count: 32,
                damage: 0
            },
            inventory.cursor()
        );
        assert_eq!(
            LegacySlotData::Present {
                item_id: 4,
                count: 32,
                damage: 0
            },
            inventory.slots()[37]
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
    fn window_click_followed_by_player_decodes_without_stream_desync() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_window_click(
            0,
            5,
            0,
            7,
            false,
            LegacySlotData::Empty,
        ));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        assert!(session.player().unwrap().on_ground);
    }

    #[test]
    fn close_window_followed_by_player_decodes_without_stream_desync() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_close_window(0));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        assert!(session.player().unwrap().on_ground);
    }

    #[test]
    fn confirm_transaction_is_drained_without_disconnect() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_confirm_transaction(0, 7, true));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        assert!(session.player().unwrap().on_ground);
    }

    #[test]
    fn keepalive_is_drained_without_disconnect() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_keepalive(42));
        input.extend(encoded_player(true));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        assert!(session.player().unwrap().on_ground);
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
        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 1,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[9]
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
    fn player_digging_start_and_cancel_do_not_mutate_block() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_digging(0, 0, 63, 0, 1));
        input.extend(encoded_player_digging(1, 0, 63, 0, 1));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            BlockState::GRASS,
            state.lock().unwrap().block_at(BlockPos::new(0, 63, 0))
        );
        assert!(block_changes(&stream.written).is_empty());
    }

    #[test]
    fn player_digging_finish_cannot_break_bedrock() {
        let state = GameServerState::shared_flat();
        state.lock().unwrap().set_block(0, 63, 0, 7, 0);
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player_digging(2, 0, 63, 0, 1));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            BlockState::BEDROCK,
            state.lock().unwrap().block_at(BlockPos::new(0, 63, 0))
        );
        assert!(
            block_changes(&stream.written).contains(&ClientboundBlockChangePacket {
                x: 0,
                y: 63,
                z: 0,
                block_type: 7,
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
            LegacySlotData::Present {
                item_id: 3,
                count: 63,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[36]
        );
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
    fn successful_placement_sends_set_slot_for_selected_hotbar_mapping() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        input.extend(encoded_player(true));
        input.extend(encoded_held_item_change(2));
        input.extend(encoded_player_block_placement(0, 63, 0, 1, Some((5, 1, 0))));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 5,
                count: 63,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[38]
        );
        assert_eq!(
            BlockState::new_unchecked(5, 0),
            state.lock().unwrap().block_at(BlockPos::new(0, 64, 0))
        );
        assert!(set_slot_packets(&stream.written)
            .iter()
            .any(|packet| packet.window_id == 0
                && packet.slot == 38
                && packet.slot_data
                    == LegacySlotData::Present {
                        item_id: 5,
                        count: 63,
                        damage: 0,
                    }));
    }

    #[test]
    fn rejected_placement_into_occupied_target_corrects_without_decrement() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_held_item_change(3));
        input.extend(encoded_player_block_placement(
            0,
            62,
            0,
            1,
            Some((50, 1, 0)),
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 50,
                count: 64,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[39]
        );
        assert_eq!(
            BlockState::GRASS,
            state.lock().unwrap().block_at(BlockPos::new(0, 63, 0))
        );
        let changes = block_changes(&stream.written);
        assert!(changes.contains(&ClientboundBlockChangePacket {
            x: 0,
            y: 62,
            z: 0,
            block_type: 3,
            metadata: 0,
        }));
        assert!(changes.contains(&ClientboundBlockChangePacket {
            x: 0,
            y: 63,
            z: 0,
            block_type: 2,
            metadata: 0,
        }));
    }

    #[test]
    fn rejected_placement_does_not_decrement_selected_stack() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_held_item_change(4));
        input.extend(encoded_player_block_placement(
            0,
            63,
            0,
            1,
            Some((270, 1, 0)),
        ));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            LegacySlotData::Present {
                item_id: 270,
                count: 1,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[40]
        );
        assert_eq!(
            BlockState::AIR,
            state.lock().unwrap().block_at(BlockPos::new(0, 64, 0))
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
        assert_eq!(
            LegacySlotData::Present {
                item_id: 3,
                count: 64,
                damage: 0,
            },
            session.player().unwrap().inventory.slots()[36]
        );
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
    fn chat_commands_and_echo_respond_without_disconnect() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_chat("/aurelia"));
        input.extend(encoded_chat("hello"));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(ConnectionState::Joined, session.state());
        let messages = chat_messages(&stream.written);
        assert!(messages.iter().any(|message| message.contains("Aurelia")));
        assert!(messages.iter().any(|message| message == "Luxorium: hello"));
    }

    #[test]
    fn debug_chat_commands_mutate_world_time_and_inventory_safely() {
        let state = GameServerState::shared_flat();
        let mut session = PlayerSession::with_state(playable_config(0), Arc::clone(&state));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_chat("/whereami"));
        input.extend(encoded_chat("/givebasic"));
        input.extend(encoded_chat("/setblock 1 70 1 3 0"));
        input.extend(encoded_chat("/time 6000"));
        input.extend(encoded_chat("/save"));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        assert_eq!(
            BlockState::DIRT,
            state.lock().unwrap().block_at(BlockPos::new(1, 70, 1))
        );
        assert_eq!(6000, state.lock().unwrap().world_time());
        let messages = chat_messages(&stream.written);
        assert!(messages.iter().any(|message| message.contains("pos")));
        assert!(messages
            .iter()
            .any(|message| message.contains("Starter hotbar")));
        assert!(messages.iter().any(|message| message.contains("Set block")));
        assert!(messages.iter().any(|message| message.contains("Time set")));
        assert!(messages.iter().any(|message| message.contains("Saved")));
    }

    #[test]
    fn chat_command_bad_args_return_usage_without_panic() {
        let mut session = PlayerSession::new(playable_config(0));
        let mut input = encoded_handshake("Luxorium");
        input.extend(encoded_login("Luxorium"));
        input.extend(encoded_player(true));
        input.extend(encoded_chat("/setblock 1 2"));
        input.extend(encoded_chat("/setblock nope 2 3 4"));
        input.extend(encoded_chat("/time nope"));
        input.extend(encoded_chat("/time 1 2"));
        let mut stream = Duplex::new(input);

        let outcome = session.run(&mut stream).unwrap();

        assert_eq!(SessionExit::ClientClosed, outcome);
        let messages = chat_messages(&stream.written);
        assert!(messages
            .iter()
            .any(|message| message == "Usage: /setblock x y z id [meta]"));
        assert!(messages.iter().any(|message| message == "Invalid x."));
        assert!(
            messages
                .iter()
                .filter(|message| message.as_str() == "Usage: /time [value]")
                .count()
                >= 2
        );
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

    fn encoded_player(on_ground: bool) -> Vec<u8> {
        let mut bytes = vec![0x0A];
        aurelia_protocol::write_bool(&mut bytes, on_ground).unwrap();
        bytes
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

    fn encoded_close_window(window_id: i8) -> Vec<u8> {
        let packet = ServerboundCloseWindowPacket { window_id };
        let mut bytes = vec![ServerboundCloseWindowPacket::ID];
        packet.encode(&mut bytes).unwrap();
        bytes
    }

    fn encoded_window_click(
        window_id: i8,
        slot: i16,
        mouse_button: i8,
        action_number: i16,
        shift: bool,
        clicked_item: LegacySlotData,
    ) -> Vec<u8> {
        let packet = ServerboundWindowClickPacket {
            window_id,
            slot,
            mouse_button,
            action_number,
            shift,
            clicked_item,
        };
        let mut bytes = vec![ServerboundWindowClickPacket::ID];
        packet.encode(&mut bytes).unwrap();
        bytes
    }

    fn encoded_confirm_transaction(window_id: i8, action_number: i16, accepted: bool) -> Vec<u8> {
        let packet = ServerboundConfirmTransactionPacket {
            window_id,
            action_number,
            accepted,
        };
        let mut bytes = vec![ServerboundConfirmTransactionPacket::ID];
        packet.encode(&mut bytes).unwrap();
        bytes
    }

    fn encoded_chat(message: &str) -> Vec<u8> {
        let mut bytes = vec![ChatPacket::ID];
        ChatPacket::new(message).encode(&mut bytes).unwrap();
        bytes
    }

    fn encoded_keepalive(id: i32) -> Vec<u8> {
        let mut bytes = vec![KeepAlivePacket::ID];
        KeepAlivePacket { keep_alive_id: id }
            .encode(&mut bytes)
            .unwrap();
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

    fn first_packet_position(bytes: &[u8], packet_id: u8) -> Option<usize> {
        packet_positions(bytes, packet_id).into_iter().next()
    }

    fn first_packet_payload_length(bytes: &[u8], packet_id: u8) -> Option<usize> {
        let position = first_packet_position(bytes, packet_id)?;
        let frame = &bytes[position..];
        let next = skip_clientbound_frame(frame)?;
        Some(frame.len() - next.len() - 1)
    }

    fn packet_positions(bytes: &[u8], packet_id: u8) -> Vec<usize> {
        let mut output = bytes;
        let mut offset = 0;
        let mut positions = Vec::new();
        while !output.is_empty() {
            if output[0] == packet_id {
                positions.push(offset);
            }
            let Some(next) = skip_clientbound_frame(output) else {
                panic!("unexpected packet id {:#04x}", output[0]);
            };
            offset += output.len() - next.len();
            output = next;
        }
        positions
    }

    fn skip_clientbound_frame(output: &[u8]) -> Option<&[u8]> {
        match output.first().copied()? {
            id if id == HandshakePacket::ID => Some(&output[1 + 4..]),
            id if id == ClientboundLoginResponsePacket::ID => Some(&output[1 + 15..]),
            id if id == ClientboundSpawnPositionPacket::ID => Some(&output[1 + 12..]),
            id if id == ClientboundPlayerPositionLookPacket::ID => Some(&output[1 + 41..]),
            id if id == ClientboundChunkVisibilityPacket::ID => Some(&output[1 + 9..]),
            id if id == ClientboundChunkDataPacket::ID => Some(skip_chunk_data_frame(output)),
            id if id == ClientboundBlockChangePacket::ID => Some(&output[1 + 15..]),
            id if id == DisconnectPacket::ID => {
                let length = u16::from_be_bytes([output[1], output[2]]) as usize;
                Some(&output[3 + (length * 2)..])
            }
            _ => skip_known_clientbound_frame(output),
        }
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
                ClientboundSetWindowItemsPacket::ID
                | ClientboundSetSlotPacket::ID
                | ClientboundBeta173TimeUpdatePacket::ID
                | ChatPacket::ID
                | KeepAlivePacket::ID
                | ClientboundConfirmTransactionPacket::ID => {
                    output = skip_known_clientbound_frame(output).unwrap();
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
                ClientboundSetWindowItemsPacket::ID
                | ClientboundSetSlotPacket::ID
                | ClientboundBeta173TimeUpdatePacket::ID
                | ChatPacket::ID
                | KeepAlivePacket::ID
                | ClientboundConfirmTransactionPacket::ID => {
                    output = skip_known_clientbound_frame(output).unwrap();
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

    fn set_slot_packets(bytes: &[u8]) -> Vec<ClientboundSetSlotPacket> {
        let mut output = bytes;
        let mut packets = Vec::new();
        while !output.is_empty() {
            if output[0] == ClientboundSetSlotPacket::ID {
                let window_id = output[1] as i8;
                let slot = i16::from_be_bytes([output[2], output[3]]);
                let item_id = i16::from_be_bytes([output[4], output[5]]);
                let slot_data = if item_id == -1 {
                    LegacySlotData::Empty
                } else {
                    LegacySlotData::Present {
                        item_id,
                        count: output[6],
                        damage: i16::from_be_bytes([output[7], output[8]]),
                    }
                };
                packets.push(ClientboundSetSlotPacket {
                    window_id,
                    slot,
                    slot_data,
                });
            }
            let Some(next) = skip_clientbound_frame(output) else {
                panic!("unexpected packet id {:#04x}", output[0]);
            };
            output = next;
        }
        packets
    }

    fn chat_messages(bytes: &[u8]) -> Vec<String> {
        let mut output = bytes;
        let mut messages = Vec::new();
        while !output.is_empty() {
            if output[0] == ChatPacket::ID {
                let length = u16::from_be_bytes([output[1], output[2]]) as usize;
                let end = 3 + (length * 2);
                let mut units = Vec::with_capacity(length);
                for chunk in output[3..end].chunks_exact(2) {
                    units.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
                messages.push(String::from_utf16_lossy(&units));
                output = &output[end..];
            } else if let Some(next) = skip_known_clientbound_frame(output) {
                output = next;
            } else if output[0] == ClientboundChunkDataPacket::ID {
                output = skip_chunk_data_frame(output);
            } else {
                match output[0] {
                    HandshakePacket::ID => output = &output[1 + 4..],
                    ClientboundLoginResponsePacket::ID => output = &output[1 + 15..],
                    ClientboundSpawnPositionPacket::ID => output = &output[1 + 12..],
                    ClientboundPlayerPositionLookPacket::ID => output = &output[1 + 41..],
                    ClientboundChunkVisibilityPacket::ID => output = &output[1 + 9..],
                    ClientboundBlockChangePacket::ID => output = &output[1 + 15..],
                    packet_id => panic!("unexpected packet id {packet_id:#04x}"),
                }
            }
        }
        messages
    }

    fn skip_known_clientbound_frame(output: &[u8]) -> Option<&[u8]> {
        match output.first().copied()? {
            id if id == ClientboundSetWindowItemsPacket::ID => {
                let count = i16::from_be_bytes([output[2], output[3]]) as usize;
                let mut index = 4;
                for _ in 0..count {
                    let item_id = i16::from_be_bytes([output[index], output[index + 1]]);
                    index += 2;
                    if item_id != -1 {
                        index += 3;
                    }
                }
                Some(&output[index..])
            }
            id if id == ClientboundSetSlotPacket::ID => {
                let item_id = i16::from_be_bytes([output[4], output[5]]);
                let len = if item_id == -1 { 6 } else { 9 };
                Some(&output[len..])
            }
            id if id == ClientboundBeta173TimeUpdatePacket::ID => Some(&output[1 + 8..]),
            id if id == ClientboundConfirmTransactionPacket::ID => Some(&output[1 + 4..]),
            id if id == KeepAlivePacket::ID => Some(&output[1 + 4..]),
            id if id == ChatPacket::ID => {
                let length = u16::from_be_bytes([output[1], output[2]]) as usize;
                Some(&output[3 + (length * 2)..])
            }
            _ => None,
        }
    }

    fn decode_disconnect(bytes: &[u8]) -> String {
        let mut input = bytes;
        assert_eq!(DisconnectPacket::ID, read_u8(&mut input).unwrap());
        DisconnectPacketCodec::decode(&mut input).unwrap().reason
    }
}
