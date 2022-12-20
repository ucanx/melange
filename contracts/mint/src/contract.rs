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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            oracle,
            collector,
            collateral_oracle,
            melange_factory,
            lock,
            token_code_id,
            protocol_fee_rate,
            staking,
        } => update_config(
            deps,
            info,
            owner,
            oracle,
            collector,
            collateral_oracle,
            melange_factory,
            lock,
            token_code_id,
            protocol_fee_rate,
            staking,
        ),
        ExecuteMsg::UpdateAsset {
            asset_token,
            min_collateral_ratio,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            update_asset(
                deps,
                info,
                asset_addr,
                min_collateral_ratio,
            )
        }
        ExecuteMsg::RegisterAsset {
            asset_token,
            min_collateral_ratio,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            register_asset(
                deps,
                info,
                asset_addr,
                min_collateral_ratio,
            )
        }
        ExecuteMsg::RegisterMigration {
            asset_token,
            end_price,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            register_migration(deps, info, asset_addr, end_price)
        }
        ExecuteMsg::OpenPosition {
            collateral,
            asset_info,
            collateral_ratio,
        } => {
            // todo: Check the actual deposit happens

            open_position(
                deps,
                env,
                info.sender,
                collateral,
                asset_info,
                collateral_ratio,
            )
        }
        ExecuteMsg::Deposit {
            position_idx,
            collateral,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::generic_err("unauthorized"));
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&info)?;

            deposit(deps, info.sender, position_idx, collateral)
        }
        ExecuteMsg::Withdraw {
            position_idx,
            collateral,
        } => withdraw(deps, info.sender, position_idx, collateral),
        ExecuteMsg::Mint {
            position_idx,
            asset,
        } => mint(deps, env, info.sender, position_idx, asset),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    oracle: Option<String>,
    collector: Option<String>,
    collector_oracle: Option<String>,
    melange_factory: Option<String>,
    lock: Option<String>,
    token_code_id: Option<u64>,
    protocol_fee_rate: Option<Decimal>,
    staking: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(oracle) = oracle {
        config.oracle = deps.api.addr_canonicalize(&oracle)?;
    }

    if let Some(collector) = collector {
        config.collector = deps.api.addr_canonicalize(&collector)?;
    }

    if let Some(collateral_oracle) = collector_oracle {
        config.collateral_oracle = deps.api.addr_canonicalize(&collateral_oracle)?;
    }

    if let Some(melange_factory) = melange_factory {
        config.melange_factory = deps.api.addr_canonicalize(&melange_factory)?;
    }

    if let Some(lock) = lock {
        config.lock = deps.api.addr_canonicalize(&lock)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(protocol_fee_rate) = protocol_fee_rate {
        assert_protocol_fee(protocol_fee_rate)?;
        config.protocol_fee_rate = protocol_fee_rate;
    }

    if let Some(staking) = staking {
        config.staking = deps.api.addr_canonicalize(&staking)?;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AssetConfig { asset_token } => to_binary(&query_asset_config(deps, asset_token)?),
        QueryMsg::Position { position_idx } => to_binary(&query_position(deps, position_idx)?),
        QueryMsg::Positions {
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_positions(
            deps,
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::NextPositionIdx {} => to_binary(&query_next_position_idx(deps)?),
    }
}

pub fn query_asset_config(deps: Deps, asset_token: String) -> StdResult<AssetConfigResponse> {
    let asset_config: AssetConfig = read_asset_config(
        deps.storage,
        &deps.api.addr_canonicalize(asset_token.as_str())?,
    )?;

    let resp = AssetConfigResponse {
        token: deps
            .api
            .addr_humanize(&asset_config.token)
            .unwrap()
            .to_string(),
        min_collateral_ratio: asset_config.min_collateral_ratio,
        end_price: asset_config.end_price,
    };

    Ok(resp)
}
