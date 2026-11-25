//! Hands: hit a block and the hotbar's best tool comes to your hand, use up a stack and the next
//! comes from your inventory, break a tool and one like it takes its place; and comforts earned
//! in the Skills tree Hands (eat on its own, a torch on a right-click, spawn marks). See
//! `docs/design.md`.
//!
//! - **The decisions** (`rules.rs`): which slot mines best, whether a hand emptied by use, which
//!   spare refills it, which food to eat, where a torch goes, where monsters spawn; plain values.
//! - **The comforts** (`perks.rs`): the tags the tree gives and the settings, no code tie to Skills.
//! - **The server** (`server.rs`): the first hit's swap, the way back once digging stops, and the
//!   tick's refill; (`comforts.rs`) eating, the torch, telling clients about the marks.
//! - **The client** (`client.rs`): the spawn marks.
mod client;
mod comforts;
mod payloads;
pub mod perks;
pub mod rules;
mod server;

use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientPacketEvent, ClientTickPostEvent, Flow, Hooks, HostSide, IPayloadContext, InteractionResult,
    ItemTossEvent, LeftClickBlockEvent, PayloadId, PlayerBreakSpeedEvent, PlayerLoggedOutEvent, Registrar, RegistrationContext,
    RenderLevelStageEvent, ServerHost, ServerTickEvent, ServerTickPhase, SettingSpec, UseItemOnBlockEvent, UsePhase,
};
#[allow(unused_imports)]
use mod_api::prelude::*;

pub(crate) const MOD_ID: &str = "hands";

fn on_marks(marks: payloads::Marks, _: &mut dyn IPayloadContext) -> Result<(), String> {
    client::earned(marks.earned.0 != 0);
    Ok(())
}

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        for (key, default) in perks::setting::ALL {
            if let Err(e) = r.register_setting(SettingSpec::toggle(key, format!("hands.config.{key}"), default)) {
                eprintln!("[hands] the setting {key}: {e:?}");
            }
        }
        let mut handlers = r.payload_handlers();
        let mut v1 = handlers.registrar("1").optional();
        let _ = payloads::MARKS.set(v1.play_to_client_typed(PayloadId("hands:marks".into()), on_marks));
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

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        client::tick(host);
    }

    /// Blocks or light changed around the player: the spawn marks are found again.
    fn client_packet(&self, _: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
        client::packet(event.packet);
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent) {
        client::reset();
    }

    fn render_level_stage(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        client::draw(host, event);
    }
}
