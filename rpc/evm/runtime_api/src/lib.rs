#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]
use ethereum_types::{H160, U256};
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	pub trait EVMApi {
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			gas_price: U256,
			nonce: Option<U256>,
		) -> Result<(Vec<u8>, U256), sp_runtime::DispatchError>;

		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			gas_price: U256,
			nonce: Option<U256>,
		) -> Result<(H160, U256), sp_runtime::DispatchError>;
	}
}
