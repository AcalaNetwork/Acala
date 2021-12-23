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

use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::{Change, Happened, MultiCurrency};
use primitives::{Balance, CurrencyId, EraIndex};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, Saturating, Zero},
	ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;
use support::{
	ExchangeRate, HomaProtocol, NomineesProvider, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState,
	PolkadotBridgeType, PolkadotStakingLedger, PolkadotUnlockChunk, Rate, Ratio,
};

mod mock;
mod tests;

pub use module::*;

/// The configurable params of staking pool.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Params {
	/// The target max ratio of the free pool to the total communal DOT.
	pub target_max_free_unbonded_ratio: Ratio,
	/// The target min ratio of the free pool to the total communal DOT.
	pub target_min_free_unbonded_ratio: Ratio,
	/// The target ratio of the unbonding_to_free to the total communal DOT.
	pub target_unbonding_to_free_ratio: Ratio,
	/// The target rate to unbond communal DOT to free pool per era.
	pub unbonding_to_free_adjustment: Rate,
	/// The base rate fee for redemption.
	/// It's only worked for strategy `Immediately` and `Target`.
	pub base_fee_rate: Rate,
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum Phase {
	/// Rebalance process started.
	Started,
	/// Relaychain has already `withdraw_unbonded` and `payout_stakers`.
	RelaychainUpdated,
	/// Transfer available assets from relaychain to parachain and update ledger
	/// of staking_pool.
	LedgerUpdated,
	/// Rebalance process finished.
	Finished,
}

impl Default for Phase {
	fn default() -> Self {
		Self::Finished
	}
}

/// The ledger of staking pool.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default, TypeInfo)]
pub struct Ledger {
	/// The amount of total bonded.
	pub bonded: Balance,
	/// The amount of total unbonding to free pool.
	pub unbonding_to_free: Balance,
	/// The amount of free pool.
	pub free_pool: Balance,
	/// The amount to unbond when next era beginning.
	pub to_unbond_next_era: (Balance, Balance),
	//TODO: add `debit` to record total debit caused by slahsing on relaychain.
}

impl Ledger {
	/// Total staking currency amount of staking pool.
	fn total(&self) -> Balance {
		self.bonded
			.saturating_add(self.unbonding_to_free)
			.saturating_add(self.free_pool)
	}

	/// Total amount of staking currency which is belong to liquid currency
	/// holders.
	fn total_belong_to_liquid_holders(&self) -> Balance {
		let (_, claimed_to_unbond) = self.to_unbond_next_era;
		self.total().saturating_sub(claimed_to_unbond)
	}

	/// Bonded amount which is belong to liquid currency holders.
	fn bonded_belong_to_liquid_holders(&self) -> Balance {
		let (_, claimed_to_unbond) = self.to_unbond_next_era;
		self.bonded.saturating_sub(claimed_to_unbond)
	}

	/// The ratio of `free_pool` in `total_belong_to_liquid_holders`.
	fn free_pool_ratio(&self) -> Ratio {
		Ratio::checked_from_rational(self.free_pool, self.total_belong_to_liquid_holders()).unwrap_or_default()
	}

	/// The ratio of `unbonding_to_free` in
	/// `total_belong_to_liquid_holders`.
	fn unbonding_to_free_ratio(&self) -> Ratio {
		Ratio::checked_from_rational(self.unbonding_to_free, self.total_belong_to_liquid_holders()).unwrap_or_default()
	}
}

/// Fee rate calculater.
pub trait FeeModel<Balance> {
	fn get_fee(
		remain_available_percent: Ratio,
		available_amount: Balance,
		request_amount: Balance,
		base_rate: Rate,
	) -> Option<Balance>;
}

type ChangeRate = Change<Rate>;
type ChangeRatio = Change<Ratio>;

type PolkadotAccountIdOf<T> = <<T as Config>::Bridge as PolkadotBridgeType<
	<T as frame_system::Config>::BlockNumber,
	EraIndex,
>>::PolkadotAccountId;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The staking currency id(should be DOT in acala)
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The liquid currency id(should be LDOT in acala)
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// The default exchange rate for liquid currency to staking currency.
		#[pallet::constant]
		type DefaultExchangeRate: Get<ExchangeRate>;

		/// The staking pool's module id, keep all staking currency belong to
		/// Homa protocol.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The sub account indexs of parachain to vault assets of Homa protocol
		/// in Polkadot.
		#[pallet::constant]
		type PoolAccountIndexes: Get<Vec<u32>>;

		/// The origin which may update parameters. Root can always do this.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// Calculation model for unbond fees
		type FeeModel: FeeModel<Balance>;

		/// The nominees selected by governance of Homa protocol.
		type Nominees: NomineesProvider<PolkadotAccountIdOf<Self>>;

		/// The Bridge to do accross-chain operations between parachain and
		/// relaychain.
		type Bridge: PolkadotBridge<Self::AccountId, Self::BlockNumber, Balance, EraIndex>;

		/// The currency for managing assets related to Homa protocol.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The era index is invalid.
		InvalidEra,
		/// Failed to calculate redemption fee.
		GetFeeFailed,
		/// Invalid config.
		InvalidConfig,
		/// Rebalance process is unfinished.
		RebalanceUnfinished,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Deposit staking currency(DOT) to staking pool and issue liquid currency(LDOT).
		MintLiquid {
			who: T::AccountId,
			staking_amount_deposited: Balance,
			liquid_amount_issued: Balance,
		},
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by
		/// waiting for complete unbond eras.
		RedeemByUnbond {
			who: T::AccountId,
			liquid_amount_burned: Balance,
			staking_amount_redeemed: Balance,
		},
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by free
		/// pool immediately.
		RedeemByFreeUnbonded {
			who: T::AccountId,
			fee_in_staking: Balance,
			liquid_amount_burned: Balance,
			staking_amount_redeemed: Balance,
		},
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by claim
		/// the unbonding_to_free of specific era.
		RedeemByClaimUnbonding {
			who: T::AccountId,
			target_era: EraIndex,
			fee_in_staking: Balance,
			liquid_amount_burned: Balance,
			staking_amount_redeemed: Balance,
		},
	}

	/// Current era index on Relaychain.
	///
	/// CurrentEra: EraIndex
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	/// Unbond on next era beginning by AccountId.
	/// AccountId => Unbond
	///
	/// NextEraUnbonds: AccountId => Balance
	#[pallet::storage]
	#[pallet::getter(fn next_era_unbonds)]
	pub type NextEraUnbonds<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Balance, ValueQuery>;

	/// The records of unbonding.
	/// ExpiredEraIndex => (TotalUnbounding, ClaimedUnbonding,
	/// InitialClaimedUnbonding)
	///
	/// Unbonding: map EraIndex => (Balance, Balance, Balance)
	#[pallet::storage]
	#[pallet::getter(fn unbonding)]
	pub type Unbonding<T: Config> = StorageMap<_, Twox64Concat, EraIndex, (Balance, Balance, Balance), ValueQuery>;

	/// The records of unbonding by AccountId.
	/// AccountId, ExpiredEraIndex => Unbounding
	///
	/// Unbondings: double_map AccountId, EraIndex => Balance
	#[pallet::storage]
	#[pallet::getter(fn unbondings)]
	pub type Unbondings<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, EraIndex, Balance, ValueQuery>;

	/// The ledger of staking pool.
	///
	/// StakingPoolLedger: Ledger
	#[pallet::storage]
	#[pallet::getter(fn staking_pool_ledger)]
	pub type StakingPoolLedger<T: Config> = StorageValue<_, Ledger, ValueQuery>;

	/// The rebalance phase of current era.
	///
	/// RebalancePhase: Phase
	#[pallet::storage]
	#[pallet::getter(fn rebalance_phase)]
	pub type RebalancePhase<T: Config> = StorageValue<_, Phase, ValueQuery>;

	/// The params of staking pool.
	///
	/// StakingPoolParams: Params
	#[pallet::storage]
	#[pallet::getter(fn staking_pool_params)]
	pub type StakingPoolParams<T: Config> = StorageValue<_, Params, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(Default)]
	pub struct GenesisConfig {
		pub staking_pool_params: Params,
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			StakingPoolParams::<T>::put(self.staking_pool_params.clone());
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_: T::BlockNumber) -> Weight {
			Self::rebalance();

			// TODO: return different weight according rebalance phase.
			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Update params related to staking pool
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		#[pallet::weight((10_000, DispatchClass::Operational))]
		#[transactional]
		pub fn set_staking_pool_params(
			origin: OriginFor<T>,
			target_max_free_unbonded_ratio: ChangeRatio,
			target_min_free_unbonded_ratio: ChangeRatio,
			target_unbonding_to_free_ratio: ChangeRatio,
			unbonding_to_free_adjustment: ChangeRate,
			base_fee_rate: ChangeRate,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			StakingPoolParams::<T>::try_mutate(|params| -> DispatchResult {
				if let Change::NewValue(update) = target_max_free_unbonded_ratio {
					params.target_max_free_unbonded_ratio = update;
				}
				if let Change::NewValue(update) = target_min_free_unbonded_ratio {
					params.target_min_free_unbonded_ratio = update;
				}
				if let Change::NewValue(update) = target_unbonding_to_free_ratio {
					params.target_unbonding_to_free_ratio = update;
				}
				if let Change::NewValue(update) = unbonding_to_free_adjustment {
					params.unbonding_to_free_adjustment = update;
				}
				if let Change::NewValue(update) = base_fee_rate {
					params.base_fee_rate = update;
				}

				ensure!(
					params.target_min_free_unbonded_ratio <= params.target_max_free_unbonded_ratio,
					Error::<T>::InvalidConfig
				);
				Ok(())
			})?;
			Ok(())
		}
	}
}

/// Impl helper for managing staking currency which distributed on multiple
/// sub accounts by polkadot bridge.
impl<T: Config> Pallet<T> {
	/// Pass the sorted list, pick the first item
	pub fn distribute_increment(amount_list: Vec<(u32, Balance)>, increment: Balance) -> Vec<(u32, Balance)> {
		if amount_list.len().is_zero() {
			vec![]
		} else {
			vec![(amount_list[0].0, increment)]
		}
	}

	/// Pass the sorted list, consume available by order.
	pub fn distribute_decrement(amount_list: Vec<(u32, Balance)>, decrement: Balance) -> Vec<(u32, Balance)> {
		let mut distribution: Vec<(u32, Balance)> = vec![];
		let mut remain_decrement = decrement;

		for (sub_account_index, available) in amount_list {
			if remain_decrement.is_zero() {
				break;
			}
			distribution.push((sub_account_index, sp_std::cmp::min(available, remain_decrement)));
			remain_decrement = remain_decrement.saturating_sub(available);
		}

		distribution
	}

	/// Require polkadot bridge to bind more staking currency on relaychain.
	pub fn bond_extra(amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let staking_ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::free_balance(*account_index);
				(staking_ledger.active, *account_index, free)
			})
			.collect::<Vec<_>>();

		// Sort by bonded amount in ascending order
		current_available.sort_by(|a, b| a.0.cmp(&b.0));
		let current_available = current_available
			.iter()
			.map(|(_, account_index, free)| (*account_index, *free))
			.collect::<Vec<_>>();
		let distribution = Self::distribute_decrement(current_available, amount);

		for (account_index, val) in distribution {
			T::Bridge::bond_extra(account_index, val)?;
		}

		Ok(())
	}

	/// Require bridge to unbond on relaychain.
	pub fn unbond(amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_bonded = sub_accounts
			.iter()
			.map(|account_index| (*account_index, T::Bridge::staking_ledger(*account_index).active))
			.collect::<Vec<_>>();

		// Sort by bonded amount in descending order
		current_bonded.sort_by(|a, b| b.1.cmp(&a.1));
		let distribution = Self::distribute_decrement(current_bonded, amount);

		for (account_index, val) in distribution {
			T::Bridge::unbond(account_index, val)?;
		}

		Ok(())
	}

	/// Require bridge to transfer staking currency to specific
	/// account from relaychain.
	pub fn receive_from_bridge(to: &T::AccountId, amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::free_balance(*account_index);
				(ledger.active, *account_index, free)
			})
			.collect::<Vec<_>>();

		// Sort by bonded amount in descending order
		current_available.sort_by(|a, b| b.0.cmp(&a.0));
		let current_available = current_available
			.iter()
			.map(|(_, account_index, free)| (*account_index, *free))
			.collect::<Vec<_>>();
		let distribution = Self::distribute_decrement(current_available, amount);

		for (account_index, val) in distribution {
			T::Bridge::receive_from_bridge(account_index, to, val)?;
		}

		Ok(())
	}

	/// Transfer staking currency from specific account to relaychain by bridge.
	pub fn transfer_to_bridge(from: &T::AccountId, amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_balance = sub_accounts
			.iter()
			.map(|account_index| (*account_index, T::Bridge::staking_ledger(*account_index).active))
			.collect::<Vec<_>>();

		// Sort by bonded amount in ascending order
		current_balance.sort_by(|a, b| a.1.cmp(&b.1));
		let distribution = Self::distribute_increment(current_balance, amount);

		for (account_index, val) in distribution.iter() {
			T::Bridge::transfer_to_bridge(*account_index, from, *val)?;
		}

		Ok(())
	}

	/// Require bridge to withdraw unbonded on relaychain.
	pub fn withdraw_unbonded() {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::withdraw_unbonded(sub_account_index);
		}
	}

	/// Require bridge to get staking rewards on relaychain.
	pub fn payout_stakers(era: EraIndex) {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::payout_stakers(sub_account_index, era);
		}
	}

	/// Require bridge to nominate validators of relaychain.
	pub fn nominate(targets: Vec<PolkadotAccountIdOf<T>>) {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::nominate(sub_account_index, targets.clone());
		}
	}

	/// Merge ledger of sub accounts on relaychain.
	pub fn relaychain_staking_ledger() -> PolkadotStakingLedger<Balance, EraIndex> {
		let mut active: Balance = Zero::zero();
		let mut total: Balance = Zero::zero();

		let mut accumulated_unlocking: Vec<PolkadotUnlockChunk<Balance, EraIndex>> = vec![];

		for sub_account_index in T::PoolAccountIndexes::get() {
			let ledger = T::Bridge::staking_ledger(sub_account_index);
			active = active.saturating_add(ledger.active);
			total = total.saturating_add(ledger.total);

			for chunk in ledger.unlocking {
				let mut find: bool = false;
				for (index, existd_chunk) in accumulated_unlocking.iter().enumerate() {
					if chunk.era == existd_chunk.era {
						accumulated_unlocking[index].value = existd_chunk.value.saturating_add(chunk.value);
						find = true;
						break;
					}
				}
				if !find {
					accumulated_unlocking.push(chunk.clone());
				}
			}
		}

		// sort list
		accumulated_unlocking.sort_by(|a, b| a.era.cmp(&b.era));

		PolkadotStakingLedger::<Balance, EraIndex> {
			total,
			active,
			unlocking: accumulated_unlocking,
		}
	}

	/// Merge total balance of sub accounts on relaychain.
	pub fn relaychain_free_balance() -> Balance {
		let mut total: Balance = Zero::zero();
		for sub_account_index in T::PoolAccountIndexes::get() {
			total = total.saturating_add(T::Bridge::free_balance(sub_account_index));
		}
		total
	}
}

impl<T: Config> Pallet<T> {
	/// Module account id
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}

	/// Get the exchange rate for liquid currency to staking currency.
	pub fn liquid_exchange_rate() -> ExchangeRate {
		let exchange_rate = ExchangeRate::checked_from_rational(
			Self::staking_pool_ledger().total_belong_to_liquid_holders(),
			T::Currency::total_issuance(T::LiquidCurrencyId::get()),
		)
		.unwrap_or_default();

		if exchange_rate == Default::default() {
			T::DefaultExchangeRate::get()
		} else {
			exchange_rate
		}
	}

	/// Get how much available unbonded of `who` in current era.
	pub fn get_available_unbonded(who: &T::AccountId) -> Balance {
		Unbondings::<T>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &Self::current_era())
			.fold(Zero::zero(), |available_unbonded, (_, unbonded)| {
				available_unbonded.saturating_add(unbonded)
			})
	}

	pub fn rebalance() {
		match Self::rebalance_phase() {
			Phase::Started => {
				// require relaychain to update nominees.
				Self::nominate(T::Nominees::nominees());

				// require relaychain to withdraw unbonded.
				Self::withdraw_unbonded();

				// require relaychain to payout stakers.
				Self::payout_stakers(Self::current_era().saturating_sub(1));

				RebalancePhase::<T>::put(Phase::RelaychainUpdated);
			}

			Phase::RelaychainUpdated => {
				StakingPoolLedger::<T>::mutate(|ledger| {
					let relaychain_staking_ledger = Self::relaychain_staking_ledger();
					let relaychain_free_balance = Self::relaychain_free_balance();

					// update bonded of staking pool to the active(bonded) of relaychain ledger.
					ledger.bonded = relaychain_staking_ledger.active;

					// withdraw available staking currency from polkadot bridge to staking pool.
					if Self::receive_from_bridge(&Self::account_id(), relaychain_free_balance).is_ok() {
						let current_era = Self::current_era();
						let mut total_unbonded: Balance = Zero::zero();
						let mut total_claimed_unbonded: Balance = Zero::zero();

						// iterator all expired unbonding to get total unbonded amount.
						for (era_index, (total_unbonding, claimed_unbonding, _)) in Unbonding::<T>::iter() {
							if era_index <= current_era {
								total_unbonded = total_unbonded.saturating_add(total_unbonding);
								total_claimed_unbonded = total_claimed_unbonded.saturating_add(claimed_unbonding);
								Unbonding::<T>::remove(era_index);
							}
						}

						ledger.unbonding_to_free = ledger
							.unbonding_to_free
							.saturating_sub(total_unbonded.saturating_sub(total_claimed_unbonded));
						ledger.free_pool = ledger
							.free_pool
							.saturating_add(relaychain_free_balance.saturating_sub(total_claimed_unbonded));
					}
				});

				RebalancePhase::<T>::put(Phase::LedgerUpdated);
			}

			Phase::LedgerUpdated => {
				StakingPoolLedger::<T>::mutate(|ledger| {
					let staking_pool_params = Self::staking_pool_params();
					let (mut total_unbond, claimed_unbond) = ledger.to_unbond_next_era;

					let bond_rate = ledger
						.free_pool_ratio()
						.saturating_sub(staking_pool_params.target_max_free_unbonded_ratio);
					let amount_to_bond = bond_rate
						.saturating_mul_int(ledger.total_belong_to_liquid_holders())
						.min(ledger.free_pool);
					let unbond_to_free_rate = staking_pool_params
						.target_unbonding_to_free_ratio
						.saturating_sub(ledger.unbonding_to_free_ratio())
						.min(staking_pool_params.unbonding_to_free_adjustment);
					let amount_to_unbond_to_free = unbond_to_free_rate
						.saturating_mul_int(ledger.total_belong_to_liquid_holders())
						.min(ledger.bonded_belong_to_liquid_holders());
					total_unbond = total_unbond.saturating_add(amount_to_unbond_to_free);

					if !amount_to_bond.is_zero() {
						if Self::transfer_to_bridge(&Self::account_id(), amount_to_bond).is_ok() {
							ledger.free_pool = ledger.free_pool.saturating_sub(amount_to_bond);
						}

						if Self::bond_extra(amount_to_bond).is_ok() {
							ledger.bonded = ledger.bonded.saturating_add(amount_to_bond);
						}
					}

					if !total_unbond.is_zero() {
						// if failed, will try unbonding on next era beginning.
						if Self::unbond(total_unbond).is_ok() {
							let expired_era_index =
								Self::current_era()
									.saturating_add(
										<<T as Config>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get(),
									);

							Unbonding::<T>::insert(expired_era_index, (total_unbond, claimed_unbond, claimed_unbond));
							for (who, claimed) in NextEraUnbonds::<T>::drain() {
								Unbondings::<T>::insert(who, expired_era_index, claimed);
							}

							ledger.bonded = ledger.bonded.saturating_sub(total_unbond);
							ledger.unbonding_to_free = ledger
								.unbonding_to_free
								.saturating_add(total_unbond.saturating_sub(claimed_unbond));
							ledger.to_unbond_next_era = (Zero::zero(), Zero::zero());
						}
					}
				});

				RebalancePhase::<T>::put(Phase::Finished);
			}

			_ => {}
		}
	}
}

impl<T: Config> OnNewEra<EraIndex> for Pallet<T> {
	fn on_new_era(new_era: EraIndex) {
		CurrentEra::<T>::put(new_era);
		RebalancePhase::<T>::put(Phase::Started);
	}
}

impl<T: Config> HomaProtocol<T::AccountId, Balance, EraIndex> for Pallet<T> {
	type Balance = Balance;

	#[transactional]
	fn mint(who: &T::AccountId, amount: Self::Balance) -> sp_std::result::Result<Self::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero());
		}

		ensure!(
			Self::rebalance_phase() == Phase::Finished,
			Error::<T>::RebalanceUnfinished
		);

		StakingPoolLedger::<T>::try_mutate(|ledger| -> sp_std::result::Result<Self::Balance, DispatchError> {
			let liquid_amount_to_issue = Self::liquid_exchange_rate()
				.reciprocal()
				.unwrap_or_default()
				.checked_mul_int(amount)
				.ok_or(ArithmeticError::Overflow)?;

			T::Currency::transfer(T::StakingCurrencyId::get(), who, &Self::account_id(), amount)?;
			T::Currency::deposit(T::LiquidCurrencyId::get(), who, liquid_amount_to_issue)?;

			ledger.free_pool = ledger.free_pool.saturating_add(amount);

			Self::deposit_event(Event::MintLiquid {
				who: who.clone(),
				staking_amount_deposited: amount,
				liquid_amount_issued: liquid_amount_to_issue,
			});
			Ok(liquid_amount_to_issue)
		})
	}

	#[transactional]
	fn redeem_by_unbond(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		ensure!(
			Self::rebalance_phase() == Phase::Finished,
			Error::<T>::RebalanceUnfinished
		);

		StakingPoolLedger::<T>::try_mutate(|ledger| -> DispatchResult {
			let mut liquid_amount_to_burn = amount;
			let liquid_exchange_rate = Self::liquid_exchange_rate();
			let mut staking_amount_to_unbond = liquid_exchange_rate
				.checked_mul_int(liquid_amount_to_burn)
				.ok_or(ArithmeticError::Overflow)?;
			let communal_bonded_staking_amount = ledger.bonded_belong_to_liquid_holders();

			if !staking_amount_to_unbond.is_zero() && !communal_bonded_staking_amount.is_zero() {
				// communal_bonded_staking_amount is not enough, re-calculate
				if staking_amount_to_unbond > communal_bonded_staking_amount {
					liquid_amount_to_burn = liquid_exchange_rate
						.reciprocal()
						.unwrap_or_default()
						.saturating_mul_int(communal_bonded_staking_amount);
					staking_amount_to_unbond = communal_bonded_staking_amount;
				}

				// burn liquid currency
				T::Currency::withdraw(T::LiquidCurrencyId::get(), who, liquid_amount_to_burn)?;

				NextEraUnbonds::<T>::mutate(who, |unbond| {
					*unbond = unbond.saturating_add(staking_amount_to_unbond);
				});

				let (total_unbond, claimed_unbond) = ledger.to_unbond_next_era;
				ledger.to_unbond_next_era = (
					total_unbond.saturating_add(staking_amount_to_unbond),
					claimed_unbond.saturating_add(staking_amount_to_unbond),
				);

				Self::deposit_event(Event::RedeemByUnbond {
					who: who.clone(),
					liquid_amount_burned: liquid_amount_to_burn,
					staking_amount_redeemed: staking_amount_to_unbond,
				});
			}

			Ok(())
		})
	}

	#[transactional]
	fn redeem_by_free_unbonded(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		ensure!(
			Self::rebalance_phase() == Phase::Finished,
			Error::<T>::RebalanceUnfinished
		);

		StakingPoolLedger::<T>::try_mutate(|ledger| -> DispatchResult {
			let mut liquid_amount_to_burn = amount;
			let liquid_exchange_rate = Self::liquid_exchange_rate();
			let mut demand_staking_amount = liquid_exchange_rate
				.checked_mul_int(liquid_amount_to_burn)
				.ok_or(ArithmeticError::Overflow)?;
			let staking_pool_params = Self::staking_pool_params();
			let available_free_pool = ledger.free_pool.saturating_sub(
				staking_pool_params
					.target_min_free_unbonded_ratio
					.saturating_mul_int(ledger.total_belong_to_liquid_holders()),
			);

			if !demand_staking_amount.is_zero() && !available_free_pool.is_zero() {
				// if available_free_pool is not enough, need re-calculate
				if demand_staking_amount > available_free_pool {
					let ratio = Ratio::checked_from_rational(available_free_pool, demand_staking_amount)
						.expect("demand_staking_amount is gt available_free_pool and not zero; qed");
					liquid_amount_to_burn = ratio.saturating_mul_int(liquid_amount_to_burn);
					demand_staking_amount = available_free_pool;
				}

				let current_free_pool_ratio = ledger.free_pool_ratio();
				let remain_available_percent = current_free_pool_ratio
					.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio)
					.checked_div(
						&sp_std::cmp::max(
							staking_pool_params.target_max_free_unbonded_ratio,
							current_free_pool_ratio,
						)
						.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio),
					)
					.unwrap_or_default();
				let fee_in_staking = T::FeeModel::get_fee(
					remain_available_percent,
					available_free_pool,
					demand_staking_amount,
					staking_pool_params.base_fee_rate,
				)
				.ok_or(Error::<T>::GetFeeFailed)?;

				let staking_amount_to_retrieve = demand_staking_amount.saturating_sub(fee_in_staking);

				T::Currency::withdraw(T::LiquidCurrencyId::get(), who, liquid_amount_to_burn)?;
				T::Currency::transfer(
					T::StakingCurrencyId::get(),
					&Self::account_id(),
					who,
					staking_amount_to_retrieve,
				)?;

				ledger.free_pool = ledger.free_pool.saturating_sub(staking_amount_to_retrieve);

				Self::deposit_event(Event::RedeemByFreeUnbonded {
					who: who.clone(),
					fee_in_staking: liquid_amount_to_burn,
					liquid_amount_burned: staking_amount_to_retrieve,
					staking_amount_redeemed: fee_in_staking,
				});
			}

			Ok(())
		})
	}

	#[transactional]
	fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Self::Balance, target_era: EraIndex) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		ensure!(
			Self::rebalance_phase() == Phase::Finished,
			Error::<T>::RebalanceUnfinished
		);

		let current_era = Self::current_era();
		let bonding_duration = <<T as Config>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		ensure!(
			target_era > current_era && target_era <= current_era + bonding_duration,
			Error::<T>::InvalidEra,
		);

		StakingPoolLedger::<T>::try_mutate(|ledger| -> DispatchResult {
			let mut liquid_amount_to_burn = amount;
			let mut demand_staking_amount = Self::liquid_exchange_rate()
				.checked_mul_int(liquid_amount_to_burn)
				.ok_or(ArithmeticError::Overflow)?;
			let (unbonding, claimed_unbonding, initial_claimed_unbonding) = Self::unbonding(target_era);
			let initial_unclaimed = unbonding.saturating_sub(initial_claimed_unbonding);
			let unclaimed = unbonding.saturating_sub(claimed_unbonding);
			let staking_pool_params = Self::staking_pool_params();
			let available_unclaimed_unbonding = unclaimed.saturating_sub(
				staking_pool_params
					.target_min_free_unbonded_ratio
					.saturating_mul_int(initial_unclaimed),
			);

			if !demand_staking_amount.is_zero() && !available_unclaimed_unbonding.is_zero() {
				// if available_unclaimed_unbonding is not enough, need re-calculate
				if demand_staking_amount > available_unclaimed_unbonding {
					let ratio = Ratio::checked_from_rational(available_unclaimed_unbonding, demand_staking_amount)
						.expect("demand_staking_amount is gt available_unclaimed_unbonding and not zero; qed");
					liquid_amount_to_burn = ratio.saturating_mul_int(liquid_amount_to_burn);
					demand_staking_amount = available_unclaimed_unbonding;
				}
				let current_unclaimed_ratio = Ratio::checked_from_rational(unclaimed, initial_unclaimed)
					.expect("if available_unclaimed_unbonding is not zero, initial_unclaimed must not be zero; qed");
				let remain_available_percent = current_unclaimed_ratio
					.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio)
					.checked_div(
						&sp_std::cmp::max(
							staking_pool_params.target_max_free_unbonded_ratio,
							current_unclaimed_ratio,
						)
						.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio),
					)
					.unwrap_or_default();
				let fee_in_staking = T::FeeModel::get_fee(
					remain_available_percent,
					available_unclaimed_unbonding,
					demand_staking_amount,
					staking_pool_params.base_fee_rate,
				)
				.ok_or(Error::<T>::GetFeeFailed)?;
				let staking_amount_to_claim = demand_staking_amount.saturating_sub(fee_in_staking);

				T::Currency::withdraw(T::LiquidCurrencyId::get(), who, liquid_amount_to_burn)?;

				Unbondings::<T>::mutate(who, target_era, |unbonding| {
					*unbonding = unbonding.saturating_add(staking_amount_to_claim);
				});
				Unbonding::<T>::mutate(target_era, |(_, claimed_unbonding, _)| {
					*claimed_unbonding = claimed_unbonding.saturating_add(staking_amount_to_claim);
				});
				ledger.unbonding_to_free = ledger.unbonding_to_free.saturating_sub(staking_amount_to_claim);

				Self::deposit_event(Event::RedeemByClaimUnbonding {
					who: who.clone(),
					target_era,
					fee_in_staking: liquid_amount_to_burn,
					liquid_amount_burned: staking_amount_to_claim,
					staking_amount_redeemed: fee_in_staking,
				});
			}

			Ok(())
		})
	}

	#[transactional]
	fn withdraw_redemption(who: &T::AccountId) -> sp_std::result::Result<Self::Balance, DispatchError> {
		let mut withdrawn_amount: Balance = Zero::zero();

		Unbondings::<T>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &Self::current_era())
			.for_each(|(expired_era_index, unbonded)| {
				withdrawn_amount = withdrawn_amount.saturating_add(unbonded);
				Unbondings::<T>::remove(who, expired_era_index);
			});

		T::Currency::transfer(T::StakingCurrencyId::get(), &Self::account_id(), who, withdrawn_amount)?;
		Ok(withdrawn_amount)
	}
}

pub struct OnSlash<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<Balance> for OnSlash<T> {
	fn happened(_amount: &Balance) {
		// TODO: should reduce debit when homa_validator_list module burn
		// insurance to compensate LDOT holders.
	}
}
