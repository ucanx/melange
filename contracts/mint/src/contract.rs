use crate::{
    asserts::{assert_min_collateral_ratio, assert_protocol_fee},
    migration::migrate_asset_configs,
    positions::{
        auction, burn, deposit, mint, open_position, query_next_position_idx, query_position,
        query_positions, withdraw,
    },
    state::{
        read_asset_config, read_config, store_asset_config, store_config, store_position_idx,
        AssetConfig, Config,
    },
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ReceiveMsg;
use melange_protocol::mint::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
    QueryMsg,
};
use melange_protocol::{
    collateral_oracle::{ExecuteMsg as CollateralOracleExecuteMsg, SourceType},
    mint::MigrateMsg,
};

use sei_cosmwasm::{
    BulkOrderPlacementsResponse, ContractOrderResult, DepositInfo, DexTwapsResponse, EpochResponse,
    ExchangeRatesResponse, GetLatestPriceResponse, GetOrderByIdResponse, GetOrdersResponse,
    LiquidationRequest, LiquidationResponse, MsgPlaceOrdersResponse, OracleTwapsResponse, Order,
    OrderSimulationResponse, OrderType, PositionDirection, SeiMsg, SeiQuerier, SeiQueryWrapper,
    SettlementEntry, SudoMsg,
};

pub const MIN_CR_ALLOWED: &str = "1.2";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        oracle: deps.api.addr_canonicalize(&msg.oracle)?,
        collector: deps.api.addr_canonicalize(&msg.collector)?,
        collateral_oracle: deps.api.addr_canonicalize(&msg.collateral_oracle)?,
        staking: deps.api.addr_canonicalize(&msg.staking)?,
        melange_factory: deps.api.addr_canonicalize(&msg.melange_factory)?,
        lock: deps.api.addr_canonicalize(&msg.lock)?,
        base_denom: msg.base_denom,
        token_code_id: msg.token_code_id,
        protocol_fee_rate: assert_protocol_fee(msg.protocol_fee_rate)?,
    };

    store_config(deps.storage, &config)?;
    store_position_idx(deps.storage, Uint128::from(1u128))?;
    Ok(Response::default())
}
