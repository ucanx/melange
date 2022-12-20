use std::str::FromStr;

use crate::{
    contract::MIN_CR_ALLOWED,
    state::{AssetConfig, Position},
};
use cosmwasm_std::{Decimal, Deps, Env, StdError, StdResult};
use melange_protocol::asset::Asset;

pub fn assert_revoked_collateral(
    load_collateral_res: (Decimal, Decimal, bool),
) -> StdResult<(Decimal, Decimal)> {
    if load_collateral_res.2 {
        return Err(StdError::generic_err(
            "The collateral asset provided is no longer valid",
        ));
    }

    Ok((load_collateral_res.0, load_collateral_res.1))
}

pub fn assert_migrated_asset(asset_config: &AssetConfig) -> StdResult<()> {
    if asset_config.end_price.is_some() {
        return Err(StdError::generic_err(
            "Operation is not allowed for the deprecated asset",
        ));
    }

    Ok(())
}

// Check zero balance & same collateral with position
pub fn assert_collateral(deps: Deps, position: &Position, collateral: &Asset) -> StdResult<()> {
    if !collateral
        .info
        .equal(&position.collateral.info.to_normal(deps.api)?)
        || collateral.amount.is_zero()
    {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    Ok(())
}

// Check zero balance & same asset with position
pub fn assert_asset(deps: Deps, position: &Position, asset: &Asset) -> StdResult<()> {
    if !asset.info.equal(&position.asset.info.to_normal(deps.api)?) || asset.amount.is_zero() {
        return Err(StdError::generic_err("Wrong asset"));
    }

    Ok(())
}
