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

//! # Perpetual Relaychain Token (PRT) Module
//!
//! This module interfaces with the Gilt module in substrate (substrate/frame/pallet-gilt).
//! TThe user can place bid, retract bid, issue and thaw Gilts issued on the relaychain via the use
//! of XCM.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use scale_info::TypeInfo;

use frame_support::{
	pallet_prelude::*,
	traits::tokens::nonfungibles::{Inspect, Mutate},
	transactional,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{BlockNumberProvider, Zero};

use orml_traits::{MultiCurrencyExtended, MultiReservableCurrency, NFT};

use module_support::GiltXcm;
use primitives::{Balance, CurrencyId};

// mod mock;
// mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type ActiveIndex = u32;
	pub type ClassIdOf<T> = <T as orml_nft::Config>::ClassId;

	#[derive(Encode, Decode, Clone, Default, Debug, Eq, PartialEq, TypeInfo)]
	pub struct PrtMetadata<BlockNumber, TokenId> {
		pub expiry: BlockNumber,
		pub amount: Balance,
		pub token_id: TokenId,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_nft::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency ID used to buy Gilts on the Relaychain.
		#[pallet::constant]
		type RelaychainCurrency: Get<CurrencyId>;

		/// The NFT's module id
		#[pallet::constant]
		type PalletAccount: Get<Self::AccountId>;

		/// Minimum amount of relaychian currency allowed to bid.
		#[pallet::constant]
		type MinimumBidAmount: Get<Balance>;

		/// Multi-currency support for asset management.
		type Currency: MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>
			+ MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The RelaychainInterface to communicate with the relaychain via XCM.
		type RelaychainInterface: GiltXcm<Balance>;

		/// Block number provider for the relaychain.
		type RelayChainBlockNumber: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// Origin used by Oracles. Used to confirm operations on the Relaychain.
		type OracleOrigin: EnsureOrigin<Self::Origin>;

		type NFTInterface: Inspect<Self::AccountId, ClassId = Self::ClassId, InstanceId = Self::TokenId>
			+ Mutate<Self::AccountId>
			+ NFT<Self::AccountId, ClassId = Self::ClassId, TokenId = Self::TokenId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The amount of relaychain currency to be bid is too low.
		BidAmountBelowMinimum,
		/// The specific bid was not found.
		BidNotFound,
		/// The user does not have enough Relaychain Currency.
		InsufficientBalance,
		/// The Prt's NFT class ID as not yet been set.
		PrtClassIdNotSet,
		/// The PRT is already issued to the user.
		PrtAlreadyIssued,
		/// The specific PRT was not issued.
		PrtNotIssued,
		/// The PRT token has not expired yet.
		PrtNotExpired,
		/// The caller is unauthorized to make this transaction
		CallerUnauthorized,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// The class ID of the PRT has been set
		PrtClassIdSet { class_id: ClassIdOf<T> },
		/// A bid to mint PRT is placed. Duration is in number of Periods.
		BidPlaced {
			who: T::AccountId,
			duration: u32,
			amount: Balance,
		},
		/// User requested to retract the Gilt bid.
		RetractBidRequested {
			who: T::AccountId,
			duration: u32,
			amount: Balance,
		},
		/// a bid to mint PRT is retracted.
		BidRetracted {
			who: T::AccountId,
			duration: u32,
			amount: Balance,
		},
		/// The Gilt has already been minted, therefore the Retraction is cancelled.
		BidRetractionCancelled {
			who: T::AccountId,
			duration: u32,
			amount: Balance,
		},
		/// PRT is issued
		PrtIssued {
			who: T::AccountId,
			active_index: ActiveIndex,
			expiry: T::BlockNumber,
			amount: Balance,
			nft_id: T::TokenId,
		},
		/// Request to thaw PRT
		ThawRequested {
			index: ActiveIndex,
			who: T::AccountId,
			amount: Balance,
		},
		/// PRT is traded in and Relaychain currency thawed.
		PrtThawed {
			who: T::AccountId,
			active_index: ActiveIndex,
			amount: Balance,
		},
	}

	/// Stores the NFT's class ID. Settable by authorized Oracle. Used to mint and burn PRTs.
	#[pallet::storage]
	#[pallet::getter(fn prt_class_id)]
	type PrtClassId<T: Config> = StorageValue<_, ClassIdOf<T>, OptionQuery>;

	/// Stores confirmed Gilt tokens that are issued on the Relaychain.
	#[pallet::storage]
	#[pallet::getter(fn issued_prt)]
	type IssuedPrt<T: Config> =
		StorageMap<_, Twox64Concat, ActiveIndex, PrtMetadata<T::BlockNumber, T::TokenId>, OptionQuery>;

	/// Stores bids for Gilt tokens on the Relaychain.
	#[pallet::storage]
	#[pallet::getter(fn placed_bids)]
	type PlacedBids<T: Config> = StorageDoubleMap<_, Identity, u32, Identity, Balance, Vec<T::AccountId>, ValueQuery>;

	/// Stores pending bids that are being retracted.
	#[pallet::storage]
	#[pallet::getter(fn retracted_bids)]
	type RetractedBids<T: Config> =
		StorageDoubleMap<_, Identity, u32, Identity, Balance, Vec<T::AccountId>, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the class ID of the NFT that will be representing PRT.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_nft_id(origin: OriginFor<T>, nft_id: ClassIdOf<T>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			PrtClassId::<T>::put(nft_id.clone());
			Self::deposit_event(Event::<T>::PrtClassIdSet { class_id: nft_id });
			Ok(())
		}

		/// Sends a request to the relaychain to place a bid to freeze some Relaychain currency to
		/// mint some Gilts. The relaychain tokens are reserved, but no PRT will be minted until the
		/// relaychain confirms that the bid is accepted and Gilts issued.
		#[pallet::weight(0)]
		#[transactional]
		pub fn place_bid(origin: OriginFor<T>, #[pallet::compact] amount: Balance, duration: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Ensure PRT's class ID has been set.
			ensure!(Self::prt_class_id().is_some(), Error::<T>::PrtClassIdNotSet);
			// Ensure the bid amount is above minimum.
			ensure!(amount >= T::MinimumBidAmount::get(), Error::<T>::BidAmountBelowMinimum);

			// Ensure user has enough funds.
			ensure!(
				T::Currency::can_reserve(T::RelaychainCurrency::get(), &who, amount),
				Error::<T>::InsufficientBalance
			);

			// Reserve relaychain currency needed for this bid
			T::Currency::reserve(T::RelaychainCurrency::get(), &who, amount)?;

			// Place this bid on relaychain via XCM
			T::RelaychainInterface::gilt_place_bid(amount, duration)?;

			// Put the user's bid into a queue.
			PlacedBids::<T>::mutate(duration, amount, |bidders| {
				// FIFO: last ... first, push from the front
				bidders.insert(0, who.clone())
			});

			Self::deposit_event(Event::BidPlaced { who, duration, amount });
			Ok(())
		}

		/// Sends a request to the relaychain to retract the bid for Gilts. The bid is moved from
		/// PlacedBids to RetractedBids. The relaychain tokens stays reserved until the relaychain
		/// confirms that the bid is successfully retracted.
		#[pallet::weight(0)]
		#[transactional]
		pub fn retract_bid(origin: OriginFor<T>, #[pallet::compact] amount: Balance, duration: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Remove the bids from `PlacedBids`.
			PlacedBids::<T>::try_mutate(duration, amount, |bidders| -> DispatchResult {
				let position = bidders
					.iter()
					.position(|bidder| *bidder == who)
					.ok_or(Error::<T>::BidNotFound)?;
				bidders.remove(position);

				Ok(())
			})?;

			RetractedBids::<T>::mutate(duration, amount, |bidders| {
				// Add the bid to the `RetractedBid` list.
				bidders.insert(0, who.clone());
			});

			// Retract this bid on relaychain via XCM
			T::RelaychainInterface::gilt_retract_bid(amount, duration)?;

			Self::deposit_event(Event::RetractBidRequested { who, duration, amount });
			Ok(())
		}

		/// Confirm that a specific user's bid on Gilt has been retracted on the relaychain.
		/// Only Callable by authorized oracles origin.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_bid_retracted(origin: OriginFor<T>, duration: u32, amount: Balance) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			// Remove the bid from the `RetractedBid` list.
			RetractedBids::<T>::try_mutate(duration, amount, |bidders| -> DispatchResult {
				// Ensure that the bid exists in storage
				let bidder = bidders.pop().ok_or(Error::<T>::BidNotFound)?;

				// Unreserve user's relaychain currency
				let remaining = T::Currency::unreserve(T::RelaychainCurrency::get(), &bidder, amount);
				ensure!(remaining.is_zero(), Error::<T>::InsufficientBalance);

				//deposit event
				Self::deposit_event(Event::BidRetracted {
					who: bidder,
					duration,
					amount,
				});
				Ok(())
			})?;
			Ok(())
		}

		/// Called by oracles to confirm when bids has been accepted and Gilts minted on the
		/// relaychain. If a bid is matched in `PlacedBids`, the bid is resolved.
		/// Otherwise attempt to match bids in `RetractedBids`. Retracted bids are resolved and
		/// cancelled. Once a bid is matched, an appropriate PRT is issued, and NFT minted to the
		/// bidder's account.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_gilt_issued(
			origin: OriginFor<T>,
			duration: u32,
			#[pallet::compact] amount: Balance,
			index: ActiveIndex,
			expiry: T::BlockNumber,
		) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			let prt_class_id = Self::prt_class_id().ok_or(Error::<T>::PrtClassIdNotSet)?;

			// Ensure we do not double-issue
			ensure!(Self::issued_prt(index).is_none(), Error::<T>::PrtAlreadyIssued);

			// Try to find the bid in `PlacedBids`
			let maybe_bidder =
				PlacedBids::<T>::mutate(duration, amount, |bidders| -> Option<T::AccountId> { bidders.pop() });

			// If no bids are found, try match from `RetractedBids`. Throw an error if no bids are matched.
			let bidder = match maybe_bidder {
				Some(account) => account,
				None => {
					let bidder = RetractedBids::<T>::mutate(duration, amount, |bidders| -> Option<T::AccountId> {
						bidders.pop()
					})
					.ok_or(Error::<T>::BidNotFound)?;

					// If a bid is matched here, cancel the retraction.
					Self::deposit_event(Event::<T>::BidRetractionCancelled {
						who: bidder.clone(),
						duration,
						amount,
					});
					bidder
				}
			};

			let token_id = T::NFTInterface::next_token_id(prt_class_id);
			T::NFTInterface::mint_into(&prt_class_id, &token_id, &bidder)?;

			// Create a record of the PRT and insert it into storage
			IssuedPrt::<T>::insert(
				index,
				PrtMetadata {
					expiry,
					amount,
					token_id,
				},
			);

			Self::deposit_event(Event::PrtIssued {
				who: bidder.clone(),
				active_index: index,
				expiry,
				amount,
				nft_id: token_id,
			});
			Ok(())
		}

		/// Sends a request to the relaychain to thaw frozen Relaychain currency and consumes the
		/// PRT/minted Gilts. The user's PRT must have already expired.
		///
		/// The PRT will not be thawed until it is confirmed by the Relaychain.
		#[pallet::weight(0)]
		#[transactional]
		pub fn request_thaw(origin: OriginFor<T>, index: ActiveIndex) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Ensure PRT's class ID has been set.
			let _ = Self::prt_class_id().ok_or(Error::<T>::PrtClassIdNotSet)?;

			// Ensure the PRT exists.
			let prt_issued = Self::issued_prt(index).ok_or(Error::<T>::PrtNotIssued)?;

			// Ensure the caller is the owner of the PRT.
			let owner = T::NFTInterface::owner(&Self::prt_class_id().unwrap(), &prt_issued.token_id)
				.ok_or(Error::<T>::PrtNotIssued)?;
			ensure!(owner == who, Error::<T>::CallerUnauthorized);

			// Ensure the PRT has expired.
			ensure!(
				T::RelayChainBlockNumber::current_block_number() >= prt_issued.expiry,
				Error::<T>::PrtNotExpired
			);

			// Send the XCM to the relaychain to request thaw.
			T::RelaychainInterface::gilt_thaw(index)?;

			Self::deposit_event(Event::ThawRequested {
				index,
				who,
				amount: prt_issued.amount,
			});
			Ok(())
		}

		/// Called by authorized oracle to confirm that some Gilts has been thawed.
		/// The PRT NFT is burned, and the user's frozen Relaychain currency is unreserved.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_thaw(origin: OriginFor<T>, index: ActiveIndex) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			// Ensure PRT's class ID has been set.
			let prt_class_id = Self::prt_class_id().ok_or(Error::<T>::PrtClassIdNotSet)?;

			// Ensure the PRT exists.
			let prt_issued = Self::issued_prt(index).ok_or(Error::<T>::PrtNotIssued)?;
			let owner = T::NFTInterface::owner(&Self::prt_class_id().unwrap(), &prt_issued.token_id)
				.ok_or(Error::<T>::PrtNotIssued)?;

			// Find the NFT and burn it
			T::NFTInterface::burn_from(&prt_class_id, &prt_issued.token_id)?;

			// Unreserve the user's locked relaychain currencies.
			let remaining = T::Currency::unreserve(T::RelaychainCurrency::get(), &owner, prt_issued.amount);
			ensure!(remaining.is_zero(), Error::<T>::InsufficientBalance);

			Self::deposit_event(Event::PrtThawed {
				who: owner,
				active_index: index,
				amount: prt_issued.amount,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
