pub mod handler;
pub mod native;

use crate::{BalanceOf, Trait};
use primitives::evm::{CallInfo, CreateInfo};
use sp_core::{H160, H256};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

pub trait Runner<T: Trait> {
	fn call(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
	) -> Result<CallInfo, DispatchError>;

	fn create(source: H160, init: Vec<u8>, value: BalanceOf<T>, gas_limit: u32) -> Result<CreateInfo, DispatchError>;

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u32,
	) -> Result<CreateInfo, DispatchError>;

	fn create_at_address(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		assigned_address: H160,
		gas_limit: u32,
	) -> Result<CreateInfo, DispatchError>;
}
