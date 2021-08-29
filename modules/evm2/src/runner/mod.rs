pub mod stack;
pub mod storage_meter;

use crate::{CallInfo, Config, CreateInfo};
pub use primitives::{
	evm::{Account, EvmAddress, Log, Vicinity},
	ReserveIdentifier, MIRRORED_NFT_ADDRESS_START,
};
use sp_core::{H160, H256, U256};
use sp_std::vec::Vec;

pub trait Runner<T: Config> {
	type Error: Into<sp_runtime::DispatchError>;

	fn call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, Self::Error>;

	fn create(
		source: H160,
		init: Vec<u8>,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error>;

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error>;

	fn create_at_address(
		source: H160,
		init: Vec<u8>,
		value: U256,
		assigned_address: H160,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error>;
}
