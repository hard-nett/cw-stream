use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::state::Stream;

#[cw_serde]
pub struct InstantiateMsg {
    /// provider address to recieve payment for resources
    pub provider_addr: String,
    /// minimum duration of streaming (in hours)
    pub min_duration: u64,
    // pub accepted_payment: String,
}

#[cw_serde]
pub enum MigrateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    CreateNewStream {},
    CloseStream { id: u64 },
    CloseExpiredStream {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(Option<Stream>)]
    Stream { streamer: String, id: u64 },
    #[returns(Option<Stream>)]
    StreamById { id: u64 },
}

