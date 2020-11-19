use codec::{Decode, Encode};
use evm::ExitReason;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H160, U256};
use sp_runtime::{traits::BadOrigin, RuntimeDebug};
use sp_std::vec::Vec;

pub use evm::backend::{Basic as Account, Log};

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
	/// Current transaction gas price.
	pub gas_price: U256,
	/// Origin of the transaction.
	pub origin: H160,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreateInfo {
	pub exit_reason: ExitReason,
	pub address: H160,
	pub value: Vec<u8>,
	pub used_gas: U256,
	pub logs: Vec<Log>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CallInfo {
	pub exit_reason: ExitReason,
	pub value: Vec<u8>,
	pub used_gas: U256,
	pub logs: Vec<Log>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CallOrCreateInfo {
	Call(CallInfo),
	Create(CreateInfo),
}

pub trait EnsureAddressOrigin<OuterOrigin> {
	/// Success return type.
	type Success;

	/// Perform the origin check.
	fn ensure_address_origin(address: &H160, origin: OuterOrigin) -> Result<Self::Success, BadOrigin> {
		Self::try_address_origin(address, origin).map_err(|_| BadOrigin)
	}

	/// Try with origin.
	fn try_address_origin(address: &H160, origin: OuterOrigin) -> Result<Self::Success, OuterOrigin>;
}
