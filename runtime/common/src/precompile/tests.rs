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

#![allow(clippy::erasing_op)]
#![cfg(test)]
use super::*;
use crate::precompile::{
	mock::{
		aca_evm_address, alice, alice_evm_addr, ausd_evm_address, bob, bob_evm_addr, erc20_address_not_exists,
		get_task_id, lp_aca_ausd_evm_address, new_test_ext, renbtc_evm_address, run_to_block, Balances, DexModule,
		EVMModule, Event as TestEvent, ExistentialDeposit, Oracle, Origin, Price, System, Test, ALICE, AUSD,
		INITIAL_BALANCE, RENBTC,
	},
	schedule_call::TaskInfo,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use module_evm::{Context, ExitError, ExitReason, ExitSucceed, Precompile, Runner};
use module_support::AddressMapping;
use orml_traits::DataFeeder;
use primitives::{
	evm::{PRECOMPILE_ADDRESS_START, PREDEPLOY_ADDRESS_START},
	Balance,
};
use sp_core::{bytes::from_hex, H160, U256};
use sp_runtime::FixedPointNumber;
use std::str::FromStr;

pub type WithSystemContractFilter = AllPrecompiles<Test>;
type MultiCurrencyPrecompile = crate::MultiCurrencyPrecompile<Test>;
type OraclePrecompile = crate::OraclePrecompile<Test>;
type DexPrecompile = crate::DexPrecompile<Test>;
type ScheduleCallPrecompile = crate::ScheduleCallPrecompile<Test>;
type StateRentPrecompile = crate::StateRentPrecompile<Test>;

#[test]
fn precompile_filter_works_on_acala_precompiles() {
	let precompile = PRECOMPILE_ADDRESS_START;

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
	let system = PREDEPLOY_ADDRESS_START;

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
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QuerySymbol).to_be_bytes());
		assert_noop!(
			MultiCurrencyPrecompile::execute(&input, None, &context),
			ExitError::Other("invalid currency id".into())
		);

		// 1.QueryName
		let mut input = [0u8; 4];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryName).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 96];
		// skip offset
		expected_output[31] = 32;
		// length
		expected_output[63] = 5;
		expected_output[64..64 + 5].copy_from_slice(&b"Acala"[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 96];
		// skip offset
		expected_output[31] = 32;
		// length
		expected_output[63] = 23;
		expected_output[64..64 + 23].copy_from_slice(&b"LP Acala - Acala Dollar"[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// 2.QuerySymbol
		let mut input = [0u8; 4];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QuerySymbol).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 96];
		// skip offset
		expected_output[31] = 32;
		// length
		expected_output[63] = 3;
		expected_output[64..64 + 3].copy_from_slice(&b"ACA"[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 96];
		// skip offset
		expected_output[31] = 32;
		// length
		expected_output[63] = 11;
		expected_output[64..64 + 11].copy_from_slice(&b"LP_ACA_AUSD"[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// 3.QueryDecimals
		let mut input = [0u8; 4];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryDecimals).to_be_bytes());

		// Token
		context.caller = aca_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[31] = 12;
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[31] = 12;
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// 4.QueryTotalIssuance
		let mut input = [0u8; 4];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryTotalIssuance).to_be_bytes());

		// Token
		context.caller = ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[28..32].copy_from_slice(&1_000_000_000u32.to_be_bytes()[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let expected_output = [0u8; 32];
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// 5.QueryBalance
		let mut input = [0u8; 36];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::QueryBalance).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		// Token
		context.caller = aca_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let mut expected_output = [0u8; 32];
		expected_output[16..32].copy_from_slice(&(INITIAL_BALANCE - ExistentialDeposit::get()).to_be_bytes()[..]);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// DexShare
		context.caller = lp_aca_ausd_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let expected_output = [0u8; 32];
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// 6.Transfer
		let mut input = [0u8; 4 + 3 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(multicurrency::Action::Transfer).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// to
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// amount
		U256::from(1).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		let from_balance = Balances::free_balance(alice());
		let to_balance = Balances::free_balance(bob());

		// Token
		context.caller = aca_evm_address();
		let resp = MultiCurrencyPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		let expected_output: Vec<u8> = vec![];
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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
		let mut input = [0u8; 36];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(oracle::Action::GetPrice).to_be_bytes());
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4..4 + 32]);

		// no price yet
		let resp = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, [0u8; 32]);
		assert_eq!(resp.cost, 0);

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

		let resp = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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
				&[0u8; 3],
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
				&[1u8; 32],
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

		let mut input = [0u8; 11 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Schedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// target
		U256::from(aca_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// skip offset
		// input_len
		U256::from(4 + 32 + 32).to_big_endian(&mut input[4 + 7 * 32..4 + 8 * 32]);

		// input_data
		let mut transfer_to_bob = [0u8; 68];
		// transfer bytes4(keccak256(signature)) 0xa9059cbb
		transfer_to_bob[0..4].copy_from_slice(&hex!("a9059cbb"));
		// to address
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut transfer_to_bob[4..36]);
		// amount
		U256::from(1000).to_big_endian(&mut transfer_to_bob[36..68]);

		U256::from(&transfer_to_bob[0..32]).to_big_endian(&mut input[4 + 8 * 32..4 + 9 * 32]);
		U256::from(&transfer_to_bob[32..64]).to_big_endian(&mut input[4 + 9 * 32..4 + 10 * 32]);
		input[4 + 10 * 32..4 + 10 * 32 + 4].copy_from_slice(&transfer_to_bob[64..68]);

		let resp = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.cost, 0);
		let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Scheduled(3, 0));
		assert!(System::events().iter().any(|record| record.event == event));

		// cancel schedule
		let task_id = get_task_id(resp.output);
		let mut cancel_input = [0u8; 5 * 32];
		// action
		cancel_input[0..4].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Cancel).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut cancel_input[4 + 0 * 32..4 + 1 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut cancel_input[4 + 2 * 32..4 + 3 * 32]);
		// task_id
		cancel_input[4 + 3 * 32..4 + 3 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		let resp = ScheduleCallPrecompile::execute(&cancel_input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.cost, 0);
		let event = TestEvent::Scheduler(pallet_scheduler::Event::<Test>::Canceled(3, 0));
		assert!(System::events().iter().any(|record| record.event == event));

		let resp = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.cost, 0);

		run_to_block(2);

		// reschedule call
		let task_id = get_task_id(resp.output);
		let mut reschedule_input = [0u8; 6 * 32];
		// action
		reschedule_input[0..4].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Reschedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut reschedule_input[4 + 0 * 32..4 + 1 * 32]);
		// min_delay
		U256::from(2).to_big_endian(&mut reschedule_input[4 + 1 * 32..4 + 2 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut reschedule_input[4 + 3 * 32..4 + 4 * 32]);
		// task_id
		reschedule_input[4 + 4 * 32..4 + 4 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		let resp = ScheduleCallPrecompile::execute(&reschedule_input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.cost, 0);
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
			assert_eq!(Balances::free_balance(from_account.clone()), 999999972553);
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
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Schedule).to_be_bytes());
		// from
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// target
		U256::from(aca_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// skip offset
		// input_len
		U256::from(1).to_big_endian(&mut input[4 + 7 * 32..4 + 8 * 32]);

		// input_data = 0x12
		input[4 + 9 * 32] = hex!("12")[0];

		let resp = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.cost, 0);

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
		let task_id = get_task_id(resp.output);
		let mut cancel_input = [0u8; 6 * 32];
		// action
		cancel_input[0..4].copy_from_slice(&Into::<u32>::into(schedule_call::Action::Cancel).to_be_bytes());
		// from
		U256::from(bob_evm_addr().as_bytes()).to_big_endian(&mut cancel_input[4 + 0 * 32..4 + 1 * 32]);
		// skip offset
		// task_id_len
		U256::from(task_id.len()).to_big_endian(&mut cancel_input[4 + 2 * 32..4 + 3 * 32]);
		// task_id
		cancel_input[4 + 3 * 32..4 + 3 * 32 + task_id.len()].copy_from_slice(&task_id[..]);

		assert_eq!(
			ScheduleCallPrecompile::execute(&cancel_input, None, &context),
			Err(ExitError::Other("NoPermission".into()))
		);

		run_to_block(4);
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999978926);
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

		// action + currency_id_a + currency_id_b
		let mut input = [0u8; 3 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::GetLiquidityPool).to_be_bytes());
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);

		let mut expected_output = [0u8; 64];
		U256::from(1_000).to_big_endian(&mut expected_output[..32]);
		U256::from(1_000_000).to_big_endian(&mut expected_output[32..64]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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

		// action + currency_id_a + currency_id_b
		let mut input = [0u8; 4 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::GetLiquidityTokenAddress).to_be_bytes());
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);

		let mut expected_output = [0u8; 32];
		let address = H160::from_str("0x0000000000000000000200000000010000000014").unwrap();
		U256::from(address.as_bytes()).to_big_endian(&mut expected_output[..32]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);

		// unkonwn token
		let mut id = [0u8; 32];
		id[21] = 1; // token type
		id[31] = u8::MAX; // not exists
		U256::from_big_endian(&id.to_vec()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
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

		// action + path_len + currency_id_a + currency_id_b +
		// supply_amount
		let mut input = [0u8; 6 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::GetSwapTargetAmount).to_be_bytes());
		// skip offset
		// supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(989).to_big_endian(&mut expected_output[..32]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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

		// action + path_len + currency_id_a + currency_id_b +
		// target_amount
		let mut input = [0u8; 6 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::GetSwapSupplyAmount).to_be_bytes());
		// skip offset
		// target_amount
		U256::from(1).to_big_endian(&mut input[4 + 1 * 32..4 + 2 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(1).to_big_endian(&mut expected_output[..32]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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

		// action + who + path_len + currency_id_a + currency_id_b +
		// supply_amount + min_target_amount
		let mut input = [0u8; 8 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::SwapWithExactSupply).to_be_bytes());
		// who
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// skip offset
		// supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// min_target_amount
		U256::from(0).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(989).to_big_endian(&mut expected_output[..32]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
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

		// action + who + path_len + currency_id_a + currency_id_b +
		// target_amount + max_supply_amount
		let mut input = [0u8; 8 * 32];
		// action
		input[0..4].copy_from_slice(&Into::<u32>::into(dex::Action::SwapWithExactTarget).to_be_bytes());
		// who
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		// skip offset
		// target_amount
		U256::from(1).to_big_endian(&mut input[4 + 2 * 32..4 + 3 * 32]);
		// max_supply_amount
		U256::from(1).to_big_endian(&mut input[4 + 3 * 32..4 + 4 * 32]);
		// path_len
		U256::from(2).to_big_endian(&mut input[4 + 4 * 32..4 + 5 * 32]);
		// RENBTC
		U256::from_big_endian(renbtc_evm_address().as_bytes()).to_big_endian(&mut input[4 + 5 * 32..4 + 6 * 32]);
		// AUSD
		U256::from_big_endian(ausd_evm_address().as_bytes()).to_big_endian(&mut input[4 + 6 * 32..4 + 7 * 32]);

		let mut expected_output = [0u8; 32];
		U256::from(1).to_big_endian(&mut expected_output[..32]);

		let resp = DexPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(resp.exit_status, ExitSucceed::Returned);
		assert_eq!(resp.output, expected_output);
		assert_eq!(resp.cost, 0);
	});
}

#[test]
fn developer_status_precompile_works() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// action + who
		let mut input = [0u8; 36];

		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::QueryDeveloperStatus).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		// expect output is false as alice has not put a deposit down
		let expected_output = [0u8; 32];
		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);
		assert_eq!(res.output, expected_output);

		// enable account for developer mode
		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::EnableDeveloperAccount).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);

		// query developer status again but this time it is enabled
		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::QueryDeveloperStatus).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		// expect output is now true as alice now is enabled for developer mode
		let expected_output: [u8; 32] = U256::from(true as u8).into();
		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);
		assert_eq!(res.output, expected_output);

		// disable alice account for developer mode
		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::DisableDeveloperAccount).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);

		// query developer status
		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::QueryDeveloperStatus).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);

		// expect output is now false as alice now is disabled again for developer mode
		let expected_output = [0u8; 32];
		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);
		assert_eq!(res.output, expected_output);
	});
}

#[test]
fn publish_contract_precompile_works() {
	new_test_ext().execute_with(|| {
		// pragma solidity ^0.5.0;
		//
		// contract Test {
		//	 function multiply(uint a, uint b) public pure returns(uint) {
		// 	 	return a * b;
		// 	 }
		// }
		let contract = from_hex(
			"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
		).unwrap();

		// create contract
		let info = <Test as module_evm::Config>::Runner::create(alice_evm_addr(), contract.clone(), 0, 21_000_000, 21_000_000, <Test as module_evm::Config>::config()).unwrap();
		let contract_address = info.value;

		// multiply(2, 3)
		let multiply = from_hex(
			"0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003"
		).unwrap();

		// call method `multiply` will fail, not published yet.
		// The error is shown in the last event.
		// The call extrinsic still succeeds, the evm emits a executed failed event
		assert_ok!(EVMModule::call(
			Origin::signed(bob()),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
		));
		System::assert_last_event(TestEvent::EVMModule(module_evm::Event::ExecutedFailed {
			from: bob_evm_addr(),
			contract: contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(module_evm::Error::<Test>::NoPermission).into())),
			output: vec![],
			logs: vec![],
		}));

		let context = Context {
			address: Default::default(),
			caller: alice_evm_addr(),
			apparent_value: Default::default(),
		};

		// action + who + contract_address
		let mut input = [0u8; 4 * 32];

		input[0..4].copy_from_slice(&Into::<u32>::into(state_rent::Action::PublishContract).to_be_bytes());
		U256::from(alice_evm_addr().as_bytes()).to_big_endian(&mut input[4 + 0 * 32..4 + 1 * 32]);
		U256::from(contract_address.as_bytes()).to_big_endian(&mut input[4 + 1 * 32..4 + 2 *32]);

		// publish contract with precompile
		let res = StateRentPrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(res.exit_status, ExitSucceed::Returned);

		// Same call as above now works as contract is now published
		assert_ok!(EVMModule::call(
			Origin::signed(bob()),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
		));
		System::assert_last_event(TestEvent::EVMModule(module_evm::Event::Executed {
			from: bob_evm_addr(),
			contract: contract_address,
			logs: vec![],
		}));
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
