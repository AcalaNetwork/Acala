use pvq_extension::{extensions_impl, metadata::Metadata, ExtensionsExecutor, InvokeSource};

#[extensions_impl]
pub mod extensions {
	use parity_scale_codec::Encode;
	#[extensions_impl::impl_struct]
	pub struct ExtensionImpl;

	#[extensions_impl::extension]
	impl pvq_extension_swap::extension::ExtensionSwap for ExtensionImpl {
		type AssetId = crate::Vec<u8>;
		type Balance = crate::Balance;
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

		fn list_pools() -> scale_info::prelude::vec::Vec<(Self::AssetId, Self::AssetId, Self::Balance, Self::Balance)> {
			let pools = module_dex::LiquidityPool::<crate::Runtime>::iter()
				.map(|(trading_pair, (balance1, balance2))| {
					(
						trading_pair.first().encode(),
						trading_pair.second().encode(),
						balance1,
						balance2,
					)
				})
				.collect();
			pools
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
