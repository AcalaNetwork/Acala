#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]

use ethereum_types::H160;
use primitives::evm::{CallInfo, CreateInfo};
use sp_runtime::{
	codec::Codec,
	traits::{MaybeDisplay, MaybeFromStr},
};
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	pub trait EVMRuntimeRPCApi<Balance> where
		Balance: Codec + MaybeDisplay + MaybeFromStr,
	{
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u32,
			storage_limit: u32,
			estimate: bool,
		) -> Result<CallInfo, sp_runtime::DispatchError>;

		fn create(
			from: H160,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u32,
			storage_limit: u32,
			estimate: bool,
		) -> Result<CreateInfo, sp_runtime::DispatchError>;
	}
}
