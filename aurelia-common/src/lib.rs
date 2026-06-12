use std::collections::HashSet;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkView {
    center: ChunkPos,
    radius: i32,
    visible: HashSet<ChunkPos>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkViewDiff {
    pub load: Vec<ChunkPos>,
    pub unload: Vec<ChunkPos>,
}

impl ChunkView {
    pub fn new(center: ChunkPos, radius: i32) -> Self {
        let radius = radius.max(0);
        Self {
            center,
            radius,
            visible: HashSet::new(),
        }
    }

    pub const fn center(&self) -> ChunkPos {
        self.center
    }

    pub const fn radius(&self) -> i32 {
        self.radius
    }

    pub fn visible(&self) -> &HashSet<ChunkPos> {
        &self.visible
    }

    pub fn contains(&self, pos: ChunkPos) -> bool {
        self.visible.contains(&pos)
    }

    pub fn update(&mut self, center: ChunkPos, radius: i32) -> ChunkViewDiff {
        self.center = center;
        self.radius = radius.max(0);
        let required = chunks_in_radius(center, self.radius);
        let required_set: HashSet<ChunkPos> = required.iter().copied().collect();

        let mut load = required
            .into_iter()
            .filter(|pos| !self.visible.contains(pos))
            .collect::<Vec<_>>();
        let mut unload = self
            .visible
            .iter()
            .copied()
            .filter(|pos| !required_set.contains(pos))
            .collect::<Vec<_>>();
        unload.sort_by_key(chunk_sort_key);

        self.visible = required_set;
        load.sort_by_key(chunk_sort_key);

        ChunkViewDiff { load, unload }
    }
}

pub fn chunks_in_radius(center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
    let radius = radius.max(0);
    let side = (radius * 2 + 1) as usize;
    let mut chunks = Vec::with_capacity(side * side);
    for z in (center.z - radius)..=(center.z + radius) {
        for x in (center.x - radius)..=(center.x + radius) {
            chunks.push(ChunkPos::new(x, z));
        }
    }
    chunks
}

fn chunk_sort_key(pos: &ChunkPos) -> (i32, i32) {
    (pos.z, pos.x)
}

pub fn clamp(value: i32, min: i32, max: i32) -> Result<i32, AureliaError> {
    if min > max {
        return Err(AureliaError::InvalidArgument(
            "min must be less than or equal to max".to_string(),
        ));
    }
    Ok(value.max(min).min(max))
}

pub const TICKS_PER_SECOND: u32 = 20;
pub const MILLIS_PER_TICK: u64 = 50;

pub mod beta173 {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ToolCategory {
        Shovel,
        Pickaxe,
        Axe,
        Sword,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ToolTier {
        Wood,
        Stone,
        Iron,
        Diamond,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ItemKind {
        Air,
        Block,
        Tool {
            category: ToolCategory,
            tier: ToolTier,
        },
        Material,
        ConsumablePlaceholder,
        Unknown,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ItemRule {
        pub id: i16,
        pub debug_name: &'static str,
        pub max_stack_size: u8,
        pub kind: ItemKind,
    }

    impl ItemRule {
        pub const fn is_placeable(self) -> bool {
            matches!(self.kind, ItemKind::Block)
        }

        pub const fn is_tool(self) -> bool {
            matches!(self.kind, ItemKind::Tool { .. })
        }

        pub const fn tool_category(self) -> Option<ToolCategory> {
            match self.kind {
                ItemKind::Tool { category, .. } => Some(category),
                _ => None,
            }
        }

        pub const fn tool_tier(self) -> Option<ToolTier> {
            match self.kind {
                ItemKind::Tool { tier, .. } => Some(tier),
                _ => None,
            }
        }

        pub const fn can_damage_blocks_faster(self) -> bool {
            self.is_tool()
        }

        pub const fn is_consumable_placeholder(self) -> bool {
            matches!(self.kind, ItemKind::ConsumablePlaceholder)
        }
    }

    pub const AIR: i16 = 0;
    pub const STONE: i16 = 1;
    pub const GRASS: i16 = 2;
    pub const DIRT: i16 = 3;
    pub const COBBLESTONE: i16 = 4;
    pub const PLANKS: i16 = 5;
    pub const SAPLING: i16 = 6;
    pub const BEDROCK: i16 = 7;
    pub const WATER: i16 = 8;
    pub const STATIONARY_WATER: i16 = 9;
    pub const LAVA: i16 = 10;
    pub const STATIONARY_LAVA: i16 = 11;
    pub const SAND: i16 = 12;
    pub const GRAVEL: i16 = 13;
    pub const GOLD_ORE: i16 = 14;
    pub const IRON_ORE: i16 = 15;
    pub const COAL_ORE: i16 = 16;
    pub const LOG: i16 = 17;
    pub const LEAVES: i16 = 18;
    pub const GLASS: i16 = 20;
    pub const DIAMOND_ORE: i16 = 56;
    pub const REDSTONE_ORE: i16 = 73;
    pub const GLOWING_REDSTONE_ORE: i16 = 74;
    pub const TORCH: i16 = 50;
    pub const CHEST: i16 = 54;
    pub const CRAFTING_TABLE: i16 = 58;
    pub const FURNACE: i16 = 61;
    pub const LIT_FURNACE: i16 = 62;
    pub const STICK: i16 = 280;
    pub const COAL: i16 = 263;
    pub const WOODEN_SHOVEL: i16 = 269;
    pub const WOODEN_PICKAXE: i16 = 270;
    pub const WOODEN_AXE: i16 = 271;
    pub const WOODEN_SWORD: i16 = 268;
    pub const STONE_SHOVEL: i16 = 273;
    pub const STONE_PICKAXE: i16 = 274;
    pub const STONE_AXE: i16 = 275;
    pub const STONE_SWORD: i16 = 272;
    pub const IRON_SHOVEL: i16 = 256;
    pub const IRON_PICKAXE: i16 = 257;
    pub const IRON_AXE: i16 = 258;
    pub const IRON_SWORD: i16 = 267;
    pub const DIAMOND_SHOVEL: i16 = 277;
    pub const DIAMOND_PICKAXE: i16 = 278;
    pub const DIAMOND_AXE: i16 = 279;
    pub const DIAMOND_SWORD: i16 = 276;

    pub fn item_rule(id: i16) -> ItemRule {
        match id {
            AIR => item(id, "air", 0, ItemKind::Air),
            STONE => block(id, "stone"),
            GRASS => block(id, "grass"),
            DIRT => block(id, "dirt"),
            COBBLESTONE => block(id, "cobblestone"),
            PLANKS => block(id, "planks"),
            SAPLING => block(id, "sapling"),
            BEDROCK => block(id, "bedrock"),
            WATER => block(id, "water"),
            STATIONARY_WATER => block(id, "stationary_water"),
            LAVA => block(id, "lava"),
            STATIONARY_LAVA => block(id, "stationary_lava"),
            SAND => block(id, "sand"),
            GRAVEL => block(id, "gravel"),
            GOLD_ORE => block(id, "gold_ore"),
            IRON_ORE => block(id, "iron_ore"),
            COAL_ORE => block(id, "coal_ore"),
            LOG => block(id, "log"),
            LEAVES => block(id, "leaves"),
            GLASS => block(id, "glass"),
            TORCH => block(id, "torch"),
            CHEST => block(id, "chest"),
            DIAMOND_ORE => block(id, "diamond_ore"),
            CRAFTING_TABLE => block(id, "crafting_table"),
            FURNACE => block(id, "furnace"),
            LIT_FURNACE => block(id, "lit_furnace"),
            REDSTONE_ORE => block(id, "redstone_ore"),
            GLOWING_REDSTONE_ORE => block(id, "glowing_redstone_ore"),
            STICK => item(id, "stick", 64, ItemKind::Material),
            COAL => item(id, "coal", 64, ItemKind::Material),
            WOODEN_SHOVEL => tool(id, "wooden_shovel", ToolCategory::Shovel, ToolTier::Wood),
            WOODEN_PICKAXE => tool(id, "wooden_pickaxe", ToolCategory::Pickaxe, ToolTier::Wood),
            WOODEN_AXE => tool(id, "wooden_axe", ToolCategory::Axe, ToolTier::Wood),
            WOODEN_SWORD => tool(id, "wooden_sword", ToolCategory::Sword, ToolTier::Wood),
            STONE_SHOVEL => tool(id, "stone_shovel", ToolCategory::Shovel, ToolTier::Stone),
            STONE_PICKAXE => tool(id, "stone_pickaxe", ToolCategory::Pickaxe, ToolTier::Stone),
            STONE_AXE => tool(id, "stone_axe", ToolCategory::Axe, ToolTier::Stone),
            STONE_SWORD => tool(id, "stone_sword", ToolCategory::Sword, ToolTier::Stone),
            IRON_SHOVEL => tool(id, "iron_shovel", ToolCategory::Shovel, ToolTier::Iron),
            IRON_PICKAXE => tool(id, "iron_pickaxe", ToolCategory::Pickaxe, ToolTier::Iron),
            IRON_AXE => tool(id, "iron_axe", ToolCategory::Axe, ToolTier::Iron),
            IRON_SWORD => tool(id, "iron_sword", ToolCategory::Sword, ToolTier::Iron),
            DIAMOND_SHOVEL => tool(
                id,
                "diamond_shovel",
                ToolCategory::Shovel,
                ToolTier::Diamond,
            ),
            DIAMOND_PICKAXE => tool(
                id,
                "diamond_pickaxe",
                ToolCategory::Pickaxe,
                ToolTier::Diamond,
            ),
            DIAMOND_AXE => tool(id, "diamond_axe", ToolCategory::Axe, ToolTier::Diamond),
            DIAMOND_SWORD => tool(id, "diamond_sword", ToolCategory::Sword, ToolTier::Diamond),
            _ => item(id, "unknown", 64, ItemKind::Unknown),
        }
    }

    const fn block(id: i16, debug_name: &'static str) -> ItemRule {
        item(id, debug_name, 64, ItemKind::Block)
    }

    const fn tool(
        id: i16,
        debug_name: &'static str,
        category: ToolCategory,
        tier: ToolTier,
    ) -> ItemRule {
        item(id, debug_name, 1, ItemKind::Tool { category, tier })
    }

    const fn item(
        id: i16,
        debug_name: &'static str,
        max_stack_size: u8,
        kind: ItemKind,
    ) -> ItemRule {
        ItemRule {
            id,
            debug_name,
            max_stack_size,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn chunk_view_radius_zero_loads_only_center() {
        let mut view = ChunkView::new(ChunkPos::new(0, 0), 0);

        let diff = view.update(ChunkPos::new(4, -2), 0);

        assert_eq!(vec![ChunkPos::new(4, -2)], diff.load);
        assert!(diff.unload.is_empty());
        assert_eq!(1, view.visible().len());
        assert!(view.contains(ChunkPos::new(4, -2)));
    }

    #[test]
    fn chunk_view_radius_one_loads_three_by_three() {
        let mut view = ChunkView::new(ChunkPos::new(0, 0), 1);

        let diff = view.update(ChunkPos::new(0, 0), 1);

        assert_eq!(9, diff.load.len());
        assert!(diff.unload.is_empty());
        assert_eq!(
            vec![
                ChunkPos::new(-1, -1),
                ChunkPos::new(0, -1),
                ChunkPos::new(1, -1),
                ChunkPos::new(-1, 0),
                ChunkPos::new(0, 0),
                ChunkPos::new(1, 0),
                ChunkPos::new(-1, 1),
                ChunkPos::new(0, 1),
                ChunkPos::new(1, 1),
            ],
            diff.load
        );
    }

    #[test]
    fn chunk_view_crossing_boundary_loads_and_unloads_diff() {
        let mut view = ChunkView::new(ChunkPos::new(0, 0), 1);
        let _ = view.update(ChunkPos::new(0, 0), 1);

        let diff = view.update(ChunkPos::new(1, 0), 1);

        assert_eq!(
            vec![
                ChunkPos::new(2, -1),
                ChunkPos::new(2, 0),
                ChunkPos::new(2, 1),
            ],
            diff.load
        );
        assert_eq!(
            vec![
                ChunkPos::new(-1, -1),
                ChunkPos::new(-1, 0),
                ChunkPos::new(-1, 1),
            ],
            diff.unload
        );
    }

    #[test]
    fn chunk_view_does_not_duplicate_unchanged_diff() {
        let mut view = ChunkView::new(ChunkPos::new(0, 0), 1);
        let _ = view.update(ChunkPos::new(0, 0), 1);

        let diff = view.update(ChunkPos::new(0, 0), 1);

        assert!(diff.load.is_empty());
        assert!(diff.unload.is_empty());
    }

    #[test]
    fn beta173_item_rules_classify_stack_limits_and_tools() {
        let dirt = beta173::item_rule(beta173::DIRT);
        assert_eq!(64, dirt.max_stack_size);
        assert!(dirt.is_placeable());
        assert!(!dirt.is_tool());

        let pickaxe = beta173::item_rule(beta173::WOODEN_PICKAXE);
        assert_eq!(1, pickaxe.max_stack_size);
        assert!(pickaxe.is_tool());
        assert_eq!(
            Some(beta173::ToolCategory::Pickaxe),
            pickaxe.tool_category()
        );
        assert!(pickaxe.can_damage_blocks_faster());

        let coal = beta173::item_rule(beta173::COAL);
        assert_eq!(64, coal.max_stack_size);
        assert!(!coal.is_placeable());
        assert!(!coal.is_consumable_placeholder());
    }
}
