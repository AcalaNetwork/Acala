//! Tests to make sure that Acala's weights and fees match what we
//! expect from Substrate or ORML.
//!
//! These test are not meant to be exhaustive, as it is inevitable that
//! weights in Substrate will change. Instead they are supposed to provide
//! some sort of indicator that calls we consider important (e.g Balances::transfer)
//! have not suddenly changed from under us.

use frame_support::{
	traits::ContainsLengthBound,
	weights::{constants::*, GetDispatchInfo, Weight},
};

use acala_runtime::{self, CurrencyId, MaximumBlockWeight, Runtime};

use frame_system::Call as SystemCall;
use orml_auction::Call as AuctionCall;
use orml_currencies::Call as CurrenciesCall;
use orml_oracle::Call as OracleCall;
use orml_vesting::Call as VestingCall;
use pallet_indices::Call as IndicesCall;
use pallet_recovery::Call as RecoveryCall;
use pallet_session::Call as SessionCall;
use pallet_timestamp::Call as TimestampCall;
use pallet_treasury::Call as TreasuryCall;

type DbWeight = <Runtime as frame_system::Trait>::DbWeight;

#[test]
fn sanity_check_weight_per_time_constants_are_as_expected() {
	// These values comes from Substrate, we want to make sure that if it
	// ever changes we don't accidently break Polkadot
	assert_eq!(WEIGHT_PER_SECOND, 1_000_000_000_000);
	assert_eq!(WEIGHT_PER_MILLIS, WEIGHT_PER_SECOND / 1000);
	assert_eq!(WEIGHT_PER_MICROS, WEIGHT_PER_MILLIS / 1000);
	assert_eq!(WEIGHT_PER_NANOS, WEIGHT_PER_MICROS / 1000);
}

/// orml_currencies call
#[test]
fn weight_of_currencies_transfer_is_correct() {
	// #[weight = 30 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(2, 2)]
	let expected_weight = 30 * WEIGHT_PER_MICROS + 2 * DbWeight::get().read + 2 * DbWeight::get().write;

	let weight = CurrenciesCall::transfer::<Runtime>(Default::default(), CurrencyId::ACA, Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_currencies_transfer_native_currency_is_correct() {
	// #[weight = 30 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(2, 2)]
	let expected_weight = 30 * WEIGHT_PER_MICROS + 2 * DbWeight::get().read + 2 * DbWeight::get().write;

	let weight = CurrenciesCall::transfer_native_currency::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_currencies_update_balance_is_correct() {
	// #[weight = 27 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(1, 1)]
	let expected_weight = 27 * WEIGHT_PER_MICROS + 1 * DbWeight::get().read + 1 * DbWeight::get().write;

	let weight = CurrenciesCall::update_balance::<Runtime>(Default::default(), CurrencyId::ACA, Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

/// orml_vesting call
#[test]
fn weight_of_vesting_claim_is_correct() {
	// #[weight = 30 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(2, 2)]
	let expected_weight = 30 * WEIGHT_PER_MICROS + 2 * DbWeight::get().read + 2 * DbWeight::get().write;

	let weight = VestingCall::claim::<Runtime>().get_dispatch_info().weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_vesting_add_vesting_schedule_is_correct() {
	// #[weight = 48 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(4, 4)]
	let expected_weight = 48 * WEIGHT_PER_MICROS + 4 * DbWeight::get().read + 4 * DbWeight::get().write;

	let weight = VestingCall::add_vesting_schedule::<Runtime>(
		Default::default(),
		orml_vesting::VestingSchedule {
			start: Default::default(),
			period: Default::default(),
			period_count: Default::default(),
			per_period: Default::default(),
		},
	)
	.get_dispatch_info()
	.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_vesting_update_vesting_schedules_is_correct() {
	// #[weight = 28 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(4, 4)]
	let expected_weight = 28 * WEIGHT_PER_MICROS + 4 * DbWeight::get().read + 4 * DbWeight::get().write;

	let weight = VestingCall::update_vesting_schedules::<Runtime>(Default::default(), vec![])
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

/// orml_auction call
#[test]
fn weight_of_auction_bid_is_correct() {
	// #[weight = 84 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(9, 9)]
	let expected_weight = 84 * WEIGHT_PER_MICROS + 9 * DbWeight::get().read + 9 * DbWeight::get().write;

	let weight = AuctionCall::bid::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

/// orml_oracle call
#[test]
fn weight_of_oracle_feed_values_is_correct() {
	// #[weight = FunctionOf(0, DispatchClass::Operational, Pays::No)]
	let expected_weight = 0;

	let weight = OracleCall::feed_values::<Runtime>(vec![], Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_oracle_set_session_key_is_correct() {
	// #[weight = 10_000_000]
	let expected_weight = 10_000_000;

	let weight = OracleCall::set_session_key::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

/// frame_system call
#[test]
fn weight_of_system_set_code_is_correct() {
	// #[weight = (T::MaximumBlockWeight::get(), DispatchClass::Operational)]
	let expected_weight = MaximumBlockWeight::get();
	let weight = SystemCall::set_code::<Runtime>(vec![]).get_dispatch_info().weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_system_set_storage_is_correct() {
	let storage_items = vec![(vec![12], vec![34]), (vec![45], vec![83])];
	let len = storage_items.len() as Weight;

	// #[weight = FunctionOf(
	// 	|(items,): (&Vec<KeyValue>,)| {
	// 		T::DbWeight::get().writes(items.len() as Weight)
	// 			.saturating_add((items.len() as Weight).saturating_mul(600_000))
	// 	},
	// 	DispatchClass::Operational,
	// 	Pays::Yes,
	// )]
	let expected_weight = (DbWeight::get().write * len).saturating_add(len.saturating_mul(600_000));
	let weight = SystemCall::set_storage::<Runtime>(storage_items)
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_system_remark_is_correct() {
	// #[weight = 700_000]
	let expected_weight = 700_000;
	let weight = SystemCall::remark::<Runtime>(vec![]).get_dispatch_info().weight;
	assert_eq!(weight, expected_weight);
}

/// pallet_timestamp call
#[test]
fn weight_of_timestamp_set_is_correct() {
	// #[weight = T::DbWeight::get().reads_writes(2, 1) + 8_000_000]
	let expected_weight = 8_000_000 + 2 * DbWeight::get().read + DbWeight::get().write;
	let weight = TimestampCall::set::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

// pallet_indices call
#[test]
fn weight_of_indices_claim_is_correct() {
	// #[weight = T::DbWeight::get().reads_writes(1, 1) + 30 * WEIGHT_PER_MICROS]
	let expected_weight = 30 * WEIGHT_PER_MICROS + DbWeight::get().read + DbWeight::get().write;
	let weight = IndicesCall::claim::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_indices_transfer_is_correct() {
	// #[weight = T::DbWeight::get().reads_writes(2, 2) + 35 * WEIGHT_PER_MICROS]
	let expected_weight = 35 * WEIGHT_PER_MICROS + 2 * DbWeight::get().read + 2 * DbWeight::get().write;
	let weight = IndicesCall::transfer::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_indices_free_is_correct() {
	// #[weight = T::DbWeight::get().reads_writes(1, 1) + 25 * WEIGHT_PER_MICROS]
	let expected_weight = 25 * WEIGHT_PER_MICROS + DbWeight::get().read + DbWeight::get().write;
	let weight = IndicesCall::free::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_indices_force_transfer_is_correct() {
	// #[weight = T::DbWeight::get().reads_writes(2, 2) + 25 * WEIGHT_PER_MICROS]
	let expected_weight = 25 * WEIGHT_PER_MICROS + 2 * DbWeight::get().read + 2 * DbWeight::get().write;
	let weight = IndicesCall::force_transfer::<Runtime>(Default::default(), Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

// pallet_treasury call
#[test]
fn weight_of_treasury_propose_spend_is_correct() {
	// #[weight = 120_000_000 + T::DbWeight::get().reads_writes(1, 2)]
	let expected_weight = 120_000_000 + DbWeight::get().read + 2 * DbWeight::get().write;
	let weight = TreasuryCall::propose_spend::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;

	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_treasury_approve_proposal_is_correct() {
	// #[weight = (34_000_000 + T::DbWeight::get().reads_writes(2, 1), DispatchClass::Operational)]
	let expected_weight = 34_000_000 + 2 * DbWeight::get().read + DbWeight::get().write;
	let weight = TreasuryCall::approve_proposal::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;

	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_treasury_tip_is_correct() {
	let max_len: Weight = <Runtime as pallet_treasury::Trait>::Tippers::max_len() as Weight;

	// #[weight = 68_000_000 + 2_000_000 * T::Tippers::max_len() as Weight
	// 	+ T::DbWeight::get().reads_writes(2, 1)]
	let expected_weight = 68_000_000 + 2_000_000 * max_len + 2 * DbWeight::get().read + DbWeight::get().write;
	let weight = TreasuryCall::tip::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;

	assert_eq!(weight, expected_weight);
}

// pallet_session call
#[test]
fn weight_of_session_set_keys_is_correct() {
	// #[weight = 200_000_000
	// 	+ T::DbWeight::get().reads(2 + T::Keys::key_ids().len() as Weight)
	// 	+ T::DbWeight::get().writes(1 + T::Keys::key_ids().len() as Weight)]
	//
	// Acala has 2 possible session keys, so we default to key_ids.len() = 2
	let expected_weight = 200_000_000 + (DbWeight::get().read * (2 + 2)) + (DbWeight::get().write * (1 + 2));
	let weight = SessionCall::set_keys::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;

	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_session_purge_keys_is_correct() {
	// #[weight = 120_000_000
	// 	+ T::DbWeight::get().reads_writes(2, 1 + T::Keys::key_ids().len() as Weight)]
	//
	// Acala has 2 possible session keys, so we default to key_ids.len() = 2
	let expected_weight = 120_000_000 + (DbWeight::get().read * 2) + (DbWeight::get().write * (1 + 2));
	let weight = SessionCall::purge_keys::<Runtime>().get_dispatch_info().weight;

	assert_eq!(weight, expected_weight);
}

// pallet_recovery call
#[test]
fn weight_of_recovery_set_recovered_is_correct() {
	// #[weight = 0]
	let expected_weight = 0;
	let weight = RecoveryCall::set_recovered::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_create_recovery_is_correct() {
	// #[weight = 100_000_000]
	let expected_weight = 100_000_000;
	let weight = RecoveryCall::create_recovery::<Runtime>(vec![], Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_initiate_recovery_is_correct() {
	// #[weight = 100_000_000]
	let expected_weight = 100_000_000;
	let weight = RecoveryCall::initiate_recovery::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_vouch_recovery_is_correct() {
	// #[weight = 100_000_000]
	let expected_weight = 100_000_000;
	let weight = RecoveryCall::vouch_recovery::<Runtime>(Default::default(), Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_claim_recovery_is_correct() {
	// #[weight = 100_000_000]
	let expected_weight = 100_000_000;
	let weight = RecoveryCall::claim_recovery::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_close_recovery_is_correct() {
	// #[weight = 30_000_000]
	let expected_weight = 30_000_000;
	let weight = RecoveryCall::close_recovery::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_remove_recovery_is_correct() {
	// #[weight = 30_000_000]
	let expected_weight = 30_000_000;
	let weight = RecoveryCall::remove_recovery::<Runtime>().get_dispatch_info().weight;
	assert_eq!(weight, expected_weight);
}

#[test]
fn weight_of_recovery_cancel_recovered_is_correct() {
	// #[weight = 0]
	let expected_weight = 0;
	let weight = RecoveryCall::cancel_recovered::<Runtime>(Default::default())
		.get_dispatch_info()
		.weight;
	assert_eq!(weight, expected_weight);
}
