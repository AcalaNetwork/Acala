#![cfg_attr(not(feature = "std"), no_std)]

use ethereum_types::BigEndianHash;
use frame_support::{
	decl_error, decl_module,
	dispatch::{DispatchError, DispatchResult},
};
use hex_literal::hex;
use module_evm::{ExitReason, ExitSucceed};
use primitive_types::H256;
use sp_core::{H160, U256};
use sp_runtime::SaturatedConversion;
use support::{EVMBridge as EVMBridgeTrait, InvokeContext, EVM};

mod mock;
mod tests;

pub type BalanceOf<T> = <<T as Config>::EVM as EVM>::Balance;

/// EvmBridge module trait
pub trait Config: frame_system::Config {
	type EVM: EVM;
}

decl_error! {
	pub enum Error for Module<T: Config> {
		ExecutionFail,
		ExecutionRevert,
		ExecutionFatal,
		ExecutionError
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;
	}
}

impl<T: Config> EVMBridgeTrait<BalanceOf<T>> for Module<T> {
	fn total_supply(context: InvokeContext) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.totalSupply method hash
		let input = hex!("18160ddd").to_vec();

		let info = T::EVM::execute(
			H160::default(),
			context.contract,
			input,
			Default::default(),
			2_100_000,
			None,
		)?;

		Self::handle_exit_reason(info.exit_reason)?;

		let value = U256::from(info.output.as_slice()).saturated_into::<u128>();
		Ok(value.saturated_into::<BalanceOf<T>>())
	}

	fn balance_of(context: InvokeContext, address: H160) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.balanceOf method hash
		let mut input = hex!("70a08231").to_vec();
		// append address
		input.extend_from_slice(H256::from(address).as_bytes());

		let info = T::EVM::execute(
			H160::default(),
			context.contract,
			input,
			Default::default(),
			2_100_000,
			None,
		)?;

		Self::handle_exit_reason(info.exit_reason)?;

		Ok(U256::from(info.output.as_slice())
			.saturated_into::<u128>()
			.saturated_into::<BalanceOf<T>>())
	}

	fn transfer(context: InvokeContext, to: H160, value: BalanceOf<T>) -> DispatchResult {
		// ERC20.transfer method hash
		let mut input = hex!("a9059cbb").to_vec();
		// append receiver address
		input.extend_from_slice(H256::from(to).as_bytes());
		// append amount to be transferred
		input.extend_from_slice(H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes());

		let info = T::EVM::execute(
			context.source,
			context.contract,
			input,
			Default::default(),
			2_100_000,
			None,
		)?;

		Self::handle_exit_reason(info.exit_reason)
	}
}

impl<T: Config> Module<T> {
	fn handle_exit_reason(exit_reason: ExitReason) -> Result<(), DispatchError> {
		match exit_reason {
			ExitReason::Succeed(ExitSucceed::Returned) => Ok(()),
			ExitReason::Succeed(_) => Err(Error::<T>::ExecutionFail.into()),
			ExitReason::Revert(_) => Err(Error::<T>::ExecutionRevert.into()),
			ExitReason::Fatal(_) => Err(Error::<T>::ExecutionFatal.into()),
			ExitReason::Error(_) => Err(Error::<T>::ExecutionError.into()),
		}
	}
}
