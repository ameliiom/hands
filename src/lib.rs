//! Hands: hit a block and the hotbar's best tool comes to your hand, use up a stack and the next
//! comes from your inventory, break a tool and one like it takes its place; and comforts earned
//! in Skills' own trees (eat on its own, a torch on a right-click, quick climb, horse whistle). See
//! `docs/design.md`.
//!
//! - **The decisions** (`rules.rs`): which slot mines best, whether a hand emptied by use, which
//!   spare refills it, which food to eat, where a torch goes, how fast to climb, which horse comes; plain values.
//! - **The comforts** (`perks.rs`): the tags the tree gives and the settings, no code tie to Skills.
//! - **The server** (`server.rs`): the first hit's swap, the way back once digging stops, and the
//!   tick's refill; (`comforts.rs`) eating, the torch, climbing, the whistle.
// Kit (ameliiom/kit), the shared code: the installed Kit mod, or its source in place for
// local builds (`kit_local`, horizon-mods/.cargo/config.toml).
#[cfg(kit_local)]
#[allow(dead_code)]
#[path = "../../kit/src/lib.rs"]
mod kit;
#[cfg(not(kit_local))]
use ::kit;
mod comforts;
pub mod perks;
pub mod rules;
mod server;

use mod_api::{
    Flow, Hooks, HostSide, InteractionResult,
    ItemTossEvent, LeftClickBlockEvent, PlayerBreakSpeedEvent, PlayerLoggedOutEvent, Registrar, RegistrationContext,
    RightClickItemEvent, ServerHost, ServerTickEvent, ServerTickPhase, UseItemOnBlockEvent, UsePhase,
};
#[allow(unused_imports)]
use mod_api::prelude::*;

pub(crate) const MOD_ID: &str = "hands";

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        for key in perks::setting::ALL {
            if let Err(e) = r.register_setting(key.spec(MOD_ID)) {
                eprintln!("[hands] the setting {}: {e:?}", key.key);
            }
        }
    }

    /// The first hit on a block (a dig's start): the right tool to the hand.
    fn left_click_block(&self, host: &mut dyn ServerHost, event: &mut LeftClickBlockEvent) -> Flow {
        server::hit(host, event.pos);
        Flow::Continue
    }

    /// Asked each tick a dig goes on: the hand is in use, it doesn't go back yet.
    fn player_break_speed(&self, host: HostSide, event: &mut PlayerBreakSpeedEvent) -> Flow {
        if let HostSide::Server(_) = host {
            server::digging(event.player);
        }
        Flow::Continue
    }

    /// A right-click the block let through (a chest or door still opens): with a pickaxe or a
    /// shovel on a dark spot, a torch goes there.
    fn use_item_on_block(&self, host: HostSide, event: &mut UseItemOnBlockEvent<'_>) -> Flow {
        if let (HostSide::Server(host), UsePhase::ItemAfterBlock) = (host, event.phase)
            && comforts::torch(host, event.click)
        {
            *event.cancellation_result = InteractionResult::SuccessServer;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// A goat horn blown while sneaking calls the last ridden horse; the horn sounds as always.
    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        if let HostSide::Server(host) = host {
            comforts::whistle(host, event.click.stack);
        }
        Flow::Continue
    }

    /// A toss empties a hand without using it: no refill.
    fn item_toss(&self, _: &mut dyn ServerHost, event: &mut ItemTossEvent) -> Flow {
        server::tossed(event.player);
        Flow::Continue
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            server::tick(host);
            comforts::tick(host);
        }
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        server::forget(event.player);
        comforts::forget(event.player);
    }




}
