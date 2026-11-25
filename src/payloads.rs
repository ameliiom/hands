//! What crosses the network: the server tells each player's client whether it has earned the
//! spawn marks, which only the client draws. Registered (version "1", optional) in `register`.
use mod_api::{PayloadCodec, ToClient, VarInt};
use std::sync::OnceLock;

/// Server → client: 1 when this player has earned the spawn marks, else 0.
#[derive(PayloadCodec, Clone, Debug, PartialEq)]
pub struct Marks {
    pub earned: VarInt,
}

pub static MARKS: OnceLock<ToClient<Marks>> = OnceLock::new();
