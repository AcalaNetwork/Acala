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

use crate::{
	AccountId, ActiveSubAccountsIndexList, Balance, Currencies, GetLiquidCurrencyId, GetStakingCurrencyId, Homa, Rate,
	Runtime,
};

use super::utils::set_balance;
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use module_homa::UnlockChunk;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_homa }

	on_initialize {
	}: {
		Homa::on_initialize(1)
	}

	on_initialize_with_bump_era {
		let minter: AccountId = account("minter", 0, SEED);
		let redeemer: AccountId = account("redeemer", 0, SEED);
		let sub_account_index = ActiveSubAccountsIndexList::get().first().unwrap().clone();

		set_balance(GetStakingCurrencyId::get(), &minter, 1_000_000_000_000_000);
		set_balance(GetLiquidCurrencyId::get(), &redeemer, 1_000_000_000_000_000 * 10);
		Homa::reset_ledgers(
			RawOrigin::Root.into(),
			vec![(sub_account_index, Some(1_000_000_000_000_000), Some(vec![UnlockChunk{value: 1_000_000_000_000, era: 10}]))]
		)?;
		Homa::reset_current_era(RawOrigin::Root.into(), 9)?;
		Homa::update_homa_params(
			RawOrigin::Root.into(),
			Some(10_000_000_000_000_000),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(Rate::saturating_from_rational(20, 100)),
			None,
		)?;
		Homa::update_bump_era_params(RawOrigin::Root.into(), None, Some(1))?;

		Homa::mint(RawOrigin::Signed(minter).into(), 100_000_000_000_000)?;
		Homa::request_redeem(RawOrigin::Signed(redeemer).into(), 5_000_000_000_000_000, true)?;
	}: {
		Homa::on_initialize(1)
	}

	mint {
		let caller: AccountId = whitelisted_caller();
		let amount = 10_000_000_000_000;

		Homa::update_homa_params(
			RawOrigin::Root.into(),
			Some(amount * 10),
			Some(Rate::saturating_from_rational(1, 10000)),
			None,
			None,
		)?;
		set_balance(GetStakingCurrencyId::get(), &caller, amount * 2);
	}: _(RawOrigin::Signed(caller), amount)

	request_redeem {
		let caller: AccountId = whitelisted_caller();
		let amount = 10_000_000_000_000;

		set_balance(GetLiquidCurrencyId::get(), &caller, amount * 2);
	}: _(RawOrigin::Signed(caller), amount, true)

	fast_match_redeems {
		let n in 1 .. 50;
		let caller: AccountId = whitelisted_caller();
		let minter: AccountId = account("minter", 0, SEED);
		let mint_amount = 1_000_000_000_000_000;

		set_balance(GetStakingCurrencyId::get(), &minter, mint_amount * 2);
		Homa::update_homa_params(
			RawOrigin::Root.into(),
			Some(mint_amount * 10),
			Some(Rate::saturating_from_rational(1, 10000)),
			None,
			None,
		)?;
		Homa::mint(RawOrigin::Signed(minter.clone()).into(), mint_amount)?;

		let mut redeem_request_list: Vec<AccountId> = vec![];
		let redeem_amount = 10_000_000_000_000;
		for i in 0 .. n {
			let redeemer = account("redeemer", i, SEED);
			<Currencies as MultiCurrency<_>>::transfer(GetLiquidCurrencyId::get(), &minter, &redeemer, redeem_amount * 2)?;
			Homa::request_redeem(RawOrigin::Signed(redeemer.clone()).into(), redeem_amount, true)?;
			redeem_request_list.push(redeemer);
		}
	}: _(RawOrigin::Signed(caller), redeem_request_list)

	claim_redemption {
		let caller: AccountId = whitelisted_caller();
		let redeemer: AccountId = account("redeemer", 0, SEED);
		let redeption_amount = 1_000_000_000_000;

		module_homa::Unbondings::<Runtime>::insert(&redeemer, 1, redeption_amount);
		set_balance(GetStakingCurrencyId::get(), &Homa::account_id(), redeption_amount);
		module_homa::UnclaimedRedemption::<Runtime>::put(redeption_amount);
		Homa::reset_current_era(RawOrigin::Root.into(), 1)?;
	}: _(RawOrigin::Signed(caller), redeemer)

	update_homa_params {}: _(
		RawOrigin::Root,
		Some(1_000_000_000_000),
		Some(Rate::saturating_from_rational(1, 100)),
		Some(Rate::saturating_from_rational(1, 100)),
		Some(Rate::saturating_from_rational(1, 100)))

	update_bump_era_params {}: _(RawOrigin::Root, Some(3000), Some(7200))

	reset_ledgers {
		let n in 0 .. 10;
		let mut updates: Vec<(u16, Option<Balance>, Option<Vec<UnlockChunk>>)> = vec![];
		for i in 0..n {
			updates.push((i.try_into().unwrap(), Some(1), Some(vec![UnlockChunk{value: 1, era: 1}])))
		}
	}: _(RawOrigin::Root, updates)

	reset_current_era {}: _(RawOrigin::Root, 1)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
