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
        Hoe,
    }

    /// Ordered by harvest capability: gold tools harvest the same blocks as
    /// wood tools, so Gold sits between Wood and Stone.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ToolTier {
        Wood,
        Gold,
        Stone,
        Iron,
        Diamond,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ArmorSlot {
        Helmet,
        Chestplate,
        Leggings,
        Boots,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ItemKind {
        Air,
        Block,
        /// Block ids that only exist as placed blocks (fluids, fire, the
        /// block forms of items like doors and signs). Never placeable from
        /// an inventory stack.
        TechnicalBlock,
        Tool {
            category: ToolCategory,
            tier: ToolTier,
        },
        Armor {
            slot: ArmorSlot,
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

        pub const fn is_armor(self) -> bool {
            matches!(self.kind, ItemKind::Armor { .. })
        }

        pub const fn armor_slot(self) -> Option<ArmorSlot> {
            match self.kind {
                ItemKind::Armor { slot } => Some(slot),
                _ => None,
            }
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
    pub const SPONGE: i16 = 19;
    pub const GLASS: i16 = 20;
    pub const LAPIS_ORE: i16 = 21;
    pub const LAPIS_BLOCK: i16 = 22;
    pub const DISPENSER: i16 = 23;
    pub const SANDSTONE: i16 = 24;
    pub const NOTE_BLOCK: i16 = 25;
    pub const BED_BLOCK: i16 = 26;
    pub const POWERED_RAIL: i16 = 27;
    pub const DETECTOR_RAIL: i16 = 28;
    pub const STICKY_PISTON: i16 = 29;
    pub const COBWEB: i16 = 30;
    pub const TALL_GRASS: i16 = 31;
    pub const DEAD_BUSH: i16 = 32;
    pub const PISTON: i16 = 33;
    pub const PISTON_HEAD: i16 = 34;
    pub const WOOL: i16 = 35;
    pub const PISTON_MOVING: i16 = 36;
    pub const DANDELION: i16 = 37;
    pub const ROSE: i16 = 38;
    pub const BROWN_MUSHROOM: i16 = 39;
    pub const RED_MUSHROOM: i16 = 40;
    pub const GOLD_BLOCK: i16 = 41;
    pub const IRON_BLOCK: i16 = 42;
    pub const DOUBLE_STONE_SLAB: i16 = 43;
    pub const STONE_SLAB: i16 = 44;
    pub const BRICKS: i16 = 45;
    pub const TNT: i16 = 46;
    pub const BOOKSHELF: i16 = 47;
    pub const MOSSY_COBBLESTONE: i16 = 48;
    pub const OBSIDIAN: i16 = 49;
    pub const TORCH: i16 = 50;
    pub const FIRE: i16 = 51;
    pub const MOB_SPAWNER: i16 = 52;
    pub const WOODEN_STAIRS: i16 = 53;
    pub const CHEST: i16 = 54;
    pub const REDSTONE_WIRE: i16 = 55;
    pub const DIAMOND_ORE: i16 = 56;
    pub const DIAMOND_BLOCK: i16 = 57;
    pub const CRAFTING_TABLE: i16 = 58;
    pub const CROPS: i16 = 59;
    pub const FARMLAND: i16 = 60;
    pub const FURNACE: i16 = 61;
    pub const LIT_FURNACE: i16 = 62;
    pub const SIGN_POST: i16 = 63;
    pub const WOODEN_DOOR_BLOCK: i16 = 64;
    pub const LADDER: i16 = 65;
    pub const RAIL: i16 = 66;
    pub const COBBLESTONE_STAIRS: i16 = 67;
    pub const WALL_SIGN: i16 = 68;
    pub const LEVER: i16 = 69;
    pub const STONE_PRESSURE_PLATE: i16 = 70;
    pub const IRON_DOOR_BLOCK: i16 = 71;
    pub const WOODEN_PRESSURE_PLATE: i16 = 72;
    pub const REDSTONE_ORE: i16 = 73;
    pub const GLOWING_REDSTONE_ORE: i16 = 74;
    pub const REDSTONE_TORCH_OFF: i16 = 75;
    pub const REDSTONE_TORCH: i16 = 76;
    pub const STONE_BUTTON: i16 = 77;
    pub const SNOW_LAYER: i16 = 78;
    pub const ICE: i16 = 79;
    pub const SNOW_BLOCK: i16 = 80;
    pub const CACTUS: i16 = 81;
    pub const CLAY_BLOCK: i16 = 82;
    pub const SUGAR_CANE_BLOCK: i16 = 83;
    pub const JUKEBOX: i16 = 84;
    pub const FENCE: i16 = 85;
    pub const PUMPKIN: i16 = 86;
    pub const NETHERRACK: i16 = 87;
    pub const SOUL_SAND: i16 = 88;
    pub const GLOWSTONE: i16 = 89;
    pub const PORTAL: i16 = 90;
    pub const JACK_O_LANTERN: i16 = 91;
    pub const CAKE_BLOCK: i16 = 92;
    pub const REPEATER_OFF: i16 = 93;
    pub const REPEATER_ON: i16 = 94;
    pub const LOCKED_CHEST: i16 = 95;
    pub const TRAPDOOR: i16 = 96;
    pub const MAX_BLOCK_ID: i16 = TRAPDOOR;

    pub const IRON_SHOVEL: i16 = 256;
    pub const IRON_PICKAXE: i16 = 257;
    pub const IRON_AXE: i16 = 258;
    pub const FLINT_AND_STEEL: i16 = 259;
    pub const APPLE: i16 = 260;
    pub const BOW: i16 = 261;
    pub const ARROW: i16 = 262;
    pub const COAL: i16 = 263;
    pub const DIAMOND: i16 = 264;
    pub const IRON_INGOT: i16 = 265;
    pub const GOLD_INGOT: i16 = 266;
    pub const IRON_SWORD: i16 = 267;
    pub const WOODEN_SWORD: i16 = 268;
    pub const WOODEN_SHOVEL: i16 = 269;
    pub const WOODEN_PICKAXE: i16 = 270;
    pub const WOODEN_AXE: i16 = 271;
    pub const STONE_SWORD: i16 = 272;
    pub const STONE_SHOVEL: i16 = 273;
    pub const STONE_PICKAXE: i16 = 274;
    pub const STONE_AXE: i16 = 275;
    pub const DIAMOND_SWORD: i16 = 276;
    pub const DIAMOND_SHOVEL: i16 = 277;
    pub const DIAMOND_PICKAXE: i16 = 278;
    pub const DIAMOND_AXE: i16 = 279;
    pub const STICK: i16 = 280;
    pub const BOWL: i16 = 281;
    pub const MUSHROOM_STEW: i16 = 282;
    pub const GOLDEN_SWORD: i16 = 283;
    pub const GOLDEN_SHOVEL: i16 = 284;
    pub const GOLDEN_PICKAXE: i16 = 285;
    pub const GOLDEN_AXE: i16 = 286;
    pub const STRING: i16 = 287;
    pub const FEATHER: i16 = 288;
    pub const GUNPOWDER: i16 = 289;
    pub const WOODEN_HOE: i16 = 290;
    pub const STONE_HOE: i16 = 291;
    pub const IRON_HOE: i16 = 292;
    pub const DIAMOND_HOE: i16 = 293;
    pub const GOLDEN_HOE: i16 = 294;
    pub const SEEDS: i16 = 295;
    pub const WHEAT: i16 = 296;
    pub const BREAD: i16 = 297;
    pub const LEATHER_CAP: i16 = 298;
    pub const LEATHER_TUNIC: i16 = 299;
    pub const LEATHER_PANTS: i16 = 300;
    pub const LEATHER_BOOTS: i16 = 301;
    pub const CHAINMAIL_HELMET: i16 = 302;
    pub const CHAINMAIL_CHESTPLATE: i16 = 303;
    pub const CHAINMAIL_LEGGINGS: i16 = 304;
    pub const CHAINMAIL_BOOTS: i16 = 305;
    pub const IRON_HELMET: i16 = 306;
    pub const IRON_CHESTPLATE: i16 = 307;
    pub const IRON_LEGGINGS: i16 = 308;
    pub const IRON_BOOTS: i16 = 309;
    pub const DIAMOND_HELMET: i16 = 310;
    pub const DIAMOND_CHESTPLATE: i16 = 311;
    pub const DIAMOND_LEGGINGS: i16 = 312;
    pub const DIAMOND_BOOTS: i16 = 313;
    pub const GOLDEN_HELMET: i16 = 314;
    pub const GOLDEN_CHESTPLATE: i16 = 315;
    pub const GOLDEN_LEGGINGS: i16 = 316;
    pub const GOLDEN_BOOTS: i16 = 317;
    pub const FLINT: i16 = 318;
    pub const RAW_PORKCHOP: i16 = 319;
    pub const COOKED_PORKCHOP: i16 = 320;
    pub const PAINTING: i16 = 321;
    pub const GOLDEN_APPLE: i16 = 322;
    pub const SIGN: i16 = 323;
    pub const WOODEN_DOOR: i16 = 324;
    pub const BUCKET: i16 = 325;
    pub const WATER_BUCKET: i16 = 326;
    pub const LAVA_BUCKET: i16 = 327;
    pub const MINECART: i16 = 328;
    pub const SADDLE: i16 = 329;
    pub const IRON_DOOR: i16 = 330;
    pub const REDSTONE: i16 = 331;
    pub const SNOWBALL: i16 = 332;
    pub const BOAT: i16 = 333;
    pub const LEATHER: i16 = 334;
    pub const MILK_BUCKET: i16 = 335;
    pub const BRICK: i16 = 336;
    pub const CLAY_BALL: i16 = 337;
    pub const SUGAR_CANE: i16 = 338;
    pub const PAPER: i16 = 339;
    pub const BOOK: i16 = 340;
    pub const SLIME_BALL: i16 = 341;
    pub const CHEST_MINECART: i16 = 342;
    pub const FURNACE_MINECART: i16 = 343;
    pub const EGG: i16 = 344;
    pub const COMPASS: i16 = 345;
    pub const FISHING_ROD: i16 = 346;
    pub const CLOCK: i16 = 347;
    pub const GLOWSTONE_DUST: i16 = 348;
    pub const RAW_FISH: i16 = 349;
    pub const COOKED_FISH: i16 = 350;
    pub const DYE: i16 = 351;
    pub const BONE: i16 = 352;
    pub const SUGAR: i16 = 353;
    pub const CAKE: i16 = 354;
    pub const BED: i16 = 355;
    pub const REPEATER: i16 = 356;
    pub const COOKIE: i16 = 357;
    pub const MAX_SIMPLE_ITEM_ID: i16 = COOKIE;
    pub const MUSIC_DISC_13: i16 = 2256;
    pub const MUSIC_DISC_CAT: i16 = 2257;

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
            WATER => technical(id, "water"),
            STATIONARY_WATER => technical(id, "stationary_water"),
            LAVA => technical(id, "lava"),
            STATIONARY_LAVA => technical(id, "stationary_lava"),
            SAND => block(id, "sand"),
            GRAVEL => block(id, "gravel"),
            GOLD_ORE => block(id, "gold_ore"),
            IRON_ORE => block(id, "iron_ore"),
            COAL_ORE => block(id, "coal_ore"),
            LOG => block(id, "log"),
            LEAVES => block(id, "leaves"),
            SPONGE => block(id, "sponge"),
            GLASS => block(id, "glass"),
            LAPIS_ORE => block(id, "lapis_ore"),
            LAPIS_BLOCK => block(id, "lapis_block"),
            DISPENSER => block(id, "dispenser"),
            SANDSTONE => block(id, "sandstone"),
            NOTE_BLOCK => block(id, "note_block"),
            BED_BLOCK => technical(id, "bed_block"),
            POWERED_RAIL => block(id, "powered_rail"),
            DETECTOR_RAIL => block(id, "detector_rail"),
            STICKY_PISTON => block(id, "sticky_piston"),
            COBWEB => block(id, "cobweb"),
            TALL_GRASS => block(id, "tall_grass"),
            DEAD_BUSH => block(id, "dead_bush"),
            PISTON => block(id, "piston"),
            PISTON_HEAD => technical(id, "piston_head"),
            WOOL => block(id, "wool"),
            PISTON_MOVING => technical(id, "piston_moving"),
            DANDELION => block(id, "dandelion"),
            ROSE => block(id, "rose"),
            BROWN_MUSHROOM => block(id, "brown_mushroom"),
            RED_MUSHROOM => block(id, "red_mushroom"),
            GOLD_BLOCK => block(id, "gold_block"),
            IRON_BLOCK => block(id, "iron_block"),
            DOUBLE_STONE_SLAB => technical(id, "double_stone_slab"),
            STONE_SLAB => block(id, "stone_slab"),
            BRICKS => block(id, "bricks"),
            TNT => block(id, "tnt"),
            BOOKSHELF => block(id, "bookshelf"),
            MOSSY_COBBLESTONE => block(id, "mossy_cobblestone"),
            OBSIDIAN => block(id, "obsidian"),
            TORCH => block(id, "torch"),
            FIRE => technical(id, "fire"),
            MOB_SPAWNER => block(id, "mob_spawner"),
            WOODEN_STAIRS => block(id, "wooden_stairs"),
            CHEST => block(id, "chest"),
            REDSTONE_WIRE => technical(id, "redstone_wire"),
            DIAMOND_ORE => block(id, "diamond_ore"),
            DIAMOND_BLOCK => block(id, "diamond_block"),
            CRAFTING_TABLE => block(id, "crafting_table"),
            CROPS => technical(id, "crops"),
            FARMLAND => block(id, "farmland"),
            FURNACE => block(id, "furnace"),
            LIT_FURNACE => block(id, "lit_furnace"),
            SIGN_POST => technical(id, "sign_post"),
            WOODEN_DOOR_BLOCK => technical(id, "wooden_door_block"),
            LADDER => block(id, "ladder"),
            RAIL => block(id, "rail"),
            COBBLESTONE_STAIRS => block(id, "cobblestone_stairs"),
            WALL_SIGN => technical(id, "wall_sign"),
            LEVER => block(id, "lever"),
            STONE_PRESSURE_PLATE => block(id, "stone_pressure_plate"),
            IRON_DOOR_BLOCK => technical(id, "iron_door_block"),
            WOODEN_PRESSURE_PLATE => block(id, "wooden_pressure_plate"),
            REDSTONE_ORE => block(id, "redstone_ore"),
            GLOWING_REDSTONE_ORE => technical(id, "glowing_redstone_ore"),
            REDSTONE_TORCH_OFF => technical(id, "redstone_torch_off"),
            REDSTONE_TORCH => block(id, "redstone_torch"),
            STONE_BUTTON => block(id, "stone_button"),
            SNOW_LAYER => block(id, "snow_layer"),
            ICE => block(id, "ice"),
            SNOW_BLOCK => block(id, "snow_block"),
            CACTUS => block(id, "cactus"),
            CLAY_BLOCK => block(id, "clay_block"),
            SUGAR_CANE_BLOCK => technical(id, "sugar_cane_block"),
            JUKEBOX => block(id, "jukebox"),
            FENCE => block(id, "fence"),
            PUMPKIN => block(id, "pumpkin"),
            NETHERRACK => block(id, "netherrack"),
            SOUL_SAND => block(id, "soul_sand"),
            GLOWSTONE => block(id, "glowstone"),
            PORTAL => technical(id, "portal"),
            JACK_O_LANTERN => block(id, "jack_o_lantern"),
            CAKE_BLOCK => technical(id, "cake_block"),
            REPEATER_OFF => technical(id, "repeater_off"),
            REPEATER_ON => technical(id, "repeater_on"),
            LOCKED_CHEST => block(id, "locked_chest"),
            TRAPDOOR => block(id, "trapdoor"),
            IRON_SHOVEL => tool(id, "iron_shovel", ToolCategory::Shovel, ToolTier::Iron),
            IRON_PICKAXE => tool(id, "iron_pickaxe", ToolCategory::Pickaxe, ToolTier::Iron),
            IRON_AXE => tool(id, "iron_axe", ToolCategory::Axe, ToolTier::Iron),
            FLINT_AND_STEEL => item(id, "flint_and_steel", 1, ItemKind::Material),
            APPLE => food(id, "apple", 1),
            BOW => item(id, "bow", 1, ItemKind::Material),
            ARROW => item(id, "arrow", 64, ItemKind::Material),
            COAL => item(id, "coal", 64, ItemKind::Material),
            DIAMOND => item(id, "diamond", 64, ItemKind::Material),
            IRON_INGOT => item(id, "iron_ingot", 64, ItemKind::Material),
            GOLD_INGOT => item(id, "gold_ingot", 64, ItemKind::Material),
            IRON_SWORD => tool(id, "iron_sword", ToolCategory::Sword, ToolTier::Iron),
            WOODEN_SWORD => tool(id, "wooden_sword", ToolCategory::Sword, ToolTier::Wood),
            WOODEN_SHOVEL => tool(id, "wooden_shovel", ToolCategory::Shovel, ToolTier::Wood),
            WOODEN_PICKAXE => tool(id, "wooden_pickaxe", ToolCategory::Pickaxe, ToolTier::Wood),
            WOODEN_AXE => tool(id, "wooden_axe", ToolCategory::Axe, ToolTier::Wood),
            STONE_SWORD => tool(id, "stone_sword", ToolCategory::Sword, ToolTier::Stone),
            STONE_SHOVEL => tool(id, "stone_shovel", ToolCategory::Shovel, ToolTier::Stone),
            STONE_PICKAXE => tool(id, "stone_pickaxe", ToolCategory::Pickaxe, ToolTier::Stone),
            STONE_AXE => tool(id, "stone_axe", ToolCategory::Axe, ToolTier::Stone),
            DIAMOND_SWORD => tool(id, "diamond_sword", ToolCategory::Sword, ToolTier::Diamond),
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
            STICK => item(id, "stick", 64, ItemKind::Material),
            BOWL => item(id, "bowl", 64, ItemKind::Material),
            MUSHROOM_STEW => food(id, "mushroom_stew", 1),
            GOLDEN_SWORD => tool(id, "golden_sword", ToolCategory::Sword, ToolTier::Gold),
            GOLDEN_SHOVEL => tool(id, "golden_shovel", ToolCategory::Shovel, ToolTier::Gold),
            GOLDEN_PICKAXE => tool(id, "golden_pickaxe", ToolCategory::Pickaxe, ToolTier::Gold),
            GOLDEN_AXE => tool(id, "golden_axe", ToolCategory::Axe, ToolTier::Gold),
            STRING => item(id, "string", 64, ItemKind::Material),
            FEATHER => item(id, "feather", 64, ItemKind::Material),
            GUNPOWDER => item(id, "gunpowder", 64, ItemKind::Material),
            WOODEN_HOE => tool(id, "wooden_hoe", ToolCategory::Hoe, ToolTier::Wood),
            STONE_HOE => tool(id, "stone_hoe", ToolCategory::Hoe, ToolTier::Stone),
            IRON_HOE => tool(id, "iron_hoe", ToolCategory::Hoe, ToolTier::Iron),
            DIAMOND_HOE => tool(id, "diamond_hoe", ToolCategory::Hoe, ToolTier::Diamond),
            GOLDEN_HOE => tool(id, "golden_hoe", ToolCategory::Hoe, ToolTier::Gold),
            SEEDS => item(id, "seeds", 64, ItemKind::Material),
            WHEAT => item(id, "wheat", 64, ItemKind::Material),
            BREAD => food(id, "bread", 1),
            LEATHER_CAP => armor(id, "leather_cap", ArmorSlot::Helmet),
            LEATHER_TUNIC => armor(id, "leather_tunic", ArmorSlot::Chestplate),
            LEATHER_PANTS => armor(id, "leather_pants", ArmorSlot::Leggings),
            LEATHER_BOOTS => armor(id, "leather_boots", ArmorSlot::Boots),
            CHAINMAIL_HELMET => armor(id, "chainmail_helmet", ArmorSlot::Helmet),
            CHAINMAIL_CHESTPLATE => armor(id, "chainmail_chestplate", ArmorSlot::Chestplate),
            CHAINMAIL_LEGGINGS => armor(id, "chainmail_leggings", ArmorSlot::Leggings),
            CHAINMAIL_BOOTS => armor(id, "chainmail_boots", ArmorSlot::Boots),
            IRON_HELMET => armor(id, "iron_helmet", ArmorSlot::Helmet),
            IRON_CHESTPLATE => armor(id, "iron_chestplate", ArmorSlot::Chestplate),
            IRON_LEGGINGS => armor(id, "iron_leggings", ArmorSlot::Leggings),
            IRON_BOOTS => armor(id, "iron_boots", ArmorSlot::Boots),
            DIAMOND_HELMET => armor(id, "diamond_helmet", ArmorSlot::Helmet),
            DIAMOND_CHESTPLATE => armor(id, "diamond_chestplate", ArmorSlot::Chestplate),
            DIAMOND_LEGGINGS => armor(id, "diamond_leggings", ArmorSlot::Leggings),
            DIAMOND_BOOTS => armor(id, "diamond_boots", ArmorSlot::Boots),
            GOLDEN_HELMET => armor(id, "golden_helmet", ArmorSlot::Helmet),
            GOLDEN_CHESTPLATE => armor(id, "golden_chestplate", ArmorSlot::Chestplate),
            GOLDEN_LEGGINGS => armor(id, "golden_leggings", ArmorSlot::Leggings),
            GOLDEN_BOOTS => armor(id, "golden_boots", ArmorSlot::Boots),
            FLINT => item(id, "flint", 64, ItemKind::Material),
            RAW_PORKCHOP => food(id, "raw_porkchop", 1),
            COOKED_PORKCHOP => food(id, "cooked_porkchop", 1),
            PAINTING => item(id, "painting", 64, ItemKind::Material),
            GOLDEN_APPLE => food(id, "golden_apple", 1),
            SIGN => item(id, "sign", 1, ItemKind::Material),
            WOODEN_DOOR => item(id, "wooden_door", 1, ItemKind::Material),
            BUCKET => item(id, "bucket", 1, ItemKind::Material),
            WATER_BUCKET => item(id, "water_bucket", 1, ItemKind::Material),
            LAVA_BUCKET => item(id, "lava_bucket", 1, ItemKind::Material),
            MINECART => item(id, "minecart", 1, ItemKind::Material),
            SADDLE => item(id, "saddle", 1, ItemKind::Material),
            IRON_DOOR => item(id, "iron_door", 1, ItemKind::Material),
            REDSTONE => item(id, "redstone", 64, ItemKind::Material),
            SNOWBALL => item(id, "snowball", 16, ItemKind::Material),
            BOAT => item(id, "boat", 1, ItemKind::Material),
            LEATHER => item(id, "leather", 64, ItemKind::Material),
            MILK_BUCKET => item(id, "milk_bucket", 1, ItemKind::Material),
            BRICK => item(id, "brick", 64, ItemKind::Material),
            CLAY_BALL => item(id, "clay_ball", 64, ItemKind::Material),
            SUGAR_CANE => item(id, "sugar_cane", 64, ItemKind::Material),
            PAPER => item(id, "paper", 64, ItemKind::Material),
            BOOK => item(id, "book", 64, ItemKind::Material),
            SLIME_BALL => item(id, "slime_ball", 64, ItemKind::Material),
            CHEST_MINECART => item(id, "chest_minecart", 1, ItemKind::Material),
            FURNACE_MINECART => item(id, "furnace_minecart", 1, ItemKind::Material),
            EGG => item(id, "egg", 16, ItemKind::Material),
            COMPASS => item(id, "compass", 64, ItemKind::Material),
            FISHING_ROD => item(id, "fishing_rod", 1, ItemKind::Material),
            CLOCK => item(id, "clock", 64, ItemKind::Material),
            GLOWSTONE_DUST => item(id, "glowstone_dust", 64, ItemKind::Material),
            RAW_FISH => food(id, "raw_fish", 1),
            COOKED_FISH => food(id, "cooked_fish", 1),
            DYE => item(id, "dye", 64, ItemKind::Material),
            BONE => item(id, "bone", 64, ItemKind::Material),
            SUGAR => item(id, "sugar", 64, ItemKind::Material),
            CAKE => item(id, "cake", 1, ItemKind::Material),
            BED => item(id, "bed", 1, ItemKind::Material),
            REPEATER => item(id, "repeater", 64, ItemKind::Material),
            COOKIE => food(id, "cookie", 8),
            MUSIC_DISC_13 => item(id, "music_disc_13", 1, ItemKind::Material),
            MUSIC_DISC_CAT => item(id, "music_disc_cat", 1, ItemKind::Material),
            _ => item(id, "unknown", 64, ItemKind::Unknown),
        }
    }

    const fn block(id: i16, debug_name: &'static str) -> ItemRule {
        item(id, debug_name, 64, ItemKind::Block)
    }

    const fn technical(id: i16, debug_name: &'static str) -> ItemRule {
        item(id, debug_name, 64, ItemKind::TechnicalBlock)
    }

    const fn armor(id: i16, debug_name: &'static str, slot: ArmorSlot) -> ItemRule {
        item(id, debug_name, 1, ItemKind::Armor { slot })
    }

    const fn food(id: i16, debug_name: &'static str, max_stack_size: u8) -> ItemRule {
        item(
            id,
            debug_name,
            max_stack_size,
            ItemKind::ConsumablePlaceholder,
        )
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

    #[test]
    fn beta173_item_rules_cover_all_block_and_item_ids() {
        for id in 1..=beta173::MAX_BLOCK_ID {
            assert_ne!(
                "unknown",
                beta173::item_rule(id).debug_name,
                "block id {id} is missing an item rule"
            );
        }
        for id in 256..=beta173::MAX_SIMPLE_ITEM_ID {
            assert_ne!(
                "unknown",
                beta173::item_rule(id).debug_name,
                "item id {id} is missing an item rule"
            );
        }
        assert_eq!(
            "music_disc_13",
            beta173::item_rule(beta173::MUSIC_DISC_13).debug_name
        );
        assert_eq!(
            "music_disc_cat",
            beta173::item_rule(beta173::MUSIC_DISC_CAT).debug_name
        );
    }

    #[test]
    fn beta173_gold_tier_harvests_like_wood() {
        use beta173::ToolTier;

        assert!(ToolTier::Gold > ToolTier::Wood);
        assert!(ToolTier::Gold < ToolTier::Stone);
        assert_eq!(
            Some(ToolTier::Gold),
            beta173::item_rule(beta173::GOLDEN_PICKAXE).tool_tier()
        );
    }

    #[test]
    fn beta173_item_kinds_classify_armor_food_and_technical_blocks() {
        let helmet = beta173::item_rule(beta173::IRON_HELMET);
        assert!(helmet.is_armor());
        assert_eq!(Some(beta173::ArmorSlot::Helmet), helmet.armor_slot());
        assert_eq!(1, helmet.max_stack_size);

        let bread = beta173::item_rule(beta173::BREAD);
        assert!(bread.is_consumable_placeholder());
        assert_eq!(1, bread.max_stack_size);
        assert_eq!(8, beta173::item_rule(beta173::COOKIE).max_stack_size);
        assert_eq!(16, beta173::item_rule(beta173::EGG).max_stack_size);
        assert_eq!(16, beta173::item_rule(beta173::SNOWBALL).max_stack_size);

        assert!(!beta173::item_rule(beta173::WATER).is_placeable());
        assert!(!beta173::item_rule(beta173::FIRE).is_placeable());
        assert!(!beta173::item_rule(beta173::WOODEN_DOOR_BLOCK).is_placeable());
        assert!(beta173::item_rule(beta173::WOOL).is_placeable());

        let hoe = beta173::item_rule(beta173::DIAMOND_HOE);
        assert_eq!(Some(beta173::ToolCategory::Hoe), hoe.tool_category());
        assert_eq!(1, hoe.max_stack_size);
    }
}
