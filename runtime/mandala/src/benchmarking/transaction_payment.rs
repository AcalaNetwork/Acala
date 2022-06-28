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

use super::utils::{dollar, set_balance};
use crate::{
	AccountId, Balance, Currencies, CurrencyId, Dex, Event, GetNativeCurrencyId, GetStableCurrencyId,
	NativeTokenExistentialDeposit, Origin, Runtime, System, TransactionPayment, TreasuryPalletId,
};
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::OnFinalize;
use frame_system::RawOrigin;
use module_support::{DEXManager, Ratio, SwapLimit};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::traits::{AccountIdConversion, One, UniqueSaturatedInto};

use sp_std::prelude::*;

const SEED: u32 = 0;

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const NATIVECOIN: CurrencyId = GetNativeCurrencyId::get();

fn assert_last_event(generic_event: Event) {
	System::assert_last_event(generic_event.into());
}

fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	// set balance
	set_balance(currency_id_a, &maker, max_amount_a.unique_saturated_into());
	set_balance(currency_id_b, &maker, max_amount_b.unique_saturated_into());

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_transaction_payment }

	set_alternative_fee_swap_path {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVECOIN, &caller, NativeTokenExistentialDeposit::get());
	}: _(RawOrigin::Signed(caller.clone()), Some(vec![STABLECOIN, NATIVECOIN]))
	verify {
		assert_eq!(TransactionPayment::alternative_fee_swap_path(&caller).unwrap().into_inner(), vec![STABLECOIN, NATIVECOIN]);
	}

	enable_charge_fee_pool {
		let funder: AccountId = account("funder", 0, SEED);
		let treasury_account: AccountId = TreasuryPalletId::get().into_account_truncating();
		let sub_account: AccountId = <Runtime as module_transaction_payment::Config>::PalletId::get().into_sub_account_truncating(STABLECOIN);
		let native_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(NATIVECOIN);
		let stable_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(STABLECOIN);
		let pool_size: Balance = native_ed * 50;
		let swap_threshold: Balance = native_ed * 2;
		let fee_swap_path: Vec<CurrencyId> = vec![STABLECOIN, NATIVECOIN];

		// set balance
		set_balance(NATIVECOIN, &sub_account, NativeTokenExistentialDeposit::get());

		let path = vec![STABLECOIN, NATIVECOIN];
		TransactionPayment::set_alternative_fee_swap_path(Origin::signed(sub_account.clone()), Some(path.clone()))?;
		assert_eq!(TransactionPayment::alternative_fee_swap_path(&sub_account).unwrap().into_inner(), vec![STABLECOIN, NATIVECOIN]);

		inject_liquidity(funder.clone(), STABLECOIN, NATIVECOIN, 1_000 * dollar(STABLECOIN), 10_000 * dollar(NATIVECOIN))?;
		assert!(Dex::get_swap_amount(&path, SwapLimit::ExactTarget(Balance::MAX, native_ed)).is_some());

		set_balance(NATIVECOIN, &treasury_account, pool_size * 10);
		set_balance(STABLECOIN, &treasury_account, stable_ed * 10);
	}: _(RawOrigin::Root, STABLECOIN, fee_swap_path.clone(), pool_size, swap_threshold)
	verify {
		let exchange_rate = TransactionPayment::token_exchange_rate(STABLECOIN).unwrap();
		assert_eq!(TransactionPayment::pool_size(STABLECOIN), pool_size);
		assert!(TransactionPayment::token_exchange_rate(STABLECOIN).is_some());
		assert_eq!(<Currencies as MultiCurrency<AccountId>>::free_balance(STABLECOIN, &sub_account), stable_ed);
		assert_eq!(<Currencies as MultiCurrency<AccountId>>::free_balance(NATIVECOIN, &sub_account), pool_size);
		assert_last_event(module_transaction_payment::Event::ChargeFeePoolEnabled {
			sub_account,
			currency_id: STABLECOIN,
			fee_swap_path,
			exchange_rate,
			pool_size,
			swap_threshold
		}.into());
	}

	disable_charge_fee_pool {
		let treasury_account: AccountId = TreasuryPalletId::get().into_account_truncating();
		let sub_account: AccountId = <Runtime as module_transaction_payment::Config>::PalletId::get().into_sub_account_truncating(STABLECOIN);
		let native_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(NATIVECOIN);
		let stable_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(STABLECOIN);
		let pool_size: Balance = native_ed * 50;

		set_balance(NATIVECOIN, &sub_account, native_ed * 10);
		set_balance(STABLECOIN, &sub_account, stable_ed * 10);

		module_transaction_payment::TokenExchangeRate::<Runtime>::insert(STABLECOIN, Ratio::one());
	}: _(RawOrigin::Root, STABLECOIN)
	verify {
		assert_last_event(module_transaction_payment::Event::ChargeFeePoolDisabled {
			currency_id: STABLECOIN,
			foreign_amount: stable_ed * 10,
			native_amount: native_ed * 10,
		}.into());
		assert_eq!(module_transaction_payment::TokenExchangeRate::<Runtime>::get(STABLECOIN), None);
		assert_eq!(module_transaction_payment::GlobalFeeSwapPath::<Runtime>::get(STABLECOIN), None);
	}

	with_fee_path {
		let caller = whitelisted_caller();
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		let fee_swap_path: Vec<CurrencyId> = vec![STABLECOIN, NATIVECOIN];
	}: _(RawOrigin::Signed(caller), fee_swap_path.clone(), call)

	with_fee_currency {
		let caller: AccountId = whitelisted_caller();
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		module_transaction_payment::TokenExchangeRate::<Runtime>::insert(STABLECOIN, Ratio::one());
	}: _(RawOrigin::Signed(caller.clone()), STABLECOIN, call)

	with_fee_paid_by {
		let caller: AccountId = whitelisted_caller();
		let payer: AccountId = account("payer", 0, SEED);
		let call = Box::new(frame_system::Call::remark { remark: vec![] }.into());
		let signature = sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature([0u8; 64]));
	}: _(RawOrigin::Signed(caller.clone()), call, payer, signature)

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
