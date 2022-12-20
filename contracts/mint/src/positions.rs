use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, CosmosMsg, Decimal, Deps, DepsMut, Env, Response, StdError,
    StdResult, Uint128, WasmMsg,
};

use crate::{
    asserts::{
        assert_asset, assert_burn_period, assert_collateral, assert_migrated_asset,
        assert_mint_period, assert_pre_ipo_collateral, assert_revoked_collateral,
    },
    math::{
        decimal_division, decimal_min, decimal_multiplication, decimal_subtraction, reverse_decimal,
    },
    querier::{load_asset_price, load_collateral_info},
    state::{
        create_position, read_asset_config, read_config, read_position,
        read_position_idx, read_positions, read_positions_with_asset_indexer,
        read_positions_with_user_indexer, remove_position, store_position,
        store_position_idx, AssetConfig, Config, Position,
    }
};

use cw20::Cw20ExecuteMsg;
use melange_protocol::{
    common::OrderBy,
    lock::ExecuteMsg as LockExecuteMsg,
    mint::{NextPositionIdxResponse, PositionResponse, PositionsResponse},
    staking::ExecuteMsg as StakingExecuteMsg,
    asset::{Asset, AssetRaw, AssetInfo, AssetInfoRaw}
};


pub fn open_position(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    collateral: Asset,
    asset_info: AssetInfo,
    collateral_ratio: Decimal,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_info_raw: AssetInfoRaw = collateral.info.to_raw(deps.api)?;
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) = assert_revoked_collateral(
        load_collateral_info(deps.as_ref(), collateral_oracle, &collateral_info_raw, true)?,
    )?;

    // assert asset migrated
    let asset_info_raw: AssetInfoRaw = asset_info.to_raw(deps.api)?;
    let asset_token_raw = match asset_info_raw.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    if collateral_ratio
        < decimal_multiplication(asset_config.min_collateral_ratio, collateral_multiplier)
    {
        return Err(StdError::generic_err(
            "Can not open a position with low collateral ratio than minimum",
        ));
    }

    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(deps.as_ref(), oracle, &asset_info_raw, true)?;

    let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price);

    // Convert collateral to mint amount
    let mint_amount =
        collateral.amount * asset_price_in_collateral_asset * reverse_decimal(collateral_ratio);
    if mint_amount.is_zero() {
        return Err(StdError::generic_err("collateral is too small"));
    }

    let position_idx = read_position_idx(deps.storage)?;
    let asset_info_raw = asset_info.to_raw(deps.api)?;

    create_position(
        deps.storage,
        position_idx,
        &Position {
            idx: position_idx,
            owner: deps.api.addr_canonicalize(sender.as_str())?,
            collateral: AssetRaw {
                amount: collateral.amount,
                info: collateral_info_raw,
            },
            asset: AssetRaw {
                amount: mint_amount,
                info: asset_info_raw,
            },
        },
    )?;

    let asset_token = deps.api.addr_humanize(&asset_config.token)?.to_string();
    let messages: Vec<CosmosMsg> = {
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token,
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount: mint_amount,
            })?,
        })]
    };

    store_position_idx(deps.storage, position_idx + Uint128::from(1u128))?;
    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "open_position"),
            attr("position_idx", position_idx.to_string()),
            attr(
                "mint_amount",
                mint_amount.to_string() + &asset_info.to_string(),
            ),
            attr("collateral_amount", collateral.to_string()),
        ])
        .add_messages(messages))
}

pub fn deposit(
    deps: DepsMut,
    sender: Addr,
    position_idx: Uint128,
    collateral: Asset,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    // Check the given collateral has same asset info
    // with position's collateral token
    // also Check the collateral amount is non-zero
    assert_collateral(deps.as_ref(), &position, &collateral)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    assert_revoked_collateral(load_collateral_info(
        deps.as_ref(),
        collateral_oracle,
        &position.collateral.info,
        false,
    )?)?;

    // assert asset migrated
    match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => {
            assert_migrated_asset(&read_asset_config(deps.storage, &contract_addr)?)?
        }
        _ => panic!("DO NOT ENTER HERE"),
    };

    // Increase collateral amount
    position.collateral.amount += collateral.amount;
    store_position(deps.storage, position_idx, &position)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit"),
        attr("position_idx", position_idx.to_string()),
        attr("deposit_amount", collateral.to_string()),
    ]))
}

pub fn withdraw(
    deps: DepsMut,
    sender: Addr,
    position_idx: Uint128,
    collateral: Option<Asset>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    // if collateral is not provided, withraw all collateral
    let collateral: Asset = if let Some(collateral) = collateral {
        // Check the given collateral has same asset info
        // with position's collateral token
        // also Check the collateral amount is non-zero
        assert_collateral(deps.as_ref(), &position, &collateral)?;

        if position.collateral.amount < collateral.amount {
            return Err(StdError::generic_err(
                "Cannot withdraw more than you provide",
            ));
        }

        collateral
    } else {
        position.collateral.to_normal(deps.api)?
    };

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(deps.as_ref(), oracle, &position.asset.info, true)?;

    // Fetch collateral info from collateral oracle
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, mut collateral_multiplier, _collateral_is_revoked) =
        load_collateral_info(
            deps.as_ref(),
            collateral_oracle,
            &position.collateral.info,
            true,
        )?;

    // ignore multiplier for delisted assets
    if asset_config.end_price.is_some() {
        collateral_multiplier = Decimal::one();
    }

    // Compute new collateral amount
    let collateral_amount: Uint128 = position.collateral.amount.checked_sub(collateral.amount)?;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * decimal_division(asset_price, collateral_price);

    // Check minimum collateral ratio is satisfied
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio * collateral_multiplier
        > collateral_amount
    {
        return Err(StdError::generic_err(
            "Cannot withdraw collateral over than minimum collateral ratio",
        ));
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    position.collateral.amount = collateral_amount;

    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {

        store_position(deps.storage, position_idx, &position)?;
    }

    Ok(Response::new()
        .add_messages(
            vec![
                vec![collateral.clone().into_msg(&deps.querier, position_owner)?],
                messages,
            ]
                .concat(),
        )
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("position_idx", position_idx.to_string()),
            attr("withdraw_amount", collateral.to_string()),
        ]))
}

pub fn mint(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mint_amount = asset.amount;

    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    assert_asset(deps.as_ref(), &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    // assert the asset migrated
    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) =
        assert_revoked_collateral(load_collateral_info(
            deps.as_ref(),
            collateral_oracle,
            &position.collateral.info,
            true,
        )?)?;

    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(deps.as_ref(), oracle, &position.asset.info, true)?;

    // Compute new asset amount
    let asset_amount: Uint128 = mint_amount + position.asset.amount;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        asset_amount * decimal_division(asset_price, collateral_price);

    // Check minimum collateral ratio is satisfied
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio * collateral_multiplier
        > position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot mint asset over than min collateral ratio",
        ));
    }

    position.asset.amount += mint_amount;
    store_position(deps.storage, position_idx, &position)?;

    let asset_token = deps.api.addr_humanize(&asset_config.token)?;

    let messages: Vec<CosmosMsg> = {
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&asset_config.token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                amount: mint_amount,
                recipient: position_owner.to_string(),
            })?,
            funds: vec![],
        })]
    };

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "mint"),
            attr("position_idx", position_idx.to_string()),
            attr("mint_amount", asset.to_string()),
        ])
        .add_messages(messages))
}

