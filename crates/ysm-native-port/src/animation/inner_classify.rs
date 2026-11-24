//! 1:1 port of `com.elfmcys.ysm.client.animation.condition.InnerClassify`.
//!
//! Classifies held items into official YSM categories (`sword`, `axe`, `pickaxe`,
//! `shovel`, `hoe`, `shield`, `crossbow`, `bow`, `fishing_rod`, `spear`,
//! `throwable_potion`, `slashblade`). Plain blocks and unclassified items MUST
//! return `""` so `ConditionalHold` does not force a raised-arm pose.

/// Returns the official YSM inner classification for an item id and optional item tags.
pub fn get_classify(item_id: &str, tags: &[&str]) -> &'static str {
    let id = item_id.to_ascii_lowercase();
    if id.is_empty() || id == "minecraft:air" {
        return "";
    }
    let has_tag = |needle: &str| tags.iter().any(|t| t.eq_ignore_ascii_case(needle));

    if id.contains("slashblade") || id.contains("katana") {
        return "slashblade";
    }
    if id.ends_with("_sword") || id == "minecraft:sword" || has_tag("c:swords") || has_tag("minecraft:swords") {
        return "sword";
    }
    if id.ends_with("_pickaxe") || has_tag("minecraft:pickaxes") {
        return "pickaxe";
    }
    if id.ends_with("_axe") || has_tag("minecraft:axes") {
        return "axe";
    }
    if id.ends_with("_shovel") || has_tag("minecraft:shovels") {
        return "shovel";
    }
    if id.ends_with("_hoe") || has_tag("minecraft:hoes") {
        return "hoe";
    }
    if id.contains("shield") || has_tag("c:shields") {
        return "shield";
    }
    if id.contains("crossbow") || has_tag("c:crossbows") {
        return "crossbow";
    }
    if id.ends_with(":bow") || id.ends_with("_bow") || has_tag("c:bows") {
        return "bow";
    }
    if id.contains("fishing_rod") || has_tag("c:fishing_rods") {
        return "fishing_rod";
    }
    if id.contains("trident") || id.contains("mace") || id.contains("spear") || has_tag("c:tridents") {
        return "spear";
    }
    if id.contains("splash_potion") || id.contains("lingering_potion") {
        return "throwable_potion";
    }
    ""
}

/// 1:1 port of `InnerClassify.doClassifyTest(extraPre, entity, hand)`.
pub fn do_classify_test(extra_pre: &str, item_id: &str, tags: &[&str]) -> String {
    let classify = get_classify(item_id, tags);
    if classify.is_empty() {
        String::new()
    } else {
        format!("{extra_pre}{classify}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_official_ysm_weapons_tools_and_fishing_rod() {
        assert_eq!(get_classify("minecraft:diamond_sword", &[]), "sword");
        assert_eq!(get_classify("minecraft:netherite_axe", &[]), "axe");
        assert_eq!(get_classify("minecraft:iron_pickaxe", &[]), "pickaxe");
        assert_eq!(get_classify("minecraft:golden_shovel", &[]), "shovel");
        assert_eq!(get_classify("minecraft:stone_hoe", &[]), "hoe");
        assert_eq!(get_classify("minecraft:shield", &[]), "shield");
        assert_eq!(get_classify("minecraft:crossbow", &[]), "crossbow");
        assert_eq!(get_classify("minecraft:bow", &[]), "bow");
        assert_eq!(get_classify("minecraft:fishing_rod", &[]), "fishing_rod");
        assert_eq!(get_classify("minecraft:trident", &[]), "spear");
        assert_eq!(get_classify("minecraft:splash_potion", &[]), "throwable_potion");
        assert_eq!(get_classify("slashblade:slashblade", &[]), "slashblade");
    }

    #[test]
    fn plain_blocks_and_misc_items_return_empty_like_official_ysm() {
        for item in [
            "minecraft:air",
            "minecraft:obsidian",
            "minecraft:cobblestone",
            "minecraft:dirt",
            "minecraft:rail",
            "minecraft:oak_planks",
            "minecraft:torch",
            "minecraft:cooked_beef",
        ] {
            assert_eq!(get_classify(item, &[]), "", "expected empty for {item}");
            assert_eq!(do_classify_test("hold_mainhand:", item, &[]), "");
        }
    }
}
