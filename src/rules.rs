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

/// Foods never eaten on their own: they hurt, poison, teleport, or are worth saving.
pub const NOT_FOOD: [&str; 10] = [
    "minecraft:rotten_flesh",
    "minecraft:spider_eye",
    "minecraft:poisonous_potato",
    "minecraft:pufferfish",
    "minecraft:golden_apple",
    "minecraft:enchanted_golden_apple",
    "minecraft:suspicious_stew",
    "minecraft:chorus_fruit",
    "minecraft:chicken",
    "minecraft:honey_bottle",
];

/// An inventory stack that might be eaten.
#[derive(Clone, Debug, PartialEq)]
pub struct Food {
    pub slot: u32,
    pub item: String,
    pub nutrition: i32,
    /// The saturation it gives (the food component's `saturation`).
    pub saturation: f32,
    /// It does something besides feeding (an effect on eating) or leaves something behind (a
    /// bowl, a bottle): never eaten on its own.
    pub extra: bool,
}

/// The food to eat now, if hunger is below `line`: the plain food that fills the most of what is
/// missing, wasting the least, then the most saturation, then the lowest slot.
pub fn pick_food(level: i32, line: i32, foods: &[Food]) -> Option<u32> {
    if level >= line {
        return None;
    }
    let need = 20 - level;
    foods
        .iter()
        .filter(|f| f.nutrition > 0 && !f.extra && !NOT_FOOD.contains(&f.item.as_str()))
        .max_by(|a, b| {
            let key = |f: &Food| (f.nutrition.min(need), -(f.nutrition - need).max(0));
            key(a).cmp(&key(b)).then(a.saturation.total_cmp(&b.saturation)).then(b.slot.cmp(&a.slot))
        })
        .map(|f| f.slot)
}

/// The vanilla feed call's saturation modifier for a food component (`saturation = nutrition ×
/// modifier × 2`).
pub fn saturation_modifier(nutrition: i32, saturation: f32) -> f32 {
    if nutrition <= 0 { 0.0 } else { saturation / (2.0 * nutrition as f32) }
}

/// Ticks between two meals eaten on their own, so two bites never land in one tick.
pub const EAT_EVERY: u32 = 10;

/// Which torch a click on `face` (the clicked face's normal) places: on the floor, or on a wall
/// facing out of it. None for a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TorchAt {
    Floor,
    /// `facing` is the wall torch's `facing` property.
    Wall { facing: &'static str },
}

pub fn torch_at(face: [i32; 3]) -> Option<TorchAt> {
    Some(match face {
        [0, 1, 0] => TorchAt::Floor,
        [0, 0, -1] => TorchAt::Wall { facing: "north" },
        [0, 0, 1] => TorchAt::Wall { facing: "south" },
        [-1, 0, 0] => TorchAt::Wall { facing: "west" },
        [1, 0, 0] => TorchAt::Wall { facing: "east" },
        _ => return None,
    })
}

/// Whether a spot is dark enough for monsters: no block light, and the sky's light, after the
/// time of day and weather took their share (`sky_darken`), at most 7.
pub fn dark(block: i32, sky: i32, sky_darken: i32) -> bool {
    block <= 0 && sky - sky_darken <= 7
}

/// The torch stack to pay with: from the main inventory before the hotbar, the smallest stack
/// first (so stacks tidy up), then the highest slot.
pub fn torch_from(stacks: &[(u32, u32)]) -> Option<u32> {
    stacks.iter().min_by_key(|(slot, count)| (*slot < 9, *count, std::cmp::Reverse(*slot))).map(|(slot, _)| *slot)
}

/// The upward speed that climbs as fast as the game does: the game climbs 0.115 blocks a tick
/// (4.59 blocks in 2 s, measured), and a speed set from the server moves 0.89 of itself in the
/// client's tick (0.3 → 0.268, 0.4 → 0.355, measured), so 0.129 is "1x".
pub const GAME_CLIMB: f64 = 0.129;
/// Looking at least this far up (degrees; the game's pitch is negative upward) counts as
/// looking up.
pub const LOOK_UP: f32 = 20.0;

/// The upward speed to give a player this tick, if faster climbing applies: earned (`percent`
/// over 100), on a ladder or vine, looking up, and already going up (`dy` since the last tick).
pub fn climb_speed(percent: i32, climbing: bool, pitch: f32, dy: f64) -> Option<f64> {
    (percent > 100 && climbing && pitch <= -LOOK_UP && dy > 0.01).then(|| GAME_CLIMB * f64::from(percent) / 100.0)
}

/// What a horse whistle calls: these, within this many blocks.
pub const HORSES: [&str; 6] =
    ["minecraft:horse", "minecraft:donkey", "minecraft:mule", "minecraft:skeleton_horse", "minecraft:zombie_horse", "minecraft:camel"];
pub const WHISTLE_REACH: f64 = 128.0;

/// A horse the player last rode, as the whistle sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horse {
    /// Squared distance to the player.
    pub dist2: f64,
    pub tamed: bool,
    /// Someone sits on it.
    pub ridden: bool,
}

/// The horse to call: tamed, nobody on it, within reach, the nearest.
pub fn call_horse(horses: &[Horse]) -> Option<usize> {
    horses
        .iter()
        .enumerate()
        .filter(|(_, h)| h.tamed && !h.ridden && h.dist2 <= WHISTLE_REACH * WHISTLE_REACH)
        .min_by(|a, b| a.1.dist2.total_cmp(&b.1.dist2))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn climbing_is_faster_only_up_a_ladder_looking_up() {
        assert!(climb_speed(150, true, -45.0, 0.1).is_some_and(|v| (v - 0.1935).abs() < 1e-9));
        assert!(climb_speed(200, true, -20.0, 0.1).is_some_and(|v| (v - 0.258).abs() < 1e-9));
        assert_eq!(climb_speed(100, true, -45.0, 0.1), None, "not earned");
        assert_eq!(climb_speed(200, false, -45.0, 0.1), None, "not on a ladder");
        assert_eq!(climb_speed(200, true, 0.0, 0.1), None, "looking ahead");
        assert_eq!(climb_speed(200, true, -45.0, -0.1), None, "going down");
    }

    #[test]
    fn the_whistle_calls_the_nearest_free_tamed_horse_in_reach() {
        let h = |d: f64, tamed, ridden| Horse { dist2: d * d, tamed, ridden };
        assert_eq!(call_horse(&[h(50.0, true, false), h(20.0, true, false)]), Some(1));
        assert_eq!(call_horse(&[h(20.0, true, true), h(50.0, true, false)]), Some(1), "a friend rides the near one");
        assert_eq!(call_horse(&[h(20.0, false, false)]), None, "wild");
        assert_eq!(call_horse(&[h(129.0, true, false)]), None, "too far");
        assert_eq!(call_horse(&[]), None);
    }

    fn f(slot: u32, item: &str, nutrition: i32, saturation: f32) -> Food {
        Food { slot, item: format!("minecraft:{item}"), nutrition, saturation, extra: false }
    }

    #[test]
    fn eats_only_below_the_line() {
        let bag = [f(3, "bread", 5, 6.0)];
        assert_eq!(pick_food(14, 14, &bag), None);
        assert_eq!(pick_food(13, 14, &bag), Some(3));
        assert_eq!(pick_food(16, 17, &bag), Some(3));
        assert_eq!(pick_food(13, 0, &bag), None, "no comfort");
    }

    #[test]
    fn the_food_that_fills_best_wins() {
        let bag = [f(1, "cooked_beef", 8, 12.8), f(2, "bread", 5, 6.0), f(3, "carrot", 3, 3.6)];
        // 7 missing: the steak fills 7, bread 5.
        assert_eq!(pick_food(13, 14, &bag), Some(1));
        // 4 missing: bread fills it wasting 1, the steak wastes 4.
        assert_eq!(pick_food(16, 17, &bag), Some(2));
        // Same fill and waste: more saturation.
        let tie = [f(1, "baked_potato", 5, 6.0), f(2, "cooked_cod", 5, 6.0), f(4, "cooked_salmon", 6, 9.6)];
        assert_eq!(pick_food(12, 14, &tie), Some(4));
        assert_eq!(pick_food(12, 14, &tie[..2]), Some(1), "then the lowest slot");
    }

    #[test]
    fn bad_and_precious_food_is_never_eaten() {
        for bad in ["rotten_flesh", "spider_eye", "poisonous_potato", "pufferfish", "golden_apple", "enchanted_golden_apple", "suspicious_stew", "chorus_fruit"] {
            assert_eq!(pick_food(2, 17, &[f(0, bad, 4, 2.0)]), None, "{bad}");
        }
        assert_eq!(pick_food(2, 17, &[Food { extra: true, ..f(0, "mushroom_stew", 6, 7.2) }]), None, "leaves a bowl");
        assert_eq!(pick_food(2, 17, &[f(0, "rotten_flesh", 4, 0.8), f(7, "dried_kelp", 1, 0.6)]), Some(7));
    }

    #[test]
    fn the_feed_modifier_gives_back_the_food_saturation() {
        assert!((saturation_modifier(8, 12.8) - 0.8).abs() < 1e-6);
        assert_eq!(saturation_modifier(0, 1.0), 0.0);
    }

    #[test]
    fn torches_go_on_floors_and_walls_never_ceilings() {
        assert_eq!(torch_at([0, 1, 0]), Some(TorchAt::Floor));
        assert_eq!(torch_at([0, -1, 0]), None);
        assert_eq!(torch_at([0, 0, -1]), Some(TorchAt::Wall { facing: "north" }));
        assert_eq!(torch_at([1, 0, 0]), Some(TorchAt::Wall { facing: "east" }));
    }

    #[test]
    fn dark_is_monster_dark() {
        assert!(dark(0, 0, 0), "a cave");
        assert!(!dark(1, 0, 0), "a torch's light far off");
        assert!(!dark(0, 15, 0), "noon outside");
        assert!(dark(0, 15, 11), "midnight outside");
        assert!(!dark(0, 15, 3), "an evening");
    }

    #[test]
    fn torches_are_paid_from_the_inventory_first() {
        assert_eq!(torch_from(&[(2, 64), (20, 30), (30, 5)]), Some(30));
        assert_eq!(torch_from(&[(2, 64), (5, 3)]), Some(5));
        assert_eq!(torch_from(&[]), None);
    }

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
