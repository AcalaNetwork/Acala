#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use ethereum_types::BigEndianHash;
use frame_support::{
	decl_error, decl_module,
	dispatch::{DispatchError, DispatchResult},
};
use module_evm::ExitReason;
use primitive_types::H256;
use sp_core::{H160, U256};
use sp_runtime::{RuntimeDebug, SaturatedConversion};
use sp_std::fmt::Debug;
use support::EVM;

mod mock;
mod tests;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
pub struct InvokeContext {
	pub contract: H160,
	pub source: H160,
}

pub type BalanceOf<T> = <<T as Trait>::EVM as EVM>::Balance;

/// EvmBridge module trait
pub trait Trait: frame_system::Trait {
	type EVM: EVM;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		Revert,
		Fatal,
		Error
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
	}
}

impl<T: Trait> Module<T> {
	fn total_supply(context: InvokeContext) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.totalSupply method hash
		let input = vec![0x18, 0x16, 0x0d, 0xdd];

		let info = T::EVM::execute(H160::default(), context.contract, input, Default::default(), 2_100_000)?;

		Self::handle_exit_reason(info.exit_reason)?;

		let value = U256::from(info.output.as_slice()).saturated_into::<u128>();
		Ok(value.saturated_into::<BalanceOf<T>>())
	}

	pub fn balance_of(context: InvokeContext, address: H160) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.balanceOf method hash
		let mut input = vec![0x70, 0xa0, 0x82, 0x31];
		// append address
		input.append(&mut Vec::from(H256::from(address).as_bytes()));

		let info = T::EVM::execute(H160::default(), context.contract, input, Default::default(), 2_100_000)?;

		Self::handle_exit_reason(info.exit_reason)?;

		Ok(U256::from(info.output.as_slice())
			.saturated_into::<u128>()
			.saturated_into::<BalanceOf<T>>())
	}

	pub fn transfer(context: InvokeContext, to: H160, value: BalanceOf<T>) -> DispatchResult {
		// ERC20.transfer method hash
		let mut input = vec![0xa9, 0x05, 0x9c, 0xbb];
		// append receiver address
		input.append(&mut Vec::from(H256::from(to).as_bytes()));
		// append amount to be transferred
		input.append(&mut Vec::from(
			H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes(),
		));

		let info = T::EVM::execute(context.source, context.contract, input, Default::default(), 2_100_000)?;

		Self::handle_exit_reason(info.exit_reason)
	}

	fn handle_exit_reason(exit_reason: ExitReason) -> Result<(), DispatchError> {
		match exit_reason {
			ExitReason::Succeed(_) => Ok(()),
			ExitReason::Revert(_) => Err(Error::<T>::Revert.into()),
			ExitReason::Fatal(_) => Err(Error::<T>::Fatal.into()),
			ExitReason::Error(_) => Err(Error::<T>::Error.into()),
		}
	}
}
