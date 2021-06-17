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
	dollar, AccountId, AccumulatePeriod, CollateralCurrencyIds, Currencies, CurrencyId, GetNativeCurrencyId,
	GetStableCurrencyId, Incentives, Rate, Rewards, Runtime, System, TokenSymbol, KAR, KSM, KUSD, LKSM,
};

use super::utils::set_balance;
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use module_incentives::PoolId;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use primitives::DexShare;
use sp_std::prelude::*;

const SEED: u32 = 0;
const BTC_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::RENBTC), DexShare::Token(TokenSymbol::KUSD));

runtime_benchmarks! {
	{ Runtime, module_incentives }

	on_initialize {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let block_number = AccumulatePeriod::get();

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let pool_id = PoolId::LoansIncentive(currency_id);

			Incentives::update_incentive_rewards(RawOrigin::Root.into(), vec![(pool_id.clone(), 100 * dollar(KAR))])?;
			orml_rewards::Pools::<Runtime>::mutate(pool_id, |pool_info| {
				pool_info.total_shares += 100;
			});
		}

		Incentives::on_initialize(1);
		System::set_block_number(block_number);
	}: {
		Incentives::on_initialize(System::block_number());
	}

	deposit_dex_share {
		let caller: AccountId = whitelisted_caller();
		set_balance(BTC_AUSD_LP, &caller, 10_000 * dollar(KUSD));
	}: _(RawOrigin::Signed(caller), BTC_AUSD_LP, 10_000 * dollar(KUSD))

	withdraw_dex_share {
		let caller: AccountId = whitelisted_caller();
		set_balance(BTC_AUSD_LP, &caller, 10_000 * dollar(KUSD));
		Incentives::deposit_dex_share(
			RawOrigin::Signed(caller.clone()).into(),
			BTC_AUSD_LP,
			10_000 * dollar(KUSD)
		)?;
	}: _(RawOrigin::Signed(caller), BTC_AUSD_LP, 8000 * dollar(KUSD))

	claim_rewards {
		let caller: AccountId = whitelisted_caller();
		let pool_id = PoolId::LoansIncentive(KSM);
		let native_currency_id = GetNativeCurrencyId::get();

		Rewards::add_share(&caller, &pool_id, 100);
		Currencies::deposit(native_currency_id, &<Runtime as module_incentives::Config>::RewardsVaultAccountId::get(), 80 * dollar(native_currency_id))?;
		Rewards::accumulate_reward(&pool_id, 80 * dollar(native_currency_id));
	}: _(RawOrigin::Signed(caller), pool_id)

	update_incentive_rewards {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			values.push((PoolId::LoansIncentive(currency_id), 100 * dollar(KAR)));
		}
	}: _(RawOrigin::Root, values)

	update_dex_saving_rewards {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let caller: AccountId = account("caller", 0, SEED);
		let mut values = vec![];
		let base_currency_id = GetStableCurrencyId::get();

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let lp_share_currency_id = match (currency_id, base_currency_id) {
				(CurrencyId::Token(other_currency_symbol), CurrencyId::Token(base_currency_symbol)) => {
					CurrencyId::DexShare(DexShare::Token(other_currency_symbol), DexShare::Token(base_currency_symbol))
				}
				_ => return Err("invalid currency id"),
			};
			values.push((PoolId::DexSaving(lp_share_currency_id), Rate::default()));
		}
	}: _(RawOrigin::Root, values)

	add_allowance {
		let caller: AccountId = whitelisted_caller();
		set_balance(LKSM, &caller, 10_000 * dollar(KUSD));
		let pool_id = PoolId::HomaValidatorAllowance(caller.clone());
	}: _(RawOrigin::Signed(caller), pool_id, 1 * dollar(LKSM))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
