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

//! # Homa validator list Module
//!
//! ## Overview
//!
//! This will require validators to lock some Liquid Token into insurance fund
//! and if slash happened, HomaCouncil can burn those Liquid Token to compensate
//! Liquid Token holders.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::collapsible_if)]

use codec::MaxEncodedLen;
use frame_support::{pallet_prelude::*, traits::Contains, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::{BasicCurrency, BasicLockableCurrency, Happened, LockIdentifier};
use primitives::Balance;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{BlockNumberProvider, Bounded, MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
	DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::{fmt::Debug, vec::Vec};
use support::{ExchangeRateProvider, Ratio};

mod mock;
mod tests;

pub use module::*;

pub const HOMA_VALIDATOR_LIST_ID: LockIdentifier = *b"acalahvl";

pub trait WeightInfo {
	fn bond() -> Weight;
	fn unbond() -> Weight;
	fn rebond() -> Weight;
	fn withdraw_unbonded() -> Weight;
	fn freeze(u: u32) -> Weight;
	fn thaw() -> Weight;
	fn slash() -> Weight;
}

// TODO: do benchmarking test.
impl WeightInfo for () {
	fn bond() -> Weight {
		10_000
	}
	fn unbond() -> Weight {
		10_000
	}
	fn rebond() -> Weight {
		10_000
	}
	fn withdraw_unbonded() -> Weight {
		10_000
	}
	fn freeze(_u: u32) -> Weight {
		10_000
	}
	fn thaw() -> Weight {
		10_000
	}
	fn slash() -> Weight {
		10_000
	}
}

/// Insurance for a validator from a single address
#[derive(Encode, Decode, Clone, Copy, RuntimeDebug, Default, PartialEq, Eq, MaxEncodedLen, TypeInfo)]
pub struct Guarantee<BlockNumber> {
	/// The total tokens the validator has in insurance
	total: Balance,
	/// The number of tokens that are actively bonded for insurance
	bonded: Balance,
	/// The number of tokens that are in the process of unbonding for insurance
	unbonding: Option<(Balance, BlockNumber)>,
}

impl<BlockNumber: PartialOrd> Guarantee<BlockNumber> {
	/// Take `unbonding` that are sufficiently old
	fn consolidate_unbonding(mut self, current_block: BlockNumber) -> Self {
		match self.unbonding {
			Some((_, expired_block)) if expired_block <= current_block => {
				self.unbonding = None;
			}
			_ => {}
		}
		self
	}

	/// Re-bond funds that were scheduled for unbonding.
	fn rebond(mut self, rebond_amount: Balance) -> Self {
		if let Some((amount, _)) = self.unbonding.as_mut() {
			let rebond_amount = rebond_amount.min(*amount);
			self.bonded = self.bonded.saturating_add(rebond_amount);
			*amount = amount.saturating_sub(rebond_amount);
			if amount.is_zero() {
				self.unbonding = None;
			}
		}
		self
	}

	fn slash(mut self, slash_amount: Balance) -> Self {
		let mut remains = slash_amount;
		let slash_from_bonded = self.bonded.min(remains);
		self.bonded = self.bonded.saturating_sub(remains);
		self.total = self.total.saturating_sub(remains);
		remains = remains.saturating_sub(slash_from_bonded);

		if !remains.is_zero() {
			if let Some((unbonding_amount, _)) = self.unbonding.as_mut() {
				let slash_from_unbonding = remains.min(*unbonding_amount);
				*unbonding_amount = unbonding_amount.saturating_sub(slash_from_unbonding);
				if unbonding_amount.is_zero() {
					self.unbonding = None;
				}
			}
		}

		self
	}
}

/// Information on a relay chain validator's slash
#[derive(Encode, Decode, Clone, RuntimeDebug, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SlashInfo<Balance, RelaychainAccountId> {
	/// Address of a validator on the relay chain
	validator: RelaychainAccountId,
	/// The amount of tokens a validator has in backing on the relay chain
	relaychain_token_amount: Balance,
}

/// Validator insurance and frozen status
#[derive(Encode, Decode, Clone, Copy, RuntimeDebug, Default, MaxEncodedLen, TypeInfo)]
pub struct ValidatorBacking {
	/// Total insurance from all guarantors
	total_insurance: Balance,
	is_frozen: bool,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The AccountId of a relay chain account.
		type RelaychainAccountId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ Ord
			+ Default
			+ MaxEncodedLen;
		/// The liquid representation of the staking token on the relay chain.
		type LiquidTokenCurrency: BasicLockableCurrency<Self::AccountId, Balance = Balance>;
		#[pallet::constant]
		/// The minimum amount of tokens that can be bonded to a validator.
		type MinBondAmount: Get<Balance>;
		#[pallet::constant]
		/// The number of blocks a token is bonded to a validator for.
		type BondingDuration: Get<Self::BlockNumber>;
		#[pallet::constant]
		/// The minimum amount of insurance a validator needs.
		type ValidatorInsuranceThreshold: Get<Balance>;
		/// The AccountId that can perform a freeze.
		type FreezeOrigin: EnsureOrigin<Self::Origin>;
		/// The AccountId that can perform a slash.
		type SlashOrigin: EnsureOrigin<Self::Origin>;
		/// Callback to be called when a slash occurs.
		type OnSlash: Happened<Balance>;
		/// Exchange rate between staked token and liquid token equivalent.
		type LiquidStakingExchangeRateProvider: ExchangeRateProvider;
		type WeightInfo: WeightInfo;
		/// Callback to be called when a validator's insurance increases.
		type OnIncreaseGuarantee: Happened<(Self::AccountId, Self::RelaychainAccountId, Balance)>;
		/// Callback to be called when a validator's insurance decreases.
		type OnDecreaseGuarantee: Happened<(Self::AccountId, Self::RelaychainAccountId, Balance)>;

		// The block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;
	}

	#[pallet::error]
	pub enum Error<T> {
		BelowMinBondAmount,
		UnbondingExists,
		FrozenValidator,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		FreezeValidator {
			validator: T::RelaychainAccountId,
		},
		ThawValidator {
			validator: T::RelaychainAccountId,
		},
		BondGuarantee {
			who: T::AccountId,
			validator: T::RelaychainAccountId,
			bond: Balance,
		},
		UnbondGuarantee {
			who: T::AccountId,
			validator: T::RelaychainAccountId,
			bond: Balance,
		},
		WithdrawnGuarantee {
			who: T::AccountId,
			validator: T::RelaychainAccountId,
			bond: Balance,
		},
		SlashGuarantee {
			who: T::AccountId,
			validator: T::RelaychainAccountId,
			bond: Balance,
		},
	}

	/// The slash guarantee deposits for relaychain validators.
	///
	/// Guarantees: double_map RelaychainAccountId, AccountId => Option<Guarantee>
	#[pallet::storage]
	#[pallet::getter(fn guarantees)]
	pub type Guarantees<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::RelaychainAccountId,
		Twox64Concat,
		T::AccountId,
		Guarantee<T::BlockNumber>,
		OptionQuery,
	>;

	/// Total deposits for users.
	///
	/// TotalLockedByGuarantor: map AccountId => Option<Balance>
	#[pallet::storage]
	#[pallet::getter(fn total_locked_by_guarantor)]
	pub type TotalLockedByGuarantor<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Balance, OptionQuery>;

	/// Total deposit for validators.
	///
	/// ValidatorBackings: map RelaychainAccountId => Option<ValidatorBacking>
	#[pallet::storage]
	#[pallet::getter(fn validator_backings)]
	pub type ValidatorBackings<T: Config> =
		StorageMap<_, Blake2_128Concat, T::RelaychainAccountId, ValidatorBacking, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Bond tokens to a validator on the relay chain.
		/// Ensures the amount to bond is greater than the minimum bond amount.
		///
		/// - `validator`: the AccountId of a validator on the relay chain to bond to
		/// - `amount`: the number of tokens to bond to the given validator
		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn bond(
			origin: OriginFor<T>,
			validator: T::RelaychainAccountId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let guarantor = ensure_signed(origin)?;
			let free_balance = T::LiquidTokenCurrency::free_balance(&guarantor);
			let total_should_locked = Self::total_locked_by_guarantor(&guarantor).unwrap_or_default();

			if let Some(extra) = free_balance.checked_sub(total_should_locked) {
				let amount = amount.min(extra);

				if !amount.is_zero() {
					Self::update_guarantee(&guarantor, &validator, |guarantee| -> DispatchResult {
						guarantee.total = guarantee.total.saturating_add(amount);
						guarantee.bonded = guarantee.bonded.saturating_add(amount);
						ensure!(
							guarantee.bonded >= T::MinBondAmount::get(),
							Error::<T>::BelowMinBondAmount
						);
						Ok(())
					})?;
					Self::deposit_event(Event::BondGuarantee {
						who: guarantor,
						validator: validator.clone(),
						bond: amount,
					});
				}
			}
			Ok(())
		}

		/// Unbond tokens from a validator on the relay chain.
		/// Ensures the bonded amount is zero or greater than the minimum bond amount.
		///
		/// - `validator`: the AccountId of a validator on the relay chain to unbond from
		/// - `amount`: the number of tokens to unbond from the given validator
		#[pallet::weight(T::WeightInfo::unbond())]
		#[transactional]
		pub fn unbond(
			origin: OriginFor<T>,
			validator: T::RelaychainAccountId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let guarantor = ensure_signed(origin)?;

			if !amount.is_zero() {
				Self::update_guarantee(&guarantor, &validator, |guarantee| -> DispatchResult {
					ensure!(guarantee.unbonding.is_none(), Error::<T>::UnbondingExists);
					let amount = amount.min(guarantee.bonded);
					guarantee.bonded = guarantee.bonded.saturating_sub(amount);
					ensure!(
						guarantee.bonded.is_zero() || guarantee.bonded >= T::MinBondAmount::get(),
						Error::<T>::BelowMinBondAmount,
					);
					let expired_block = T::BlockNumberProvider::current_block_number() + T::BondingDuration::get();
					guarantee.unbonding = Some((amount, expired_block));

					Self::deposit_event(Event::UnbondGuarantee {
						who: guarantor.clone(),
						validator: validator.clone(),
						bond: amount,
					});
					Ok(())
				})?;
			}
			Ok(())
		}

		/// Rebond tokens to a validator on the relay chain.
		///
		/// - `validator`: The AccountId of a validator on the relay chain to rebond to
		/// - `amount`: The amount of tokens to to rebond to the given validator
		#[pallet::weight(T::WeightInfo::rebond())]
		#[transactional]
		pub fn rebond(
			origin: OriginFor<T>,
			validator: T::RelaychainAccountId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let guarantor = ensure_signed(origin)?;

			if !amount.is_zero() {
				Self::update_guarantee(&guarantor, &validator, |guarantee| -> DispatchResult {
					*guarantee = guarantee.rebond(amount);
					Ok(())
				})?;
			}
			Ok(())
		}

		/// Withdraw the unbonded tokens from a validator on the relay chain.
		/// Ensures the validator is not frozen.
		///
		/// - `validator`: The AccountId of a validator on the relay chain to withdraw from
		#[pallet::weight(T::WeightInfo::withdraw_unbonded())]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>, validator: T::RelaychainAccountId) -> DispatchResult {
			let guarantor = ensure_signed(origin)?;
			ensure!(
				!Self::validator_backings(&validator).unwrap_or_default().is_frozen,
				Error::<T>::FrozenValidator
			);
			Self::update_guarantee(&guarantor, &validator, |guarantee| -> DispatchResult {
				let old_total = guarantee.total;
				*guarantee = guarantee.consolidate_unbonding(T::BlockNumberProvider::current_block_number());
				let new_total = guarantee
					.bonded
					.saturating_add(guarantee.unbonding.unwrap_or_default().0);
				if old_total != new_total {
					guarantee.total = new_total;
					Self::deposit_event(Event::WithdrawnGuarantee {
						who: guarantor.clone(),
						validator: validator.clone(),
						bond: old_total.saturating_sub(new_total),
					});
				}
				Ok(())
			})?;
			Ok(())
		}

		/// Freezes validators on the relay chain if they are not already frozen.
		/// Ensures the caller can freeze validators.
		///
		/// - `validators`: The AccountIds of the validators on the relay chain to freeze
		#[pallet::weight(T::WeightInfo::freeze(validators.len() as u32))]
		#[transactional]
		pub fn freeze(origin: OriginFor<T>, validators: Vec<T::RelaychainAccountId>) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;
			validators.iter().for_each(|validator| {
				ValidatorBackings::<T>::mutate_exists(validator, |maybe_validator| {
					let mut v = maybe_validator.take().unwrap_or_default();
					if !v.is_frozen {
						v.is_frozen = true;
						Self::deposit_event(Event::FreezeValidator {
							validator: validator.clone(),
						});
					}
					*maybe_validator = Some(v);
				});
			});
			Ok(())
		}

		/// Unfreezes validators on the relay chain if they are frozen.
		/// Ensures the caller can perform a slash.
		///
		/// - `validators`: The AccountIds of the validators on the relay chain to unfreeze
		#[pallet::weight(T::WeightInfo::thaw())]
		#[transactional]
		pub fn thaw(origin: OriginFor<T>, validators: Vec<T::RelaychainAccountId>) -> DispatchResult {
			// Using SlashOrigin instead of FreezeOrigin so that un-freezing requires more council members than
			// freezing
			T::SlashOrigin::ensure_origin(origin)?;
			validators.iter().for_each(|validator| {
				ValidatorBackings::<T>::mutate_exists(validator, |maybe_validator| {
					let mut v = maybe_validator.take().unwrap_or_default();
					if v.is_frozen {
						v.is_frozen = false;
						Self::deposit_event(Event::ThawValidator {
							validator: validator.clone(),
						});
					}
					*maybe_validator = Some(v);
				});
			});
			Ok(())
		}

		/// Slash validators on the relay chain.
		/// Ensures the the caller can perform a slash.
		///
		/// - `slashes`: The SlashInfos of the validators to be slashed
		#[pallet::weight(T::WeightInfo::slash())]
		#[transactional]
		pub fn slash(origin: OriginFor<T>, slashes: Vec<SlashInfo<Balance, T::RelaychainAccountId>>) -> DispatchResult {
			T::SlashOrigin::ensure_origin(origin)?;
			let liquid_staking_exchange_rate = T::LiquidStakingExchangeRateProvider::get_exchange_rate();
			let staking_liquid_exchange_rate = liquid_staking_exchange_rate.reciprocal().unwrap_or_default();
			let mut actual_total_slashing: Balance = Zero::zero();

			for SlashInfo {
				validator,
				relaychain_token_amount,
			} in slashes
			{
				let ValidatorBacking { total_insurance, .. } = Self::validator_backings(&validator).unwrap_or_default();
				let insurance_loss = staking_liquid_exchange_rate
					.saturating_mul_int(relaychain_token_amount)
					.min(total_insurance);

				for (guarantor, _) in Guarantees::<T>::iter_prefix(&validator) {
					// NOTE: ignoring result because the closure will not throw err.
					let res = Self::update_guarantee(&guarantor, &validator, |guarantee| -> DispatchResult {
						let should_slashing = Ratio::checked_from_rational(guarantee.total, total_insurance)
							.unwrap_or_else(Ratio::max_value)
							.saturating_mul_int(insurance_loss);
						let gap = T::LiquidTokenCurrency::slash(&guarantor, should_slashing);
						let actual_slashing = should_slashing.saturating_sub(gap);
						*guarantee = guarantee.slash(actual_slashing);
						Self::deposit_event(Event::SlashGuarantee {
							who: guarantor.clone(),
							validator: validator.clone(),
							bond: actual_slashing,
						});
						actual_total_slashing = actual_total_slashing.saturating_add(actual_slashing);
						Ok(())
					});
					debug_assert!(res.is_ok());
				}
			}

			T::OnSlash::happened(&actual_total_slashing);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn update_guarantee(
		guarantor: &T::AccountId,
		validator: &T::RelaychainAccountId,
		f: impl FnOnce(&mut Guarantee<T::BlockNumber>) -> DispatchResult,
	) -> DispatchResult {
		Guarantees::<T>::try_mutate_exists(validator, guarantor, |maybe_guarantee| -> DispatchResult {
			let mut guarantee = maybe_guarantee.take().unwrap_or_default();
			let old_total = guarantee.total;

			f(&mut guarantee).and_then(|_| -> DispatchResult {
				let new_total = guarantee.total;
				if guarantee.total.is_zero() {
					*maybe_guarantee = None;
				} else {
					*maybe_guarantee = Some(guarantee);
				}

				// adjust total locked of nominator, validator backing and update the lock.
				if new_total != old_total {
					TotalLockedByGuarantor::<T>::try_mutate_exists(
						guarantor,
						|maybe_total_locked| -> DispatchResult {
							let mut tl = maybe_total_locked.take().unwrap_or_default();

							ValidatorBackings::<T>::try_mutate_exists(
								validator,
								|maybe_validator_backing| -> DispatchResult {
									let mut vb = maybe_validator_backing.take().unwrap_or_default();

									if new_total > old_total {
										let gap = new_total - old_total;
										vb.total_insurance = vb.total_insurance.saturating_add(gap);
										tl = tl.saturating_add(gap);
										T::OnIncreaseGuarantee::happened(&(guarantor.clone(), validator.clone(), gap));
									} else {
										let gap = old_total - new_total;
										vb.total_insurance = vb.total_insurance.saturating_sub(gap);
										tl = tl.saturating_sub(gap);
										T::OnDecreaseGuarantee::happened(&(guarantor.clone(), validator.clone(), gap));
									};

									if tl.is_zero() {
										*maybe_total_locked = None;
										T::LiquidTokenCurrency::remove_lock(HOMA_VALIDATOR_LIST_ID, guarantor)?;
									} else {
										*maybe_total_locked = Some(tl);
										T::LiquidTokenCurrency::set_lock(HOMA_VALIDATOR_LIST_ID, guarantor, tl)?;
									}

									*maybe_validator_backing = Some(vb);
									Ok(())
								},
							)
						},
					)?;
				}

				Ok(())
			})
		})
	}
}

impl<T: Config> Contains<T::RelaychainAccountId> for Pallet<T> {
	fn contains(relaychain_account_id: &T::RelaychainAccountId) -> bool {
		Self::validator_backings(relaychain_account_id)
			.unwrap_or_default()
			.total_insurance
			>= T::ValidatorInsuranceThreshold::get()
	}
}
