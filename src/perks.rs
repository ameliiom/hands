//! The comforts earned in Skills' own trees, with no code tie to Skills: an unlocked node gives
//! the player a vanilla tag that Hands reads, and the nodes ship as data files that extend
//! Skills' trees (`data/hands/skills/{mining,exploration,farming}.json`). Without Skills, the
//! setting `all_comforts` opens them; each comfort has its own off switch (`setting`).

/// Tags `hands.eat.<n>`: food from the inventory is eaten once hunger is below `n`.
pub const EAT: &str = "hands.eat.";
/// A right-click with a pickaxe or shovel on a dark spot places a torch.
pub const TORCH: &str = "hands.torch";
/// Tags `hands.climb.<percent>`: climbing while looking up goes this fast (150: 1.5x).
pub const CLIMB: &str = "hands.climb.";
/// A goat horn blown while sneaking calls your last ridden horse.
pub const WHISTLE: &str = "hands.whistle";
/// The highest hunger line, and what `all_comforts` gives.
pub const EAT_MAX: i32 = 17;
/// The fastest climb, in percent of the game's.
pub const CLIMB_MAX: i32 = 200;

/// One player's comforts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Comforts {
    /// Eat once the food level is below this (0: never).
    pub eat: i32,
    pub torch: bool,
    /// Climb speed in percent of the game's (0: the game's own).
    pub climb: i32,
    pub whistle: bool,
}

impl Comforts {
    pub fn of(tags: &[String], all: bool) -> Self {
        if all {
            return Comforts { eat: EAT_MAX, torch: true, climb: CLIMB_MAX, whistle: true };
        }
        let best = |prefix: &str, max: i32| tags.iter().filter_map(|t| t.strip_prefix(prefix)?.parse::<i32>().ok()).max().unwrap_or(0).clamp(0, max);
        Comforts {
            eat: best(EAT, EAT_MAX),
            torch: tags.iter().any(|t| t == TORCH),
            climb: best(CLIMB, CLIMB_MAX),
            whistle: tags.iter().any(|t| t == WHISTLE),
        }
    }

    /// The comforts left once the off switches (`setting::ALL` order, `all_comforts` first) apply.
    pub fn switched(self, on: impl Fn(&str) -> bool) -> Self {
        Comforts {
            eat: if on(setting::EAT) { self.eat } else { 0 },
            torch: self.torch && on(setting::TORCH),
            climb: if on(setting::CLIMB) { self.climb } else { 0 },
            whistle: self.whistle && on(setting::WHISTLE),
        }
    }
}

/// Settings (Options > Mods > Hands).
pub mod setting {
    /// Opens every comfort for a world that doesn't use Skills.
    pub const ALL_COMFORTS: &str = "all_comforts";
    /// Off switches for comforts already earned (on by default).
    pub const EAT: &str = "auto_eat";
    pub const TORCH: &str = "torch";
    pub const CLIMB: &str = "climb";
    pub const WHISTLE: &str = "whistle";
    /// Each key with its default.
    pub const ALL: [(&str, bool); 5] = [(ALL_COMFORTS, false), (EAT, true), (TORCH, true), (CLIMB, true), (WHISTLE, true)];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nothing_until_earned() {
        assert_eq!(Comforts::of(&t(&[]), false), Comforts::default());
        assert_eq!(Comforts::of(&t(&["gathering.vein.8", "hands.eat.x"]), false), Comforts::default());
    }

    #[test]
    fn comforts_read_from_tags() {
        let c = Comforts::of(&t(&["hands.eat.14", "hands.eat.17", "hands.torch"]), false);
        assert_eq!(c, Comforts { eat: 17, torch: true, ..Comforts::default() });
        assert_eq!(Comforts::of(&t(&["hands.eat.99", "hands.climb.900"]), false), Comforts { eat: EAT_MAX, climb: CLIMB_MAX, ..Comforts::default() });
        assert_eq!(Comforts::of(&t(&["hands.climb.150", "hands.whistle"]), false), Comforts { climb: 150, whistle: true, ..Comforts::default() });
        assert_eq!(Comforts::of(&t(&[]), true), Comforts { eat: 17, torch: true, climb: 200, whistle: true });
    }

    #[test]
    fn an_off_switch_wins_over_an_earned_comfort() {
        let all = Comforts::of(&[], true);
        assert_eq!(all.switched(|k| k != setting::EAT), Comforts { eat: 0, ..all });
        assert_eq!(all.switched(|k| k != setting::CLIMB && k != setting::WHISTLE), Comforts { climb: 0, whistle: false, ..all });
        assert_eq!(all.switched(|_| false), Comforts::default());
        // Switches never give what wasn't earned.
        assert_eq!(Comforts::default().switched(|_| true), Comforts::default());
    }

    /// Every shipped node file, read as one text.
    fn shipped() -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/hands/skills");
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        files.iter().map(|f| std::fs::read_to_string(f).unwrap()).collect()
    }

    /// The node files only extend Skills' trees (no tree of Hands' own), give exactly the tags
    /// Hands reads, and every comfort has a node.
    #[test]
    fn the_shipped_nodes_give_exactly_the_tags_hands_reads() {
        let text = shipped();
        assert!(!text.contains("\"title\": \"hands.tree"), "Hands defines no tree of its own");
        assert_eq!(text.matches("\"extends\": \"skills:").count(), 3);
        let given: Vec<String> =
            text.split("\"tag\"").skip(1).filter_map(|rest| rest.split('"').nth(1).map(str::to_owned)).collect();
        for tag in &given {
            let level = |p: &str, max: i32| tag.strip_prefix(p).is_some_and(|n| n.parse::<i32>().is_ok_and(|n| n <= max));
            let read = level(EAT, EAT_MAX) || level(CLIMB, CLIMB_MAX) || [TORCH, WHISTLE].contains(&tag.as_str());
            assert!(read, "the tree gives {tag}, which Hands never reads");
        }
        assert_eq!(Comforts::of(&given, false), Comforts::of(&[], true), "some comfort has no node, or its node stops short");
    }

    /// Both languages hold the same keys, and every title, perk and setting shown has its line.
    #[test]
    fn every_shown_key_is_in_both_languages() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let keys = |lang: &str| -> Vec<String> {
            let text = std::fs::read_to_string(root.join(format!("assets/hands/lang/{lang}.json"))).unwrap();
            let mut k: Vec<String> =
                text.lines().filter_map(|l| l.trim().strip_prefix('"')?.split('"').next().map(str::to_owned)).collect();
            k.sort();
            k
        };
        let (en, ar) = (keys("en_us"), keys("ar_sa"));
        assert_eq!(en, ar);
        let tree = shipped();
        let quoted =
            |field: &str| -> Vec<String> { tree.split(&format!("\"{field}\"")).skip(1).filter_map(|r| r.split('"').nth(1).map(str::to_owned)).collect() };
        for title in quoted("title") {
            assert!(en.contains(&title), "no line for {title}");
        }
        for tag in quoted("tag") {
            assert!(en.contains(&format!("skills.perk.{tag}")), "no perk line for {tag}");
        }
        for (key, _) in setting::ALL {
            assert!(en.contains(&format!("hands.config.{key}")), "no line for the setting {key}");
        }
    }
}
