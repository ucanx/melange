use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

use crate::common::OrderBy;
use crate::asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub staking: String,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),

    /// Update config; only owner is allowed to execute it
    UpdateConfig {
        owner: Option<String>,
        oracle: Option<String>,
        collector: Option<String>,
        melange_factory: Option<String>,
        lock: Option<String>,
        token_code_id: Option<u64>,
        protocol_fee_rate: Option<Decimal>,
        staking: Option<String>,
    },
    /// Update asset related parameters
    UpdateAsset {
        asset_token: String,
        min_collateral_ratio: Option<Decimal>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token: String,
        min_collateral_ratio: Decimal,
    },
    RegisterMigration {
        asset_token: String,
        end_price: Decimal,
    },
    /// Create position to meet collateral ratio
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// Deposit more collateral
    Deposit {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// Withdraw collateral
    Withdraw {
        position_idx: Uint128,
        collateral: Option<Asset>,
    },
    /// Convert all deposit collateral to asset
    Mint {
        position_idx: Uint128,
        asset: Asset,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // Create position to meet collateral ratio
    OpenPosition {
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// Deposit more collateral
    Deposit { position_idx: Uint128 },
    /// Convert specified asset amount and send back to user
    Burn { position_idx: Uint128 },
    /// Buy discounted collateral from the contract with their asset tokens
    Auction { position_idx: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AssetConfig {
        asset_token: String,
    },
    Position {
        position_idx: Uint128,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub oracle: String,
    pub collector: String,
    pub collateral_oracle: String,
    pub staking: String,
    pub melange_factory: String,
    pub lock: String,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfigResponse {
    pub token: String,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub idx: Uint128,
    pub owner: String,
    pub collateral: Asset,
    pub asset: Asset,
}
