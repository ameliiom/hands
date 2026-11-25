//! The client: while a torch is in either hand, small red crosses lie on the blocks around you
//! where monsters can spawn (dark, an opaque full top, two blocks of air above). The spots are
//! found again only when you move a block, the world around you changes (a block or light packet
//! from the server), or the sky darkens or brightens; each frame just draws them.
use crate::rules::{self, Cell};
use mod_api::{
    BlockMetadata, ClientHost, ClientPacket, EquipmentSlot, Host, HostBlockStates, LevelRef, LightLayer, RenderLevelStage, RenderLevelStageEvent,
    WorldDepth, WorldLine,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::sync::Mutex;

/// A warm red that reads on stone, deepslate and grass alike.
const COLOUR: u32 = 0xD0E0_2020;
const WIDTH: f32 = 2.0;
/// Half the cross's arm, in blocks.
const ARM: f64 = 0.18;
/// Ticks after a world change before the spots are found again, so the client's own light
/// update has run; once more a little later for light that spread slowly.
const SETTLE: [u32; 2] = [2, 12];

struct Marks {
    /// The server said this player earned the marks.
    earned: bool,
    /// The spots and what they were found for (the player's block, the sky's darkening).
    spots: Vec<[i32; 3]>,
    found_for: Option<([i32; 3], i32)>,
    /// Ticks since the last world change, while a recount waits.
    changed: Option<u32>,
    /// Torch held on the last tick.
    shown: bool,
}

static MARKS: Mutex<Marks> = Mutex::new(Marks { earned: false, spots: Vec::new(), found_for: None, changed: None, shown: false });

fn with<R>(f: impl FnOnce(&mut Marks) -> R) -> R {
    f(&mut MARKS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The server told whether this player earned the marks.
pub fn earned(on: bool) {
    with(|m| {
        m.earned = on;
        m.found_for = None;
    });
}

/// A packet that changes blocks or light around the player: the spots are found again soon.
pub fn packet(packet: &ClientPacket) {
    let id = packet.id.to_string();
    if matches!(
        id.as_str(),
        "minecraft:block_update" | "minecraft:section_blocks_update" | "minecraft:light_update" | "minecraft:level_chunk_with_light" | "minecraft:forget_level_chunk"
    ) {
        with(|m| m.changed = Some(0));
    }
}

/// The world left: nothing to show.
pub fn reset() {
    with(|m| {
        m.spots.clear();
        m.found_for = None;
        m.changed = None;
    });
}

fn holds_torch(host: &dyn Host, entity: mod_api::EntityRef) -> bool {
    let Some(living) = host.living(entity) else { return false };
    [EquipmentSlot::MainHand, EquipmentSlot::OffHand]
        .into_iter()
        .filter_map(|slot| living.equipment(host, slot))
        .any(|s| s.count() > 0 && s.item().id(host) == "minecraft:torch")
}

/// Each client tick: decides whether the marks show and finds the spots again when needed.
pub fn tick(host: &dyn ClientHost) {
    let read: &dyn Host = host;
    let Some(p) = host.local_player() else { return };
    let switched_on = crate::comforts::on(read, crate::perks::setting::MARKS);
    let show = with(|m| m.earned) && switched_on && holds_torch(read, p.entity());
    if !show {
        with(|m| {
            m.shown = false;
            m.spots.clear();
            m.found_for = None;
        });
        return;
    }
    let (Some(level), Some(at)) = (p.level(read), p.entity().block_position(read)) else { return };
    let darken = level.get_sky_darken(read).unwrap_or(0);
    let recount = with(|m| {
        let settled = match m.changed {
            Some(n) => {
                m.changed = (n < SETTLE[1]).then_some(n + 1);
                SETTLE.contains(&n)
            }
            None => false,
        };
        let moved = m.found_for != Some((at, darken));
        let started = !m.shown;
        m.shown = true;
        settled || moved || started
    });
    if recount {
        let spots = find(read, level, at, darken);
        with(|m| {
            m.spots = spots;
            m.found_for = Some((at, darken));
        });
    }
}

/// The spots around `at` where a monster could spawn now.
fn find(host: &dyn Host, level: LevelRef, at: [i32; 3], darken: i32) -> Vec<[i32; 3]> {
    let states = host.block_states();
    let (lo, hi) = (at[1] - rules::MARK_HEIGHT, at[1] + rules::MARK_HEIGHT);
    let mut spots = Vec::new();
    let mut column = Vec::with_capacity((hi - lo + 3) as usize);
    for x in at[0] - rules::MARK_REACH..=at[0] + rules::MARK_REACH {
        for z in at[2] - rules::MARK_REACH..=at[2] + rules::MARK_REACH {
            column.clear();
            // Two more on top so the highest floor sees its two airs.
            for y in lo..=hi + 2 {
                let cell = match level.block_state(host, [x, y, z]) {
                    None => Cell::Other,
                    Some(state) => match states.name(state.block()) {
                        Ok("minecraft:air" | "minecraft:cave_air") => Cell::Air,
                        Ok(name) if !rules::never_spawn_on(name) && states.is_solid_render(state) == Some(true) => Cell::Floor,
                        _ => Cell::Other,
                    },
                };
                column.push(cell);
            }
            for i in rules::spawn_floors(&column) {
                let floor = [x, lo + i as i32, z];
                let above = [x, floor[1] + 1, z];
                let light = |layer| level.brightness(host, layer, above).unwrap_or(15);
                if floor[1] <= hi && rules::dark(light(LightLayer::Block), light(LightLayer::Sky), darken) {
                    spots.push(floor);
                }
            }
        }
    }
    spots
}

/// A small cross lying on the top of the block at `p`.
fn cross([x, y, z]: [i32; 3]) -> [WorldLine; 2] {
    let (cx, cy, cz) = (f64::from(x) + 0.5, f64::from(y) + 1.01, f64::from(z) + 0.5);
    [
        WorldLine::new([cx - ARM, cy, cz - ARM], [cx + ARM, cy, cz + ARM], COLOUR, WIDTH, WorldDepth::Tested),
        WorldLine::new([cx - ARM, cy, cz + ARM], [cx + ARM, cy, cz - ARM], COLOUR, WIDTH, WorldDepth::Tested),
    ]
}

pub fn draw(_: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
    if !matches!(event.stage, RenderLevelStage::AfterTranslucentBlocks) {
        return;
    }
    let lines: Vec<WorldLine> = with(|m| m.spots.iter().flat_map(|p| cross(*p)).collect());
    if !lines.is_empty()
        && let Err(e) = event.draw.lines(&lines)
    {
        eprintln!("[hands] drawing the spawn marks: {e:?}");
    }
}

