pub mod native;
pub mod stack;

use crate::Trait;
use primitives::evm::{CallInfo, CreateInfo};
use sp_core::{H160, H256, U256};
use sp_std::vec::Vec;

pub trait Runner<T: Trait> {
	type Error: Into<sp_runtime::DispatchError>;

	fn call(source: H160, target: H160, input: Vec<u8>, value: U256, gas_limit: u32) -> Result<CallInfo, Self::Error>;

	fn create(source: H160, init: Vec<u8>, value: U256, gas_limit: u32) -> Result<CreateInfo, Self::Error>;

	fn create2(source: H160, init: Vec<u8>, salt: H256, value: U256, gas_limit: u32)
		-> Result<CreateInfo, Self::Error>;
}
