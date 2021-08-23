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

//! # Chainlink Adaptor Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unused_unit)]
#![allow(clippy::collapsible_if)]

use frame_support::{pallet_prelude::*, traits::Time, transactional};
use frame_system::pallet_prelude::*;
use orml_oracle::TimestampedValue;
use orml_traits::{DataProvider, DataProviderExtended};
use pallet_chainlink_feed::traits::OnAnswerHandler;
use pallet_chainlink_feed::{FeedInterface, FeedOracle, RoundData};
use primitives::CurrencyId;
use sp_runtime::traits::Convert;
use sp_std::prelude::*;
use support::Price;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type FeedIdOf<T> = <T as pallet_chainlink_feed::Config>::FeedId;
	pub type FeedValueOf<T> = <T as pallet_chainlink_feed::Config>::Value;
	pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_chainlink_feed::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Convert feed_value type of chainlink to price type
		type Convert: Convert<FeedValueOf<Self>, Option<Price>>;

		/// Time provider
		type Time: Time;

		/// The origin which can map feed_id of chainlink oracle to currency_id.
		type RegistorOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// CurrencyId has been mapped to FeedId already.
		AlreadyMapped,
		/// FeedId is invalid.
		InvalidFeedId,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Map feed_id to currency_id. \[feed_id, currency_id\]
		MapFeedId(FeedIdOf<T>, CurrencyId),
		/// Unmap feed_id with currency_id. \[feed_id, currency_id\]
		UnmapFeedId(FeedIdOf<T>, CurrencyId),
	}

	/// Mapping from currency_id to feed_id
	///
	/// FeedIdMapping: CurrencyId => FeedId
	#[pallet::storage]
	#[pallet::getter(fn feed_id_mapping)]
	pub type FeedIdMapping<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, FeedIdOf<T>, OptionQuery>;

	/// Mapping from feed_id to currency_id
	///
	/// CurrencyIdMapping: FeedId => CurrencyId
	#[pallet::storage]
	#[pallet::getter(fn currency_id_mapping)]
	pub type CurrencyIdMapping<T: Config> = StorageMap<_, Twox64Concat, FeedIdOf<T>, CurrencyId, OptionQuery>;

	/// Records last updated timestamp for FeedId
	///
	/// LastUpdatedTimestamp: FeedId => Moment
	#[pallet::storage]
	#[pallet::getter(fn last_updated_timestamp)]
	pub type LastUpdatedTimestamp<T: Config> = StorageMap<_, Twox64Concat, FeedIdOf<T>, MomentOf<T>, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Map currency_id to feed_id of chainlink oracle
		///
		/// The dispatch origin of this call must be `RegistorOrigin`.
		///
		/// - `feed_id`: feed_id in chainlink oracle.
		/// - `currency_id`: currency_id.
		#[pallet::weight(<T as Config>::WeightInfo::map_feed_id())]
		#[transactional]
		pub fn map_feed_id(
			origin: OriginFor<T>,
			feed_id: FeedIdOf<T>,
			currency_id: CurrencyId,
		) -> DispatchResultWithPostInfo {
			T::RegistorOrigin::ensure_origin(origin)?;
			ensure!(
				!FeedIdMapping::<T>::contains_key(currency_id) && !CurrencyIdMapping::<T>::contains_key(feed_id),
				Error::<T>::AlreadyMapped,
			);
			ensure!(
				pallet_chainlink_feed::Feeds::<T>::get(feed_id).is_some(),
				Error::<T>::InvalidFeedId,
			);

			FeedIdMapping::<T>::insert(currency_id, feed_id);
			CurrencyIdMapping::<T>::insert(feed_id, currency_id);
			Self::deposit_event(Event::MapFeedId(feed_id, currency_id));
			Ok(().into())
		}

		/// Unmap feed_id with currency_id.
		///
		/// The dispatch origin of this call must be `RegistorOrigin`.
		///
		/// - `currency_id`: currency_id.
		#[pallet::weight(<T as Config>::WeightInfo::unmap_feed_id())]
		#[transactional]
		pub fn unmap_feed_id(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResultWithPostInfo {
			T::RegistorOrigin::ensure_origin(origin)?;
			if let Some(feed_id) = FeedIdMapping::<T>::take(currency_id) {
				CurrencyIdMapping::<T>::remove(feed_id);
				LastUpdatedTimestamp::<T>::remove(feed_id);
				Self::deposit_event(Event::UnmapFeedId(feed_id, currency_id));
			}
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn get_price_from_chainlink_feed(currency_id: &CurrencyId) -> Option<Price> {
		Self::feed_id_mapping(currency_id)
			.and_then(<pallet_chainlink_feed::Pallet<T>>::feed)
			.map(|feed| feed.latest_data().answer)
			.and_then(T::Convert::convert)
	}
}

impl<T: Config> OnAnswerHandler<T> for Pallet<T> {
	fn on_answer(feed_id: FeedIdOf<T>, _new_data: RoundData<T::BlockNumber, FeedValueOf<T>>) {
		if CurrencyIdMapping::<T>::contains_key(feed_id) {
			LastUpdatedTimestamp::<T>::insert(feed_id, T::Time::now());
		}
	}
}

impl<T: Config> DataProvider<CurrencyId, Price> for Pallet<T> {
	fn get(key: &CurrencyId) -> Option<Price> {
		Self::get_price_from_chainlink_feed(key)
	}
}

impl<T: Config> DataProviderExtended<CurrencyId, TimestampedValue<Price, MomentOf<T>>> for Pallet<T> {
	fn get_no_op(key: &CurrencyId) -> Option<TimestampedValue<Price, MomentOf<T>>> {
		Self::get_price_from_chainlink_feed(key).map(|price| TimestampedValue {
			value: price,
			timestamp: Self::feed_id_mapping(key)
				.map(Self::last_updated_timestamp)
				.unwrap_or_default(),
		})
	}

	fn get_all_values() -> Vec<(CurrencyId, Option<TimestampedValue<Price, MomentOf<T>>>)> {
		FeedIdMapping::<T>::iter()
			.map(|(currency_id, _)| {
				let maybe_price = Self::get_no_op(&currency_id);
				(currency_id, maybe_price)
			})
			.collect()
	}
}
