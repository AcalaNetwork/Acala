// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

// Disable the following lints
#![allow(clippy::type_complexity)]

use crate::precompile::PrecompileOutput;
use frame_support::{
	dispatch::Dispatchable,
	ensure, log, parameter_types,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, IsType, OriginTrait,
	},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::{AddressMapping, TransactionPayment};
use primitives::{Balance, BlockNumber};
use sp_core::H160;
use sp_runtime::RuntimeDebug;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, Output};
use codec::{Decode, Encode};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_scheduler::TaskAddress;

parameter_types! {
	pub storage EvmSchedulerNextID: u32 = 0u32;
}

#[derive(RuntimeDebug, PartialEq, Encode, Decode)]
pub struct TaskInfo {
	pub prefix: Vec<u8>,
	pub id: u32,
	pub sender: H160,
	#[codec(compact)]
	pub fee: Balance,
}

/// The `ScheduleCall` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - ScheduleCall. Rest `input` bytes: `from`, `target`, `value`, `gas_limit`, `storage_limit`,
///   `min_delay`, `input_len`, `input_data`.
pub struct ScheduleCallPrecompile<Runtime>(PhantomData<Runtime>);

#[module_evm_utiltity_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Schedule = "scheduleCall(address,address,uint256,uint256,uint256,bytes)",
	Cancel = "cancelCall(address,bytes)",
	Reschedule = "rescheduleCall(address,uint256,bytes)",
}

type PalletBalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

impl<Runtime> Precompile for ScheduleCallPrecompile<Runtime>
where
	PalletBalanceOf<Runtime>: IsType<Balance>,
	module_transaction_payment::ChargeTransactionPayment<Runtime>:
		TransactionPayment<Runtime::AccountId, PalletBalanceOf<Runtime>, NegativeImbalanceOf<Runtime>>,
	Runtime: module_evm::Config
		+ module_prices::Config
		+ module_transaction_payment::Config
		+ pallet_scheduler::Config
		+ Send
		+ Sync,
	<Runtime as pallet_scheduler::Config>::Call: Dispatchable + Debug + From<module_evm::Call<Runtime>>,
	<<Runtime as pallet_scheduler::Config>::Call as Dispatchable>::Origin: IsType<<Runtime as frame_system::Config>::Origin>
		+ OriginTrait<
			AccountId = Runtime::AccountId,
			PalletsOrigin = <Runtime as pallet_scheduler::Config>::PalletsOrigin,
		>,
	pallet_scheduler::Pallet<Runtime>: ScheduleNamed<
		BlockNumber,
		<Runtime as pallet_scheduler::Config>::Call,
		<Runtime as pallet_scheduler::Config>::PalletsOrigin,
		Address = TaskAddress<BlockNumber>,
	>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::Schedule => {
				let from = input.evm_address_at(1)?;
				let target = input.evm_address_at(2)?;

				let value = input.balance_at(3)?;
				let gas_limit = input.u64_at(4)?;
				let storage_limit = input.u32_at(5)?;
				let min_delay = input.u32_at(6)?;
				// solidity abi enocde bytes will add an length at input[7]
				let input_len = input.u32_at(8)?;
				let input_data = input.bytes_at(9, input_len as usize)?;

				log::debug!(
					target: "evm",
					"schedule call: from: {:?}, target: {:?}, value: {:?}, gas_limit: {:?}, storage_limit: {:?}, min_delay: {:?}, input_len: {:?}, input_data: {:?}",
					from,
					target,
					value,
					gas_limit,
					storage_limit,
					min_delay,
					input_len,
					input_data,
				);

				let mut _fee: PalletBalanceOf<Runtime> = Default::default();
				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					// reserve the transaction fee for gas_limit and storage_limit
					// TODO: reserve storage_limit here
					// Manually charge weight fee in scheduled_call
					use sp_runtime::traits::Convert;
					let from_account = Runtime::AddressMapping::get_account_id(&from);
					let weight = <Runtime as module_evm::Config>::GasToWeight::convert(gas_limit);
					_fee = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::reserve_fee(
						&from_account,
						weight,
					)
					.map_err(|e| {
						let err_msg: &str = e.into();
						ExitError::Other(err_msg.into())
					})?;
				}

				let call = module_evm::Call::<Runtime>::scheduled_call {
					from,
					target,
					input: input_data,
					value,
					gas_limit,
					storage_limit,
				}
				.into();

				let current_id = EvmSchedulerNextID::get();
				let next_id = current_id
					.checked_add(1)
					.ok_or_else(|| ExitError::Other("Scheduler next id overflow".into()))?;
				EvmSchedulerNextID::set(&next_id);

				let task_id = TaskInfo {
					prefix: b"ScheduleCall".to_vec(),
					id: current_id,
					sender: from,
					fee: _fee,
				}
				.encode();

				log::debug!(
					target: "evm",
					"schedule call: task_id: {:?}",
					task_id,
				);

				<pallet_scheduler::Pallet<Runtime> as ScheduleNamed<
					BlockNumber,
					<Runtime as pallet_scheduler::Config>::Call,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::schedule_named(
					task_id.clone(),
					DispatchTime::After(min_delay),
					None,
					0,
					<<<Runtime as pallet_scheduler::Config>::Call as Dispatchable>::Origin>::root()
						.caller()
						.clone(),
					call,
				)
				.map_err(|_| ExitError::Other("Schedule failed".into()))?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_bytes(&task_id),
					logs: Default::default(),
				})
			}
			Action::Cancel => {
				let from = input.evm_address_at(1)?;
				// solidity abi enocde bytes will add an length at input[2]
				let task_id_len = input.u32_at(3)?;
				let task_id = input.bytes_at(4, task_id_len as usize)?;

				log::debug!(
					target: "evm",
					"cancel call: from: {:?}, task_id: {:?}",
					from,
					task_id,
				);

				let task_info = TaskInfo::decode(&mut &task_id[..])
					.map_err(|_| ExitError::Other("Decode task_id failed".into()))?;
				ensure!(task_info.sender == from, ExitError::Other("NoPermission".into()));

				<pallet_scheduler::Pallet<Runtime> as ScheduleNamed<
					BlockNumber,
					<Runtime as pallet_scheduler::Config>::Call,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::cancel_named(task_id)
				.map_err(|_| ExitError::Other("Cancel schedule failed".into()))?;

				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					// unreserve the transaction fee for gas_limit
					let from_account = Runtime::AddressMapping::get_account_id(&from);
					<module_transaction_payment::ChargeTransactionPayment<Runtime>>::unreserve_fee(
						&from_account,
						task_info.fee,
					);
				}

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::Reschedule => {
				let from = input.evm_address_at(1)?;
				let min_delay = input.u32_at(2)?;
				// solidity abi enocde bytes will add an length at input[3]
				let task_id_len = input.u32_at(4)?;
				let task_id = input.bytes_at(5, task_id_len as usize)?;

				log::debug!(
					target: "evm",
					"reschedule call: from: {:?}, task_id: {:?}, min_delay: {:?}",
					from,
					task_id,
					min_delay,
				);

				let task_info = TaskInfo::decode(&mut &task_id[..])
					.map_err(|_| ExitError::Other("Decode task_id failed".into()))?;
				ensure!(task_info.sender == from, ExitError::Other("NoPermission".into()));

				<pallet_scheduler::Pallet<Runtime> as ScheduleNamed<
					BlockNumber,
					<Runtime as pallet_scheduler::Config>::Call,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::reschedule_named(task_id, DispatchTime::After(min_delay))
				.map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: vec![],
					logs: Default::default(),
				})
			}
		}
	}
}
