use codec::{Decode, Encode};
use evm::ExitReason;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::U256;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

pub use evm::backend::{Basic as Account, Log};
pub use evm::Config;

/// Evm Address.
pub type EvmAddress = sp_core::H160;

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
	/// Current transaction gas price.
	pub gas_price: U256,
	/// Origin of the transaction.
	pub origin: EvmAddress,
	/// Create contract opcode.
	pub creating: bool,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreateInfo {
	pub exit_reason: ExitReason,
	pub address: EvmAddress,
	pub output: Vec<u8>,
	pub used_gas: U256,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CallInfo {
	pub exit_reason: ExitReason,
	pub output: Vec<u8>,
	pub used_gas: U256,
}

/// A mapping between `AccountId` and `EvmAddress`.
pub trait AddressMapping<AccountId> {
	fn to_account(evm: &EvmAddress) -> AccountId;
	fn to_evm_address(account: &AccountId) -> Option<EvmAddress>;
}
