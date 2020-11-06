#![cfg_attr(not(feature = "std"), no_std)]
use ethereum_types::{H160, U256};
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	/// API necessary for Ethereum-compatibility layer.
	pub trait EthereumApi {
		/// Returns a frame_ethereum::call response.
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			gas_price: U256,
			nonce: Option<U256>,
		) -> Result<(Vec<u8>, U256), sp_runtime::DispatchError>;
		/// Returns a frame_ethereum::create response.
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
