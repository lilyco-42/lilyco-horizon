//! 1:1 port of official YSM `ConditionalHold.java`, `ConditionalUse.java`,
//! and `ConditionalSwing.java` (`com.elfmcys.ysm.client.animation.condition`).

use std::collections::HashSet;

use super::inner_classify::do_classify_test;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UseAnim {
    None,
    Eat,
    Drink,
    Block,
    Bow,
    Spear,
    Crossbow,
    Spyglass,
    TootHorn,
    Brush,
}

impl UseAnim {
    pub fn as_ysm_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Eat => "eat",
            Self::Drink => "drink",
            Self::Block => "block",
            Self::Bow => "bow",
            Self::Spear => "spear",
            Self::Crossbow => "crossbow",
            Self::Spyglass => "spyglass",
            Self::TootHorn => "toot_horn",
            Self::Brush => "brush",
        }
    }

    pub fn from_ysm_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "eat" => Some(Self::Eat),
            "drink" => Some(Self::Drink),
            "block" => Some(Self::Block),
            "bow" => Some(Self::Bow),
            "spear" => Some(Self::Spear),
            "crossbow" => Some(Self::Crossbow),
            "spyglass" => Some(Self::Spyglass),
            "toot_horn" => Some(Self::TootHorn),
            "brush" => Some(Self::Brush),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionKind {
    HoldMainhand,
    HoldOffhand,
    UseMainhand,
    UseOffhand,
    SwingMainhand,
    SwingOffhand,
}

impl ConditionKind {
    fn prefixes(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::HoldMainhand => ("hold_mainhand$", "hold_mainhand#", "hold_mainhand:"),
            Self::HoldOffhand => ("hold_offhand$", "hold_offhand#", "hold_offhand:"),
            Self::UseMainhand => ("use_mainhand$", "use_mainhand#", "use_mainhand:"),
            Self::UseOffhand => ("use_offhand$", "use_offhand#", "use_offhand:"),
            Self::SwingMainhand => ("swing$", "swing#", "swing:"),
            Self::SwingOffhand => ("swing_offhand$", "swing_offhand#", "swing_offhand:"),
        }
    }

    fn empty_result(self) -> &'static str {
        match self {
            Self::HoldMainhand => "hold_mainhand:empty",
            Self::HoldOffhand => "hold_offhand:empty",
            _ => "",
        }
    }
}

/// 1:1 port of official `ConditionalHold` / `ConditionalUse` / `ConditionalSwing`.
#[derive(Debug, Default)]
pub struct ItemConditionMatcher {
    kind: Option<ConditionKind>,
    id_tests: HashSet<String>,
    tag_tests: Vec<String>,
    extra_tests: HashSet<UseAnim>,
    inner_tests: HashSet<String>,
}

impl ItemConditionMatcher {
    pub fn new(kind: ConditionKind) -> Self {
        Self {
            kind: Some(kind),
            ..Default::default()
        }
    }

    /// Registers an animation name from `arm.animation.json` (1:1 with `addTest`).
    pub fn add_test(&mut self, name: &str) {
        let Some(kind) = self.kind else { return };
        let (id_pre, tag_pre, extra_pre) = kind.prefixes();
        if let Some(sub) = name.strip_prefix(id_pre) {
            if !sub.is_empty() {
                self.id_tests.insert(sub.to_ascii_lowercase());
            }
        } else if let Some(sub) = name.strip_prefix(tag_pre) {
            if !sub.is_empty() {
                self.tag_tests.push(sub.to_ascii_lowercase());
            }
        } else if let Some(sub) = name.strip_prefix(extra_pre) {
            if sub.eq_ignore_ascii_case("none") || sub.is_empty() {
                return;
            }
            if let Some(anim) = UseAnim::from_ysm_name(sub) {
                self.extra_tests.insert(anim);
            }
            self.inner_tests.insert(name.to_ascii_lowercase());
        }
    }

    /// Evaluates official YSM priority: Empty -> ID (`$`) -> Tag (`#`) -> InnerClassify (`:`) -> UseAnim (`:`).
    pub fn do_test(&self, item_id: &str, tags: &[&str], use_anim: UseAnim) -> String {
        let Some(kind) = self.kind else {
            return String::new();
        };
        let id = item_id.to_ascii_lowercase();
        if id.is_empty() || id == "minecraft:air" {
            return kind.empty_result().to_string();
        }
        let (id_pre, tag_pre, extra_pre) = kind.prefixes();
        if self.id_tests.contains(&id) {
            return format!("{id_pre}{id}");
        }
        for registered_tag in &self.tag_tests {
            if tags.iter().any(|t| t.eq_ignore_ascii_case(registered_tag)) {
                return format!("{tag_pre}{registered_tag}");
            }
        }
        let inner = do_classify_test(extra_pre, &id, tags);
        if !inner.is_empty() {
            if self.inner_tests.contains(&inner) {
                return inner;
            }
            // Compatibility alias: many community YSM models name fishing rod hold `hold_mainhand:fishing`
            if inner == format!("{extra_pre}fishing_rod") {
                let alias = format!("{extra_pre}fishing");
                if self.inner_tests.contains(&alias) {
                    return alias;
                }
            }
        }
        if use_anim != UseAnim::None && self.extra_tests.contains(&use_anim) {
            return format!("{extra_pre}{}", use_anim.as_ysm_name());
        }
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_hold_matches_official_priority_and_ignores_plain_blocks() {
        let mut hold = ItemConditionMatcher::new(ConditionKind::HoldMainhand);
        for name in [
            "hold_mainhand$minecraft:mace",
            "hold_mainhand#c:tools/special",
            "hold_mainhand:sword",
            "hold_mainhand:fishing",
            "hold_mainhand:eat",
        ] {
            hold.add_test(name);
        }

        // Empty hand -> hold_mainhand:empty
        assert_eq!(hold.do_test("", &[], UseAnim::None), "hold_mainhand:empty");
        // Exact ID beats InnerClassify
        assert_eq!(
            hold.do_test("minecraft:mace", &[], UseAnim::None),
            "hold_mainhand$minecraft:mace"
        );
        // Tag beats InnerClassify
        assert_eq!(
            hold.do_test("minecraft:diamond_sword", &["c:tools/special"], UseAnim::None),
            "hold_mainhand#c:tools/special"
        );
        // InnerClassify sword
        assert_eq!(
            hold.do_test("minecraft:diamond_sword", &[], UseAnim::None),
            "hold_mainhand:sword"
        );
        // Fishing rod matches `hold_mainhand:fishing` alias!
        assert_eq!(
            hold.do_test("minecraft:fishing_rod", &[], UseAnim::None),
            "hold_mainhand:fishing"
        );
        // Plain blocks MUST return empty (no forced raised-arm pose!)
        assert_eq!(hold.do_test("minecraft:obsidian", &[], UseAnim::None), "");
        assert_eq!(hold.do_test("minecraft:cobblestone", &[], UseAnim::None), "");
    }

    #[test]
    fn conditional_swing_and_use_match_official_ysm() {
        let mut swing = ItemConditionMatcher::new(ConditionKind::SwingMainhand);
        swing.add_test("swing:sword");
        swing.add_test("swing:fishing_rod");
        assert_eq!(swing.do_test("minecraft:diamond_sword", &[], UseAnim::None), "swing:sword");
        assert_eq!(swing.do_test("minecraft:fishing_rod", &[], UseAnim::None), "swing:fishing_rod");
        assert_eq!(swing.do_test("", &[], UseAnim::None), "");

        let mut use_m = ItemConditionMatcher::new(ConditionKind::UseMainhand);
        use_m.add_test("use_mainhand:eat");
        use_m.add_test("use_mainhand:bow");
        assert_eq!(use_m.do_test("minecraft:cooked_beef", &[], UseAnim::Eat), "use_mainhand:eat");
        assert_eq!(use_m.do_test("minecraft:bow", &[], UseAnim::Bow), "use_mainhand:bow");
    }
}
