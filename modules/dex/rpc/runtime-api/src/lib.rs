//! Runtime API definition for dex module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;

sp_api::decl_runtime_apis! {
	pub trait DexApi<CurrencyId, Balance> where
		CurrencyId: Codec,
		Balance: Codec,
	{
		fn get_supply_amount(
			supply_currency_id: CurrencyId,
			target_currency_id: CurrencyId,
			target_currency_amount: Balance,
		) -> Balance;
		fn get_target_amount(
			supply_currency_id: CurrencyId,
			target_currency_id: CurrencyId,
			supply_currency_amount: Balance,
		) -> Balance;
	}
}
