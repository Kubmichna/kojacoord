pub fn map_modern_v1_16_5(state_id: u32) -> &'static str {
    // 1.16.5 has ~11,000 block states. The first ~300 map cleanly to basic terrain.
    match state_id {
        0 => "minecraft:air",
        1 => "minecraft:stone",
        2 => "minecraft:granite",
        3 => "minecraft:polished_granite",
        4 => "minecraft:diorite",
        5 => "minecraft:polished_diorite",
        6 => "minecraft:andesite",
        7 => "minecraft:polished_andesite",
        8..=9 => "minecraft:grass", // grass_block
        10 => "minecraft:dirt",
        11 => "minecraft:dirt_with_roots", // coarse_dirt
        12..=13 => "minecraft:podzol",
        14 => "minecraft:cobblestone",
        15 => "minecraft:planks", // oak
        16 => "minecraft:spruce_planks",
        17 => "minecraft:birch_planks",
        18 => "minecraft:jungle_planks",
        19 => "minecraft:acacia_planks",
        20 => "minecraft:dark_oak_planks",
        21..=22 => "minecraft:sapling", // oak sapling
        23..=24 => "minecraft:spruce_sapling",
        25..=26 => "minecraft:birch_sapling",
        27..=28 => "minecraft:jungle_sapling",
        29..=30 => "minecraft:acacia_sapling",
        31..=32 => "minecraft:dark_oak_sapling",
        33 => "minecraft:bedrock",
        34..=49 => "minecraft:water",
        50..=65 => "minecraft:lava",
        66 => "minecraft:sand",
        67 => "minecraft:red_sand",
        68 => "minecraft:gravel",
        69 => "minecraft:gold_ore",
        70 => "minecraft:iron_ore",
        71 => "minecraft:coal_ore",
        72..=110 => "minecraft:log", // log states (axes)
        111..=180 => "minecraft:leaves", // leaves (distances)
        181..=182 => "minecraft:sponge",
        183 => "minecraft:glass",
        184 => "minecraft:lapis_ore",
        185 => "minecraft:lapis_block",
        186..=190 => "minecraft:sandstone",
        191..=206 => "minecraft:wool",
        207..=222 => "minecraft:carpet",
        223..=238 => "minecraft:stained_glass",
        239..=254 => "minecraft:stained_glass_pane",
        255..=270 => "minecraft:stained_hardened_clay", // terracotta
        271..=286 => "minecraft:concrete",
        287..=302 => "minecraft:concrete_powder",
        303..=318 => "minecraft:bed",
        319..=450 => "minecraft:wooden_door", // doors have massive state combos
        451..=550 => "minecraft:fence", // fences
        551..=650 => "minecraft:fence_gate", // gates
        651..=750 => "minecraft:stone_stairs", // stairs combos
        751..=850 => "minecraft:oak_stairs",
        851..=950 => "minecraft:wooden_slab",
        951..=1050 => "minecraft:stone_slab",
        1051..=1100 => "minecraft:tallgrass",
        1101..=1150 => "minecraft:double_plant",
        1151..=1200 => "minecraft:wall_sign",
        1201..=1300 => "minecraft:standing_sign",
        1301..=1400 => "minecraft:coral",
        1401..=1500 => "minecraft:glazed_terracotta",
        1501..=2000 => "minecraft:shulker_box",
        2001..=3000 => "minecraft:redstone_wire", // massive redstone permutations
        3001..=4000 => "minecraft:piston", // pistons, observers, command blocks
        4001..=5000 => "minecraft:flower_pot", // decor, anvils, banners, skulls
        5001..=6000 => "minecraft:loom", // 1.14 Village & Pillage blocks (barrels, smokers)
        6001..=7000 => "minecraft:campfire", // 1.14 decor (lanterns, bells, sweet berries)
        7001..=8000 => "minecraft:honey_block", // 1.15 Buzzy Bees (honey, hives)
        8001..=9000 => "minecraft:crimson_planks", // 1.16 Nether Update (crimson/warped wood)
        9001..=10000 => "minecraft:basalt", // 1.16 Terrain (basalt, blackstone, nylium)
        10001..=11000 => "minecraft:soul_campfire", // 1.16 Decor (soul torches, chains)
        _ => "minecraft:stone", // 11000+ (Should not exist in 1.16.5, but catches anomalies)
    }
}

/// Architecture placeholder for Java 1.20+ Flattening (Usable MVP)

