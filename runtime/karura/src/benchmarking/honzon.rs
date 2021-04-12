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

use crate::{
	dollar, AcalaOracle, AccountId, Amount, CdpEngine, CollateralCurrencyIds, CurrencyId, Honzon, Indices, Price, Rate,
	Ratio, Runtime, KSM, KUSD,
};

use super::utils::set_balance;
use core::convert::TryInto;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::Change;
use sp_runtime::{
	traits::{StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_honzon }

	_ {}

	authorize {
		let caller: AccountId = account("caller", 0, SEED);
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);
	}: _(RawOrigin::Signed(caller), KSM, to_lookup)

	unauthorize {
		let caller: AccountId = account("caller", 0, SEED);
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);
		Honzon::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			KSM,
			to_lookup.clone()
		)?;
	}: _(RawOrigin::Signed(caller), KSM, to_lookup)

	unauthorize_all {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;

		let caller: AccountId = account("caller", 0, SEED);
		let currency_ids = CollateralCurrencyIds::get();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);

		for i in 0 .. c {
			Honzon::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				currency_ids[i as usize],
				to_lookup.clone(),
			)?;
		}
	}: _(RawOrigin::Signed(caller))

	// `adjust_loan`, best case:
	// adjust both collateral and debit
	adjust_loan {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let collateral_price = Price::one();		// 1 USD
		let debit_value = 100 * dollar(KUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(KUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &caller, collateral_amount);

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, collateral_amount.try_into().unwrap(), debit_amount)

	transfer_loan_from {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let sender: AccountId = account("sender", 0, SEED);
		let sender_lookup = Indices::unlookup(sender.clone());
		let receiver: AccountId = account("receiver", 0, SEED);
		let receiver_lookup = Indices::unlookup(receiver.clone());


		let debit_value = 100 * dollar(KUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(KUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &sender, collateral_amount);

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;

		// initialize sender's loan
		Honzon::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount,
		)?;

		// authorize receiver
		Honzon::authorize(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			receiver_lookup,
		)?;
	}: _(RawOrigin::Signed(receiver), currency_id, sender_lookup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_authorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_authorize());
		});
	}

	#[test]
	fn test_unauthorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize());
		});
	}

	#[test]
	fn test_unauthorize_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize_all());
		});
	}

	#[test]
	fn test_adjust_loan() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_adjust_loan());
		});
	}
}
