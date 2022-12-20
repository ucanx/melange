use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, StdError, StdResult, Storage, Uint128};

use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};
use melange_protocol::common::OrderBy;
use melange_protocol::asset::{AssetInfoRaw, AssetRaw};
use std::convert::TryInto;

pub static PREFIX_ASSET_CONFIG: &[u8] = b"asset_config";
static PREFIX_POSITION: &[u8] = b"position";
static PREFIX_INDEX_BY_USER: &[u8] = b"by_user";
static PREFIX_INDEX_BY_ASSET: &[u8] = b"by_asset";
pub static KEY_CONFIG: &[u8] = b"config";
static KEY_POSITION_IDX: &[u8] = b"position_idx";

pub fn store_position_idx(storage: &mut dyn Storage, position_idx: Uint128) -> StdResult<()> {
    singleton(storage, KEY_POSITION_IDX).save(&position_idx)
}

pub fn read_position_idx(storage: &dyn Storage) -> StdResult<Uint128> {
    singleton_read(storage, KEY_POSITION_IDX).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub oracle: CanonicalAddr,
    pub collector: CanonicalAddr,
    pub collateral_oracle: CanonicalAddr,
    pub staking: CanonicalAddr,
    pub melange_factory: CanonicalAddr,
    pub lock: CanonicalAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfig {
    pub token: CanonicalAddr,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
}

// check if the asset has either end_price or pre_ipo_price
pub fn read_fixed_price(storage: &dyn Storage, asset_info: &AssetInfoRaw) -> Option<Decimal> {
    match asset_info {
        AssetInfoRaw::Token { contract_addr } => {
            let asset_bucket: ReadonlyBucket<AssetConfig> =
                ReadonlyBucket::new(storage, PREFIX_ASSET_CONFIG);
            let res = asset_bucket.load(contract_addr.as_slice());
            match res {
                Ok(data) => {
                    data.end_price
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn read_asset_config(
    storage: &dyn Storage,
    asset_token: &CanonicalAddr,
) -> StdResult<AssetConfig> {
    let asset_bucket: ReadonlyBucket<AssetConfig> =
        ReadonlyBucket::new(storage, PREFIX_ASSET_CONFIG);
    let res = asset_bucket.load(asset_token.as_slice());
    match res {
        Ok(data) => Ok(data),
        _ => Err(StdError::generic_err("no asset data stored")),
    }
}

/// create position with index
pub fn create_position(
    storage: &mut dyn Storage,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    let mut position_bucket: Bucket<Position> = Bucket::new(storage, PREFIX_POSITION);
    position_bucket.save(&idx.u128().to_be_bytes(), position)?;

    let mut position_indexer_by_user: Bucket<bool> =
        Bucket::multilevel(storage, &[PREFIX_INDEX_BY_USER, position.owner.as_slice()]);
    position_indexer_by_user.save(&idx.u128().to_be_bytes(), &true)?;

    let mut position_indexer_by_asset: Bucket<bool> = Bucket::multilevel(
        storage,
        &[PREFIX_INDEX_BY_ASSET, position.asset.info.as_bytes()],
    );
    position_indexer_by_asset.save(&idx.u128().to_be_bytes(), &true)?;

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub idx: Uint128,
    pub owner: CanonicalAddr,
    pub collateral: AssetRaw,
    pub asset: AssetRaw,
}

/// store position with idx
pub fn store_position(
    storage: &mut dyn Storage,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    let mut position_bucket: Bucket<Position> = Bucket::new(storage, PREFIX_POSITION);
    position_bucket.save(&idx.u128().to_be_bytes(), position)?;
    Ok(())
}
