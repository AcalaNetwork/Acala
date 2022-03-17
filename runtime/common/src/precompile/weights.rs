

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(dead_code)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct PrecompileWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> PrecompileWeights<T> {
	// Oracle::IsUpdated (r: 1, w: 1)
	// Oracle::RawValues (r: 3, w: 0)
	// Oracle::Values (r: 1, w: 1)
	pub fn oracle_get_price() -> Weight {
		(13_903_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	pub fn evm_query_new_contract_extra_bytes() -> Weight {
		(503_000 as Weight)
	}
	pub fn evm_query_storage_deposit_per_byte() -> Weight {
		(539_000 as Weight)
	}
	// EVMModule::Accounts (r: 1, w: 0)
	pub fn evm_query_maintainer() -> Weight {
		(2_334_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
	}
	pub fn evm_query_developer_deposit() -> Weight {
		(506_000 as Weight)
	}
	pub fn evm_query_publication_fee() -> Weight {
		(498_000 as Weight)
	}
	// Balances::Reserves (r: 1, w: 0)
	pub fn evm_query_developer_status() -> Weight {
		(1_800_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
	}
}
