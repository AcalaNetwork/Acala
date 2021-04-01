// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use ethereum_types::BigEndianHash;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	pallet_prelude::*,
};
use hex_literal::hex;
use module_evm::{ExitReason, ExitSucceed};
use primitive_types::H256;
use sp_core::{H160, U256};
use sp_runtime::SaturatedConversion;
use support::{EVMBridge as EVMBridgeTrait, ExecutionMode, InvokeContext, EVM};

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::EVM as EVM<AccountIdOf<T>>>::Balance;

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	/// EvmBridge module trait
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type EVM: EVM<AccountIdOf<Self>>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Execution failed
		ExecutionFail,
		/// Execution reverted
		ExecutionRevert,
		/// Execution fatal
		ExecutionFatal,
		/// Execution error
		ExecutionError,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> EVMBridgeTrait<AccountIdOf<T>, BalanceOf<T>> for Pallet<T> {
	// Calls the totalSupply method on an ERC20 contract using the given context
	// and returns the total supply.
	fn total_supply(context: InvokeContext) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.totalSupply method hash
		let input = hex!("18160ddd").to_vec();

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;

		let value = U256::from(info.output.as_slice()).saturated_into::<u128>();
		Ok(value.saturated_into::<BalanceOf<T>>())
	}

	// Calls the balanceOf method on an ERC20 contract using the given context
	// and returns the address's balance.
	fn balance_of(context: InvokeContext, address: H160) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.balanceOf method hash
		let mut input = hex!("70a08231").to_vec();
		// append address
		input.extend_from_slice(H256::from(address).as_bytes());

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;

		Ok(U256::from(info.output.as_slice())
			.saturated_into::<u128>()
			.saturated_into::<BalanceOf<T>>())
	}

	// Calls the transfer method on an ERC20 contract using the given context.
	fn transfer(context: InvokeContext, to: H160, value: BalanceOf<T>) -> DispatchResult {
		// ERC20.transfer method hash
		let mut input = hex!("a9059cbb").to_vec();
		// append receiver address
		input.extend_from_slice(H256::from(to).as_bytes());
		// append amount to be transferred
		input.extend_from_slice(H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes());

		let storage_limit = if context.origin == Default::default() { 0 } else { 1_000 };

		let info = T::EVM::execute(
			context,
			input,
			Default::default(),
			2_100_000,
			storage_limit,
			ExecutionMode::Execute,
		)?;

		Self::handle_exit_reason(info.exit_reason)
	}

	fn get_origin() -> Option<AccountIdOf<T>> {
		T::EVM::get_origin()
	}

	fn set_origin(origin: AccountIdOf<T>) {
		T::EVM::set_origin(origin);
	}
}

impl<T: Config> Pallet<T> {
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
