use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AureliaError {
    InvalidArgument(String),
}

impl fmt::Display for AureliaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => f.write_str(message),
        }
    }
}

impl Error for AureliaError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const BLOCKS_PER_CHUNK: i32 = 16;

    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub fn from_block(block_x: i32, block_z: i32) -> Self {
        Self::new(
            block_x.div_euclid(Self::BLOCKS_PER_CHUNK),
            block_z.div_euclid(Self::BLOCKS_PER_CHUNK),
        )
    }

    pub const fn min_block_x(self) -> i32 {
        self.x * Self::BLOCKS_PER_CHUNK
    }

    pub const fn min_block_z(self) -> i32 {
        self.z * Self::BLOCKS_PER_CHUNK
    }
}

pub fn clamp(value: i32, min: i32, max: i32) -> Result<i32, AureliaError> {
    if min > max {
        return Err(AureliaError::InvalidArgument(
            "min must be less than or equal to max".to_string(),
        ));
    }
    Ok(value.max(min).min(max))
}

pub fn floor_to_int(value: f64) -> i32 {
    let truncated = value as i32;
    if value < truncated as f64 {
        truncated - 1
    } else {
        truncated
    }
}

pub const TICKS_PER_SECOND: u32 = 20;
pub const MILLIS_PER_TICK: u64 = 50;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn chunk_pos_equality_and_hash_use_coordinates() {
        let first = ChunkPos::new(3, -2);
        let second = ChunkPos::new(3, -2);
        let different = ChunkPos::new(4, -2);
        let mut positions = HashSet::new();

        positions.insert(first);

        assert_eq!(first, second);
        assert!(positions.contains(&second));
        assert_eq!(1, positions.len());
        positions.insert(different);
        assert_eq!(2, positions.len());
    }

    #[test]
    fn chunk_pos_uses_floor_division_for_negative_blocks() {
        assert_eq!(ChunkPos::new(-1, -1), ChunkPos::from_block(-1, -1));
        assert_eq!(ChunkPos::new(-2, -2), ChunkPos::from_block(-17, -17));
    }

    #[test]
    fn block_pos_offsets_coordinates() {
        assert_eq!(
            BlockPos::new(2, 5, 1),
            BlockPos::new(1, 2, 3).offset(1, 3, -2)
        );
    }

    #[test]
    fn clamp_rejects_reversed_bounds() {
        assert!(clamp(1, 5, 3).is_err());
    }
}
