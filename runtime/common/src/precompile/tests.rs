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

#![cfg(test)]
use super::*;
use crate::precompile::{
	mock::{
		aca_evm_address, alice, alice_evm_addr, ausd_evm_address, bob, bob_evm_addr, erc20_address_not_exists,
		get_task_id, lp_aca_ausd_evm_address, new_test_ext, renbtc_evm_address, run_to_block, Balances, DexModule,
		DexPrecompile, Event as TestEvent, MultiCurrencyPrecompile, Oracle, OraclePrecompile, Origin, Price,
		ScheduleCallPrecompile, System, Test, ALICE, AUSD, INITIAL_BALANCE, RENBTC,
	},
	schedule_call::TaskInfo,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use module_evm::ExitError;
use module_support::AddressMapping;
use orml_traits::DataFeeder;
use primitives::{Balance, PREDEPLOY_ADDRESS_START};
use sp_core::{H160, U256};
use sp_runtime::FixedPointNumber;
use std::str::FromStr;

pub struct DummyPrecompile;
impl Precompile for DummyPrecompile {
	fn execute(
		_input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		Ok((ExitSucceed::Stopped, vec![], 0))
	}
}

pub type WithSystemContractFilter = AllPrecompiles<
	crate::SystemContractsFilter,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
>;

#[test]
fn precompile_filter_works_on_acala_precompiles() {
	let precompile = H160::from_low_u64_be(PRECOMPILE_ADDRESS_START);

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: precompile,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert_eq!(
		WithSystemContractFilter::execute(precompile, &[0u8; 1], None, &non_system_caller_context),
		Some(Err(ExitError::Other("no permission".into()))),
	);
}

#[test]
fn precompile_filter_does_not_work_on_system_contracts() {
	let system = H160::from_low_u64_be(PREDEPLOY_ADDRESS_START);

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: system,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(
		WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
	);
}

#[test]
fn precompile_filter_does_not_work_on_non_system_contracts() {
	let mut non_system = [0u8; 20];
	non_system[0] = 1;
	let mut another_non_system = [0u8; 20];
	another_non_system[0] = 2;

	let non_system_caller_context = Context {
		address: non_system.into(),
		caller: another_non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(
		WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
	);
}

#[test]
fn multicurrency_precompile_should_work() {
	new_test_ext().execute_with(|| {
		let mut context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: Default::default(),
		};

		// call with not exists erc20
		context.caller = erc20_address_not_exists();
		let mut input = [0u8; 68];
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QuerySymbol).to_be_bytes());
		assert_noop!(
			MultiCurrencyPrecompile::execute(&input, None, &context),
			ExitError::Other("invalid currency id".into())
		);

		// 1.QueryName
		let mut input = [0u8; 36];
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryName).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[27..32].copy_from_slice(&b"Acala"[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[9..32].copy_from_slice(&b"LP Acala - Acala Dollar"[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// 2.QuerySymbol
		let mut input = [0u8; 36];
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QuerySymbol).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[29..32].copy_from_slice(&b"ACA"[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[21..32].copy_from_slice(&b"LP_ACA_AUSD"[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// 3.QueryDecimals
		let mut input = [0u8; 36];
		// action
		input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryDecimals).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[31] = 12;
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[31] = 12;
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// 4.QueryTotalIssuance
		let mut input = [0u8; 36];
		// action
		input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryTotalIssuance).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[16..32].copy_from_slice(&(INITIAL_BALANCE * 2).to_be_bytes()[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let expected_output = [0u8; 32];
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// 5.QueryBalance
		let mut input = [0u8; 4 + 2 * 32];
		// action
		input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryBalance).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[16..32].copy_from_slice(&INITIAL_BALANCE.to_be_bytes()[..]);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let expected_output = [0u8; 32];
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// 6.Transfer
		let mut input = [0u8; 4 + 4 * 32];
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(multicurrency::Action::Transfer).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// to
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// amount
		U256::from(1).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		let from_balance = Balances::free_balance(alice());
		let to_balance = Balances::free_balance(bob());

		// Token
		context.caller = aca_evm_address();
		let (reason, output, used_gas) = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		let expected_output: Vec<u8> = vec![];
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
		assert_eq!(Balances::free_balance(alice()), from_balance - 1);
		assert_eq!(Balances::free_balance(bob()), to_balance + 1);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		assert_noop!(
			MultiCurrencyPrecompile::execute(&input, None, &context),
			ExitError::Other("BalanceTooLow".into())
		);
	});
}

#[test]
fn oracle_precompile_should_work() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		let price = Price::from(30_000);

		// action + currency_id
		let mut input = [0u8; 68];
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(oracle::Action::GetPrice).to_be_bytes());
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);

		// no price yet
		let (reason, output, used_gas) = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, [0u8; 32]);
		assert_eq!(used_gas, 0);

		assert_ok!(Oracle::feed_value(ALICE, RENBTC, price));
		assert_eq!(
			Oracle::get_no_op(&RENBTC),
			Some(orml_oracle::TimestampedValue {
				value: price,
				timestamp: 1
			})
		);

		// returned price + timestamp
		let mut expected_output = [0u8; 32];
		U256::from(price.into_inner()).to_big_endian(&mut expected_output[..]);

		let (reason, output, used_gas) = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn oracle_precompile_should_handle_invalid_input() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			OraclePrecompile::execute(
				&[0u8; 0],
				None,
				&Context {
					address: Default::default(),
					caller: alice_evm_addr(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("invalid input".into())
		);

		assert_noop!(
			OraclePrecompile::execute(
				&[0u8; 32],
				None,
				&Context {
					address: Default::default(),
					caller: alice_evm_addr(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("invalid input".into())
		);

		assert_noop!(
			OraclePrecompile::execute(
				&[1u8; 64],
				None,
				&Context {
					address: Default::default(),
					caller: alice_evm_addr(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("invalid action".into())
		);
	});
}

#[test]
fn schedule_call_precompile_should_work() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		let mut input = [0u8; 12 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Schedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// target
		U256::from(aca_evm_address().as_bytes()).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);
		// skip offset
		// input_len
		U256::from(4 + 32 + 32).to_big_endian(&mut input[4 + 8 * 32..4 + 9 * 32]);

		// input_data
		let mut transfer_to_bob = [0u8; 68];
		// transfer bytes4(keccak256(signature)) 0xa9059cbb
		transfer_to_bob[0..4].copy_from_slice(&hex!("a9059cbb"));
		// to address
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut transfer_to_bob[4..36]);
		// amount
		U256::from(1000).to_big_endian(&mut transfer_to_bob[36..68]);

		U256::from(&transfer_to_bob[0..32]).to_big_endian(&mut input[4 + 9 * 32..4 + 10 * 32]);
		U256::from(&transfer_to_bob[32..64]).to_big_endian(&mut input[4 + 10 * 32..4 + 11 * 32]);
		input[4 + 11 * 32..4 + 11 * 32 + 4].copy_from_slice(&transfer_to_bob[64..68]);

		let (reason, output, used_gas) = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(used_gas, 0);
		let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Scheduled(3, 0));
		assert!(System::events().iter().any(|record| record.event == event));

		// cancel schedule
		let task_id = get_task_id(output);
		let mut cancel_input = [0u8; 6 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		cancel_input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(schedule_call::Action::Cancel).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut cancel_input[4 + 1 * 32..4 + 2 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut cancel_input[4 + 3 * 32..4 + 4 * 32]);
		// task_id
		cancel_input[4 + 4 * 32..4 + 4 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		let (reason, _output, used_gas) = ScheduleCallPrecompile::execute(&cancel_input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(used_gas, 0);
		let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Canceled(3, 0));
		assert!(System::events().iter().any(|record| record.event == event));

		let (reason, output, used_gas) = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(used_gas, 0);

		run_to_block(2);

		// reschedule call
		let task_id = get_task_id(output);
		let mut reschedule_input = [0u8; 7 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		reschedule_input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(schedule_call::Action::Reschedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut reschedule_input[4 + 1 * 32..4 + 2 * 32]);
		// min_delay
		U256::from(2).to_big_endian(&mut reschedule_input[4 + 2 * 32..4 + 3 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut reschedule_input[4 + 4 * 32..4 + 5 * 32]);
		// task_id
		reschedule_input[4 + 5 * 32..4 + 5 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		let (reason, _output, used_gas) = ScheduleCallPrecompile::execute(&reschedule_input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(used_gas, 0);
		let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Scheduled(5, 0));
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
			assert_eq!(Balances::free_balance(from_account.clone()), 999999894290);
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
fn schedule_call_precompile_should_handle_invalid_input() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		let mut input = [0u8; 10 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Schedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// target
		U256::from(aca_evm_address().as_bytes()).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);
		// skip offset
		// input_len
		U256::from(1).to_big_endian(&mut input[4 + 8 * 32..4 + 9 * 32]);

		// input_data = 0x12
		input[4 + 9 * 32] = hex!("12")[0];

		let (reason, output, used_gas) = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(used_gas, 0);

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

		// cancel schedule
		let task_id = get_task_id(output);
		let mut cancel_input = [0u8; 7 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		cancel_input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(schedule_call::Action::Cancel).to_be_bytes());
		// from
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut cancel_input[4 + 1 * 32..4 + 2 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut cancel_input[4 + 3 * 32..4 + 4 * 32]);
		// task_id
		cancel_input[4 + 4 * 32..4 + 4 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		assert_eq!(
			ScheduleCallPrecompile::execute(&cancel_input, None, &context),
			Err(ExitError::Other("NoPermission".into()))
		);

		run_to_block(4);
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999898614);
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
fn dex_precompile_get_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + currency_id_a + currency_id_b
		let mut input = [0u8; 4 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(dex::Action::GetLiquidityPool).to_be_bytes());
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);

		let mut expected_output = [0u8; 64];
		U256::from(1_000).to_big_endian(&mut expected_output[..32]);
		U256::from(1_000_000).to_big_endian(&mut expected_output[32..64]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn dex_precompile_get_liquidity_token_address_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + currency_id_a + currency_id_b
		let mut input = [0u8; 5 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32]
			.copy_from_slice(&Into::<u32>::into(dex::Action::GetLiquidityTokenAddress).to_be_bytes());
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);

		let mut expected_output = [0u8; 32];
		let address = H160::from_str("0x0000000000000000000000010000000100000004").unwrap();
		U256::from(address.as_bytes()).to_big_endian(&mut expected_output[..32]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		// unkonwn token
		let mut id = [0u8; 32];
		id[31] = u8::MAX; // not exists
		U256::from_big_endian(&id.to_vec()).to_big_endian(&mut input[2 * 32..3 * 32]);
		assert_noop!(
			DexPrecompile::execute(&input, None, &context),
			ExitError::Other("invalid currency id".into())
		);
	});
}

#[test]
fn dex_precompile_get_swap_target_amount_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + path_len + currency_id_a + currency_id_b +
		// supply_amount
		let mut input = [0u8; 7 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(dex::Action::GetSwapTargetAmount).to_be_bytes());
		// skip offset
		// supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(989).to_big_endian(&mut expected_output[..32]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn dex_precompile_get_swap_supply_amount_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + path_len + currency_id_a + currency_id_b +
		// target_amount
		let mut input = [0u8; 7 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(dex::Action::GetSwapSupplyAmount).to_be_bytes());
		// skip offset
		// target_amount
		U256::from(1).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(1).to_big_endian(&mut expected_output[..32]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn dex_precompile_swap_with_exact_supply_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + who + path_len + currency_id_a + currency_id_b +
		// supply_amount + min_target_amount
		let mut input = [0u8; 9 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(dex::Action::SwapWithExactSupply).to_be_bytes());
		// who
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// skip offset
		// supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// min_target_amount
		U256::from(0).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 7 * 32..4 + 8 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(989).to_big_endian(&mut expected_output[..32]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn dex_precompile_swap_with_exact_target_should_work() {
	new_test_ext().execute_with(|| {
		// enable RENBTC/AUSD
		assert_ok!(DexModule::enable_trading_pair(Origin::signed(ALICE), RENBTC, AUSD,));

		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			RENBTC,
			AUSD,
			1_000,
			1_000_000,
			0,
			true
		));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// array_size + action + who + path_len + currency_id_a + currency_id_b +
		// target_amount + max_supply_amount
		let mut input = [0u8; 9 * 32];
		// array size
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// action
		input[1 * 32..4 + 1 * 32].copy_from_slice(&Into::<u32>::into(dex::Action::SwapWithExactTarget).to_be_bytes());
		// who
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// skip offset
		// target_amount
		U256::from(1).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// max_supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// RENBTC
		U256::from_big_endian(&renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);
		// AUSD
		U256::from_big_endian(&ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 7 * 32..4 + 8 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(1).to_big_endian(&mut expected_output[..32]);

		let (reason, output, used_gas) = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
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
