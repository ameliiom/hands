//! The earned comforts on the server: eating on its own, the torch on a right-click, quick
//! climbing and the horse whistle.
//!
//! Hunger is read each tick rather than heard from `food_level_changed`, which the game never
//! sends for the host of a LAN world (the owner's own player).
use crate::kit;
use crate::perks::Comforts;
use crate::rules::{self, Food, TorchAt};
use crate::server::tool_kind;
use mod_api::{
    BlockClick, BlockFace, EntityRef, BlockMetadata, BlockState, ComponentId, GameMode, Host, HostBlockStates, ItemResource, ItemStack, PlayerRef,
    ServerHost, SoundSource, StatType, StateConstraint, Tag,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// What a player has earned (their tags, or everything with `all_comforts`).
pub fn earned(host: &dyn Host, p: PlayerRef) -> Comforts {
    kit::comforts::read(host, crate::MOD_ID, p, Comforts::of)
}

/// What a player has earned and not switched off.
fn comforts(host: &dyn Host, p: PlayerRef) -> Comforts {
    earned(host, p).switched(|key| key.read(host, crate::MOD_ID))
}

struct State {
    /// Ticks since each player last ate on their own.
    since_meal: BTreeMap<u128, u32>,
    /// Each player's height on the last tick, to tell climbing up from down.
    last_y: BTreeMap<u128, f64>,
    /// The horse each player rides or last rode (entity id), tagged `hands.rider.<uuid>`.
    rode: BTreeMap<u128, u32>,
}

static STATE: Mutex<State> =
    Mutex::new(State { since_meal: BTreeMap::new(), last_y: BTreeMap::new(), rode: BTreeMap::new() });

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()))
}

fn int(tag: Option<&Tag>) -> Option<i32> {
    match tag? {
        Tag::Int(n) => Some(*n),
        _ => None,
    }
}

fn float(tag: Option<&Tag>) -> Option<f32> {
    match tag? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        _ => None,
    }
}

/// A stack as food, if it is one: its food component, and whether eating it does more than feed
/// (effects on eating, a bowl or bottle left behind).
fn food(host: &dyn Host, slot: u32, stack: &ItemStack) -> Option<Food> {
    let kind = ItemResource::of(stack);
    let component = |id: &str| ComponentId::new(id).and_then(|c| kind.value(host, &c));
    let food = component("minecraft:food")?;
    let effects = match component("minecraft:consumable").as_ref().and_then(|c| c.get("on_consume_effects")) {
        Some(Tag::List(list)) => !list.is_empty(),
        _ => false,
    };
    Some(Food {
        slot,
        item: stack.item().id(host),
        nutrition: int(food.get("nutrition")).unwrap_or(0),
        saturation: float(food.get("saturation")).unwrap_or(0.0),
        extra: effects || component("minecraft:use_remainder").is_some(),
    })
}

/// The main inventory, hotbar and off hand, non-empty only.
fn stacks(host: &dyn Host, p: PlayerRef) -> Vec<(u32, ItemStack)> {
    p.inventory_slots(host).unwrap_or_default().into_iter().filter(|(s, st)| (*s < 36 || *s == 40) && st.count() > 0).collect()
}

/// Each tick: a player below their hunger line eats the best plain food they carry; climbing and
/// the horse ridden are looked at.
pub fn tick(host: &mut dyn ServerHost) {
    for p in host.players().server_players() {
        let c = comforts(&*host, p);
        let since = with(|s| {
            let n = s.since_meal.entry(p.uuid()).or_insert(rules::EAT_EVERY);
            *n = n.saturating_add(1);
            *n
        });
        if c.eat > 0
            && since >= rules::EAT_EVERY
            && let Some(level) = p.food(&*host).map(|f| f.level)
            && level < c.eat
            && matches!(p.game_mode(&*host), Some(GameMode::Survival | GameMode::Adventure))
            && eat(host, p, level, c.eat)
        {
            with(|s| s.since_meal.insert(p.uuid(), 0));
        }
        climb(host, p, c.climb);
        remember_horse(host, p);
    }
}

/// Faster climbing: while the player goes up a ladder or vine looking up, the server sets their
/// upward speed (the client owns a player's motion and takes it from its next tick).
fn climb(host: &mut dyn ServerHost, p: PlayerRef, percent: i32) {
    let Some([_, y, _]) = p.entity().position(&*host) else { return };
    let dy = with(|s| s.last_y.insert(p.uuid(), y)).map_or(0.0, |last| y - last);
    if percent <= 100 {
        return;
    }
    let climbing = host.living(p.entity()).and_then(|l| l.is_climbing(&*host)) == Some(true);
    let pitch = p.entity().rotation(&*host).map_or(0.0, |[_, pitch]| pitch);
    if let Some(up) = rules::climb_speed(percent, climbing, pitch, dy) {
        p.set_velocity(host, [0.0, up, 0.0]);
    }
}

fn rider_tag(player: u128) -> String {
    format!("hands.rider.{player:032x}")
}

fn kind(host: &dyn Host, e: EntityRef) -> String {
    e.kind(host).map(|k| k.id(host)).unwrap_or_default()
}

/// The player's tagged horses within the whistle's reach.
fn tagged_horses(host: &dyn Host, p: PlayerRef) -> Vec<EntityRef> {
    let (Some(level), Some([x, y, z])) = (p.level(host), p.entity().position(host)) else { return Vec::new() };
    let r = rules::WHISTLE_REACH;
    let tag = rider_tag(p.uuid());
    level
        .entities_in(host, [x - r, y - r, z - r], [x + r, y + r, z + r])
        .unwrap_or_default()
        .into_iter()
        .filter(|e| rules::HORSES.contains(&kind(host, *e).as_str()) && e.tags(host).unwrap_or_default().contains(&tag))
        .collect()
}

/// A player on a horse: that horse becomes the one their whistle calls (its tag survives a
/// restart; the tag leaves the horses they rode before).
fn remember_horse(host: &mut dyn ServerHost, p: PlayerRef) {
    let Some(horse) = p.entity().vehicle(&*host) else { return };
    if with(|s| s.rode.get(&p.uuid()) == Some(&horse.id())) || !rules::HORSES.contains(&kind(&*host, horse).as_str()) {
        return;
    }
    let tag = rider_tag(p.uuid());
    for old in tagged_horses(&*host, p) {
        if old.id() != horse.id() {
            old.remove_tag(host, &tag);
        }
    }
    if horse.add_tag(host, &tag) {
        with(|s| s.rode.insert(p.uuid(), horse.id()));
    }
}

/// A goat horn blown while sneaking: the last ridden horse, if tamed, free and within 128 blocks
/// in this world, comes to the player. The horn sounds as always.
pub fn whistle(host: &mut dyn ServerHost, stack: &ItemStack) {
    let Some(p) = host.players().acting_player() else { return };
    if stack.item().id(&*host) != "minecraft:goat_horn" || p.entity().is_crouching(&*host) != Some(true) || !comforts(&*host, p).whistle {
        return;
    }
    let Some(at) = p.entity().position(&*host) else { return };
    let horses = tagged_horses(&*host, p);
    let seen: Vec<rules::Horse> = horses
        .iter()
        .map(|h| {
            let pos = h.position(&*host).unwrap_or([f64::MAX; 3]);
            rules::Horse {
                dist2: (0..3).map(|i| (pos[i] - at[i]).powi(2)).sum(),
                tamed: host.living(*h).and_then(|l| l.is_tamed(&*host)) == Some(true),
                ridden: !h.passengers(&*host).unwrap_or_default().is_empty(),
            }
        })
        .collect();
    if let Some(i) = rules::call_horse(&seen) {
        horses[i].teleport_to(host, at, None);
    }
}

/// Eats one of the best food: the stack shrinks first, so a refused write never feeds for free.
fn eat(host: &mut dyn ServerHost, p: PlayerRef, level: i32, line: i32) -> bool {
    let all = stacks(&*host, p);
    let foods: Vec<Food> = all.iter().filter_map(|(s, st)| food(&*host, *s, st)).collect();
    let Some(slot) = rules::pick_food(level, line, &foods) else { return false };
    let (Some((_, stack)), Some(f)) = (all.iter().find(|(s, _)| *s == slot), foods.iter().find(|f| f.slot == slot)) else { return false };
    if p.set_inventory_slot(host, slot, stack.clone().shrink(1)).is_err() {
        return false;
    }
    p.feed(host, f.nutrition, rules::saturation_modifier(f.nutrition, f.saturation));
    p.award_stat(host, StatType::Used, &f.item, 1);
    p.entity().play_sound(host, "minecraft:entity.generic.eat", 0.5, 1.0);
    p.entity().play_sound(host, "minecraft:entity.player.burp", 0.5, 1.0);
    true
}

fn face_of(n: [i32; 3]) -> Option<BlockFace> {
    Some(match n {
        [0, 1, 0] => BlockFace::Up,
        [0, -1, 0] => BlockFace::Down,
        [0, 0, -1] => BlockFace::North,
        [0, 0, 1] => BlockFace::South,
        [-1, 0, 0] => BlockFace::West,
        [1, 0, 0] => BlockFace::East,
        _ => return None,
    })
}

fn is_air(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air")
}

/// A right-click with a pickaxe or shovel that no block took (a chest or a door still opens):
/// on a dark spot, a torch from the inventory goes there. True when it did.
pub fn torch(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> bool {
    let Some(p) = host.players().acting_player() else { return false };
    let Some(held) = click.stack.filter(|s| s.count() > 0) else { return false };
    let kind = tool_kind(&*host, &held.item().id(&*host));
    if !matches!(kind.as_deref(), Some("minecraft:pickaxes" | "minecraft:shovels")) || !comforts(&*host, p).torch {
        return false;
    }
    let creative = match p.game_mode(&*host) {
        Some(GameMode::Creative) => true,
        Some(GameMode::Survival | GameMode::Adventure) => false,
        _ => return false,
    };
    let (Some(at), Some(face), Some(level)) = (rules::torch_at(click.hit.face), face_of(click.hit.face), p.level(&*host)) else {
        return false;
    };
    let n = click.hit.face;
    let spot = [click.pos[0] + n[0], click.pos[1] + n[1], click.pos[2] + n[2]];
    let states = host.block_states();
    let name = |pos| level.block_state(&*host, pos).and_then(|s| states.name(s.block()).ok().map(str::to_owned)).unwrap_or_default();
    let light = |layer| level.brightness(&*host, layer, spot).unwrap_or(15);
    let darken = level.get_sky_darken(&*host).unwrap_or(0);
    if !is_air(&name(spot)) || !kit::light::monster_dark(light(mod_api::LightLayer::Block), light(mod_api::LightLayer::Sky), darken) {
        return false;
    }
    let support = match at {
        TorchAt::Floor => level.block_face_sturdy(&*host, click.pos, BlockFace::Up, mod_api::SupportType::Center),
        TorchAt::Wall { .. } => level.block_face_sturdy(&*host, click.pos, face, mod_api::SupportType::Full),
    };
    if support != Some(true) {
        return false;
    }
    let all = stacks(&*host, p);
    let torches: Vec<(u32, u32)> =
        all.iter().filter(|(_, st)| st.item().id(&*host) == "minecraft:torch").map(|(s, st)| (*s, st.count())).collect();
    let Some(slot) = rules::torch_from(&torches) else { return false };
    let Some((_, stack)) = all.iter().find(|(s, _)| *s == slot) else { return false };
    if p.may_use_item_at(&*host, spot, face, stack) != Some(true) {
        return false;
    }
    let Some(state) = torch_state(&*host, at) else { return false };
    if !creative && p.set_inventory_slot(host, slot, stack.clone().shrink(1)).is_err() {
        return false;
    }
    if p.place_block(host, level, spot, state, true).is_err() {
        // Nothing placed: the torch goes back.
        if !creative {
            let _ = p.set_inventory_slot(host, slot, Some(stack.clone()));
        }
        return false;
    }
    p.award_stat(host, StatType::Used, "minecraft:torch", 1);
    let centre = [f64::from(spot[0]) + 0.5, f64::from(spot[1]) + 0.5, f64::from(spot[2]) + 0.5];
    level.play_sound(host, "minecraft:block.wood.place", centre, SoundSource::Blocks, 1.0, 0.8);
    true
}

fn torch_state(host: &dyn Host, at: TorchAt) -> Option<BlockState> {
    let states = host.block_states();
    let meta: &dyn BlockMetadata = &states;
    match at {
        TorchAt::Floor => BlockState::of(meta, host.block_key("minecraft:torch")?, &[]).ok(),
        TorchAt::Wall { facing } => {
            BlockState::of(meta, host.block_key("minecraft:wall_torch")?, &[StateConstraint::new("facing", facing)]).ok()
        }
    }
}

/// A player left.
pub fn forget(player: u128) {
    with(|s| {
        s.since_meal.remove(&player);
        s.last_y.remove(&player);
        s.rode.remove(&player);
    });
}
