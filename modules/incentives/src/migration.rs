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

use super::*;
use crate::log;
use frame_support::traits::OnRuntimeUpgrade;
use sp_std::marker::PhantomData;

/// Clear all DexSavingRewardRates storage
pub struct ClearDexSavingRewardRates<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for ClearDexSavingRewardRates<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "incentives",
			"ClearDexSavingRewardRates::on_runtime_upgrade execute, will clear Storage DexSavingRewardRates",
		);

		// clear storage DexSavingRewardRates,
		let _ = DexSavingRewardRates::<T>::clear(u32::max_value(), None);

		0
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		assert_eq!(DexSavingRewardRates::<T>::iter().count(), 0);

		log::info!(
			target: "incentives",
			"ClearDexSavingRewardRates done!",
		);

		Ok(())
	}
}

type WithdrawnRewards = BTreeMap<CurrencyId, Balance>;

/// Clear all PendingMultiRewards for specific Pool
pub struct ClearPendingMultiRewards<T, GetPoolId>(PhantomData<T>, PhantomData<GetPoolId>);
impl<T: Config, GetPoolId: Get<PoolId>> OnRuntimeUpgrade for ClearPendingMultiRewards<T, GetPoolId> {
	fn on_runtime_upgrade() -> Weight {
		let pool_id = GetPoolId::get();
		log::info!(
			target: "incentives",
			"ClearPendingMultiRewards::on_runtime_upgrade execute, will clear Storage PendingMultiRewards for Pool {:?}",
			pool_id,
		);

		// clear all PendingMultiRewards for specific pool
		let _ = PendingMultiRewards::<T>::clear_prefix(pool_id, u32::max_value(), None);

		0
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		let pool_id = GetPoolId::get();
		assert_eq!(PendingMultiRewards::<T>::iter_prefix(pool_id).count(), 0);

		log::info!(
			target: "incentives",
			"ClearPendingMultiRewards for Pool {:?} done!",
			pool_id,
		);

		Ok(())
	}
}

/// Reset rewards record for storage rewards.SharesAndWithdrawnRewards and rewards.PoolInfos at
/// specific PoolId
pub struct ResetRewardsRecord<T, GetPoolId>(PhantomData<T>, PhantomData<GetPoolId>);
impl<T: Config, GetPoolId: Get<PoolId>> OnRuntimeUpgrade for ResetRewardsRecord<T, GetPoolId> {
	fn on_runtime_upgrade() -> Weight {
		let pool_id = GetPoolId::get();
		log::info!(
			target: "rewards",
			"ResetRewardsRecord::on_runtime_upgrade execute, will reset Storage SharesAndWithdrawnRewards and PoolInfos for Pool {:?}",
			pool_id
		);

		let mut total_share: Balance = Default::default();

		// reset SharesAndWithdrawnRewards
		for (who, (share, _)) in orml_rewards::SharesAndWithdrawnRewards::<T>::iter_prefix(&pool_id) {
			orml_rewards::SharesAndWithdrawnRewards::<T>::mutate(&pool_id, &who, |(_, withdrawn_rewards)| {
				*withdrawn_rewards = WithdrawnRewards::new();
			});

			total_share = total_share.saturating_add(share);
		}

		// reset PoolInfos
		let pool_info = orml_rewards::PoolInfo::<Balance, Balance, CurrencyId> {
			total_shares: total_share,
			..Default::default()
		};
		orml_rewards::PoolInfos::<T>::insert(&pool_id, pool_info);

		0
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		let pool_id = GetPoolId::get();
		let mut total_share = Balance::default();

		for (_, (share, withdrawn_rewards)) in orml_rewards::SharesAndWithdrawnRewards::<T>::iter_prefix(&pool_id) {
			assert_eq!(withdrawn_rewards, WithdrawnRewards::new());
			total_share = total_share.saturating_add(share);
		}

		assert_eq!(
			orml_rewards::PoolInfos::<T>::get(&pool_id),
			orml_rewards::PoolInfo::<Balance, Balance, CurrencyId> {
				total_shares: total_share,
				..Default::default()
			}
		);

		log::info!(
			target: "rewards",
			"ResetRewardsRecord for Pool {:?} done!",
			pool_id,
		);

		Ok(())
	}
}
