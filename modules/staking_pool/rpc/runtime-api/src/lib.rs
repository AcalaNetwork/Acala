//! Runtime API definition for staking pool module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
	pub trait StakingPoolApi<AccountId, Balance> where
		AccountId: Codec,
		Balance: Codec,
	{
		fn get_available_unbonded(
			account: AccountId
		) -> Balance;

		fn get_liquid_staking_exchange_rate() -> support::ExchangeRate;
	}
}
