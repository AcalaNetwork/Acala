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

use super::utils::set_balance;
use crate::{
	dollar, AccountId, Currencies, GetStakingCurrencyId, Homa, PolkadotBondingDuration, PolkadotBridge, Runtime,
	StakingPool,
};
use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_homa::RedeemStrategy;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn new_era() {
	PolkadotBridge::new_era(Default::default());
	StakingPool::rebalance();
	StakingPool::rebalance();
	StakingPool::rebalance();
}

runtime_benchmarks! {
	{ Runtime, module_homa }

	// inject DOT to staking pool and mint LDOT
	mint {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = GetStakingCurrencyId::get();
		set_balance(currency_id, &caller, 1_000 * dollar(currency_id));
	}: _(RawOrigin::Signed(caller), 1_000 * dollar(currency_id))

	// redeem DOT from free pool
	redeem_immediately {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = GetStakingCurrencyId::get();
		set_balance(currency_id, &caller, 1_000 * dollar(currency_id));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), 1_000 * dollar(currency_id))?;
		for era_index in 0..=PolkadotBondingDuration::get() {
			new_era();
		}
	}: redeem(RawOrigin::Signed(caller.clone()), dollar(currency_id), RedeemStrategy::Immediately)
	verify {
		assert!(<Currencies as MultiCurrency<_>>::total_balance(currency_id, &caller) > 0);
	}

	// redeem DOT by wait for complete unbonding eras
	redeem_wait_for_unbonding {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = GetStakingCurrencyId::get();
		set_balance(currency_id, &caller, 1_000 * dollar(currency_id));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), 1_000 * dollar(currency_id))?;
		new_era();
	}: redeem(RawOrigin::Signed(caller), dollar(currency_id), RedeemStrategy::WaitForUnbonding)

	// redeem DOT by claim unbonding
	redeem_by_claim_unbonding {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = GetStakingCurrencyId::get();
		set_balance(currency_id, &caller, 1_000 * dollar(currency_id));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), 1_000 * dollar(currency_id))?;
		new_era();
		new_era();
	}: redeem(RawOrigin::Signed(caller.clone()), dollar(currency_id), RedeemStrategy::Target(PolkadotBondingDuration::get() + 2))

	withdraw_redemption {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = GetStakingCurrencyId::get();
		set_balance(currency_id, &caller, 1_000 * dollar(currency_id));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), 1_000 * dollar(currency_id))?;
		new_era();
		Homa::redeem(RawOrigin::Signed(caller.clone()).into(), dollar(currency_id), RedeemStrategy::WaitForUnbonding)?;
		for era_index in 0..=PolkadotBondingDuration::get() {
			new_era();
		}
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert!(<Currencies as MultiCurrency<_>>::total_balance(GetStakingCurrencyId::get(), &caller) > 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
