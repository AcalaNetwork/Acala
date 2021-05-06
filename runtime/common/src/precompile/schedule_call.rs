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

// Disable the following lints
#![allow(clippy::type_complexity)]

use frame_support::{
	dispatch::Dispatchable,
	ensure, log, parameter_types,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, IsType, OriginTrait,
	},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::{AddressMapping as AddressMappingT, CurrencyIdMapping as CurrencyIdMappingT, TransactionPayment};
use primitives::{Balance, BlockNumber};
use sp_core::{H160, U256};
use sp_runtime::RuntimeDebug;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, PER_PARAM_BYTES};
use codec::{Decode, Encode};
use num_enum::TryFromPrimitive;
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
pub struct ScheduleCallPrecompile<
	AccountId,
	AddressMapping,
	CurrencyIdMapping,
	Scheduler,
	ChargeTransactionPayment,
	Call,
	Origin,
	PalletsOrigin,
	Runtime,
>(
	PhantomData<(
		AccountId,
		AddressMapping,
		CurrencyIdMapping,
		Scheduler,
		ChargeTransactionPayment,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	)>,
);

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum Action {
	Schedule = 0,
	Cancel = 1,
	Reschedule = 2,
}

type PalletBalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

impl<
		AccountId,
		AddressMapping,
		CurrencyIdMapping,
		Scheduler,
		ChargeTransactionPayment,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	> Precompile
	for ScheduleCallPrecompile<
		AccountId,
		AddressMapping,
		CurrencyIdMapping,
		Scheduler,
		ChargeTransactionPayment,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	> where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	CurrencyIdMapping: CurrencyIdMappingT,
	Scheduler: ScheduleNamed<BlockNumber, Call, PalletsOrigin, Address = TaskAddress<BlockNumber>>,
	ChargeTransactionPayment: TransactionPayment<AccountId, PalletBalanceOf<Runtime>, NegativeImbalanceOf<Runtime>>,
	Call: Dispatchable<Origin = Origin> + Debug + From<module_evm::Call<Runtime>>,
	Origin: IsType<<Runtime as frame_system::Config>::Origin>
		+ OriginTrait<AccountId = AccountId, PalletsOrigin = PalletsOrigin>,
	PalletsOrigin: Into<<Runtime as frame_system::Config>::Origin> + From<frame_system::RawOrigin<AccountId>> + Clone,
	Runtime: module_evm::Config + frame_system::Config<AccountId = AccountId>,
	PalletBalanceOf<Runtime>: IsType<Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		log::debug!(target: "evm", "schedule call: input: {:?}", input);

		// Solidity dynamic arrays will add the array size to the front of the array,
		// pre-compile needs to deal with the `size`.
		let input = Input::<Action, AccountId, AddressMapping, CurrencyIdMapping>::new(&input[32..]);

		let action = input.action()?;

		match action {
			Action::Schedule => {
				let from = input.evm_address_at(1)?;
				let target = input.evm_address_at(2)?;

				let value = input.balance_at(3)?;
				let gas_limit = input.u64_at(4)?;
				let storage_limit = input.u32_at(5)?;
				let min_delay = input.u32_at(6)?;
				let input_len = input.u32_at(7)?;
				let input_data = input.bytes_at(8 * PER_PARAM_BYTES, input_len as usize)?;

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
					// reserve the transaction fee for gas_limit
					use sp_runtime::traits::Convert;
					let from_account = AddressMapping::get_account_id(&from);
					let weight = <Runtime as module_evm::Config>::GasToWeight::convert(gas_limit);
					_fee = ChargeTransactionPayment::reserve_fee(&from_account, weight).map_err(|e| {
						let err_msg: &str = e.into();
						ExitError::Other(err_msg.into())
					})?;
				}

				let call = module_evm::Call::<Runtime>::scheduled_call(
					from,
					target,
					input_data,
					value.into(),
					gas_limit,
					storage_limit,
				)
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
					fee: _fee.into(),
				}
				.encode();

				log::debug!(
					target: "evm",
					"schedule call: task_id: {:?}",
					task_id,
				);

				Scheduler::schedule_named(
					task_id.clone(),
					DispatchTime::After(min_delay),
					None,
					0,
					Origin::root().caller().clone(),
					call,
				)
				.map_err(|_| ExitError::Other("Schedule failed".into()))?;

				// add task_id len prefix
				let mut task_id_with_len = [0u8; 96];
				U256::from(task_id.len()).to_big_endian(&mut task_id_with_len[0..32]);
				task_id_with_len[32..32 + task_id.len()].copy_from_slice(&task_id[..]);

				Ok((ExitSucceed::Returned, task_id_with_len.to_vec(), 0))
			}
			Action::Cancel => {
				let from = input.evm_address_at(1)?;
				let task_id_len = input.u32_at(2)?;
				let task_id = input.bytes_at(3 * PER_PARAM_BYTES, task_id_len as usize)?;

				log::debug!(
					target: "evm",
					"cancel call: from: {:?}, task_id: {:?}",
					from,
					task_id,
				);

				let task_info = TaskInfo::decode(&mut &task_id[..])
					.map_err(|_| ExitError::Other("Decode task_id failed".into()))?;
				ensure!(task_info.sender == from, ExitError::Other("NoPermission".into()));

				Scheduler::cancel_named(task_id).map_err(|_| ExitError::Other("Cancel schedule failed".into()))?;

				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					// unreserve the transaction fee for gas_limit
					let from_account = AddressMapping::get_account_id(&from);
					ChargeTransactionPayment::unreserve_fee(&from_account, task_info.fee.into());
				}

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Reschedule => {
				let from = input.evm_address_at(1)?;
				let min_delay = input.u32_at(2)?;
				let task_id_len = input.u32_at(3)?;
				let task_id = input.bytes_at(4 * PER_PARAM_BYTES, task_id_len as usize)?;

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

				Scheduler::reschedule_named(task_id, DispatchTime::After(min_delay)).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
		}
	}
}
