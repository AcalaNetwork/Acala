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

use super::utils::{dollar, inject_liquidity, set_balance, LIQUID, NATIVE, STABLECOIN, STAKING};
use crate::{
	AccountId, AssetRegistry, Balance, Currencies, CurrencyId, Dex, NativeTokenExistentialDeposit, Runtime,
	RuntimeEvent, RuntimeOrigin, StableAsset, System, TransactionPayment, TreasuryPalletId,
};
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::{assert_ok, traits::OnFinalize};
use frame_system::RawOrigin;
use module_support::{AggregatedSwapPath, DEXManager, Ratio, SwapLimit};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use primitives::currency::AssetMetadata;
use sp_runtime::traits::{AccountIdConversion, One};
use sp_std::prelude::*;

const SEED: u32 = 0;

fn assert_has_event(generic_event: RuntimeEvent) {
	System::assert_has_event(generic_event.into());
}

fn enable_fee_pool() -> (AccountId, Balance, Balance, Balance) {
	let funder: AccountId = account("funder", 0, SEED);
	let treasury_account: AccountId = TreasuryPalletId::get().into_account_truncating();
	let sub_account: AccountId =
		<Runtime as module_transaction_payment::Config>::PalletId::get().into_sub_account_truncating(STABLECOIN);
	let native_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(NATIVE);
	let stable_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(STABLECOIN);
	let pool_size: Balance = native_ed * 50;
	let swap_threshold: Balance = native_ed * 2;

	inject_liquidity(
		funder.clone(),
		STABLECOIN,
		NATIVE,
		1_000 * dollar(STABLECOIN),
		10_000 * dollar(NATIVE),
		false,
	)
	.unwrap();
	assert!(Dex::get_swap_amount(
		&vec![STABLECOIN, NATIVE],
		SwapLimit::ExactTarget(Balance::MAX, native_ed)
	)
	.is_some());
	assert_eq!(
		Dex::get_liquidity_pool(STABLECOIN, NATIVE),
		(1_000 * dollar(STABLECOIN), 10_000 * dollar(NATIVE))
	);

	set_balance(NATIVE, &treasury_account, pool_size * 10);
	set_balance(STABLECOIN, &treasury_account, stable_ed * 10);
	(sub_account, stable_ed, pool_size, swap_threshold)
}

fn enable_stable_asset() {
	let funder: AccountId = account("funder", 0, SEED);

	set_balance(STAKING, &funder, 1000 * dollar(STAKING));
	set_balance(LIQUID, &funder, 1000 * dollar(LIQUID));
	set_balance(NATIVE, &funder, 1000 * dollar(NATIVE));

	// create stable asset pool
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	assert_ok!(StableAsset::create_pool(
		RuntimeOrigin::root(),
		pool_asset,
		vec![STAKING, LIQUID],
		vec![1u128, 1u128],
		10_000_000u128,
		20_000_000u128,
		50_000_000u128,
		1_000u128,
		funder.clone(),
		funder.clone(),
		1_000_000_000_000u128,
	));
	let asset_metadata = AssetMetadata {
		name: b"Token Name".to_vec(),
		symbol: b"TN".to_vec(),
		decimals: 12,
		minimal_balance: 1,
	};
	assert_ok!(AssetRegistry::register_stable_asset(
		RawOrigin::Root.into(),
		Box::new(asset_metadata.clone())
	));
	assert_ok!(StableAsset::mint(
		RuntimeOrigin::signed(funder.clone()),
		0,
		vec![100 * dollar(STAKING), 100 * dollar(LIQUID)],
		0u128
	));

	inject_liquidity(
		funder.clone(),
		LIQUID,
		NATIVE,
		100 * dollar(LIQUID),
		100 * dollar(NATIVE),
		false,
	)
	.unwrap();
}

runtime_benchmarks! {
	{ Runtime, module_transaction_payment }

	set_alternative_fee_swap_path {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVE, &caller, 2 * NativeTokenExistentialDeposit::get());
	}: _(RawOrigin::Signed(caller.clone()), Some(vec![STABLECOIN, NATIVE]))
	verify {
		assert_eq!(TransactionPayment::alternative_fee_swap_path(&caller).unwrap().into_inner(), vec![STABLECOIN, NATIVE]);
	}

	enable_charge_fee_pool {
		let (sub_account, stable_ed, pool_size, swap_threshold) = enable_fee_pool();
	}: _(RawOrigin::Root, STABLECOIN, pool_size, swap_threshold)
	verify {
		let exchange_rate = TransactionPayment::token_exchange_rate(STABLECOIN).unwrap();
		assert_eq!(TransactionPayment::pool_size(STABLECOIN), pool_size);
		assert!(TransactionPayment::token_exchange_rate(STABLECOIN).is_some());
		assert_eq!(<Currencies as MultiCurrency<AccountId>>::free_balance(STABLECOIN, &sub_account), stable_ed);
		assert_eq!(<Currencies as MultiCurrency<AccountId>>::free_balance(NATIVE, &sub_account), pool_size);
		assert_has_event(module_transaction_payment::Event::ChargeFeePoolEnabled {
			sub_account,
			currency_id: STABLECOIN,
			exchange_rate,
			pool_size,
			swap_threshold
		}.into());
	}

	disable_charge_fee_pool {
		let treasury_account: AccountId = TreasuryPalletId::get().into_account_truncating();
		let sub_account: AccountId = <Runtime as module_transaction_payment::Config>::PalletId::get().into_sub_account_truncating(STABLECOIN);
		let native_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(NATIVE);
		let stable_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(STABLECOIN);
		let pool_size: Balance = native_ed * 50;

		set_balance(NATIVE, &sub_account, native_ed * 10);
		set_balance(STABLECOIN, &sub_account, stable_ed * 10);

		module_transaction_payment::TokenExchangeRate::<Runtime>::insert(STABLECOIN, Ratio::one());
	}: _(RawOrigin::Root, STABLECOIN)
	verify {
		assert_has_event(module_transaction_payment::Event::ChargeFeePoolDisabled {
			currency_id: STABLECOIN,
			foreign_amount: stable_ed * 10,
			native_amount: native_ed * 10,
		}.into());
		assert_eq!(module_transaction_payment::TokenExchangeRate::<Runtime>::get(STABLECOIN), None);
		assert_eq!(module_transaction_payment::GlobalFeeSwapPath::<Runtime>::get(STABLECOIN), None);
	}

	with_fee_path {
		System::set_block_number(1);

		let funder: AccountId = account("funder", 0, SEED);
		inject_liquidity(funder.clone(), STABLECOIN, NATIVE, 100 * dollar(STABLECOIN), 100 * dollar(NATIVE), false)?;

		let caller: AccountId = whitelisted_caller();
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		set_balance(STABLECOIN, &caller, 100 * dollar(STABLECOIN));
		set_balance(NATIVE, &caller, 100 * dollar(NATIVE));

		let fee_swap_path: Vec<CurrencyId> = vec![STABLECOIN, NATIVE];
	}: _(RawOrigin::Signed(caller), fee_swap_path.clone(), call)

	with_fee_currency {
		System::set_block_number(1);

		let caller: AccountId = whitelisted_caller();
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		set_balance(STABLECOIN, &caller, 100 * dollar(STABLECOIN));
		set_balance(NATIVE, &caller, 100 * dollar(NATIVE));

		let (sub_account, stable_ed, pool_size, swap_threshold) = enable_fee_pool();
		TransactionPayment::enable_charge_fee_pool(RawOrigin::Root.into(), STABLECOIN, pool_size, swap_threshold).unwrap();

		let exchange_rate = TransactionPayment::token_exchange_rate(STABLECOIN).unwrap();
		assert_has_event(module_transaction_payment::Event::ChargeFeePoolEnabled {
			sub_account,
			currency_id: STABLECOIN,
			exchange_rate,
			pool_size,
			swap_threshold
		}.into());
	}: _(RawOrigin::Signed(caller.clone()), STABLECOIN, call)

	with_fee_aggregated_path {
		System::set_block_number(1);

		let caller: AccountId = whitelisted_caller();
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		set_balance(STAKING, &caller, 100 * dollar(STAKING));
		set_balance(NATIVE, &caller, 100 * dollar(NATIVE));

		enable_stable_asset();

		// Taiga(STAKING, LIQUID), Dex(LIQUID, NATIVE)
		let fee_aggregated_path = vec![
			AggregatedSwapPath::<CurrencyId>::Taiga(0, 0, 1),
			AggregatedSwapPath::<CurrencyId>::Dex(vec![LIQUID, NATIVE]),
		];
	}: _(RawOrigin::Signed(caller.clone()), fee_aggregated_path, call)

	on_finalize {
	}: {
		TransactionPayment::on_finalize(System::block_number());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
