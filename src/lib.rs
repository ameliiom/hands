//! Hands: hit a block and the hotbar's best tool comes to your hand, use up a stack and the next
//! comes from your inventory, break a tool and one like it takes its place. See `docs/design.md`.
//!
//! - **The decisions** (`rules.rs`): which slot mines best, whether a hand emptied by use, and
//!   which spare refills it, on plain values.
//! - **The server** (`server.rs`): the first hit's swap, the way back once digging stops, and the
//!   tick's refill.
pub mod rules;
mod server;

use mod_api::{
    Flow, Hooks, HostSide, ItemTossEvent, LeftClickBlockEvent, PlayerBreakSpeedEvent, PlayerLoggedOutEvent, ServerHost, ServerTickEvent,
    ServerTickPhase,
};
#[allow(unused_imports)]
use mod_api::prelude::*;

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
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

    /// A toss empties a hand without using it: no refill.
    fn item_toss(&self, _: &mut dyn ServerHost, event: &mut ItemTossEvent) -> Flow {
        server::tossed(event.player);
        Flow::Continue
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            server::tick(host);
        }
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        server::forget(event.player);
    }
}
