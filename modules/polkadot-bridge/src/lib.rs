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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::BasicCurrency;
use primitives::{Balance, EraIndex};
use sp_runtime::{
	traits::{CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member, StaticLookup, Zero},
	DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::{fmt::Debug, prelude::*};
use support::{
	OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState, PolkadotBridgeType, PolkadotStakingLedger,
	PolkadotUnlockChunk, Rate,
};

pub use module::*;

/// The params related to rebalance per era
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
pub struct SubAccountStatus {
	/// Bonded amount
	pub bonded: Balance,
	/// Free amount
	pub available: Balance,
	/// Unbonding list
	pub unbonding: Vec<(EraIndex, Balance)>,
	pub mock_reward_rate: Rate,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DOTCurrency: BasicCurrency<Self::AccountId, Balance = Balance>;
		type OnNewEra: OnNewEra<EraIndex>;
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;
		#[pallet::constant]
		type EraLength: Get<Self::BlockNumber>;
		type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
	}

	#[pallet::error]
	pub enum Error<T> {
		NotEnough,
		Overflow,
	}

	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn era_start_block_number)]
	pub type EraStartBlockNumber<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn forced_era)]
	pub type ForcedEra<T: Config> = StorageValue<_, T::BlockNumber, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn sub_accounts)]
	pub type SubAccounts<T: Config> = StorageMap<_, Twox64Concat, u32, SubAccountStatus, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_finalize(now: T::BlockNumber) {
			let force_era = Self::forced_era().map_or(false, |block| {
				if block == now {
					<ForcedEra<T>>::kill();
					true
				} else {
					false
				}
			});
			let len = now.checked_sub(&Self::era_start_block_number()).unwrap_or_default();

			if len >= T::EraLength::get() || force_era {
				Self::new_era(now);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn set_mock_reward_rate(
			origin: OriginFor<T>,
			account_index: u32,
			reward_rate: Rate,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			SubAccounts::<T>::mutate(account_index, |status| {
				status.mock_reward_rate = reward_rate;
			});
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_bond_extra(
			origin: OriginFor<T>,
			account_index: u32,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::sub_account_bond_extra(account_index, amount)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_unbond(
			origin: OriginFor<T>,
			account_index: u32,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::sub_account_unbond(account_index, amount)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_rebond(
			origin: OriginFor<T>,
			account_index: u32,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Self::sub_account_rebond(account_index, amount)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_withdraw_unbonded(origin: OriginFor<T>, account_index: u32) -> DispatchResultWithPostInfo {
			// ignore because we don't care who send the message
			let _ = ensure_signed(origin)?;
			Self::sub_account_withdraw_unbonded(account_index);
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_payout_nominator(origin: OriginFor<T>, account_index: u32) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Self::payout_nominator(account_index);
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		fn simulate_transfer_to_sub_account(
			origin: OriginFor<T>,
			account_index: u32,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::transfer_to_sub_account(account_index, &who, amount)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simualte_receive_from_sub_account(
			origin: OriginFor<T>,
			account_index: u32,
			to: <T::Lookup as StaticLookup>::Source,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let to = T::Lookup::lookup(to)?;
			Self::receive_from_sub_account(account_index, &to, amount)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn simulate_slash_sub_account(
			origin: OriginFor<T>,
			account_index: u32,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			SubAccounts::<T>::mutate(account_index, |status| {
				status.bonded = status.bonded.saturating_sub(amount);
			});
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn force_era(origin: OriginFor<T>, at: T::BlockNumber) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			if at > <frame_system::Module<T>>::block_number() {
				ForcedEra::<T>::put(at);
			}
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn new_era(now: T::BlockNumber) {
		let new_era = CurrentEra::<T>::mutate(|era| {
			*era += 1;
			*era
		});
		EraStartBlockNumber::<T>::put(now);
		T::OnNewEra::on_new_era(new_era);
	}

	/// simulate bond extra by sub account
	fn sub_account_bond_extra(account_index: u32, amount: Balance) -> DispatchResult {
		if !amount.is_zero() {
			SubAccounts::<T>::try_mutate(account_index, |status| -> DispatchResult {
				status.available = status.available.checked_sub(amount).ok_or(Error::<T>::NotEnough)?;
				status.bonded = status.bonded.checked_add(amount).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
		}

		Ok(())
	}

	/// simulate unbond by sub account
	fn sub_account_unbond(account_index: u32, amount: Balance) -> DispatchResult {
		if !amount.is_zero() {
			SubAccounts::<T>::try_mutate(account_index, |status| -> DispatchResult {
				status.bonded = status.bonded.checked_sub(amount).ok_or(Error::<T>::NotEnough)?;
				let current_era = Self::current_era();
				let unbonded_era_index = current_era + T::BondingDuration::get();
				status.unbonding.push((unbonded_era_index, amount));
				debug::debug!(
					target: "polkadot bridge simulator",
					"sub account {:?} unbond: {:?} at {:?}",
					account_index, amount, current_era,
				);

				Ok(())
			})?;
		}

		Ok(())
	}

	/// simulate rebond by sub account
	fn sub_account_rebond(account_index: u32, amount: Balance) -> DispatchResult {
		SubAccounts::<T>::try_mutate(account_index, |status| -> DispatchResult {
			let mut unbonding = status.unbonding.clone();
			let mut bonded = status.bonded;
			let mut rebond_balance: Balance = Zero::zero();

			while let Some(last) = unbonding.last_mut() {
				if rebond_balance + last.1 <= amount {
					rebond_balance += last.1;
					bonded += last.1;
					unbonding.pop();
				} else {
					let diff = amount - rebond_balance;

					rebond_balance += diff;
					bonded += diff;
					last.1 -= diff;
				}

				if rebond_balance >= amount {
					break;
				}
			}
			ensure!(rebond_balance >= amount, Error::<T>::NotEnough);
			if !rebond_balance.is_zero() {
				status.bonded = bonded;
				status.unbonding = unbonding;

				debug::debug!(
					target: "polkadot bridge simulator",
					"sub account {:?} rebond: {:?}",
					account_index, rebond_balance,
				);
			}

			Ok(())
		})
	}

	/// simulate withdraw unbonded by sub account
	fn sub_account_withdraw_unbonded(account_index: u32) {
		SubAccounts::<T>::mutate(account_index, |status| {
			let current_era = Self::current_era();
			let mut available = status.available;
			let unbonding = status
				.unbonding
				.clone()
				.into_iter()
				.filter(|(era_index, value)| {
					if *era_index > current_era {
						true
					} else {
						available = available.saturating_add(*value);
						false
					}
				})
				.collect::<Vec<_>>();

			status.available = available;
			status.unbonding = unbonding;
		});
	}

	/// simulate receive staking reward by sub account
	fn sub_account_payout_nominator(account_index: u32) {
		SubAccounts::<T>::mutate(account_index, |status| {
			let reward = status.mock_reward_rate.saturating_mul_int(status.bonded);
			status.bonded = status.bonded.saturating_add(reward);

			debug::debug!(
				target: "polkadot bridge simulator",
				"sub account {:?} get reward: {:?}",
				account_index, reward,
			);
		});
	}

	/// simulate nominate by sub account
	fn sub_account_nominate(_account_index: u32, _targets: Vec<T::PolkadotAccountId>) {}

	/// simulate transfer dot from acala to parachain sub account in
	/// polkadot
	fn transfer_to_sub_account(account_index: u32, from: &T::AccountId, amount: Balance) -> DispatchResult {
		T::DOTCurrency::withdraw(from, amount)?;
		SubAccounts::<T>::mutate(account_index, |status| {
			status.available = status.available.saturating_add(amount);
		});
		Ok(())
	}

	/// simulate receive dot from parachain sub account in polkadot to acala
	fn receive_from_sub_account(account_index: u32, to: &T::AccountId, amount: Balance) -> DispatchResult {
		SubAccounts::<T>::try_mutate(account_index, |status| -> DispatchResult {
			status.available = status.available.checked_sub(amount).ok_or(Error::<T>::NotEnough)?;
			T::DOTCurrency::deposit(&to, amount)
		})
	}
}

impl<T: Config> PolkadotBridgeType<T::BlockNumber, EraIndex> for Pallet<T> {
	type BondingDuration = T::BondingDuration;
	type EraLength = T::EraLength;
	type PolkadotAccountId = T::PolkadotAccountId;
}

impl<T: Config> PolkadotBridgeCall<T::AccountId, T::BlockNumber, Balance, EraIndex> for Pallet<T> {
	fn bond_extra(account_index: u32, amount: Balance) -> DispatchResult {
		Self::sub_account_bond_extra(account_index, amount)
	}

	fn unbond(account_index: u32, amount: Balance) -> DispatchResult {
		Self::sub_account_unbond(account_index, amount)
	}

	fn rebond(account_index: u32, amount: Balance) -> DispatchResult {
		Self::sub_account_rebond(account_index, amount)
	}

	fn withdraw_unbonded(account_index: u32) {
		Self::sub_account_withdraw_unbonded(account_index)
	}

	fn payout_nominator(account_index: u32) {
		Self::sub_account_payout_nominator(account_index)
	}

	fn nominate(account_index: u32, targets: Vec<Self::PolkadotAccountId>) {
		Self::sub_account_nominate(account_index, targets)
	}

	fn transfer_to_bridge(account_index: u32, from: &T::AccountId, amount: Balance) -> DispatchResult {
		Self::transfer_to_sub_account(account_index, from, amount)
	}

	fn receive_from_bridge(account_index: u32, to: &T::AccountId, amount: Balance) -> DispatchResult {
		Self::receive_from_sub_account(account_index, to, amount)
	}
}

impl<T: Config> PolkadotBridgeState<Balance, EraIndex> for Pallet<T> {
	fn staking_ledger(account_index: u32) -> PolkadotStakingLedger<Balance, EraIndex> {
		let status = Self::sub_accounts(account_index);
		let active = status.bonded;
		let mut total = active;
		let unlocking = status
			.unbonding
			.into_iter()
			.map(|(era_index, balance)| {
				total = total.saturating_add(balance);
				PolkadotUnlockChunk {
					value: balance,
					era: era_index,
				}
			})
			.collect::<_>();

		PolkadotStakingLedger {
			total,
			active,
			unlocking,
		}
	}

	/// bonded + available + total_unlocking
	fn balance(account_index: u32) -> Balance {
		let status = Self::sub_accounts(account_index);

		status
			.unbonding
			.iter()
			.fold(status.bonded.saturating_add(status.available), |x, (_, balance)| {
				x.saturating_add(*balance)
			})
	}

	fn current_era() -> EraIndex {
		Self::current_era()
	}
}

impl<T: Config> PolkadotBridge<T::AccountId, T::BlockNumber, Balance, EraIndex> for Pallet<T> {}
