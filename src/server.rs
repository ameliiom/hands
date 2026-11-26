//! The server: the first hit on a block brings the hotbar's best tool to the hand; each tick the
//! hand is compared with the last, and a hand emptied by use is refilled from the main inventory.
//! Comparing hands each tick (not the tool-break hook) works for the LAN host too.
use crate::rules::{self, Back, Candidate, Hand, Spare};
use mod_api::{Host, ItemResource, ItemStack, PlayerRef, ServerHost};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// The tool kinds a broken tool may be replaced within.
const KINDS: [&str; 5] = ["minecraft:pickaxes", "minecraft:axes", "minecraft:shovels", "minecraft:hoes", "minecraft:swords"];

struct State {
    /// Each player's hand at the end of the last tick.
    hands: BTreeMap<u128, Hand>,
    /// Players who tossed an item this tick: their emptied hand isn't refilled.
    tossed: BTreeSet<u128>,
    /// Swaps to undo once each player stops digging.
    back: BTreeMap<u128, Back>,
    /// Entity ids of the players whose dig went on this tick (the game asks their break speed).
    dug: BTreeSet<u32>,
}

static STATE: Mutex<State> =
    Mutex::new(State { hands: BTreeMap::new(), tossed: BTreeSet::new(), back: BTreeMap::new(), dug: BTreeSet::new() });

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Uses left before `stack` breaks, None for an item that doesn't wear.
fn uses(host: &dyn Host, stack: &ItemStack) -> Option<i64> {
    crate::kit::wear::uses_left(host, &ItemResource::of(stack))
}

pub(crate) fn tool_kind(host: &dyn Host, item: &str) -> Option<String> {
    let entry = host.registry_entry("minecraft:item", item)?;
    KINDS.into_iter().find(|k| host.registry_entry_in_tag(&entry, k).unwrap_or(false)).map(str::to_owned)
}

/// The main inventory and hotbar, slot by slot (0 to 35), non-empty only.
fn slots(host: &dyn Host, p: PlayerRef) -> Vec<(u32, ItemStack)> {
    p.inventory_slots(host).unwrap_or_default().into_iter().filter(|(s, st)| *s < 36 && st.count() > 0).collect()
}

/// The first hit on a block: the hotbar's fastest tool for it comes to the hand. Sneak to keep
/// what you hold.
pub fn hit(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let Some(p) = host.players().acting_player() else { return };
    if p.entity().is_crouching(&*host) == Some(true) {
        return;
    }
    let (Some(level), Some(held)) = (p.level(&*host), p.selected_slot(&*host)) else { return };
    let bar = slots(&*host, p);
    let candidates: Vec<Candidate> = (0..9usize)
        .map(|slot| {
            let stack = bar.iter().find(|(s, _)| *s as usize == slot).map(|(_, st)| st);
            Candidate {
                slot,
                speed: level.block_break_speed(&*host, pos, stack).unwrap_or(1.0),
                uses: stack.and_then(|st| uses(&*host, st)),
            }
        })
        .collect();
    if let Some(slot) = rules::best_tool(held, &candidates)
        && p.set_selected_slot(host, slot)
    {
        with(|s| {
            let waiting = s.back.get(&p.uuid()).copied();
            s.back.insert(p.uuid(), rules::swapped(waiting, held, slot));
        });
    }
}

/// The game asked a player's break speed: they are still digging.
pub fn digging(entity: u32) {
    with(|s| s.dug.insert(entity));
}

/// A player tossed an item: if that emptied the hand, it was no use.
pub fn tossed(player: u128) {
    with(|s| s.tossed.insert(player));
}

/// Each tick: a hand emptied by use since the last tick is refilled from the main inventory.
pub fn tick(host: &mut dyn ServerHost) {
    let (tossed, dug) = with(|s| (std::mem::take(&mut s.tossed), std::mem::take(&mut s.dug)));
    for p in host.players().server_players() {
        let Some(slot) = p.selected_slot(&*host) else { continue };
        // Back to the slot held before a swap, a second after the digging stopped.
        if let Some(b) = with(|s| s.back.get(&p.uuid()).copied()) {
            let (wait, go) = rules::back_tick(b, slot, dug.contains(&p.entity().id()));
            with(|s| match wait {
                Some(w) => s.back.insert(p.uuid(), w),
                None => s.back.remove(&p.uuid()),
            });
            if let Some(to) = go {
                p.set_selected_slot(host, to);
                continue;
            }
        }
        let all = slots(&*host, p);
        let now = all.iter().find(|(s, _)| *s as usize == slot).map(|(_, st)| Hand {
            slot,
            item: st.item().id(&*host),
            count: st.count(),
            uses: uses(&*host, st),
        });
        let before = with(|s| s.hands.get(&p.uuid()).cloned());
        if let Some(before) = before
            && rules::used_up(&before, slot, now.is_none(), tossed.contains(&p.uuid()))
        {
            refill(host, p, &before, &all);
        }
        let after = p.selected_slot(&*host).and_then(|slot| {
            slots(&*host, p).into_iter().find(|(s, _)| *s as usize == slot).map(|(_, st)| Hand {
                slot,
                item: st.item().id(&*host),
                count: st.count(),
                uses: uses(&*host, &st),
            })
        });
        with(|s| match after {
            Some(h) => s.hands.insert(p.uuid(), h),
            None => s.hands.remove(&p.uuid()),
        });
    }
}

/// Moves the best spare from the main inventory (not the hotbar) into the emptied hand slot.
fn refill(host: &mut dyn ServerHost, p: PlayerRef, before: &Hand, all: &[(u32, ItemStack)]) {
    let spares: Vec<Spare> = all
        .iter()
        .filter(|(s, _)| (9..36).contains(s))
        .map(|(s, st)| {
            let item = st.item().id(&*host);
            Spare { slot: *s, count: st.count(), kind: tool_kind(&*host, &item), item }
        })
        .collect();
    let kind = tool_kind(&*host, &before.item);
    let Some(from) = rules::refill_from(before, kind.as_deref(), &spares) else { return };
    let Some((_, stack)) = all.iter().find(|(s, _)| *s == from).cloned() else { return };
    // Into the hand first: if that is refused, the spare stays where it was.
    if p.set_inventory_slot(host, before.slot as u32, Some(stack)).is_ok() && p.set_inventory_slot(host, from, None).is_err() {
        // Both copies would exist: take the hand's back rather than duplicate.
        let _ = p.set_inventory_slot(host, before.slot as u32, None);
    }
}

/// A player left.
pub fn forget(player: u128) {
    with(|s| {
        s.hands.remove(&player);
        s.tossed.remove(&player);
        s.back.remove(&player);
    });
}
