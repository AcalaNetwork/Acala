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

//! Rate limit module.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::UnixTime, transactional, BoundedVec, PalletId};
use frame_system::pallet_prelude::*;
use module_support::Rate;
use orml_traits::{RateLimiter, RateLimiterError};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, Bounded, CheckedDiv, CheckedSub, MaybeSerializeDeserialize, One, Saturating,
		UniqueSaturatedInto, Zero,
	},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{prelude::*, vec::Vec};
use xcm::latest::prelude::*;

pub use module::*;
// pub use weights::WeightInfo;

mod mock;
mod tests;
pub mod weights;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub enum RateLimit {
		PerBlock {
			blocks: u64,
			limit: u128,
		},
		PerSeconds {
			seconds: u64,
			limit: u128,
		},
		TokenBucket {
			blocks: u64, // add `increment` limits per `blocks`
			max: u128,   // max limits
			increment: u128,
		},
		Unlimited,
		NotAllowed,
	}

	impl Default for RateLimit {
		fn default() -> Self {
			RateLimit::Unlimited
		}
	}

	#[derive(PartialOrd, Ord, PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub enum KeyFilter {
		Match(Vec<u8>),
		StartsWith(Vec<u8>),
		EndsWith(Vec<u8>),
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		type RateLimiterId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		#[pallet::constant]
		type MaxWhitelistFilterCount: Get<u32>;

		type UnixTime: UnixTime;

		// /// Weight information for the extrinsics in this module.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		FilterExisted,
		FilterNotExisted,
		MaxFilterExceeded,
		DecodeKeyFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		LimiRateUpdated {
			rate_limiter_id: T::RateLimiterId,
			key: Vec<u8>,
			update: Option<RateLimit>,
		},
		WhitelistFilterAdded {
			rate_limiter_id: T::RateLimiterId,
		},
		WhitelistFilterRemoved {
			rate_limiter_id: T::RateLimiterId,
		},
		WhitelistFilterReset {
			rate_limiter_id: T::RateLimiterId,
		},
	}

	#[pallet::storage]
	#[pallet::getter(fn rate_limits)]
	pub type RateLimits<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::RateLimiterId, Twox64Concat, Vec<u8>, RateLimit, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rate_limit_accumulation)]
	pub type RateLimitAccumulation<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::RateLimiterId, Twox64Concat, Vec<u8>, u128, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bypass_limit_whitelist)]
	pub type BypassLimitWhitelist<T: Config> =
		StorageMap<_, Twox64Concat, T::RateLimiterId, BoundedVec<KeyFilter, T::MaxWhitelistFilterCount>, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		#[transactional]
		pub fn update_rate_limit(
			origin: OriginFor<T>,
			rate_limiter_id: T::RateLimiterId,
			key: Vec<u8>,
			update: Option<RateLimit>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			RateLimits::<T>::mutate_exists(&rate_limiter_id, key.clone(), |maybe_limit| {
				*maybe_limit = update.clone();

				// remove RateLimitAccumulation when delete rate limit
				if maybe_limit.is_none() {
					RateLimitAccumulation::<T>::remove(&rate_limiter_id, &key);
				}

				Self::deposit_event(Event::LimiRateUpdated {
					rate_limiter_id,
					key,
					update,
				});
			});

			Ok(())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn reset_rate_limit_accumulation(
			origin: OriginFor<T>,
			rate_limiter_id: T::RateLimiterId,
			key: Vec<u8>,
			amount: u128,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			RateLimitAccumulation::<T>::insert(rate_limiter_id, key, amount);

			Ok(())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn add_whitelist(
			origin: OriginFor<T>,
			rate_limiter_id: T::RateLimiterId,
			key_filter: KeyFilter,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			BypassLimitWhitelist::<T>::try_mutate(rate_limiter_id, |whitelist| -> DispatchResult {
				let location = whitelist
					.binary_search(&key_filter)
					.err()
					.ok_or(Error::<T>::FilterExisted)?;
				whitelist
					.try_insert(location, key_filter)
					.map_err(|_| Error::<T>::MaxFilterExceeded)?;

				Self::deposit_event(Event::WhitelistFilterAdded { rate_limiter_id });
				Ok(())
			})
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn remove_whitelist(
			origin: OriginFor<T>,
			rate_limiter_id: T::RateLimiterId,
			key_filter: KeyFilter,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			BypassLimitWhitelist::<T>::try_mutate(rate_limiter_id, |whitelist| -> DispatchResult {
				let location = whitelist
					.binary_search(&key_filter)
					.ok()
					.ok_or(Error::<T>::FilterExisted)?;
				whitelist.remove(location);

				Self::deposit_event(Event::WhitelistFilterRemoved { rate_limiter_id });
				Ok(())
			})
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn reset_whitelist(
			origin: OriginFor<T>,
			rate_limiter_id: T::RateLimiterId,
			new_list: Vec<KeyFilter>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			let mut whitelist: BoundedVec<KeyFilter, T::MaxWhitelistFilterCount> =
				BoundedVec::try_from(new_list).map_err(|_| Error::<T>::MaxFilterExceeded)?;
			whitelist.sort();
			BypassLimitWhitelist::<T>::insert(rate_limiter_id, whitelist);

			Self::deposit_event(Event::WhitelistFilterReset { rate_limiter_id });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn is_under_limit(limit: RateLimit, value: u128, accumulate: u128) -> bool {
			match limit {
				RateLimit::PerBlock { blocks, limit } => {
					let current_block = frame_system::Pallet::<T>::block_number();
					// todo: caculate formular
					false
				}
				RateLimit::PerSeconds { seconds, limit } => {
					let now: u64 = T::UnixTime::now().as_secs();
					// todo: caculate formular
					false
				}
				RateLimit::TokenBucket { blocks, max, increment } => {
					let current_block = frame_system::Pallet::<T>::block_number();
					// todo: caculate formular
					false
				}
				RateLimit::Unlimited => true,
				RateLimit::NotAllowed => false,
			}
		}
	}

	impl<T: Config> RateLimiter for Pallet<T> {
		type RateLimiterId = T::RateLimiterId;

		fn bypass_limit(limiter_id: Self::RateLimiterId, key: impl Encode) -> bool {
			let encode_key: Vec<u8> = key.encode();

			for key_filter in BypassLimitWhitelist::<T>::get(limiter_id) {
				match key_filter {
					KeyFilter::Match(vec) => {
						if encode_key == vec {
							return true;
						}
					}
					KeyFilter::StartsWith(prefix) => {
						if encode_key.starts_with(&prefix) {
							return true;
						}
					}
					KeyFilter::EndsWith(postfix) => {
						if encode_key.ends_with(&postfix) {
							return true;
						}
					}
				}
			}

			false
		}

		fn is_allowed(limiter_id: Self::RateLimiterId, key: impl Encode, value: u128) -> Result<(), RateLimiterError> {
			let encode_key: Vec<u8> = key.encode();

			let allowed = match RateLimits::<T>::get(&limiter_id, &encode_key) {
				Some(limit) => {
					let accumulation = RateLimitAccumulation::<T>::get(&limiter_id, &encode_key);
					Self::is_under_limit(limit, value, accumulation)
				}
				_ => {
					// if not defined limit for key, allow it.
					true
				}
			};

			ensure!(allowed, RateLimiterError::ExceedLimit);
			Ok(())
		}

		fn record(limiter_id: Self::RateLimiterId, key: impl Encode, value: u128) {
			let encode_key: Vec<u8> = key.encode();

			// only accumulate when rate limit is configured.
			if RateLimits::<T>::get(&limiter_id, &encode_key).is_some() {
				RateLimitAccumulation::<T>::mutate(&limiter_id, &encode_key, |acc| *acc = acc.saturating_add(value));
			}
		}
	}
}
