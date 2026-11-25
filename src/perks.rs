//! The comforts earned in the Skills tree Hands, with no code tie to Skills: an unlocked node
//! gives the player a vanilla tag that Hands reads, and the tree ships as a data file only Skills
//! reads (`data/hands/skills/hands.json`). Without Skills, the setting `all_comforts` opens them;
//! each comfort has its own off switch (`setting`).

/// Tags `hands.eat.<n>`: food from the inventory is eaten once hunger is below `n`.
pub const EAT: &str = "hands.eat.";
/// A right-click with a pickaxe or shovel on a dark spot places a torch.
pub const TORCH: &str = "hands.torch";
/// Red marks where monsters can spawn, while a torch is held.
pub const MARKS: &str = "hands.marks";
/// The highest hunger line, and what `all_comforts` gives.
pub const EAT_MAX: i32 = 17;

/// One player's comforts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Comforts {
    /// Eat once the food level is below this (0: never).
    pub eat: i32,
    pub torch: bool,
    pub marks: bool,
}

impl Comforts {
    pub fn of(tags: &[String], all: bool) -> Self {
        if all {
            return Comforts { eat: EAT_MAX, torch: true, marks: true };
        }
        Comforts {
            eat: tags.iter().filter_map(|t| t.strip_prefix(EAT)?.parse::<i32>().ok()).max().unwrap_or(0).clamp(0, EAT_MAX),
            torch: tags.iter().any(|t| t == TORCH),
            marks: tags.iter().any(|t| t == MARKS),
        }
    }

    /// The comforts left once the player's off switches are applied.
    pub fn switched(self, eat: bool, torch: bool, marks: bool) -> Self {
        Comforts { eat: if eat { self.eat } else { 0 }, torch: self.torch && torch, marks: self.marks && marks }
    }
}

/// Settings (Options > Mods > Hands).
pub mod setting {
    /// Opens every comfort for a world that doesn't use Skills.
    pub const ALL_COMFORTS: &str = "all_comforts";
    /// Off switches for comforts already earned (on by default).
    pub const EAT: &str = "auto_eat";
    pub const TORCH: &str = "torch";
    pub const MARKS: &str = "marks";
    /// Each key with its default.
    pub const ALL: [(&str, bool); 4] = [(ALL_COMFORTS, false), (EAT, true), (TORCH, true), (MARKS, true)];
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
        assert_eq!(c, Comforts { eat: 17, torch: true, marks: false });
        assert_eq!(Comforts::of(&t(&["hands.eat.99"]), false).eat, EAT_MAX);
        assert_eq!(Comforts::of(&t(&[]), true), Comforts { eat: 17, torch: true, marks: true });
    }

    #[test]
    fn an_off_switch_wins_over_an_earned_comfort() {
        let all = Comforts::of(&[], true);
        assert_eq!(all.switched(false, true, true), Comforts { eat: 0, torch: true, marks: true });
        assert_eq!(all.switched(true, false, false), Comforts { eat: 17, torch: false, marks: false });
        // Switches never give what wasn't earned.
        assert_eq!(Comforts::default().switched(true, true, true), Comforts::default());
    }

    /// The tree file gives exactly the tags Hands reads, and every comfort has a node.
    #[test]
    fn the_shipped_tree_gives_exactly_the_tags_hands_reads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/hands/skills/hands.json");
        let text = std::fs::read_to_string(path).unwrap();
        let given: Vec<String> =
            text.split("\"tag\"").skip(1).filter_map(|rest| rest.split('"').nth(1).map(str::to_owned)).collect();
        for tag in &given {
            let eat = tag.strip_prefix(EAT).is_some_and(|n| n.parse::<i32>().is_ok_and(|n| n <= EAT_MAX));
            assert!(eat || tag == TORCH || tag == MARKS, "the tree gives {tag}, which Hands never reads");
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
        let tree = std::fs::read_to_string(root.join("data/hands/skills/hands.json")).unwrap();
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
