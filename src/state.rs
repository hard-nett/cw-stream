use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Timestamp};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub proivider: Addr,
    // minimum duration in minutes
    pub min_duration: u64,
    // pub accepted_payment: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Stream {
    pub streamer: Addr,
    pub provider: Addr,
    pub stream_start: Timestamp,
    pub stream_duration: Timestamp,
    pub stream_expiration: Timestamp,
    pub key: u64,
}

pub const STATE: Item<State> = Item::new("state");

// The stream state, with a given streamer addr & key
pub const STREAM: Map<(Addr, u64), Stream> = Map::new("stream");
// map of streamer by id
pub const STREAMER_BY_ID: Map<u64, Addr> = Map::new("streamer");

pub const STREAM_ID: Item<u64> = Item::new("stream_id");

// streaming key
// verifier authorizes streaming key
