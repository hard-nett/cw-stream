#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{State, STATE, STREAM_ID};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:bitsong-streaming";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // if using akash:
    // - create ica-account
    // - store all providers accepting btsg for bids
    // - other economic validation checks

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(
        deps.storage,
        &State {
            proivider: deps.api.addr_validate(&msg.provider_addr)?,
            min_duration: msg.min_duration,
            // accepted_payment: msg.accepted_payment,
        },
    )?;
    // start steam id increment at 1.
    STREAM_ID.save(deps.storage, &0)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateNewStream {} => manifest::create_stream(deps, info, env),
        ExecuteMsg::CloseStream { id } => manifest::close_stream(deps, env, info, id),
        ExecuteMsg::CloseExpiredStream {} => todo!(),
    }
}

pub mod manifest {
    use super::*;
    use crate::state::{Stream, STREAM, STREAMER_BY_ID, STREAM_ID};
    use cosmwasm_std::{coin, CosmosMsg, Empty, Timestamp};

    pub fn create_stream(
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;
        // increment global stream-id count
        let id = STREAM_ID.update(deps.storage, |mut i| {
            i += 1u64;
            Ok::<u64, ContractError>(i)
        })?;

        let mut stream_duration: u64 = 0;
        // assert stream payment
        let payment = info.funds.iter().find(|a| a.denom.as_str() == "ubtsg");
        if let Some(stream_payment) = payment {
            let duration_in_mins: u64 = stream_payment.amount.u128().try_into()?; // errors if payment amount >= 2^64
            let duration = duration_in_mins.checked_div(1_000_000).unwrap_or_default(); // converts micro denomination
            if duration >= state.min_duration.into() {
                stream_duration = Timestamp::from_seconds(duration * 60).seconds();
            } else {
                return Err(ContractError::MinimumStreamDurationError {});
            }
        } else {
            return Err(ContractError::NoStreamPaymentProvided {});
        }

        // define stream details
        let stream_state = Stream {
            streamer: info.sender.clone(),
            provider: state.proivider,
            stream_start: env.block.time,
            stream_duration: Timestamp::from_seconds(stream_duration),
            key: id,
            stream_expiration: env.block.time.plus_seconds(stream_duration),
        };

        // register stream to map, with (streamer,stream-id) as key
        STREAM.save(deps.storage, (info.sender.clone(), id), &stream_state)?;
        STREAMER_BY_ID.save(deps.storage, id, &info.sender)?;

        Ok(Response::new())
    }

    pub fn close_stream(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        id: u64,
    ) -> Result<Response, ContractError> {
        let mut msgs = vec![];
        // load stream via sender & stream id
        let stream = STREAM.may_load(deps.storage, (info.sender, id))?;
        if let Some(stream) = stream {
            // calculate unspent funds
            let unspent_time = stream.stream_duration.minus_seconds(
                env.block
                    .time
                    .minus_seconds(stream.stream_start.seconds())
                    .seconds(),
            );

            // return any unspent funds to sender
            let unspent_btsg = unspent_time.seconds().checked_div(60).unwrap(); // test
            let unspent_msg: CosmosMsg<Empty> = CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                to_address: stream.streamer.to_string(),
                amount: vec![coin(unspent_btsg.into(), "ubtsg")],
            });

            // pay provider with sent funds
            let spent_btsg = stream.stream_duration.seconds() - unspent_btsg;
            let spent_msg: CosmosMsg<Empty> = CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                to_address: stream.provider.to_string(),
                amount: vec![coin(spent_btsg.into(), "ubtsg")],
            });

            // form msg to send to provider
            msgs.extend(vec![unspent_msg, spent_msg]);
            // close stream
        } else {
            return Err(ContractError::NoStreamExists {});
        }

        Ok(Response::new().add_messages(msgs))
    }

    pub fn close_expired_stream(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        id: u64,
    ) -> Result<Response, ContractError> {
        let mut stream = query::stream_by_id(deps.as_ref(), id)?;
        if let Some(stream) = stream {
            // only expired stream can be closed
            if stream.stream_expiration.seconds() > env.block.time.seconds() {
                return Err(ContractError::StreamNotExpired {});
            } else {
                // send expected funds from provider
            }
        } else {
            return Err(ContractError::NoStreamExists {});
        }
        Ok(Response::new())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Stream { streamer, id } => to_json_binary(&query::stream(deps, streamer, id)?),
        QueryMsg::StreamById { id } => to_json_binary(&query::stream_by_id(deps, id)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

pub mod query {
    use super::*;
    use crate::state::{Stream, STREAM, STREAMER_BY_ID};

    pub fn stream(deps: Deps, streamer: String, id: u64) -> StdResult<Option<Stream>> {
        // check if streamer has an active stream
        Ok(STREAM.may_load(deps.storage, (deps.api.addr_validate(&streamer)?, id))?)
    }

    pub fn stream_by_id(deps: Deps, id: u64) -> StdResult<Option<Stream>> {
        if let Some(streamer) = STREAMER_BY_ID.may_load(deps.storage, id.clone())? {
            Ok(STREAM.may_load(
                deps.storage,
                (deps.api.addr_validate(&streamer.as_str())?, id),
            )?)
        } else {
            Ok(None)
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
//     use cosmwasm_std::{coins, from_json};

//     #[test]
//     fn proper_initialization() {
//         let mut deps = mock_dependencies();

//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(1000, "earth"));

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//         assert_eq!(0, res.messages.len());

//         // it worked, let's query the state
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: GetCountResponse = from_json(&res).unwrap();
//         assert_eq!(17, value.count);
//     }

//     #[test]
//     fn increment() {
//         let mut deps = mock_dependencies();

//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(2, "token"));
//         let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//         // beneficiary can release it
//         let info = mock_info("anyone", &coins(2, "token"));
//         let msg = ExecuteMsg::Increment {};
//         let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

//         // should increase counter by 1
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: GetCountResponse = from_json(&res).unwrap();
//         assert_eq!(18, value.count);
//     }

//     #[test]
//     fn reset() {
//         let mut deps = mock_dependencies();

//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(2, "token"));
//         let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//         // beneficiary can release it
//         let unauth_info = mock_info("anyone", &coins(2, "token"));
//         let msg = ExecuteMsg::Reset { count: 5 };
//         let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
//         match res {
//             Err(ContractError::Unauthorized {}) => {}
//             _ => panic!("Must return unauthorized error"),
//         }

//         // only the original creator can reset the counter
//         let auth_info = mock_info("creator", &coins(2, "token"));
//         let msg = ExecuteMsg::Reset { count: 5 };
//         let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

//         // should now be 5
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: GetCountResponse = from_json(&res).unwrap();
//         assert_eq!(5, value.count);
//     }
// }
