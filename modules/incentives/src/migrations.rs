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

//! Migrations for the incentives module.

use super::*;

pub fn migrate_to_multi_currency_reward<T: Config>() -> Weight {
	let get_reward_currency = |pool: &PoolId<T::RelaychainAccountId>| match pool {
		PoolId::LoansIncentive(_) | PoolId::DexIncentive(_) | PoolId::HomaIncentive => T::NativeCurrencyId::get(),
		PoolId::DexSaving(_) => T::StableCurrencyId::get(),
		PoolId::HomaValidatorAllowance(_) => T::LiquidCurrencyId::get(),
	};

	let orml_used_weight =
		orml_rewards::migrations::migrate_to_multi_currency_reward::<T>(Box::new(get_reward_currency));

	let mut reads_writes = 0;

	PendingRewards::<T>::translate::<Balance, _>(|pool, _who, reward| {
		reads_writes += 1;
		if reward.is_zero() {
			return None;
		}
		Some(vec![(get_reward_currency(&pool), reward)].into_iter().collect())
	});

	// Return the weight consumed by the migration.
	T::DbWeight::get().reads_writes(reads_writes, reads_writes) + orml_used_weight
}

#[test]
fn migrate_to_multi_currency_reward_works() {
	use crate::mock::*;
	ExtBuilder::default().build().execute_with(|| {
		(500 as Balance).using_encoded(|data| {
			let key = PendingRewards::<Runtime>::hashed_key_for(&PoolId::DexSaving(BTC), &ALICE::get());
			sp_io::storage::set(&key[..], data);
		});

		let weight = migrate_to_multi_currency_reward::<Runtime>();
		assert_eq!(weight, 125_000_000);

		assert_eq!(
			PendingRewards::<Runtime>::get(&PoolId::DexSaving(BTC), &ALICE::get()),
			vec![(AUSD, 500)].into_iter().collect(),
		);
	});
}
