use aurelia_common::{BlockPos, ChunkPos};
use std::collections::HashMap;

pub const WORLD_HEIGHT: usize = 128;
pub const SEA_LEVEL: usize = 64;
pub const FLAT_GRASS_Y: usize = 63;
pub const SPAWN_POSITION: BlockPos = BlockPos::new(0, 65, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockState {
    pub id: u8,
    pub metadata: u8,
}

impl BlockState {
    pub const AIR: Self = Self::new_unchecked(0, 0);
    pub const STONE: Self = Self::new_unchecked(1, 0);
    pub const GRASS: Self = Self::new_unchecked(2, 0);
    pub const DIRT: Self = Self::new_unchecked(3, 0);
    pub const BEDROCK: Self = Self::new_unchecked(7, 0);

    pub fn new(id: u8, metadata: u8) -> Result<Self, String> {
        if metadata > 15 {
            return Err("metadata must fit in 4 bits".to_string());
        }
        Ok(Self { id, metadata })
    }

    pub const fn new_unchecked(id: u8, metadata: u8) -> Self {
        Self { id, metadata }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pos: ChunkPos,
    block_ids: Vec<u8>,
    metadata: Vec<u8>,
}

impl Chunk {
    pub const WIDTH: usize = 16;
    pub const DEPTH: usize = 16;
    pub const HEIGHT: usize = WORLD_HEIGHT;
    pub const BLOCK_COUNT: usize = Self::WIDTH * Self::DEPTH * Self::HEIGHT;

    pub fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            block_ids: vec![0; Self::BLOCK_COUNT],
            metadata: vec![0; Self::BLOCK_COUNT],
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn block_at(&self, x: usize, y: usize, z: usize) -> BlockState {
        let index = Self::index(x, y, z);
        BlockState::new_unchecked(self.block_ids[index], self.metadata[index])
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, state: BlockState) {
        let index = Self::index(x, y, z);
        self.block_ids[index] = state.id;
        self.metadata[index] = state.metadata;
    }

    pub fn copy_block_ids(&self) -> Vec<u8> {
        self.block_ids.clone()
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        assert!(
            x < Self::WIDTH && y < Self::HEIGHT && z < Self::DEPTH,
            "chunk coordinate out of bounds: {x},{y},{z}"
        );
        (y * Self::DEPTH + z) * Self::WIDTH + x
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FlatWorldGenerator;

impl FlatWorldGenerator {
    pub fn generate(&self, pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new(pos);
        for x in 0..Chunk::WIDTH {
            for z in 0..Chunk::DEPTH {
                for y in 0..Chunk::HEIGHT {
                    let state = match y {
                        0..=58 => BlockState::STONE,
                        59..=62 => BlockState::DIRT,
                        63 => BlockState::GRASS,
                        _ => BlockState::AIR,
                    };
                    if state != BlockState::AIR {
                        chunk.set_block(x, y, z, state);
                    }
                }
            }
        }
        chunk
    }
}

pub trait WorldStorage {
    fn load_chunk(&self, pos: ChunkPos) -> Option<Chunk>;
    fn save_chunk(&mut self, chunk: Chunk);
    fn contains_chunk(&self, pos: ChunkPos) -> bool;
}

#[derive(Debug, Default)]
pub struct InMemoryWorldStorage {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl WorldStorage for InMemoryWorldStorage {
    fn load_chunk(&self, pos: ChunkPos) -> Option<Chunk> {
        self.chunks.get(&pos).cloned()
    }

    fn save_chunk(&mut self, chunk: Chunk) {
        self.chunks.insert(chunk.pos(), chunk);
    }

    fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }
}

#[derive(Debug)]
pub struct World<S> {
    storage: S,
    generator: FlatWorldGenerator,
    time: u64,
}

impl<S: WorldStorage> World<S> {
    pub const fn new(storage: S, generator: FlatWorldGenerator) -> Self {
        Self {
            storage,
            generator,
            time: 0,
        }
    }

    pub fn get_or_create_chunk(&mut self, pos: ChunkPos) -> Chunk {
        if let Some(chunk) = self.storage.load_chunk(pos) {
            return chunk;
        }

        let generated = self.generator.generate(pos);
        self.storage.save_chunk(generated.clone());
        generated
    }

    pub fn ensure_chunk_loaded(&mut self, pos: ChunkPos) {
        let _ = self.get_or_create_chunk(pos);
    }

    pub fn is_chunk_loaded(&self, pos: ChunkPos) -> bool {
        self.storage.contains_chunk(pos)
    }

    pub const fn is_valid_block_pos(pos: BlockPos) -> bool {
        pos.y >= 0 && (pos.y as usize) < WORLD_HEIGHT
    }

    pub const fn time(&self) -> u64 {
        self.time
    }

    pub fn tick(&mut self) {
        self.time = self.time.wrapping_add(1);
    }

    pub fn block_at(&mut self, pos: BlockPos) -> BlockState {
        if !Self::is_valid_block_pos(pos) {
            return BlockState::AIR;
        }
        let chunk_pos = ChunkPos::from_block(pos.x, pos.z);
        let local_x = pos.x.rem_euclid(ChunkPos::BLOCKS_PER_CHUNK) as usize;
        let local_z = pos.z.rem_euclid(ChunkPos::BLOCKS_PER_CHUNK) as usize;
        self.get_or_create_chunk(chunk_pos)
            .block_at(local_x, pos.y as usize, local_z)
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState) {
        if !Self::is_valid_block_pos(pos) {
            return;
        }
        let chunk_pos = ChunkPos::from_block(pos.x, pos.z);
        let local_x = pos.x.rem_euclid(ChunkPos::BLOCKS_PER_CHUNK) as usize;
        let local_z = pos.z.rem_euclid(ChunkPos::BLOCKS_PER_CHUNK) as usize;
        let mut chunk = self.get_or_create_chunk(chunk_pos);
        chunk.set_block(local_x, pos.y as usize, local_z, state);
        self.storage.save_chunk(chunk);
    }

    pub fn break_block(&mut self, pos: BlockPos) -> bool {
        if !Self::is_valid_block_pos(pos) {
            return false;
        }
        self.set_block(pos, BlockState::AIR);
        true
    }

    pub fn place_block(&mut self, pos: BlockPos, state: BlockState) -> bool {
        if !Self::is_valid_block_pos(pos) {
            return false;
        }
        self.set_block(pos, state);
        true
    }

    pub fn get_block(&mut self, x: i32, y: i32, z: i32) -> BlockState {
        self.block_at(BlockPos::new(x, y, z))
    }

    pub fn set_block_id(&mut self, x: i32, y: i32, z: i32, block_id: u8, metadata: u8) -> bool {
        let Ok(state) = BlockState::new(block_id, metadata) else {
            return false;
        };
        if !Self::is_valid_block_pos(BlockPos::new(x, y, z)) {
            return false;
        }
        self.set_block(BlockPos::new(x, y, z), state);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(u32);

impl EntityId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    Zombie,
    Skeleton,
    Cow,
    Pig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Default)]
pub struct EntityManager {
    next_id: u32,
    entities: HashMap<EntityId, Entity>,
}

impl EntityManager {
    pub fn allocate_id(&mut self) -> EntityId {
        self.next_id = self.next_id.saturating_add(1).max(1);
        EntityId::new(self.next_id)
    }

    pub fn spawn(&mut self, kind: EntityKind, x: f64, y: f64, z: f64) -> EntityId {
        let id = self.allocate_id();
        self.entities.insert(id, Entity { id, kind, x, y, z });
        id
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn despawn(&mut self, id: EntityId) -> Option<Entity> {
        self.entities.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_world_generator_creates_expected_layers() {
        let chunk = FlatWorldGenerator.generate(ChunkPos::new(0, 0));

        assert_eq!(BlockState::STONE, chunk.block_at(0, 0, 0));
        assert_eq!(BlockState::STONE, chunk.block_at(0, 58, 0));
        assert_eq!(BlockState::DIRT, chunk.block_at(0, 59, 0));
        assert_eq!(BlockState::DIRT, chunk.block_at(0, 62, 0));
        assert_eq!(BlockState::GRASS, chunk.block_at(0, FLAT_GRASS_Y, 0));
        assert_eq!(BlockState::AIR, chunk.block_at(0, SEA_LEVEL, 0));
    }

    #[test]
    fn spawn_position_stands_above_flat_grass() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);

        assert_eq!(
            BlockState::GRASS,
            world.block_at(BlockPos::new(
                SPAWN_POSITION.x,
                FLAT_GRASS_Y as i32,
                SPAWN_POSITION.z
            ))
        );
        assert_eq!(BlockState::AIR, world.block_at(SPAWN_POSITION));
    }

    #[test]
    fn block_get_set_supports_negative_world_coordinates() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);
        let pos = BlockPos::new(-1, 70, -17);

        assert_eq!(BlockState::AIR, world.block_at(pos));
        world.set_block(pos, BlockState::DIRT);
        assert_eq!(BlockState::DIRT, world.block_at(pos));
    }

    #[test]
    fn break_and_place_blocks_mutate_world() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);
        let grass = BlockPos::new(0, FLAT_GRASS_Y as i32, 0);
        let air = BlockPos::new(1, SEA_LEVEL as i32, 1);

        assert_eq!(BlockState::GRASS, world.block_at(grass));
        assert!(world.break_block(grass));
        assert_eq!(BlockState::AIR, world.block_at(grass));

        assert_eq!(BlockState::AIR, world.block_at(air));
        assert!(world.place_block(air, BlockState::DIRT));
        assert_eq!(BlockState::DIRT, world.block_at(air));
    }

    #[test]
    fn out_of_height_edits_are_rejected_safely() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);

        assert!(!world.break_block(BlockPos::new(0, -1, 0)));
        assert!(!world.place_block(BlockPos::new(0, WORLD_HEIGHT as i32, 0), BlockState::DIRT));
    }

    #[test]
    fn world_generates_and_stores_missing_chunks() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);
        let pos = ChunkPos::new(1, -1);

        assert!(!world.is_chunk_loaded(pos));
        let first = world.get_or_create_chunk(pos);
        let second = world.get_or_create_chunk(pos);

        assert!(world.is_chunk_loaded(pos));
        assert_eq!(first, second);
    }

    #[test]
    fn world_time_ticks_forward() {
        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);

        world.tick();
        world.tick();

        assert_eq!(2, world.time());
    }

    #[test]
    fn entity_manager_allocates_and_stores_entities() {
        let mut entities = EntityManager::default();

        let player = entities.spawn(EntityKind::Player, 0.5, 65.0, 0.5);
        let zombie = entities.spawn(EntityKind::Zombie, 4.0, 65.0, 4.0);

        assert_ne!(player, zombie);
        assert_eq!(2, entities.len());
        assert_eq!(
            Some(EntityKind::Player),
            entities.get(player).map(|entity| entity.kind)
        );
        assert_eq!(
            Some(EntityKind::Zombie),
            entities.get(zombie).map(|entity| entity.kind)
        );
        assert_eq!(
            Some(EntityKind::Player),
            entities.despawn(player).map(|entity| entity.kind)
        );
        assert_eq!(1, entities.len());
    }
}
