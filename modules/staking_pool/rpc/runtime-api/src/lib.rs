//! Runtime API definition for staking pool module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;

sp_api::decl_runtime_apis! {
	pub trait StakingPoolApi<AccountId, Balance> where
		AccountId: Codec,
		Balance: Codec,
	{
		fn get_available_unbonded(
			account: AccountId
		) -> Balance;
	}
}
