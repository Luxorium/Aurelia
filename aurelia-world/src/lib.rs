use aurelia_common::{BlockPos, ChunkPos};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

pub const WORLD_HEIGHT: usize = 128;
pub const SEA_LEVEL: usize = 64;
pub const FLAT_GRASS_Y: usize = 63;
pub const SPAWN_POSITION: BlockPos = BlockPos::new(0, 65, 0);

pub mod beta173 {
    use aurelia_common::beta173::{self as item, ItemRule, ToolCategory, ToolTier};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Material {
        Air,
        Rock,
        Dirt,
        Wood,
        Leaves,
        Glass,
        Sand,
        Decoration,
        Container,
        Unbreakable,
        Fluid,
        Plant,
        Metal,
        Cloth,
        Snow,
        Ice,
        Clay,
        Web,
        Explosive,
        Circuit,
        Cake,
        Sponge,
        Cactus,
        Fire,
        Portal,
        Vegetable,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct BlockRule {
        pub id: u8,
        pub debug_name: &'static str,
        pub material: Material,
        pub hardness: f32,
        pub preferred_tool: Option<ToolCategory>,
        pub minimum_tier: Option<ToolTier>,
        pub solid: bool,
        pub transparent: bool,
        pub light_emission: u8,
        pub drop: BlockDrop,
        pub approximate: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BlockDrop {
        Nothing,
        SelfItem,
        Item {
            item_id: i16,
            count: u8,
            damage: i16,
        },
        RequiresTool {
            item_id: i16,
            count: u8,
            damage: i16,
        },
    }

    impl BlockRule {
        pub fn can_harvest(self, held: Option<ItemRule>) -> bool {
            match self.drop {
                BlockDrop::RequiresTool { .. } => self.has_required_tool(held),
                _ => true,
            }
        }

        pub fn drop_for(self, held: Option<ItemRule>, metadata: u8) -> Option<(i16, u8, i16)> {
            match self.drop {
                BlockDrop::Nothing => None,
                BlockDrop::SelfItem => Some((i16::from(self.id), 1, i16::from(metadata & 0x0F))),
                BlockDrop::Item {
                    item_id,
                    count,
                    damage,
                } => Some((item_id, count, damage)),
                BlockDrop::RequiresTool {
                    item_id,
                    count,
                    damage,
                } => self
                    .has_required_tool(held)
                    .then_some((item_id, count, damage)),
            }
        }

        fn has_required_tool(self, held: Option<ItemRule>) -> bool {
            let Some(required_category) = self.preferred_tool else {
                return true;
            };
            let Some(held) = held else {
                return false;
            };
            if held.tool_category() != Some(required_category) {
                return false;
            }
            match (self.minimum_tier, held.tool_tier()) {
                (Some(required), Some(actual)) => actual >= required,
                (None, Some(_)) => true,
                _ => false,
            }
        }
    }

    pub fn block_rule(id: u8) -> BlockRule {
        match i16::from(id) {
            item::AIR => rule(
                id,
                "air",
                Material::Air,
                0.0,
                None,
                None,
                false,
                true,
                0,
                BlockDrop::Nothing,
                false,
            ),
            item::STONE => rule(
                id,
                "stone",
                Material::Rock,
                1.5,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                false,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::COBBLESTONE,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::GRASS => rule(
                id,
                "grass",
                Material::Dirt,
                0.6,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::Item {
                    item_id: item::DIRT,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::DIRT => rule(
                id,
                "dirt",
                Material::Dirt,
                0.5,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::COBBLESTONE => rule(
                id,
                "cobblestone",
                Material::Rock,
                2.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                false,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::COBBLESTONE,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::PLANKS => rule(
                id,
                "planks",
                Material::Wood,
                2.0,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::BEDROCK => rule(
                id,
                "bedrock",
                Material::Unbreakable,
                -1.0,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::Nothing,
                false,
            ),
            item::SAND => rule(
                id,
                "sand",
                Material::Sand,
                0.5,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::GRAVEL => rule(
                id,
                "gravel",
                Material::Sand,
                0.6,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::GOLD_ORE => ore(id, "gold_ore", item::GOLD_ORE, ToolTier::Iron),
            item::IRON_ORE => ore(id, "iron_ore", item::IRON_ORE, ToolTier::Stone),
            item::COAL_ORE => ore(id, "coal_ore", item::COAL, ToolTier::Wood),
            item::LOG => rule(
                id,
                "log",
                Material::Wood,
                2.0,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::LEAVES => rule(
                id,
                "leaves",
                Material::Leaves,
                0.2,
                None,
                None,
                true,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::GLASS => rule(
                id,
                "glass",
                Material::Glass,
                0.3,
                None,
                None,
                true,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::TORCH => rule(
                id,
                "torch",
                Material::Decoration,
                0.0,
                None,
                None,
                false,
                true,
                14,
                BlockDrop::SelfItem,
                true,
            ),
            item::CHEST => rule(
                id,
                "chest",
                Material::Container,
                2.5,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::DIAMOND_ORE => ore(id, "diamond_ore", item::DIAMOND, ToolTier::Iron),
            item::CRAFTING_TABLE => rule(
                id,
                "crafting_table",
                Material::Wood,
                2.5,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::FURNACE | item::LIT_FURNACE => rule(
                id,
                "furnace",
                Material::Rock,
                3.5,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                false,
                if i16::from(id) == item::LIT_FURNACE {
                    13
                } else {
                    0
                },
                BlockDrop::RequiresTool {
                    item_id: item::FURNACE,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::REDSTONE_ORE | item::GLOWING_REDSTONE_ORE => rule(
                id,
                "redstone_ore",
                Material::Rock,
                3.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Iron),
                true,
                false,
                if i16::from(id) == item::GLOWING_REDSTONE_ORE {
                    9
                } else {
                    0
                },
                BlockDrop::RequiresTool {
                    item_id: item::REDSTONE,
                    count: 4,
                    damage: 0,
                },
                true,
            ),
            item::SAPLING => plant(id, "sapling", BlockDrop::SelfItem),
            item::WATER | item::STATIONARY_WATER => rule(
                id,
                "water",
                Material::Fluid,
                100.0,
                None,
                None,
                false,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::LAVA | item::STATIONARY_LAVA => rule(
                id,
                "lava",
                Material::Fluid,
                100.0,
                None,
                None,
                false,
                true,
                15,
                BlockDrop::Nothing,
                true,
            ),
            item::SPONGE => rule(
                id,
                "sponge",
                Material::Sponge,
                0.6,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::LAPIS_ORE => rule(
                id,
                "lapis_ore",
                Material::Rock,
                3.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Stone),
                true,
                false,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::DYE,
                    count: 4,
                    damage: 4,
                },
                true,
            ),
            item::LAPIS_BLOCK => {
                pick_block(id, "lapis_block", Material::Rock, 3.0, ToolTier::Stone)
            }
            item::DISPENSER => pick_block(id, "dispenser", Material::Rock, 3.5, ToolTier::Wood),
            item::SANDSTONE => pick_block(id, "sandstone", Material::Rock, 0.8, ToolTier::Wood),
            item::NOTE_BLOCK => wood_block(id, "note_block", 0.8),
            item::BED_BLOCK => rule(
                id,
                "bed",
                Material::Cloth,
                0.2,
                None,
                None,
                true,
                true,
                0,
                BlockDrop::Item {
                    item_id: item::BED,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::RAIL => circuit(id, "rail", 0.7, 0, BlockDrop::SelfItem),
            item::POWERED_RAIL => circuit(id, "powered_rail", 0.7, 0, BlockDrop::SelfItem),
            item::DETECTOR_RAIL => circuit(id, "detector_rail", 0.7, 0, BlockDrop::SelfItem),
            item::STICKY_PISTON => rule(
                id,
                "sticky_piston",
                Material::Rock,
                0.5,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::PISTON => rule(
                id,
                "piston",
                Material::Rock,
                0.5,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::PISTON_HEAD => rule(
                id,
                "piston_head",
                Material::Rock,
                0.5,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::PISTON_MOVING => rule(
                id,
                "piston_moving",
                Material::Rock,
                -1.0,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::COBWEB => rule(
                id,
                "cobweb",
                Material::Web,
                4.0,
                None,
                None,
                false,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::TALL_GRASS => plant(id, "tall_grass", BlockDrop::Nothing),
            item::DEAD_BUSH => plant(id, "dead_bush", BlockDrop::Nothing),
            item::WOOL => rule(
                id,
                "wool",
                Material::Cloth,
                0.8,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::DANDELION => plant(id, "dandelion", BlockDrop::SelfItem),
            item::ROSE => plant(id, "rose", BlockDrop::SelfItem),
            item::RED_MUSHROOM => plant(id, "red_mushroom", BlockDrop::SelfItem),
            item::BROWN_MUSHROOM => rule(
                id,
                "brown_mushroom",
                Material::Plant,
                0.0,
                None,
                None,
                false,
                true,
                1,
                BlockDrop::SelfItem,
                true,
            ),
            item::GOLD_BLOCK => pick_block(id, "gold_block", Material::Metal, 3.0, ToolTier::Iron),
            item::IRON_BLOCK => pick_block(id, "iron_block", Material::Metal, 5.0, ToolTier::Stone),
            item::DIAMOND_BLOCK => {
                pick_block(id, "diamond_block", Material::Metal, 5.0, ToolTier::Iron)
            }
            item::DOUBLE_STONE_SLAB => rule(
                id,
                "double_stone_slab",
                Material::Rock,
                2.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                false,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::STONE_SLAB,
                    count: 2,
                    damage: 0,
                },
                true,
            ),
            item::STONE_SLAB => rule(
                id,
                "stone_slab",
                Material::Rock,
                2.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                true,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::STONE_SLAB,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::BRICKS => pick_block(id, "bricks", Material::Rock, 2.0, ToolTier::Wood),
            item::TNT => rule(
                id,
                "tnt",
                Material::Explosive,
                0.0,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::BOOKSHELF => rule(
                id,
                "bookshelf",
                Material::Wood,
                1.5,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::MOSSY_COBBLESTONE => {
                pick_block(id, "mossy_cobblestone", Material::Rock, 2.0, ToolTier::Wood)
            }
            item::OBSIDIAN => pick_block(id, "obsidian", Material::Rock, 10.0, ToolTier::Diamond),
            item::FIRE => rule(
                id,
                "fire",
                Material::Fire,
                0.0,
                None,
                None,
                false,
                true,
                15,
                BlockDrop::Nothing,
                true,
            ),
            item::MOB_SPAWNER => rule(
                id,
                "mob_spawner",
                Material::Rock,
                5.0,
                Some(ToolCategory::Pickaxe),
                None,
                true,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::WOODEN_STAIRS => rule(
                id,
                "wooden_stairs",
                Material::Wood,
                2.0,
                Some(ToolCategory::Axe),
                None,
                true,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::COBBLESTONE_STAIRS => rule(
                id,
                "cobblestone_stairs",
                Material::Rock,
                2.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                true,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::COBBLESTONE_STAIRS,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::REDSTONE_WIRE => circuit(
                id,
                "redstone_wire",
                0.0,
                0,
                BlockDrop::Item {
                    item_id: item::REDSTONE,
                    count: 1,
                    damage: 0,
                },
            ),
            item::CROPS => plant(
                id,
                "crops",
                BlockDrop::Item {
                    item_id: item::SEEDS,
                    count: 1,
                    damage: 0,
                },
            ),
            item::FARMLAND => rule(
                id,
                "farmland",
                Material::Dirt,
                0.6,
                Some(ToolCategory::Shovel),
                None,
                true,
                true,
                0,
                BlockDrop::Item {
                    item_id: item::DIRT,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::SIGN_POST | item::WALL_SIGN => rule(
                id,
                "sign",
                Material::Wood,
                1.0,
                Some(ToolCategory::Axe),
                None,
                false,
                true,
                0,
                BlockDrop::Item {
                    item_id: item::SIGN,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::WOODEN_DOOR_BLOCK => rule(
                id,
                "wooden_door",
                Material::Wood,
                3.0,
                Some(ToolCategory::Axe),
                None,
                true,
                true,
                0,
                BlockDrop::Item {
                    item_id: item::WOODEN_DOOR,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::IRON_DOOR_BLOCK => rule(
                id,
                "iron_door",
                Material::Metal,
                5.0,
                Some(ToolCategory::Pickaxe),
                Some(ToolTier::Wood),
                true,
                true,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::IRON_DOOR,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::LADDER => rule(
                id,
                "ladder",
                Material::Decoration,
                0.4,
                None,
                None,
                false,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::LEVER => circuit(id, "lever", 0.5, 0, BlockDrop::SelfItem),
            item::STONE_PRESSURE_PLATE => rule(
                id,
                "stone_pressure_plate",
                Material::Rock,
                0.5,
                Some(ToolCategory::Pickaxe),
                None,
                false,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::WOODEN_PRESSURE_PLATE => rule(
                id,
                "wooden_pressure_plate",
                Material::Wood,
                0.5,
                Some(ToolCategory::Axe),
                None,
                false,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::REDSTONE_TORCH_OFF => circuit(
                id,
                "redstone_torch_off",
                0.0,
                0,
                BlockDrop::Item {
                    item_id: item::REDSTONE_TORCH,
                    count: 1,
                    damage: 0,
                },
            ),
            item::REDSTONE_TORCH => circuit(id, "redstone_torch", 0.0, 7, BlockDrop::SelfItem),
            item::STONE_BUTTON => rule(
                id,
                "stone_button",
                Material::Rock,
                0.5,
                None,
                None,
                false,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::SNOW_LAYER => rule(
                id,
                "snow_layer",
                Material::Snow,
                0.1,
                Some(ToolCategory::Shovel),
                None,
                false,
                true,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::SNOWBALL,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::ICE => rule(
                id,
                "ice",
                Material::Ice,
                0.5,
                Some(ToolCategory::Pickaxe),
                None,
                true,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::SNOW_BLOCK => rule(
                id,
                "snow_block",
                Material::Snow,
                0.2,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::RequiresTool {
                    item_id: item::SNOWBALL,
                    count: 4,
                    damage: 0,
                },
                true,
            ),
            item::CACTUS => rule(
                id,
                "cactus",
                Material::Cactus,
                0.4,
                None,
                None,
                true,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::CLAY_BLOCK => rule(
                id,
                "clay_block",
                Material::Clay,
                0.6,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::Item {
                    item_id: item::CLAY_BALL,
                    count: 4,
                    damage: 0,
                },
                true,
            ),
            item::SUGAR_CANE_BLOCK => plant(
                id,
                "sugar_cane",
                BlockDrop::Item {
                    item_id: item::SUGAR_CANE,
                    count: 1,
                    damage: 0,
                },
            ),
            item::JUKEBOX => wood_block(id, "jukebox", 2.0),
            item::FENCE => rule(
                id,
                "fence",
                Material::Wood,
                2.0,
                Some(ToolCategory::Axe),
                None,
                true,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::PUMPKIN => rule(
                id,
                "pumpkin",
                Material::Vegetable,
                1.0,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::JACK_O_LANTERN => rule(
                id,
                "jack_o_lantern",
                Material::Vegetable,
                1.0,
                Some(ToolCategory::Axe),
                None,
                true,
                false,
                15,
                BlockDrop::SelfItem,
                true,
            ),
            item::NETHERRACK => pick_block(id, "netherrack", Material::Rock, 0.4, ToolTier::Wood),
            item::SOUL_SAND => rule(
                id,
                "soul_sand",
                Material::Sand,
                0.5,
                Some(ToolCategory::Shovel),
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            item::GLOWSTONE => rule(
                id,
                "glowstone",
                Material::Glass,
                0.3,
                None,
                None,
                true,
                true,
                15,
                BlockDrop::Item {
                    item_id: item::GLOWSTONE_DUST,
                    count: 1,
                    damage: 0,
                },
                true,
            ),
            item::PORTAL => rule(
                id,
                "portal",
                Material::Portal,
                -1.0,
                None,
                None,
                false,
                true,
                11,
                BlockDrop::Nothing,
                true,
            ),
            item::CAKE_BLOCK => rule(
                id,
                "cake",
                Material::Cake,
                0.5,
                None,
                None,
                true,
                true,
                0,
                BlockDrop::Nothing,
                true,
            ),
            item::REPEATER_OFF => circuit(
                id,
                "repeater_off",
                0.0,
                0,
                BlockDrop::Item {
                    item_id: item::REPEATER,
                    count: 1,
                    damage: 0,
                },
            ),
            item::REPEATER_ON => circuit(
                id,
                "repeater_on",
                0.0,
                9,
                BlockDrop::Item {
                    item_id: item::REPEATER,
                    count: 1,
                    damage: 0,
                },
            ),
            item::LOCKED_CHEST => rule(
                id,
                "locked_chest",
                Material::Container,
                0.0,
                None,
                None,
                true,
                false,
                15,
                BlockDrop::Nothing,
                true,
            ),
            item::TRAPDOOR => rule(
                id,
                "trapdoor",
                Material::Wood,
                3.0,
                Some(ToolCategory::Axe),
                None,
                true,
                true,
                0,
                BlockDrop::SelfItem,
                true,
            ),
            _ => rule(
                id,
                "unknown",
                Material::Rock,
                1.0,
                None,
                None,
                true,
                false,
                0,
                BlockDrop::SelfItem,
                true,
            ),
        }
    }

    fn pick_block(
        id: u8,
        name: &'static str,
        material: Material,
        hardness: f32,
        tier: ToolTier,
    ) -> BlockRule {
        rule(
            id,
            name,
            material,
            hardness,
            Some(ToolCategory::Pickaxe),
            Some(tier),
            true,
            false,
            0,
            BlockDrop::RequiresTool {
                item_id: i16::from(id),
                count: 1,
                damage: 0,
            },
            true,
        )
    }

    fn wood_block(id: u8, name: &'static str, hardness: f32) -> BlockRule {
        rule(
            id,
            name,
            Material::Wood,
            hardness,
            Some(ToolCategory::Axe),
            None,
            true,
            false,
            0,
            BlockDrop::SelfItem,
            true,
        )
    }

    fn plant(id: u8, name: &'static str, drop: BlockDrop) -> BlockRule {
        rule(
            id,
            name,
            Material::Plant,
            0.0,
            None,
            None,
            false,
            true,
            0,
            drop,
            true,
        )
    }

    fn circuit(id: u8, name: &'static str, hardness: f32, light: u8, drop: BlockDrop) -> BlockRule {
        rule(
            id,
            name,
            Material::Circuit,
            hardness,
            None,
            None,
            false,
            true,
            light,
            drop,
            true,
        )
    }

    fn ore(id: u8, name: &'static str, item_id: i16, tier: ToolTier) -> BlockRule {
        rule(
            id,
            name,
            Material::Rock,
            3.0,
            Some(ToolCategory::Pickaxe),
            Some(tier),
            true,
            false,
            0,
            BlockDrop::RequiresTool {
                item_id,
                count: 1,
                damage: 0,
            },
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    const fn rule(
        id: u8,
        debug_name: &'static str,
        material: Material,
        hardness: f32,
        preferred_tool: Option<ToolCategory>,
        minimum_tier: Option<ToolTier>,
        solid: bool,
        transparent: bool,
        light_emission: u8,
        drop: BlockDrop,
        approximate: bool,
    ) -> BlockRule {
        BlockRule {
            id,
            debug_name,
            material,
            hardness,
            preferred_tool,
            minimum_tier,
            solid,
            transparent,
            light_emission,
            drop,
            approximate,
        }
    }
}

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

    pub fn copy_metadata(&self) -> Vec<u8> {
        self.metadata.clone()
    }

    pub fn from_arrays(
        pos: ChunkPos,
        block_ids: Vec<u8>,
        metadata: Vec<u8>,
    ) -> Result<Self, String> {
        if block_ids.len() != Self::BLOCK_COUNT {
            return Err(format!(
                "block id array length {} did not match {}",
                block_ids.len(),
                Self::BLOCK_COUNT
            ));
        }
        if metadata.len() != Self::BLOCK_COUNT {
            return Err(format!(
                "metadata array length {} did not match {}",
                metadata.len(),
                Self::BLOCK_COUNT
            ));
        }
        if metadata.iter().any(|value| *value > 15) {
            return Err("metadata values must fit in 4 bits".to_string());
        }
        Ok(Self {
            pos,
            block_ids,
            metadata,
        })
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
    fn mark_dirty(&mut self, pos: ChunkPos);
    fn dirty_chunk_count(&self) -> usize;
}

#[derive(Debug, Default)]
pub struct InMemoryWorldStorage {
    chunks: HashMap<ChunkPos, Chunk>,
    dirty_chunks: HashSet<ChunkPos>,
}

impl InMemoryWorldStorage {
    const CHUNK_MAGIC: &'static [u8; 15] = b"AURELIA-CHUNK-1";

    pub fn load_from_dir(path: &Path) -> io::Result<Self> {
        let mut storage = Self::default();
        if !path.exists() {
            return Ok(storage);
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("achunk") {
                continue;
            }
            let chunk = read_chunk_file(&path)?;
            storage.chunks.insert(chunk.pos(), chunk);
        }
        Ok(storage)
    }

    pub fn save_dirty_to_dir(&mut self, path: &Path) -> io::Result<usize> {
        if self.dirty_chunks.is_empty() {
            return Ok(0);
        }

        fs::create_dir_all(path)?;
        let dirty: Vec<ChunkPos> = self.dirty_chunks.iter().copied().collect();
        let mut saved = 0;
        for pos in dirty {
            if let Some(chunk) = self.chunks.get(&pos) {
                write_chunk_file(path, chunk)?;
                self.dirty_chunks.remove(&pos);
                saved += 1;
            }
        }
        Ok(saved)
    }
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

    fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty_chunks.insert(pos);
    }

    fn dirty_chunk_count(&self) -> usize {
        self.dirty_chunks.len()
    }
}

fn chunk_file_path(dir: &Path, pos: ChunkPos) -> std::path::PathBuf {
    dir.join(format!("c.{}.{}.achunk", pos.x, pos.z))
}

fn write_chunk_file(dir: &Path, chunk: &Chunk) -> io::Result<()> {
    let path = chunk_file_path(dir, chunk.pos());
    let mut file = File::create(path)?;
    file.write_all(InMemoryWorldStorage::CHUNK_MAGIC)?;
    file.write_all(&chunk.pos().x.to_be_bytes())?;
    file.write_all(&chunk.pos().z.to_be_bytes())?;
    file.write_all(&(Chunk::BLOCK_COUNT as u32).to_be_bytes())?;
    file.write_all(&chunk.copy_block_ids())?;
    file.write_all(&(Chunk::BLOCK_COUNT as u32).to_be_bytes())?;
    file.write_all(&chunk.copy_metadata())?;
    Ok(())
}

fn read_chunk_file(path: &Path) -> io::Result<Chunk> {
    let mut file = File::open(path)?;
    let mut magic = [0; 15];
    file.read_exact(&mut magic)?;
    if &magic != InMemoryWorldStorage::CHUNK_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Aurelia chunk magic",
        ));
    }

    let x = read_i32_file(&mut file)?;
    let z = read_i32_file(&mut file)?;
    let block_count = read_u32_file(&mut file)? as usize;
    if block_count != Chunk::BLOCK_COUNT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid block id array length",
        ));
    }
    let mut block_ids = vec![0; block_count];
    file.read_exact(&mut block_ids)?;

    let metadata_count = read_u32_file(&mut file)? as usize;
    if metadata_count != Chunk::BLOCK_COUNT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid metadata array length",
        ));
    }
    let mut metadata = vec![0; metadata_count];
    file.read_exact(&mut metadata)?;

    Chunk::from_arrays(ChunkPos::new(x, z), block_ids, metadata)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))
}

fn read_i32_file(input: &mut impl Read) -> io::Result<i32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(i32::from_be_bytes(bytes))
}

fn read_u32_file(input: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
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

    pub fn set_time(&mut self, time: u64) {
        self.time = time;
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
        self.storage.mark_dirty(chunk_pos);
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

    pub fn chunk_snapshot(&mut self, pos: ChunkPos) -> Chunk {
        self.get_or_create_chunk(pos)
    }

    pub fn dirty_chunk_count(&self) -> usize {
        self.storage.dirty_chunk_count()
    }
}

impl World<InMemoryWorldStorage> {
    pub fn save_dirty_chunks(&mut self, path: &Path) -> io::Result<usize> {
        self.storage.save_dirty_to_dir(path)
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
    fn dirty_chunk_save_and_reload_preserves_changed_block() {
        let dir = test_world_dir("single");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = BlockPos::new(1, 70, 1);

        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);
        world.set_block(pos, BlockState::DIRT);
        assert_eq!(1, world.dirty_chunk_count());
        assert_eq!(1, world.save_dirty_chunks(&dir).unwrap());
        assert_eq!(0, world.dirty_chunk_count());

        let storage = InMemoryWorldStorage::load_from_dir(&dir).unwrap();
        let mut reloaded = World::new(storage, FlatWorldGenerator);
        assert_eq!(BlockState::DIRT, reloaded.block_at(pos));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multiple_dirty_chunks_save_and_reload() {
        let dir = test_world_dir("multiple");
        let _ = std::fs::remove_dir_all(&dir);
        let first = BlockPos::new(0, 70, 0);
        let second = BlockPos::new(17, 71, -1);

        let mut world = World::new(InMemoryWorldStorage::default(), FlatWorldGenerator);
        world.set_block(first, BlockState::DIRT);
        world.set_block(second, BlockState::STONE);

        assert_eq!(2, world.dirty_chunk_count());
        assert_eq!(2, world.save_dirty_chunks(&dir).unwrap());

        let storage = InMemoryWorldStorage::load_from_dir(&dir).unwrap();
        let mut reloaded = World::new(storage, FlatWorldGenerator);
        assert_eq!(BlockState::DIRT, reloaded.block_at(first));
        assert_eq!(BlockState::STONE, reloaded.block_at(second));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unchanged_generated_chunks_still_work_after_loading_empty_save_dir() {
        let dir = test_world_dir("empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let storage = InMemoryWorldStorage::load_from_dir(&dir).unwrap();
        let mut world = World::new(storage, FlatWorldGenerator);

        assert_eq!(BlockState::GRASS, world.block_at(BlockPos::new(32, 63, 32)));
        assert_eq!(0, world.dirty_chunk_count());
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn beta173_block_rules_cover_harvest_and_drops() {
        let dirt = beta173::block_rule(3);
        assert_eq!(beta173::Material::Dirt, dirt.material);
        assert_eq!(Some((3, 1, 0)), dirt.drop_for(None, 0));

        let stone = beta173::block_rule(1);
        assert_eq!(None, stone.drop_for(None, 0));
        assert_eq!(
            Some((4, 1, 0)),
            stone.drop_for(Some(aurelia_common::beta173::item_rule(270)), 0)
        );

        let glass = beta173::block_rule(20);
        assert_eq!(None, glass.drop_for(None, 0));
        assert!(glass.transparent);

        let iron_ore = beta173::block_rule(15);
        assert_eq!(
            None,
            iron_ore.drop_for(Some(aurelia_common::beta173::item_rule(270)), 0)
        );
        assert_eq!(
            Some((15, 1, 0)),
            iron_ore.drop_for(Some(aurelia_common::beta173::item_rule(274)), 0)
        );
    }

    #[test]
    fn beta173_block_rules_cover_every_block_id() {
        for id in 1..=aurelia_common::beta173::MAX_BLOCK_ID {
            let rule = beta173::block_rule(id as u8);
            assert_ne!(
                "unknown", rule.debug_name,
                "block id {id} is missing a rule"
            );
        }
    }

    #[test]
    fn beta173_block_drops_match_documented_behavior() {
        use aurelia_common::beta173 as item;

        let diamond_ore = beta173::block_rule(item::DIAMOND_ORE as u8);
        assert_eq!(
            None,
            diamond_ore.drop_for(Some(item::item_rule(item::STONE_PICKAXE)), 0)
        );
        assert_eq!(
            Some((item::DIAMOND, 1, 0)),
            diamond_ore.drop_for(Some(item::item_rule(item::IRON_PICKAXE)), 0)
        );

        let redstone_ore = beta173::block_rule(item::REDSTONE_ORE as u8);
        assert_eq!(
            Some((item::REDSTONE, 4, 0)),
            redstone_ore.drop_for(Some(item::item_rule(item::IRON_PICKAXE)), 0)
        );

        let clay = beta173::block_rule(item::CLAY_BLOCK as u8);
        assert_eq!(Some((item::CLAY_BALL, 4, 0)), clay.drop_for(None, 0));

        let obsidian = beta173::block_rule(item::OBSIDIAN as u8);
        assert_eq!(
            None,
            obsidian.drop_for(Some(item::item_rule(item::IRON_PICKAXE)), 0)
        );
        assert_eq!(
            Some((item::OBSIDIAN, 1, 0)),
            obsidian.drop_for(Some(item::item_rule(item::DIAMOND_PICKAXE)), 0)
        );

        let stone = beta173::block_rule(item::STONE as u8);
        assert!(stone.can_harvest(Some(item::item_rule(item::GOLDEN_PICKAXE))));
        let iron_ore = beta173::block_rule(item::IRON_ORE as u8);
        assert!(!iron_ore.can_harvest(Some(item::item_rule(item::GOLDEN_PICKAXE))));

        let snow_layer = beta173::block_rule(item::SNOW_LAYER as u8);
        assert_eq!(None, snow_layer.drop_for(None, 0));
        assert_eq!(
            Some((item::SNOWBALL, 1, 0)),
            snow_layer.drop_for(Some(item::item_rule(item::WOODEN_SHOVEL)), 0)
        );
    }

    #[test]
    fn beta173_light_emitters_and_fluids_are_classified() {
        use aurelia_common::beta173 as item;

        assert_eq!(
            15,
            beta173::block_rule(item::GLOWSTONE as u8).light_emission
        );
        assert_eq!(15, beta173::block_rule(item::LAVA as u8).light_emission);
        assert_eq!(14, beta173::block_rule(item::TORCH as u8).light_emission);
        assert_eq!(
            1,
            beta173::block_rule(item::BROWN_MUSHROOM as u8).light_emission
        );
        assert_eq!(
            7,
            beta173::block_rule(item::REDSTONE_TORCH as u8).light_emission
        );

        let water = beta173::block_rule(item::WATER as u8);
        assert_eq!(beta173::Material::Fluid, water.material);
        assert!(!water.solid);
        assert_eq!(None, water.drop_for(None, 0));
    }

    fn test_world_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("aurelia-world-test-{name}-{}", std::process::id()))
    }
}
