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
use module_evm::{ExitReason, ExitSucceed};
use primitive_types::H256;
use sp_core::{H160, U256};
use sp_runtime::SaturatedConversion;
use sp_std::vec::Vec;
use support::{EVMBridge as EVMBridgeTrait, ExecutionMode, InvokeContext, EVM};

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::EVM as EVM<AccountIdOf<T>>>::Balance;

pub const METHOD_NAME: u32 = 0x06fdde03;
pub const METHOD_SYMBOL: u32 = 0x95d89b41;
pub const METHOD_DECIMALS: u32 = 0x313ce567;
pub const METHOD_TOTAL_SUPPLY: u32 = 0x18160ddd;
pub const METHOD_BALANCE_OF: u32 = 0x70a08231;
pub const METHOD_TRANSFER: u32 = 0xa9059cbb;

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
		/// Invalid return value
		InvalidReturnValue,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> EVMBridgeTrait<AccountIdOf<T>, BalanceOf<T>> for Pallet<T> {
	// Calls the name method on an ERC20 contract using the given context
	// and returns the token name.
	fn name(context: InvokeContext) -> Result<Vec<u8>, DispatchError> {
		// ERC20.name method hash
		let input = METHOD_NAME.to_be_bytes().to_vec();

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;
		Self::decode_string(info.output.as_slice().to_vec())
	}

	// Calls the symbol method on an ERC20 contract using the given context
	// and returns the token symbol.
	fn symbol(context: InvokeContext) -> Result<Vec<u8>, DispatchError> {
		// ERC20.symbol method hash
		let input = METHOD_SYMBOL.to_be_bytes().to_vec();

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;
		Self::decode_string(info.output.as_slice().to_vec())
	}

	// Calls the decimals method on an ERC20 contract using the given context
	// and returns the decimals.
	fn decimals(context: InvokeContext) -> Result<u8, DispatchError> {
		// ERC20.decimals method hash
		let input = METHOD_DECIMALS.to_be_bytes().to_vec();

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;

		ensure!(info.output.len() == 32, Error::<T>::InvalidReturnValue);
		let value = U256::from(info.output.as_slice()).saturated_into::<u8>();
		Ok(value)
	}

	// Calls the totalSupply method on an ERC20 contract using the given context
	// and returns the total supply.
	fn total_supply(context: InvokeContext) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.totalSupply method hash
		let input = METHOD_TOTAL_SUPPLY.to_be_bytes().to_vec();

		let info = T::EVM::execute(context, input, Default::default(), 2_100_000, 0, ExecutionMode::View)?;

		Self::handle_exit_reason(info.exit_reason)?;

		ensure!(info.output.len() == 32, Error::<T>::InvalidReturnValue);
		let value = U256::from(info.output.as_slice()).saturated_into::<u128>();
		Ok(value.saturated_into::<BalanceOf<T>>())
	}

	// Calls the balanceOf method on an ERC20 contract using the given context
	// and returns the address's balance.
	fn balance_of(context: InvokeContext, address: H160) -> Result<BalanceOf<T>, DispatchError> {
		// ERC20.balanceOf method hash
		let mut input = METHOD_BALANCE_OF.to_be_bytes().to_vec();
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
		let mut input = METHOD_TRANSFER.to_be_bytes().to_vec();
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

		Self::handle_exit_reason(info.exit_reason)?;

		// return value is true.
		let mut bytes = [0u8; 32];
		U256::from(1).to_big_endian(&mut bytes);

		// Check return value to make sure not calling on empty contracts.
		ensure!(
			!info.output.is_empty() && info.output == bytes,
			Error::<T>::InvalidReturnValue
		);
		Ok(())
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
			ExitReason::Succeed(ExitSucceed::Stopped) => Ok(()),
			ExitReason::Succeed(_) => Err(Error::<T>::ExecutionFail.into()),
			ExitReason::Revert(_) => Err(Error::<T>::ExecutionRevert.into()),
			ExitReason::Fatal(_) => Err(Error::<T>::ExecutionFatal.into()),
			ExitReason::Error(_) => Err(Error::<T>::ExecutionError.into()),
		}
	}

	fn decode_string(output: Vec<u8>) -> Result<Vec<u8>, DispatchError> {
		// output is 32-byte aligned and consists of 3 parts:
		// - part 1: 32 byte, the offset of its description is passed in the position of
		// the corresponding parameter or return value.
		// - part 2: 32 byte, string length
		// - part 3: string data
		ensure!(
			output.len() >= 64 && output.len() % 32 == 0,
			Error::<T>::InvalidReturnValue
		);

		let offset = U256::from_big_endian(&output[0..32]);
		let length = U256::from_big_endian(&output[offset.as_usize()..offset.as_usize() + 32]);
		ensure!(
			// output is 32-byte aligned. ensure total_length >= offset + string length + string data length.
			output.len() >= offset.as_usize() + 32 + length.as_usize(),
			Error::<T>::InvalidReturnValue
		);

		let mut data = Vec::new();
		data.extend_from_slice(&output[offset.as_usize() + 32..offset.as_usize() + 32 + length.as_usize()]);

		Ok(data.to_vec())
	}
}
