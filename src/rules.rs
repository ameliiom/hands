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

/// Ticks without digging before the hand goes back to the slot it had.
pub const BACK_AFTER: u32 = 20;

/// A swap to undo: back to `to` once the player has stopped digging with `from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Back {
    pub to: usize,
    pub from: usize,
    /// Ticks since the last dig.
    pub quiet: u32,
}

/// A new swap from `held` to `slot`: a swap already waiting keeps the slot first held.
pub fn swapped(waiting: Option<Back>, held: usize, slot: usize) -> Back {
    Back { to: waiting.map_or(held, |b| b.to), from: slot, quiet: 0 }
}

/// What a tick does with a waiting swap: whether it still waits, and the slot to go back to now.
/// A hand the player moved themself is left alone.
pub fn back_tick(b: Back, selected: usize, dug: bool) -> (Option<Back>, Option<usize>) {
    if selected != b.from {
        return (None, None);
    }
    let quiet = if dug { 0 } else { b.quiet + 1 };
    if quiet >= BACK_AFTER { (None, (b.to != b.from).then_some(b.to)) } else { (Some(Back { quiet, ..b }), None) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hand_goes_back_a_second_after_digging_stops() {
        let mut b = swapped(None, 1, 0);
        // Digging keeps it.
        for _ in 0..100 {
            let (next, go) = back_tick(b, 0, true);
            assert_eq!(go, None);
            b = next.unwrap();
        }
        for tick in 1..=BACK_AFTER {
            let (next, go) = back_tick(b, 0, false);
            if tick < BACK_AFTER {
                assert_eq!(go, None);
                b = next.unwrap();
            } else {
                assert_eq!((next, go), (None, Some(1)));
            }
        }
    }

    #[test]
    fn a_hand_moved_by_the_player_is_left_alone() {
        assert_eq!(back_tick(swapped(None, 1, 0), 5, false), (None, None));
    }

    #[test]
    fn a_second_swap_keeps_the_first_slot() {
        let first = swapped(None, 4, 0);
        assert_eq!(swapped(Some(first), 0, 2), Back { to: 4, from: 2, quiet: 0 });
    }

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
