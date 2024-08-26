// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
#![allow(deprecated)] // schedule::v3 is deprecated but schedule precompile is expiermental anyway

use super::input::{Input, InputT, Output};
use frame_support::{
	ensure, parameter_types,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, IsType, OriginTrait,
	},
};
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use module_support::{AddressMapping, TransactionPayment};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_scheduler::TaskAddress;
use parity_scale_codec::{Decode, Encode};
use primitives::{Balance, BlockNumber};
use sp_core::H160;
use sp_runtime::{traits::Dispatchable, RuntimeDebug};
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*};

parameter_types! {
	pub storage EvmSchedulerNextID: u32 = 0u32;
}

#[derive(RuntimeDebug, PartialEq, Eq, Encode, Decode)]
pub struct TaskInfo {
	pub prefix: Vec<u8>,
	pub id: u32,
	pub sender: H160,
	#[codec(compact)]
	pub fee: Balance,
}

/// The `Schedule` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Schedule. Rest `input` bytes: `from`, `target`, `value`, `gas_limit`, `storage_limit`,
///   `min_delay`, `input_len`, `input_data`.
pub struct SchedulePrecompile<Runtime>(PhantomData<Runtime>);

#[module_evm_utility_macro::generate_function_selector]
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

impl<Runtime> Precompile for SchedulePrecompile<Runtime>
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
	<Runtime as pallet_scheduler::Config>::RuntimeCall: Dispatchable + Debug + From<module_evm::Call<Runtime>>,
	<<Runtime as pallet_scheduler::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: IsType<<Runtime as frame_system::Config>::RuntimeOrigin>
		+ OriginTrait<
			AccountId = Runtime::AccountId,
			PalletsOrigin = <Runtime as pallet_scheduler::Config>::PalletsOrigin,
		>,
	pallet_scheduler::Pallet<Runtime>: ScheduleNamed<
		BlockNumber,
		<Runtime as pallet_scheduler::Config>::RuntimeCall,
		<Runtime as pallet_scheduler::Config>::PalletsOrigin,
		Address = TaskAddress<BlockNumber>,
	>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		match action {
			Action::Schedule => {
				let from = input.evm_address_at(1)?;
				let target = input.evm_address_at(2)?;

				let value = input.balance_at(3)?;
				let gas_limit = input.u64_at(4)?;
				let storage_limit = input.u32_at(5)?;
				let min_delay = input.u32_at(6)?;
				let input_data = input.bytes_at(7)?;

				log::debug!(
					target: "evm",
					"schedule call: from: {:?}, target: {:?}, value: {:?}, gas_limit: {:?}, storage_limit: {:?}, min_delay: {:?}, input_data: {:?}",
					from,
					target,
					value,
					gas_limit,
					storage_limit,
					min_delay,
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
					let fee = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::weight_to_fee(weight);
					_fee = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::reserve_fee(
						&from_account,
						fee,
						None,
					)
					.map_err(|_| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Scheduler charge failed".into(),
						// TODO: upgrade schedule::v3::Named
						// output: Output::encode_error_msg("Scheduler charge failed", e),
					})?;
				}

				let call = module_evm::Call::<Runtime>::scheduled_call {
					from,
					target,
					input: input_data,
					value,
					gas_limit,
					storage_limit,
					access_list: vec![],
				}
				.into();

				let current_id = EvmSchedulerNextID::get();
				let next_id = current_id.checked_add(1).ok_or_else(|| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Scheduler next id overflow".into(),
				})?;
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
					<Runtime as pallet_scheduler::Config>::RuntimeCall,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::schedule_named(
					task_id.clone(),
					DispatchTime::After(min_delay),
					None,
					0,
					<<<Runtime as pallet_scheduler::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin>::root()
						.caller()
						.clone(),
					call,
				)
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Scheduler schedule failed".into(),
					// TODO: upgrade schedule::v3::Named
					// output: Output::encode_error_msg("Scheduler schedule failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes(&task_id),
				})
			}
			Action::Cancel => {
				let from = input.evm_address_at(1)?;
				let task_id = input.bytes_at(2)?;

				log::debug!(
					target: "evm",
					"cancel call: from: {:?}, task_id: {:?}",
					from,
					task_id,
				);

				let task_info = TaskInfo::decode(&mut &task_id[..]).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Decode task_id failed".into(),
				})?;
				ensure!(
					task_info.sender == from,
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "NoPermission".into(),
					}
				);

				<pallet_scheduler::Pallet<Runtime> as ScheduleNamed<
					BlockNumber,
					<Runtime as pallet_scheduler::Config>::RuntimeCall,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::cancel_named(task_id)
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Scheduler cancel failed".into(),
					// TODO: upgrade schedule::v3::Named
					// output: Output::encode_error_msg("Scheduler cancel failed", e),
				})?;

				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					// unreserve the transaction fee for gas_limit
					let from_account = Runtime::AddressMapping::get_account_id(&from);
					let _err_amount = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::unreserve_fee(
						&from_account,
						task_info.fee,
						None,
					);
				}

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::Reschedule => {
				let from = input.evm_address_at(1)?;
				let min_delay = input.u32_at(2)?;
				let task_id = input.bytes_at(3)?;

				log::debug!(
					target: "evm",
					"reschedule call: from: {:?}, task_id: {:?}, min_delay: {:?}",
					from,
					task_id,
					min_delay,
				);

				let task_info = TaskInfo::decode(&mut &task_id[..]).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Decode task_id failed".into(),
				})?;
				ensure!(
					task_info.sender == from,
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "NoPermission".into(),
					}
				);

				<pallet_scheduler::Pallet<Runtime> as ScheduleNamed<
					BlockNumber,
					<Runtime as pallet_scheduler::Config>::RuntimeCall,
					<Runtime as pallet_scheduler::Config>::PalletsOrigin,
				>>::reschedule_named(task_id, DispatchTime::After(min_delay))
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("Scheduler reschedule failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_transaction_payment::Config + pallet_scheduler::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let _action = input.action()?;
		// TODO: gas cost
		Ok(Self::BASE_COST)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		alice_evm_addr, bob_evm_addr, new_test_ext, run_to_block, Balances, RuntimeEvent as TestEvent, System, Test,
	};
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context};
	use sp_core::H160;

	type SchedulePrecompile = crate::SchedulePrecompile<Test>;

	#[test]
	fn schedule_precompile_should_work() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// scheduleCall(address,address,uint256,uint256,uint256,bytes) -> 0x64c91905
			// from
			// target
			// value
			// gas_limit
			// storage_limit
			// min_delay
			// offset
			// input_len
			// transfer bytes4(keccak256(signature)) 0xa9059cbb
			// to address
			// amount
			let input = hex! {"
				64c91905
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000000
				000000000000000000000000000000000000000000000000 00000000000493e0
				00000000000000000000000000000000000000000000000000000000 00000064
				00000000000000000000000000000000000000000000000000000000 00000001
				00000000000000000000000000000000000000000000000000000000 000000e0
				00000000000000000000000000000000000000000000000000000000 00000044
				a9059cbb
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000 000000000000000000000000000003e8
				00000000000000000000000000000000000000000000000000000000
			"};

			let resp = SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(sp_core::bytes::to_hex(&resp.output[..], false), "0x\
				0000000000000000000000000000000000000000000000000000000000000020\
				0000000000000000000000000000000000000000000000000000000000000029\
				305363686564756c6543616c6c000000001000000000000000000000000000000000000001824f12000000000000000000000000000000000000000000000000\
			");

			let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Scheduled { when: 3, index: 0 });
			assert!(System::events().iter().any(|record| record.event == event));

			// cancelCall(address,bytes) -> 0x93e32661
			// who
			// offset
			// task_id_len
			// task_id
			let cancel_input = hex! {"
				93e32661
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000040
				00000000000000000000000000000000000000000000000000000000 00000029
				305363686564756c6543616c6c00000000100000000000000000000000000000
				0000000001824f12000000000000000000000000000000000000000000000000
			"};

			let resp = SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&cancel_input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Canceled { when: 3, index: 0 });
			assert!(System::events().iter().any(|record| record.event == event));

			// schedule call again
			let resp = SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(sp_core::bytes::to_hex(&resp.output[..], false), "0x\
				0000000000000000000000000000000000000000000000000000000000000020\
				0000000000000000000000000000000000000000000000000000000000000029\
				305363686564756c6543616c6c010000001000000000000000000000000000000000000001824f12000000000000000000000000000000000000000000000000\
			");

			run_to_block(2);

			// rescheduleCall(address,uint256,bytes) -> 0x28302f34
			// who
			// min_delay
			// offset
			// task_id_len
			// task_id
			let reschedule_input = hex! {"
				28302f34
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000000000000000000000000000 00000060
				00000000000000000000000000000000000000000000000000000000 00000029
				305363686564756c6543616c6c01000000100000000000000000000000000000
				0000000001824f12000000000000000000000000000000000000000000000000
			"};

			let resp = SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&reschedule_input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Scheduled { when: 5, index: 0 });
			assert!(System::events().iter().any(|record| record.event == event));

			let from_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr());
			let to_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&bob_evm_addr());
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 999999700000);
				assert_eq!(Balances::reserved_balance(from_account.clone()), 300000);
				assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
			}
			#[cfg(feature = "with-ethereum-compatibility")]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 1000000000000);
				assert_eq!(Balances::reserved_balance(from_account.clone()), 0);
				assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
			}

			run_to_block(5);
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 999999949650);
				assert_eq!(Balances::reserved_balance(from_account), 0);
				assert_eq!(Balances::free_balance(to_account), 1000000001000);
			}
			#[cfg(feature = "with-ethereum-compatibility")]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 999999999000);
				assert_eq!(Balances::reserved_balance(from_account), 0);
				assert_eq!(Balances::free_balance(to_account), 1000000001000);
			}
		});
	}

	#[test]
	fn schedule_precompile_should_handle_invalid_input() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// scheduleCall(address,address,uint256,uint256,uint256,bytes) -> 0x64c91905
			// from
			// target
			// value
			// gas_limit
			// storage_limit
			// min_delay
			// offset
			// input_len
			// input_data
			let input = hex! {"
				64c91905
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000000
				000000000000000000000000000000000000000000000000 00000000000493e0
				00000000000000000000000000000000000000000000000000000000 00000064
				00000000000000000000000000000000000000000000000000000000 00000001
				00000000000000000000000000000000000000000000000000000000 000000e0
				00000000000000000000000000000000000000000000000000000000 0000003c
				0000000000000000000000000000000000000000000000000000000000000000
				1200000000000000000000000000000000000000000000000000000000000000
			"};

			let resp = SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(sp_core::bytes::to_hex(&resp.output[..], false), "0x\
				0000000000000000000000000000000000000000000000000000000000000020\
				0000000000000000000000000000000000000000000000000000000000000029\
				305363686564756c6543616c6c000000001000000000000000000000000000000000000001824f12000000000000000000000000000000000000000000000000\
			");

			let from_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr());
			let to_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&bob_evm_addr());
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 999999700000);
				assert_eq!(Balances::reserved_balance(from_account.clone()), 300000);
				assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
			}
			#[cfg(feature = "with-ethereum-compatibility")]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 1000000000000);
				assert_eq!(Balances::reserved_balance(from_account.clone()), 0);
				assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
			}

			// cancelCall(address,bytes) -> 0x93e32661
			// who
			// offset
			// task_id_len
			// task_id
			let cancel_input = hex! {"
				93e32661
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000000000000000000000000000 00000040
				00000000000000000000000000000000000000000000000000000000 00000029
				305363686564756c6543616c6c00000000100000000000000000000000000000
				0000000001824f12000000000000000000000000000000000000000000000000
			"};
			assert_eq!(
				SchedulePrecompile::execute(&mut MockPrecompileHandle::new(&cancel_input, Some(10_000), &context, false)),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "NoPermission".into(),
				})
			);

			run_to_block(4);
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 999999978554);
				assert_eq!(Balances::reserved_balance(from_account), 0);
				assert_eq!(Balances::free_balance(to_account), 1000000000000);
			}
			#[cfg(feature = "with-ethereum-compatibility")]
			{
				assert_eq!(Balances::free_balance(from_account.clone()), 1000000000000);
				assert_eq!(Balances::reserved_balance(from_account.clone()), 0);
				assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
			}
		});
	}

	#[test]
	fn task_id_max_and_min() {
		let task_id = TaskInfo {
			prefix: b"ScheduleCall".to_vec(),
			id: u32::MAX,
			sender: H160::default(),
			fee: Balance::MAX,
		}
		.encode();

		assert_eq!(54, task_id.len());

		let task_id = TaskInfo {
			prefix: b"ScheduleCall".to_vec(),
			id: u32::MIN,
			sender: H160::default(),
			fee: Balance::MIN,
		}
		.encode();

		assert_eq!(38, task_id.len());
	}
}
