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
use orml_traits::{Change, MultiCurrency};
use primitives::{Balance, CurrencyId, EraIndex};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, Saturating, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId, RuntimeDebug,
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
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
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

/// The ledger of staking pool.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct Ledger {
	/// The amount of total bonded.
	pub bonded: Balance,
	/// The amount of total unbonding to free pool.
	pub unbonding_to_free: Balance,
	/// The amount of free pool.
	pub free_pool: Balance,
	/// The amount to unbond when next era beginning.
	pub to_unbond_next_era: (Balance, Balance),
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
		type ModuleId: Get<ModuleId>;

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
		/// Overflow.
		Overflow,
		/// Failed to calculate redemption fee.
		GetFeeFailed,
		/// Invalid config.
		InvalidConfig,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Deposit staking currency(DOT) to staking pool and issue liquid
		/// currency(LDOT). \[who, staking_amount_deposited,
		/// liquid_amount_issued\]
		MintLiquid(T::AccountId, Balance, Balance),
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by
		/// waiting for complete unbond eras. \[who, liquid_amount_burned,
		/// staking_amount_redeemed\]
		RedeemByUnbond(T::AccountId, Balance, Balance),
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by free
		/// pool immediately. \[who, fee_in_staking, liquid_amount_burned,
		/// staking_amount_redeemed\]
		RedeemByFreeUnbonded(T::AccountId, Balance, Balance, Balance),
		/// Burn liquid currency(LDOT) and redeem staking currency(DOT) by claim
		/// the unbonding_to_free of specific era. \[who, target_era,
		/// fee_in_staking, liquid_amount_burned, staking_amount_redeemed\]
		RedeemByClaimUnbonding(T::AccountId, EraIndex, Balance, Balance, Balance),
	}

	/// Current era index of Polkadot.
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	/// Unbond on next era beginning by AccountId.
	/// AccountId => Unbond
	#[pallet::storage]
	#[pallet::getter(fn next_era_unbonds)]
	pub type NextEraUnbonds<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Balance, ValueQuery>;

	/// The records of unbonding.
	/// ExpiredEraIndex => (TotalUnbounding, ClaimedUnbonding,
	/// InitialClaimedUnbonding)
	#[pallet::storage]
	#[pallet::getter(fn unbonding)]
	pub type Unbonding<T: Config> = StorageMap<_, Twox64Concat, EraIndex, (Balance, Balance, Balance), ValueQuery>;

	/// The records of unbonding by AccountId.
	/// AccountId, ExpiredEraIndex => Unbounding
	#[pallet::storage]
	#[pallet::getter(fn unbondings)]
	pub type Unbondings<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, EraIndex, Balance, ValueQuery>;

	/// The ledger of staking pool.
	#[pallet::storage]
	#[pallet::getter(fn staking_pool_ledger)]
	pub type StakingPoolLedger<T: Config> = StorageValue<_, Ledger, ValueQuery>;

	/// The params of staking pool.
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

	#[cfg(feature = "std")]
	impl GenesisConfig {
		/// Direct implementation of `GenesisBuild::build_storage`.
		///
		/// Kept in order not to break dependency.
		pub fn build_storage<T: Config>(&self) -> Result<sp_runtime::Storage, String> {
			<Self as frame_support::traits::GenesisBuild<T>>::build_storage(self)
		}

		/// Direct implementation of `GenesisBuild::assimilate_storage`.
		///
		/// Kept in order not to break dependency.
		pub fn assimilate_storage<T: Config>(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
			<Self as frame_support::traits::GenesisBuild<T>>::assimilate_storage(self, storage)
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

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
		) -> DispatchResultWithPostInfo {
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
			Ok(().into())
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

	/// Require polkadot bridge to bind more staking currency on Polkadot.
	pub fn bond_extra(amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let staking_ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::balance(*account_index).saturating_sub(staking_ledger.total);
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

	/// Require polkadot bridge to unbond on Polkadot.
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

	/// Require polkadot bridge to transfer staking currency to specific
	/// account from Polkadot.
	pub fn receive_from_bridge(to: &T::AccountId, amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::balance(*account_index).saturating_sub(ledger.total);
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

	/// Transfer staking currency from specific account to Polkadot by
	/// polkadot bridge.
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

	/// Require polkadot bridge to withdraw unbonded on Polkadot.
	pub fn withdraw_unbonded() {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::withdraw_unbonded(sub_account_index);
		}
	}

	/// Require polkadot bridge to get staking rewards on Polkadot.
	pub fn payout_nominator() {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::payout_nominator(sub_account_index);
		}
	}

	/// Require polkadot bridge to nominate validators of Polkadot.
	pub fn nominate(targets: Vec<PolkadotAccountIdOf<T>>) {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::nominate(sub_account_index, targets.clone());
		}
	}

	/// Merge ledger of sub accounts on Polkadot.
	pub fn staking_ledger() -> PolkadotStakingLedger<Balance, EraIndex> {
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

	/// Merge total balance of sub accounts on Polkadot.
	pub fn balance() -> Balance {
		let mut total: Balance = Zero::zero();
		for sub_account_index in T::PoolAccountIndexes::get() {
			total = total.saturating_add(T::Bridge::balance(sub_account_index));
		}
		total
	}
}

impl<T: Config> Pallet<T> {
	/// Module account id
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
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

	pub fn update_ledger_with_bridge(current_era: EraIndex) {
		// require polkadot bridge to withdraw unbonded.
		Self::withdraw_unbonded();

		// require polkadot bridge to payout nominator.
		// TODO: record the balances of bridge before and after payout_nominator,
		// and oncommision to homa treasury according to `RewardFeeRatio`.
		Self::payout_nominator();

		StakingPoolLedger::<T>::mutate(|ledger| {
			let polkadot_bridge_ledger = Self::staking_ledger();
			let available_on_polkadot_bridge = Self::balance().saturating_sub(polkadot_bridge_ledger.total);

			// update bonded of staking pool to the active(bonded) of polkadot bridge
			// ledger.
			ledger.bonded = polkadot_bridge_ledger.active;

			// withdraw available staking currency from polkadot bridge to staking pool.
			if Self::receive_from_bridge(&Self::account_id(), available_on_polkadot_bridge).is_ok() {
				let (total_unbonded, claimed_unbonded, _) = Unbonding::<T>::take(current_era);
				ledger.unbonding_to_free = ledger
					.unbonding_to_free
					.saturating_sub(total_unbonded.saturating_sub(claimed_unbonded));
				ledger.free_pool = ledger
					.free_pool
					.saturating_add(available_on_polkadot_bridge.saturating_sub(claimed_unbonded));
			}
		});
	}

	pub fn rebalance(current_era: EraIndex) {
		// require polkadot bridge to update nominees.
		Self::nominate(T::Nominees::nominees());

		// require polkadot bridge to withdraw unbonded and withdraw payout and update
		// staking pool ledger.
		Self::update_ledger_with_bridge(current_era);

		// staking pool require polkadot bridge to bond and unbond according to ledger,
		// and update related records.
		StakingPoolLedger::<T>::mutate(|ledger| {
			let (mut total_unbond, claimed_unbond) = ledger.to_unbond_next_era;
			let staking_pool_params = Self::staking_pool_params();

			let bond_rate = ledger
				.free_pool_ratio()
				.saturating_sub(staking_pool_params.target_max_free_unbonded_ratio);
			let bond_amount = bond_rate
				.saturating_mul_int(ledger.total_belong_to_liquid_holders())
				.min(ledger.free_pool);

			let unbond_to_free_rate = staking_pool_params
				.target_unbonding_to_free_ratio
				.saturating_sub(ledger.unbonding_to_free_ratio())
				.min(staking_pool_params.unbonding_to_free_adjustment);
			let unbond_to_free_amount = unbond_to_free_rate
				.saturating_mul_int(ledger.total_belong_to_liquid_holders())
				.min(ledger.bonded_belong_to_liquid_holders());
			total_unbond = total_unbond.saturating_add(unbond_to_free_amount);

			if !bond_amount.is_zero() {
				if Self::transfer_to_bridge(&Self::account_id(), bond_amount).is_ok() {
					ledger.free_pool = ledger.free_pool.saturating_sub(bond_amount);
				}

				if Self::bond_extra(bond_amount).is_ok() {
					ledger.bonded = ledger.bonded.saturating_add(bond_amount);
				}
			}

			if !total_unbond.is_zero() {
				// if failed, will try unbonding on next era beginning.
				if Self::unbond(total_unbond).is_ok() {
					let expired_era_index = current_era
						.saturating_add(<<T as Config>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get());

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
	}
}

impl<T: Config> OnNewEra<EraIndex> for Pallet<T> {
	fn on_new_era(new_era: EraIndex) {
		CurrentEra::<T>::put(new_era);
		Self::rebalance(new_era);
	}
}

impl<T: Config> HomaProtocol<T::AccountId, Balance, EraIndex> for Pallet<T> {
	type Balance = Balance;

	#[transactional]
	fn mint(who: &T::AccountId, amount: Self::Balance) -> sp_std::result::Result<Self::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero());
		}

		StakingPoolLedger::<T>::try_mutate(|ledger| -> sp_std::result::Result<Self::Balance, DispatchError> {
			let liquid_amount_to_issue = Self::liquid_exchange_rate()
				.reciprocal()
				.unwrap_or_default()
				.checked_mul_int(amount)
				.ok_or(Error::<T>::Overflow)?;

			T::Currency::transfer(T::StakingCurrencyId::get(), who, &Self::account_id(), amount)?;
			T::Currency::deposit(T::LiquidCurrencyId::get(), who, liquid_amount_to_issue)?;

			ledger.free_pool = ledger.free_pool.saturating_add(amount);

			Self::deposit_event(Event::MintLiquid(who.clone(), amount, liquid_amount_to_issue));
			Ok(liquid_amount_to_issue)
		})
	}

	#[transactional]
	fn redeem_by_unbond(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		StakingPoolLedger::<T>::try_mutate(|ledger| -> DispatchResult {
			let mut liquid_amount_to_burn = amount;
			let liquid_exchange_rate = Self::liquid_exchange_rate();
			let mut staking_amount_to_unbond = liquid_exchange_rate
				.checked_mul_int(liquid_amount_to_burn)
				.ok_or(Error::<T>::Overflow)?;
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

				Self::deposit_event(Event::RedeemByUnbond(
					who.clone(),
					liquid_amount_to_burn,
					staking_amount_to_unbond,
				));
			}

			Ok(())
		})
	}

	#[transactional]
	fn redeem_by_free_unbonded(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		StakingPoolLedger::<T>::try_mutate(|ledger| -> DispatchResult {
			let mut liquid_amount_to_burn = amount;
			let liquid_exchange_rate = Self::liquid_exchange_rate();
			let mut demand_staking_amount = liquid_exchange_rate
				.checked_mul_int(liquid_amount_to_burn)
				.ok_or(Error::<T>::Overflow)?;
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
						.expect("demand_staking_amount is not zero; qed");
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

				Self::deposit_event(Event::RedeemByFreeUnbonded(
					who.clone(),
					liquid_amount_to_burn,
					staking_amount_to_retrieve,
					fee_in_staking,
				));
			}

			Ok(())
		})
	}

	#[transactional]
	fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Self::Balance, target_era: EraIndex) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

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
				.ok_or(Error::<T>::Overflow)?;
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
						.expect("staking_amount_to_claim is not zero; qed");
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

				Self::deposit_event(Event::RedeemByClaimUnbonding(
					who.clone(),
					target_era,
					liquid_amount_to_burn,
					staking_amount_to_claim,
					fee_in_staking,
				));
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
