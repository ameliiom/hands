//! The two decisions, on plain values so they are tested without a game: which hotbar slot mines a
//! block best, and whether an emptied hand is refilled and from where.

/// A hotbar slot as the choice sees it: how fast its item mines the block, and how many uses it
/// has left before it breaks (None: it doesn't wear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub slot: usize,
    pub speed: f32,
    pub uses: Option<i64>,
}

/// The slot to switch to, if one mines faster than the slot held. A tool with one use left is
/// never picked; ties keep the slot held, then the lowest slot.
pub fn best_tool(held: usize, candidates: &[Candidate]) -> Option<usize> {
    let now = candidates.iter().find(|c| c.slot == held).map_or(1.0, |c| c.speed);
    candidates
        .iter()
        .filter(|c| c.slot != held && c.uses.is_none_or(|u| u > 1) && c.speed > now)
        .fold(None::<Candidate>, |best, c| match best {
            Some(b) if b.speed >= c.speed => Some(b),
            _ => Some(*c),
        })
        .map(|c| c.slot)
}

/// The hand one tick ago.
#[derive(Clone, Debug, PartialEq)]
pub struct Hand {
    pub slot: usize,
    pub item: String,
    pub count: u32,
    /// Uses left before it breaks (None: it doesn't wear).
    pub uses: Option<i64>,
}

/// Why the hand emptied, if by use: the last of a stack used (placed, eaten, thrown), or a tool
/// worn out. None when it emptied some other way (dropped, moved, cleared), or didn't.
pub fn used_up(before: &Hand, slot_now: usize, empty_now: bool, tossed: bool) -> bool {
    empty_now && !tossed && slot_now == before.slot && (before.count == 1 || before.uses.is_some_and(|u| u <= 1))
}

/// An inventory stack that might refill the hand: where, what, how many, and of what tool kind
/// (`minecraft:pickaxes`…, None for a non-tool).
#[derive(Clone, Debug, PartialEq)]
pub struct Spare {
    pub slot: u32,
    pub item: String,
    pub count: u32,
    pub kind: Option<String>,
}

/// The stack to refill the hand from: the same item first (the fullest stack, and for a tool the
/// one with the most uses comes from the caller's order), else, for a tool, one of its kind.
pub fn refill_from(before: &Hand, before_kind: Option<&str>, spares: &[Spare]) -> Option<u32> {
    let same = spares.iter().filter(|s| s.item == before.item).max_by_key(|s| (s.count, std::cmp::Reverse(s.slot)));
    let kin = || {
        let kind = before_kind?;
        spares.iter().filter(|s| s.kind.as_deref() == Some(kind)).min_by_key(|s| s.slot)
    };
    same.or_else(kin).map(|s| s.slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(slot: usize, speed: f32, uses: Option<i64>) -> Candidate {
        Candidate { slot, speed, uses }
    }

    #[test]
    fn the_fastest_tool_wins_unless_held_one_is_as_fast() {
        let bar = [c(0, 8.0, Some(100)), c(1, 1.0, None), c(2, 6.0, Some(50))];
        assert_eq!(best_tool(1, &bar), Some(0));
        assert_eq!(best_tool(0, &bar), None);
        assert_eq!(best_tool(2, &bar), Some(0));
        // Equal speed: keep what's held.
        assert_eq!(best_tool(2, &[c(0, 6.0, None), c(2, 6.0, None)]), None);
        // Equal speed elsewhere: the lowest slot.
        assert_eq!(best_tool(4, &[c(3, 8.0, None), c(1, 8.0, None), c(4, 1.0, None)]), Some(3));
    }

    #[test]
    fn a_tool_with_one_use_left_is_passed_over() {
        let bar = [c(0, 8.0, Some(1)), c(1, 1.0, None), c(2, 6.0, Some(2))];
        assert_eq!(best_tool(1, &bar), Some(2));
        assert_eq!(best_tool(1, &[c(0, 8.0, Some(1)), c(1, 1.0, None)]), None);
    }

    fn hand(count: u32, uses: Option<i64>) -> Hand {
        Hand { slot: 4, item: "minecraft:cobblestone".into(), count, uses }
    }

    #[test]
    fn only_use_empties_count() {
        assert!(used_up(&hand(1, None), 4, true, false));
        assert!(!used_up(&hand(1, None), 4, true, true), "dropped");
        assert!(!used_up(&hand(1, None), 5, true, false), "another slot chosen");
        assert!(!used_up(&hand(30, None), 4, true, false), "a whole stack taken away");
        assert!(!used_up(&hand(1, None), 4, false, false), "still there");
        assert!(used_up(&Hand { count: 1, uses: Some(1), ..hand(1, None) }, 4, true, false), "worn out");
    }

    fn s(slot: u32, item: &str, count: u32, kind: Option<&str>) -> Spare {
        Spare { slot, item: item.into(), count, kind: kind.map(str::to_owned) }
    }

    #[test]
    fn refill_prefers_the_same_item_then_its_kind() {
        let spares = [
            s(20, "minecraft:cobblestone", 12, None),
            s(30, "minecraft:cobblestone", 64, None),
            s(10, "minecraft:stone_pickaxe", 1, Some("minecraft:pickaxes")),
            s(9, "minecraft:diamond_pickaxe", 1, Some("minecraft:pickaxes")),
        ];
        assert_eq!(refill_from(&hand(1, None), None, &spares), Some(30));
        let pick = Hand { item: "minecraft:iron_pickaxe".into(), ..hand(1, Some(1)) };
        assert_eq!(refill_from(&pick, Some("minecraft:pickaxes"), &spares), Some(9));
        let dia = Hand { item: "minecraft:diamond_pickaxe".into(), ..pick.clone() };
        assert_eq!(refill_from(&dia, Some("minecraft:pickaxes"), &spares), Some(9));
        assert_eq!(refill_from(&Hand { item: "minecraft:bread".into(), ..hand(1, None) }, None, &spares), None);
    }
}
