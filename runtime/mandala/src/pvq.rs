use parity_scale_codec::{Decode, Encode};
use pvq_extension::{extensions_impl, metadata::Metadata, ExtensionsExecutor, InvokeSource};
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct AssetInfo {
	pub asset_id: Vec<u8>,
	pub name: Vec<u8>,
	pub symbol: Vec<u8>,
	pub decimals: u8,
}

#[extensions_impl]
pub mod extensions {
	use alloc::collections::BTreeMap;
	use parity_scale_codec::Encode;
	use primitives::currency::{AssetIds, CurrencyId, TokenInfo};
	#[extensions_impl::impl_struct]
	pub struct ExtensionImpl;

	#[extensions_impl::extension]
	impl pvq_extension_swap::extension::ExtensionSwap for ExtensionImpl {
		type AssetId = crate::Vec<u8>;
		type Balance = crate::Balance;
		type AssetInfo = super::AssetInfo;
		fn quote_price_tokens_for_exact_tokens(
			asset1: Self::AssetId,
			asset2: Self::AssetId,
			amount: Self::Balance,
			include_fee: bool,
		) -> Option<Self::Balance> {
			if let Ok(asset1) = parity_scale_codec::Decode::decode(&mut &asset1[..]) {
				if let Ok(asset2) = parity_scale_codec::Decode::decode(&mut &asset2[..]) {
					if !include_fee {
						return None;
					}
					let (supply_pool, target_pool) = crate::Dex::get_liquidity(asset1, asset2);
					// No option for include_fee = false
					let supply_amount = crate::Dex::get_supply_amount(supply_pool, target_pool, amount);
					return Some(supply_amount);
				}
			}
			None
		}

		fn quote_price_exact_tokens_for_tokens(
			asset1: Self::AssetId,
			asset2: Self::AssetId,
			amount: Self::Balance,
			include_fee: bool,
		) -> Option<Self::Balance> {
			if let Ok(asset1) = parity_scale_codec::Decode::decode(&mut &asset1[..]) {
				if let Ok(asset2) = parity_scale_codec::Decode::decode(&mut &asset2[..]) {
					if !include_fee {
						return None;
					}
					let (supply_pool, target_pool) = crate::Dex::get_liquidity(asset1, asset2);
					// No option for include_fee = false
					let target_amount = crate::Dex::get_target_amount(supply_pool, target_pool, amount);
					return Some(target_amount);
				}
			}
			None
		}

		fn get_liquidity_pool(asset1: Self::AssetId, asset2: Self::AssetId) -> Option<(Self::Balance, Self::Balance)> {
			if let Ok(asset1) = parity_scale_codec::Decode::decode(&mut &asset1[..]) {
				if let Ok(asset2) = parity_scale_codec::Decode::decode(&mut &asset2[..]) {
					let (balance1, balance2) = crate::Dex::get_liquidity(asset1, asset2);
					if balance1 == 0 && balance2 == 0 {
						return None;
					} else {
						return Some((balance1, balance2));
					}
				}
			}
			None
		}

		fn list_pools() -> scale_info::prelude::vec::Vec<(Self::AssetId, Self::AssetId)> {
			let pools = module_dex::LiquidityPool::<crate::Runtime>::iter()
				.map(|(trading_pair, _)| (trading_pair.first().encode(), trading_pair.second().encode()))
				.collect();
			pools
		}

		fn asset_info(asset: Self::AssetId) -> Option<Self::AssetInfo> {
			if let Ok(asset) = <CurrencyId as parity_scale_codec::Decode>::decode(&mut &asset[..]) {
				if asset.is_token_currency_id() {
					return Some(Self::AssetInfo {
						asset_id: asset.encode(),
						name: asset.name().expect("name is not empty").as_bytes().to_vec(),
						symbol: asset.symbol().expect("symbol is not empty").as_bytes().to_vec(),
						decimals: asset.decimals().expect("decimal is not empty"),
					});
				} else {
					// Query AssetRegistry
					let asset_id: AssetIds = asset.into();
					let asset_info = crate::AssetRegistry::asset_metadatas(asset_id);
					if let Some(asset_info) = asset_info {
						return Some(Self::AssetInfo {
							asset_id: asset.encode(),
							name: asset_info.name,
							symbol: asset_info.symbol,
							decimals: asset_info.decimals,
						});
					}
				}
			}
			None
		}

		fn assets_info() -> BTreeMap<Self::AssetId, Self::AssetInfo> {
			let mut assets = BTreeMap::new();
			for (asset_id, asset_info) in crate::AssetRegistry::<crate::Runtime>::iter() {
				assets.insert(
					asset_id.encode(),
					Self::AssetInfo {
						asset_id: asset_id.encode(),
						name: asset_info.name,
						symbol: asset_info.symbol,
						decimals: asset_info.decimals,
					},
				);
			}
			assets
		}
	}
}

pub fn execute_query(program: &[u8], args: &[u8], gas_limit: i64) -> pvq_primitives::PvqResult {
	let mut executor = ExtensionsExecutor::<extensions::Extensions, ()>::new(InvokeSource::RuntimeAPI);
	let (result, _) = executor.execute(program, args, Some(gas_limit));
	result
}

pub fn metadata() -> Metadata {
	extensions::metadata()
}
