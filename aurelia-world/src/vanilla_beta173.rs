use crate::nbt::{self, Compound, Document, Tag};
use crate::{BlockState, Chunk, InMemoryWorldStorage, WorldStorage};
use aurelia_common::ChunkPos;
use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const REGION_SIZE: i32 = 32;
const REGION_ENTRY_COUNT: usize = 1024;
const SECTOR_BYTES: usize = 4096;
const HEADER_BYTES: usize = SECTOR_BYTES * 2;
const NIBBLE_ARRAY_BYTES: usize = Chunk::BLOCK_COUNT / 2;
const HEIGHTMAP_BYTES: usize = Chunk::WIDTH * Chunk::DEPTH;
const COMPRESSION_GZIP: u8 = 1;
const COMPRESSION_ZLIB: u8 = 2;

pub type Result<T> = std::result::Result<T, WorldFormatError>;

#[derive(Debug)]
pub enum WorldFormatError {
    Io(io::Error),
    InvalidData(String),
}

impl fmt::Display for WorldFormatError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(output, "{error}"),
            Self::InvalidData(message) => write!(output, "{message}"),
        }
    }
}

impl std::error::Error for WorldFormatError {}

impl From<io::Error> for WorldFormatError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct LevelDat {
    document: Document,
}

impl LevelDat {
    pub fn load(path: &Path) -> Result<Self> {
        Ok(Self {
            document: read_gzip_nbt_file(path)?,
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_gzip_nbt_file(path, &self.document)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub fn level_name(&self) -> Option<&str> {
        self.data().ok()?.get("LevelName")?.as_str()
    }

    pub fn random_seed(&self) -> Option<i64> {
        self.data().ok()?.get("RandomSeed")?.as_i64()
    }

    pub fn spawn(&self) -> Result<(i32, i32, i32)> {
        let data = self.data()?;
        Ok((
            required_i32(data, "SpawnX")?,
            required_i32(data, "SpawnY")?,
            required_i32(data, "SpawnZ")?,
        ))
    }

    pub fn time(&self) -> u64 {
        self.data()
            .ok()
            .and_then(|data| data.get("Time"))
            .and_then(Tag::as_i64)
            .unwrap_or(0)
            .max(0) as u64
    }

    pub fn set_time(&mut self, time: u64) {
        if let Ok(data) = self.data_mut() {
            data.insert(
                "Time".to_string(),
                Tag::Long(time.min(i64::MAX as u64) as i64),
            );
            data.insert("LastPlayed".to_string(), Tag::Long(current_time_millis()));
        }
    }

    fn data(&self) -> Result<&Compound> {
        if let Some(data) = self.document.root.get("Data").and_then(Tag::as_compound) {
            return Ok(data);
        }
        Ok(&self.document.root)
    }

    fn data_mut(&mut self) -> Result<&mut Compound> {
        if self.document.root.contains_key("Data") {
            return self
                .document
                .root
                .get_mut("Data")
                .and_then(Tag::as_compound_mut)
                .ok_or_else(|| invalid("level.dat Data tag is not a compound"));
        }
        Ok(&mut self.document.root)
    }
}

#[derive(Debug, Clone)]
struct VanillaChunkRecord {
    chunk: Chunk,
    document: Option<Document>,
}

#[derive(Debug)]
pub struct VanillaBeta173Storage {
    world_dir: PathBuf,
    cache: RefCell<HashMap<ChunkPos, VanillaChunkRecord>>,
    dirty_chunks: HashSet<ChunkPos>,
}

impl VanillaBeta173Storage {
    pub fn new(world_dir: impl Into<PathBuf>) -> Self {
        Self {
            world_dir: world_dir.into(),
            cache: RefCell::new(HashMap::new()),
            dirty_chunks: HashSet::new(),
        }
    }

    pub fn world_dir(&self) -> &Path {
        &self.world_dir
    }

    pub fn save_dirty_chunks(&mut self) -> io::Result<usize> {
        if self.dirty_chunks.is_empty() {
            return Ok(0);
        }

        let mut dirty_by_region: HashMap<(i32, i32), Vec<ChunkPos>> = HashMap::new();
        for pos in self.dirty_chunks.iter().copied() {
            dirty_by_region
                .entry(region_pos_for_chunk(pos))
                .or_default()
                .push(pos);
        }

        let mut saved = 0;
        for ((region_x, region_z), chunks) in dirty_by_region {
            self.write_dirty_region(region_x, region_z, &chunks)?;
            for pos in chunks {
                self.dirty_chunks.remove(&pos);
                saved += 1;
            }
        }
        Ok(saved)
    }

    fn write_dirty_region(
        &mut self,
        region_x: i32,
        region_z: i32,
        dirty: &[ChunkPos],
    ) -> io::Result<()> {
        let region_dir = self.world_dir.join("region");
        fs::create_dir_all(&region_dir)?;
        let path = region_file_path_for_region(&self.world_dir, region_x, region_z);
        let mut sectors = read_region_sectors(&path)?;
        let mut timestamps = read_region_timestamps(&path)?;
        let now = current_time_seconds();
        let cache = self.cache.get_mut();

        for pos in dirty {
            let Some(record) = cache.get_mut(pos) else {
                continue;
            };
            let document = record
                .document
                .get_or_insert_with(|| new_chunk_document(*pos));
            update_chunk_document(document, &record.chunk)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
            sectors[local_chunk_index(*pos)] =
                Some(encode_chunk_sector(document).map_err(|error| {
                    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
                })?);
            timestamps[local_chunk_index(*pos)] = now;
        }

        write_region_sectors_atomic(&path, &sectors, &timestamps)
    }

    fn load_chunk_from_region(&self, pos: ChunkPos) -> Result<Option<VanillaChunkRecord>> {
        let path = region_file_path(&self.world_dir, pos);
        if !path.exists() {
            return Ok(None);
        }
        let Some(raw_sector) = read_chunk_sector(&path, pos)? else {
            return Ok(None);
        };
        let document = decode_chunk_sector(&raw_sector)?;
        let chunk = chunk_from_document(pos, &document)?;
        Ok(Some(VanillaChunkRecord {
            chunk,
            document: Some(document),
        }))
    }
}

impl WorldStorage for VanillaBeta173Storage {
    fn load_chunk(&self, pos: ChunkPos) -> Option<Chunk> {
        if let Some(record) = self.cache.borrow().get(&pos) {
            return Some(record.chunk.clone());
        }
        match self.load_chunk_from_region(pos) {
            Ok(Some(record)) => {
                let chunk = record.chunk.clone();
                self.cache.borrow_mut().insert(pos, record);
                Some(chunk)
            }
            Ok(None) => None,
            Err(error) => {
                eprintln!("[world] failed to load vanilla Beta 1.7.3 chunk {pos:?}: {error}");
                None
            }
        }
    }

    fn save_chunk(&mut self, chunk: Chunk) {
        let pos = chunk.pos();
        let document = self
            .cache
            .get_mut()
            .remove(&pos)
            .and_then(|record| record.document);
        self.cache
            .get_mut()
            .insert(pos, VanillaChunkRecord { chunk, document });
    }

    fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.cache.borrow().contains_key(&pos) || region_contains_chunk(&self.world_dir, pos)
    }

    fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty_chunks.insert(pos);
    }

    fn dirty_chunk_count(&self) -> usize {
        self.dirty_chunks.len()
    }

    fn should_generate_missing_chunks(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ActiveWorldStorage {
    AureliaFlat(InMemoryWorldStorage),
    VanillaBeta173(VanillaBeta173Storage),
}

impl ActiveWorldStorage {
    pub fn save_dirty_chunks(&mut self, aurelia_flat_dir: Option<&Path>) -> io::Result<usize> {
        match self {
            Self::AureliaFlat(storage) => {
                let Some(path) = aurelia_flat_dir else {
                    return Ok(0);
                };
                storage.save_dirty_to_dir(path)
            }
            Self::VanillaBeta173(storage) => storage.save_dirty_chunks(),
        }
    }
}

impl WorldStorage for ActiveWorldStorage {
    fn load_chunk(&self, pos: ChunkPos) -> Option<Chunk> {
        match self {
            Self::AureliaFlat(storage) => storage.load_chunk(pos),
            Self::VanillaBeta173(storage) => storage.load_chunk(pos),
        }
    }

    fn save_chunk(&mut self, chunk: Chunk) {
        match self {
            Self::AureliaFlat(storage) => storage.save_chunk(chunk),
            Self::VanillaBeta173(storage) => storage.save_chunk(chunk),
        }
    }

    fn contains_chunk(&self, pos: ChunkPos) -> bool {
        match self {
            Self::AureliaFlat(storage) => storage.contains_chunk(pos),
            Self::VanillaBeta173(storage) => storage.contains_chunk(pos),
        }
    }

    fn mark_dirty(&mut self, pos: ChunkPos) {
        match self {
            Self::AureliaFlat(storage) => storage.mark_dirty(pos),
            Self::VanillaBeta173(storage) => storage.mark_dirty(pos),
        }
    }

    fn dirty_chunk_count(&self) -> usize {
        match self {
            Self::AureliaFlat(storage) => storage.dirty_chunk_count(),
            Self::VanillaBeta173(storage) => storage.dirty_chunk_count(),
        }
    }

    fn should_generate_missing_chunks(&self) -> bool {
        match self {
            Self::AureliaFlat(storage) => storage.should_generate_missing_chunks(),
            Self::VanillaBeta173(storage) => storage.should_generate_missing_chunks(),
        }
    }
}

pub fn read_gzip_nbt_file(path: &Path) -> Result<Document> {
    let file = File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes)?;
    Ok(nbt::read_document(&mut bytes.as_slice())?)
}

pub fn write_gzip_nbt_file(path: &Path, document: &Document) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut nbt_bytes = Vec::new();
    nbt::write_document(document, &mut nbt_bytes)?;
    let file = File::create(path)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(&nbt_bytes)?;
    encoder.finish()?;
    Ok(())
}

pub fn expand_nibbles(bytes: &[u8], expected_values: usize) -> Result<Vec<u8>> {
    let expected_bytes = expected_values.div_ceil(2);
    if bytes.len() != expected_bytes {
        return Err(invalid(format!(
            "nibble array length {} did not match expected {}",
            bytes.len(),
            expected_bytes
        )));
    }
    let mut values = Vec::with_capacity(expected_values);
    for index in 0..expected_values {
        let byte = bytes[index / 2];
        let value = if index % 2 == 0 {
            byte & 0x0F
        } else {
            (byte >> 4) & 0x0F
        };
        values.push(value);
    }
    Ok(values)
}

pub fn pack_nibbles(values: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = vec![0; values.len().div_ceil(2)];
    for (index, value) in values.iter().copied().enumerate() {
        if value > 15 {
            return Err(invalid("metadata value does not fit in 4 bits"));
        }
        if index % 2 == 0 {
            bytes[index / 2] |= value;
        } else {
            bytes[index / 2] |= value << 4;
        }
    }
    Ok(bytes)
}

pub fn region_pos_for_chunk(pos: ChunkPos) -> (i32, i32) {
    (pos.x.div_euclid(REGION_SIZE), pos.z.div_euclid(REGION_SIZE))
}

pub fn local_chunk_index(pos: ChunkPos) -> usize {
    let local_x = pos.x.rem_euclid(REGION_SIZE) as usize;
    let local_z = pos.z.rem_euclid(REGION_SIZE) as usize;
    local_x + (local_z * REGION_SIZE as usize)
}

pub fn region_file_path(world_dir: &Path, pos: ChunkPos) -> PathBuf {
    let (region_x, region_z) = region_pos_for_chunk(pos);
    region_file_path_for_region(world_dir, region_x, region_z)
}

pub fn chunk_from_document(requested_pos: ChunkPos, document: &Document) -> Result<Chunk> {
    let level = chunk_level(document)?;
    let nbt_x = required_i32(level, "xPos")?;
    let nbt_z = required_i32(level, "zPos")?;
    if nbt_x != requested_pos.x || nbt_z != requested_pos.z {
        eprintln!(
            "[world] vanilla chunk coordinate mismatch requested={},{} nbt={},{}",
            requested_pos.x, requested_pos.z, nbt_x, nbt_z
        );
    }

    let blocks = required_byte_array(level, "Blocks")?;
    if blocks.len() != Chunk::BLOCK_COUNT {
        return Err(invalid(format!(
            "Blocks length {} did not match {}",
            blocks.len(),
            Chunk::BLOCK_COUNT
        )));
    }
    let data = required_byte_array(level, "Data")?;
    let metadata = expand_nibbles(data, Chunk::BLOCK_COUNT)?;

    let mut chunk = Chunk::new(requested_pos);
    for x in 0..Chunk::WIDTH {
        for z in 0..Chunk::DEPTH {
            for y in 0..Chunk::HEIGHT {
                let source = vanilla_block_index(x, y, z);
                chunk.set_block(
                    x,
                    y,
                    z,
                    BlockState::new_unchecked(blocks[source], metadata[source]),
                );
            }
        }
    }
    Ok(chunk)
}

fn update_chunk_document(document: &mut Document, chunk: &Chunk) -> Result<()> {
    ensure_chunk_level(document)?;
    let level = chunk_level_mut(document)?;
    level.insert("xPos".to_string(), Tag::Int(chunk.pos().x));
    level.insert("zPos".to_string(), Tag::Int(chunk.pos().z));

    let mut blocks = vec![0; Chunk::BLOCK_COUNT];
    let mut metadata = vec![0; Chunk::BLOCK_COUNT];
    for x in 0..Chunk::WIDTH {
        for z in 0..Chunk::DEPTH {
            for y in 0..Chunk::HEIGHT {
                let target = vanilla_block_index(x, y, z);
                let state = chunk.block_at(x, y, z);
                blocks[target] = state.id;
                metadata[target] = state.metadata;
            }
        }
    }
    level.insert("Blocks".to_string(), Tag::ByteArray(blocks));
    level.insert("Data".to_string(), Tag::ByteArray(pack_nibbles(&metadata)?));
    level
        .entry("BlockLight".to_string())
        .or_insert_with(|| Tag::ByteArray(vec![0; NIBBLE_ARRAY_BYTES]));
    level
        .entry("SkyLight".to_string())
        .or_insert_with(|| Tag::ByteArray(vec![0xFF; NIBBLE_ARRAY_BYTES]));
    level
        .entry("HeightMap".to_string())
        .or_insert_with(|| Tag::ByteArray(vec![0; HEIGHTMAP_BYTES]));
    level
        .entry("Entities".to_string())
        .or_insert_with(|| Tag::List {
            element_type: nbt::TAG_COMPOUND,
            elements: Vec::new(),
        });
    level
        .entry("TileEntities".to_string())
        .or_insert_with(|| Tag::List {
            element_type: nbt::TAG_COMPOUND,
            elements: Vec::new(),
        });
    level
        .entry("LastUpdate".to_string())
        .or_insert(Tag::Long(0));
    level
        .entry("TerrainPopulated".to_string())
        .or_insert(Tag::Byte(1));
    Ok(())
}

fn new_chunk_document(pos: ChunkPos) -> Document {
    let mut level = Compound::new();
    level.insert("xPos".to_string(), Tag::Int(pos.x));
    level.insert("zPos".to_string(), Tag::Int(pos.z));
    let mut root = Compound::new();
    root.insert("Level".to_string(), Tag::Compound(level));
    Document {
        root_name: String::new(),
        root,
    }
}

fn chunk_level(document: &Document) -> Result<&Compound> {
    document
        .root
        .get("Level")
        .and_then(Tag::as_compound)
        .ok_or_else(|| invalid("chunk NBT is missing Level compound"))
}

fn chunk_level_mut(document: &mut Document) -> Result<&mut Compound> {
    document
        .root
        .get_mut("Level")
        .and_then(Tag::as_compound_mut)
        .ok_or_else(|| invalid("chunk NBT is missing Level compound"))
}

fn ensure_chunk_level(document: &mut Document) -> Result<()> {
    if !document.root.contains_key("Level") {
        document
            .root
            .insert("Level".to_string(), Tag::Compound(Compound::new()));
    }
    if !matches!(document.root.get("Level"), Some(Tag::Compound(_))) {
        return Err(invalid("chunk NBT Level tag is not a compound"));
    }
    Ok(())
}

fn required_i32(compound: &Compound, name: &str) -> Result<i32> {
    compound
        .get(name)
        .and_then(Tag::as_i32)
        .ok_or_else(|| invalid(format!("missing or invalid Int tag {name}")))
}

fn required_byte_array<'a>(compound: &'a Compound, name: &str) -> Result<&'a [u8]> {
    compound
        .get(name)
        .and_then(Tag::as_byte_array)
        .ok_or_else(|| invalid(format!("missing or invalid ByteArray tag {name}")))
}

fn vanilla_block_index(x: usize, y: usize, z: usize) -> usize {
    y + (z * Chunk::HEIGHT) + (x * Chunk::HEIGHT * Chunk::DEPTH)
}

fn region_file_path_for_region(world_dir: &Path, region_x: i32, region_z: i32) -> PathBuf {
    world_dir
        .join("region")
        .join(format!("r.{region_x}.{region_z}.mcr"))
}

fn region_contains_chunk(world_dir: &Path, pos: ChunkPos) -> bool {
    let path = region_file_path(world_dir, pos);
    read_location_entry(&path, pos)
        .map(|entry| entry.offset > 0 && entry.sector_count > 0)
        .unwrap_or(false)
}

#[derive(Debug, Default, Clone, Copy)]
struct LocationEntry {
    offset: u32,
    sector_count: u8,
}

fn read_location_entry(path: &Path, pos: ChunkPos) -> io::Result<LocationEntry> {
    if !path.exists() {
        return Ok(LocationEntry::default());
    }
    let mut file = File::open(path)?;
    let mut header = [0; SECTOR_BYTES];
    file.read_exact(&mut header)?;
    Ok(parse_location_entry(&header, local_chunk_index(pos)))
}

fn read_region_timestamps(path: &Path) -> io::Result<Vec<u32>> {
    if !path.exists() {
        return Ok(vec![0; REGION_ENTRY_COUNT]);
    }
    let bytes = fs::read(path)?;
    if bytes.len() < HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "McRegion file is smaller than its header",
        ));
    }
    let mut timestamps = vec![0; REGION_ENTRY_COUNT];
    for index in 0..REGION_ENTRY_COUNT {
        let offset = SECTOR_BYTES + (index * 4);
        timestamps[index] = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
    }
    Ok(timestamps)
}

fn read_region_sectors(path: &Path) -> io::Result<Vec<Option<Vec<u8>>>> {
    let mut sectors = vec![None; REGION_ENTRY_COUNT];
    if !path.exists() {
        return Ok(sectors);
    }

    let bytes = fs::read(path)?;
    if bytes.len() < HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "McRegion file is smaller than its header",
        ));
    }
    let header = &bytes[..SECTOR_BYTES];
    for (index, sector) in sectors.iter_mut().enumerate() {
        let entry = parse_location_entry(header, index);
        if entry.offset == 0 || entry.sector_count == 0 {
            continue;
        }
        let start = entry.offset as usize * SECTOR_BYTES;
        let end = start + (entry.sector_count as usize * SECTOR_BYTES);
        if end > bytes.len() || start < HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "McRegion location entry points outside file",
            ));
        }
        *sector = Some(bytes[start..end].to_vec());
    }
    Ok(sectors)
}

fn read_chunk_sector(path: &Path, pos: ChunkPos) -> Result<Option<Vec<u8>>> {
    let sectors = read_region_sectors(path)?;
    Ok(sectors.into_iter().nth(local_chunk_index(pos)).flatten())
}

fn parse_location_entry(header: &[u8], index: usize) -> LocationEntry {
    let offset = index * 4;
    LocationEntry {
        offset: ((header[offset] as u32) << 16)
            | ((header[offset + 1] as u32) << 8)
            | header[offset + 2] as u32,
        sector_count: header[offset + 3],
    }
}

fn write_region_sectors_atomic(
    path: &Path,
    sectors: &[Option<Vec<u8>>],
    timestamps: &[u32],
) -> io::Result<()> {
    if sectors.len() != REGION_ENTRY_COUNT || timestamps.len() != REGION_ENTRY_COUNT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid McRegion table length",
        ));
    }

    let mut output = vec![0; HEADER_BYTES];
    let mut next_sector = 2u32;
    for (index, sector) in sectors.iter().enumerate() {
        let Some(raw) = sector else {
            continue;
        };
        let sector_count = raw.len().div_ceil(SECTOR_BYTES);
        if sector_count > u8::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "chunk payload exceeds maximum McRegion sector count",
            ));
        }
        let header_offset = index * 4;
        output[header_offset] = ((next_sector >> 16) & 0xFF) as u8;
        output[header_offset + 1] = ((next_sector >> 8) & 0xFF) as u8;
        output[header_offset + 2] = (next_sector & 0xFF) as u8;
        output[header_offset + 3] = sector_count as u8;

        let timestamp_offset = SECTOR_BYTES + (index * 4);
        output[timestamp_offset..timestamp_offset + 4]
            .copy_from_slice(&timestamps[index].to_be_bytes());

        output.extend_from_slice(raw);
        let padding = (sector_count * SECTOR_BYTES) - raw.len();
        output.extend(std::iter::repeat(0).take(padding));
        next_sector += sector_count as u32;
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("mcr.tmp");
    fs::write(&tmp_path, output)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn decode_chunk_sector(raw_sector: &[u8]) -> Result<Document> {
    if raw_sector.len() < 5 {
        return Err(invalid("McRegion chunk sector is too small"));
    }
    let length = u32::from_be_bytes(raw_sector[0..4].try_into().unwrap()) as usize;
    if length == 0 || 4 + length > raw_sector.len() {
        return Err(invalid("McRegion chunk length points outside sector data"));
    }
    let compression_type = raw_sector[4];
    let compressed = &raw_sector[5..4 + length];
    let mut nbt_bytes = Vec::new();
    match compression_type {
        COMPRESSION_GZIP => GzDecoder::new(compressed).read_to_end(&mut nbt_bytes)?,
        COMPRESSION_ZLIB => ZlibDecoder::new(compressed).read_to_end(&mut nbt_bytes)?,
        other => {
            return Err(invalid(format!(
                "unsupported chunk compression type {other}"
            )))
        }
    };
    Ok(nbt::read_document(&mut nbt_bytes.as_slice())?)
}

fn encode_chunk_sector(document: &Document) -> Result<Vec<u8>> {
    let mut nbt_bytes = Vec::new();
    nbt::write_document(document, &mut nbt_bytes)?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&nbt_bytes)?;
    let compressed = encoder.finish()?;
    let length = compressed
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid("compressed chunk length overflow"))?;
    let length = u32::try_from(length).map_err(|_| invalid("compressed chunk is too large"))?;

    let mut sector = Vec::with_capacity(5 + compressed.len());
    sector.extend_from_slice(&length.to_be_bytes());
    sector.push(COMPRESSION_ZLIB);
    sector.extend_from_slice(&compressed);
    Ok(sector)
}

fn current_time_seconds() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(u64::from(u32::MAX)) as u32)
        .unwrap_or(0)
}

fn current_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn invalid(message: impl Into<String>) -> WorldFormatError {
    WorldFormatError::InvalidData(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    #[test]
    fn nibble_arrays_use_low_nibble_for_even_indices() {
        assert_eq!(vec![0x21, 0x43], pack_nibbles(&[1, 2, 3, 4]).unwrap());
        assert_eq!(vec![1, 2, 3, 4], expand_nibbles(&[0x21, 0x43], 4).unwrap());
    }

    #[test]
    fn nibble_arrays_round_trip_all_metadata_values() {
        let values: Vec<u8> = (0..Chunk::BLOCK_COUNT)
            .map(|index| (index % 16) as u8)
            .collect();
        let packed = pack_nibbles(&values).unwrap();
        let expanded = expand_nibbles(&packed, Chunk::BLOCK_COUNT).unwrap();

        assert_eq!(values, expanded);
    }

    #[test]
    fn region_coordinates_and_indices_use_floor_division() {
        assert_eq!((0, 0), region_pos_for_chunk(ChunkPos::new(0, 0)));
        assert_eq!((0, 0), region_pos_for_chunk(ChunkPos::new(31, 31)));
        assert_eq!((1, 0), region_pos_for_chunk(ChunkPos::new(32, 0)));
        assert_eq!((-1, 0), region_pos_for_chunk(ChunkPos::new(-1, 0)));
        assert_eq!((-2, -2), region_pos_for_chunk(ChunkPos::new(-33, -33)));
        assert_eq!(0, local_chunk_index(ChunkPos::new(0, 0)));
        assert_eq!(31, local_chunk_index(ChunkPos::new(-1, 0)));
        assert_eq!(992, local_chunk_index(ChunkPos::new(0, -1)));
        assert_eq!(1023, local_chunk_index(ChunkPos::new(-1, -1)));
    }

    #[test]
    fn mcregion_chunk_loads_blocks_and_metadata() {
        let dir = test_world_dir("mcregion-load");
        let _ = fs::remove_dir_all(&dir);
        write_synthetic_region_chunk(&dir, ChunkPos::new(0, 0)).unwrap();
        let storage = VanillaBeta173Storage::new(&dir);

        let chunk = storage.load_chunk(ChunkPos::new(0, 0)).unwrap();

        assert_eq!(BlockState::STONE, chunk.block_at(0, 0, 0));
        assert_eq!(BlockState::new_unchecked(3, 7), chunk.block_at(1, 64, 1));
        assert_eq!(BlockState::GRASS, chunk.block_at(2, 65, 2));
        assert_eq!(BlockState::AIR, chunk.block_at(3, 66, 3));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mcregion_save_preserves_unrelated_fields_and_persists_mutation() {
        let dir = test_world_dir("mcregion-save");
        let _ = fs::remove_dir_all(&dir);
        write_synthetic_region_chunk(&dir, ChunkPos::new(0, 0)).unwrap();
        let mut world = World::new(VanillaBeta173Storage::new(&dir), crate::FlatWorldGenerator);

        world.set_block(
            aurelia_common::BlockPos::new(1, 64, 1),
            BlockState::new_unchecked(4, 2),
        );
        assert_eq!(1, world.dirty_chunk_count());
        assert_eq!(1, world.storage_mut().save_dirty_chunks().unwrap());

        let storage = VanillaBeta173Storage::new(&dir);
        let chunk = storage.load_chunk(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(BlockState::new_unchecked(4, 2), chunk.block_at(1, 64, 1));

        let sector = read_chunk_sector(
            &region_file_path(&dir, ChunkPos::new(0, 0)),
            ChunkPos::new(0, 0),
        )
        .unwrap()
        .unwrap();
        let document = decode_chunk_sector(&sector).unwrap();
        let level = chunk_level(&document).unwrap();
        assert_eq!(
            Some("keep-me"),
            level.get("AureliaTest").and_then(Tag::as_str)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    fn write_synthetic_region_chunk(dir: &Path, pos: ChunkPos) -> Result<()> {
        let mut chunk = Chunk::new(pos);
        chunk.set_block(0, 0, 0, BlockState::STONE);
        chunk.set_block(1, 64, 1, BlockState::new_unchecked(3, 7));
        chunk.set_block(2, 65, 2, BlockState::GRASS);
        chunk.set_block(3, 66, 3, BlockState::AIR);

        let mut document = new_chunk_document(pos);
        update_chunk_document(&mut document, &chunk)?;
        chunk_level_mut(&mut document).unwrap().insert(
            "AureliaTest".to_string(),
            Tag::String("keep-me".to_string()),
        );
        let sector = encode_chunk_sector(&document)?;
        let mut sectors = vec![None; REGION_ENTRY_COUNT];
        sectors[local_chunk_index(pos)] = Some(sector);
        let mut timestamps = vec![0; REGION_ENTRY_COUNT];
        timestamps[local_chunk_index(pos)] = 1;
        write_region_sectors_atomic(&region_file_path(dir, pos), &sectors, &timestamps)?;
        Ok(())
    }

    fn test_world_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("aurelia-world-{name}-{}", std::process::id()))
    }
}
