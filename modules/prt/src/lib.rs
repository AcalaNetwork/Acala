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

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{BlockNumberProvider, Saturating, Zero};

use orml_traits::{ManageNFT, MultiCurrencyExtended, MultiReservableCurrency, NFT};

use module_support::GiltXcm;
use primitives::{
	nft::{Attributes, CID},
	Balance, CurrencyId,
};

// mod mock;
// mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type ActiveIndex = u32;
	pub type ClassIdOf<T> = <T as orml_nft::Config>::ClassId;

	#[derive(Encode, Decode, Clone, Default, Debug, Eq, PartialEq)]
	pub struct PrtMetadata<T: Config> {
		pub index: ActiveIndex,
		pub expiry: T::BlockNumber,
		pub amount: Balance,
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

		type NFTInterface: ManageNFT<Self::AccountId, CID, Attributes, ClassId = Self::ClassId> + NFT<Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The amount of relaychain currency to be bid is too low.
		BidAmountBelowMinimum,
		/// Too many bids with the same Amount and Duration in the current queue.
		BidNotFound,
		/// The user does not have enough Relaychain Currency.
		InsufficientBalance,
		/// The Prt's NFT class ID as not yet been set.
		PrtClassIdNotSet,
		/// The PRT is already issued to the user.
		PrtAlreadyIssued,
		/// Insufficient amount of relaychain currency placed in bids.
		InsufficientBidAmount,
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
		/// PRT is issued
		PrtIssued {
			who: T::AccountId,
			active_index: ActiveIndex,
			expiry: T::BlockNumber,
			amount: Balance,
		},
		PrtIssueConfirmed {
			duration: u32,
			amount: Balance,
			index: ActiveIndex,
			expiry: T::BlockNumber,
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
		StorageMap<_, Twox64Concat, ActiveIndex, (T::BlockNumber, Vec<(T::AccountId, Balance)>), OptionQuery>;

	/// Stores bids for Gilt tokens on the Relaychain.
	#[pallet::storage]
	#[pallet::getter(fn placed_bids)]
	type PlacedBids<T: Config> = StorageMap<_, Identity, u32, Vec<(T::AccountId, Balance)>, ValueQuery>;

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

			// Put the user's bid into storage
			PlacedBids::<T>::mutate(duration, |bids_in_storage| {
				let maybe_position = bids_in_storage.iter().position(|(user, _)| *user == who);
				match maybe_position {
					// Add to the user's existing bid.
					Some(i) => bids_in_storage[i].1 = bids_in_storage[i].1.saturating_add(amount),
					// Insert the user's bid to index 0.
					None => bids_in_storage.insert(0, (who.clone(), amount)),
				}
			});

			Self::deposit_event(Event::BidPlaced { who, duration, amount });
			Ok(())
		}

		/// This should be called only by oracles to confirm when bid has been accepted and Gilts'
		/// been minted on the relaychain. This function will then mint as much PRT as allowed.
		/// This is the get around the async nature of cross-chain communications.
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
			let prt_class_id = Self::prt_class_id();

			// Ensure PRT's class ID has been set.
			ensure!(prt_class_id.is_some(), Error::<T>::PrtClassIdNotSet);

			// Ensure we do not double-issue
			ensure!(Self::issued_prt(index).is_none(), Error::<T>::PrtAlreadyIssued);

			// Consume bids in order and mint the NFT that represents the PRT.
			PlacedBids::<T>::try_mutate_exists(duration, |maybe_current_bids| -> DispatchResult {
				let mut current_bids = maybe_current_bids.take().unwrap_or_default();
				let mut issue_amount_remaining = amount;
				let mut issued_prt = vec![];
				while let Some((bidder, mut bid_amount)) = current_bids.pop() {
					// Deduct minted amount from Total and user's bid.
					if issue_amount_remaining < bid_amount {
						let bid_amount_remaining = bid_amount.saturating_sub(issue_amount_remaining);
						bid_amount = issue_amount_remaining;
						current_bids.push((bidder.clone(), bid_amount_remaining));
					}

					issue_amount_remaining = issue_amount_remaining.saturating_sub(bid_amount);

					issued_prt.push((bidder.clone(), bid_amount));

					// Mint `bid_amount` amount of PRT into the user's account.
					let metadata = PrtMetadata::<T> {
						index,
						expiry,
						amount: bid_amount,
					}
					.encode();
					T::NFTInterface::mint(
						T::PalletAccount::get(),
						bidder.clone(),
						prt_class_id.unwrap(),
						metadata,
						Default::default(),
						1u32,
					)?;

					Self::deposit_event(Event::PrtIssued {
						who: bidder.clone(),
						active_index: index,
						expiry,
						amount: bid_amount,
					});

					// Break if we run out of PRT to issue.
					if issue_amount_remaining.is_zero() {
						break;
					}
				}

				// Put the updated bids into storage.
				*maybe_current_bids = match current_bids.len() {
					0 => None,
					_ => Some(current_bids),
				};

				// Insert the issued PRT into storage to prevent double-minting.
				IssuedPrt::<T>::insert(index, (expiry, issued_prt));

				Ok(())
			})?;

			Self::deposit_event(Event::<T>::PrtIssueConfirmed {
				duration,
				amount,
				index,
				expiry,
			});
			Ok(())
		}

		/*
		/// Sends a request to the relaychain to retract the bid for Gilts. The relaychain tokens
		/// stays reserved until the relaychain confirms that the bid is successfully retracted.
		#[pallet::weight(0)]
		#[transactional]
		pub fn retract_bid(origin: OriginFor<T>, #[pallet::compact] amount: Balance, duration: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Consume bids in order and mint the NFT that represents the PRT.
			PlacedBids::<T>::try_mutate_exists(duration, |maybe_current_bids| -> DispatchResult {
				let mut current_bids = maybe_current_bids.take().unwrap_or_default();
				let maybe_position = current_bids.iter().position(|(_, bid_amount)| *bid_amount == amount );
				match maybe_position {
					Some(i) => {
						// Deduct amount from the user's existing bid.
						bids_in_storage[i].1 = bids_in_storage[i].1.saturating_add(amount);
						Ok(())
					},
					None => {
						// Append the user's bid to the back of the queue.
						//ensure!(bids_in_storage.len() <= T::MaxBidsPerDuration::get() as usize, Error::<T>::MaxBidsPerDurationExceeded);
						bids_in_storage.try_insert(bids_in_storage.len(), (who.clone(), amount)).map_err(|_|Error::<T>::MaxBidsPerDurationExceeded)?;
						Ok(())
					},
				}
			})?;
			// Only bids placed from this module can be retracted here.
			// This is to ensure the consistency of reserved assets.
			ensure!(
				Self::placed_bids(&who, duration) >= amount,
				Error::<T>::InsufficientBidAmount
			);

			// Retract this bid on relaychain via XCM
			T::RelaychainInterface::gilt_retract_bid(amount, duration)?;

			Self::deposit_event(Event::RetractBidRequested { who, duration, amount });
			Ok(())
		}


		/// Confirm that a specific user's bid on Gilt has been retracted on the relaychain.
		/// Only Callable by authorised oracles origin.
		/// This is the get around the async nature of cross-chain communications.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_bid_retracted(
			origin: OriginFor<T>,
			user: T::AccountId,
			duration: u32,
			amount: Balance,
		) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			PlacedBids::<T>::mutate_exists(&user, duration, |current| {
				let current_amount = current.unwrap_or_default();
				let actual = min(current_amount, amount);
				let remaining = current_amount.saturating_sub(actual);
				*current = if remaining.is_zero() { None } else { Some(remaining) };

				// Unreserve user's relaychain currency
				let unreserved = T::Currency::unreserve(T::RelaychainCurrency::get(), &user, actual);
				ensure!(unreserved >= actual, Error::<T>::InsufficientBalance);

				//deposit event
				Self::deposit_event(Event::BidRetracted {
					who: user,
					duration,
					amount: actual,
				});
				Ok(())
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
			ensure!(Self::prt_class_id().is_some(), Error::<T>::PrtClassIdNotSet);

			// Ensure the PRT exists.
			let prt_issued = Self::issued_gilt(index);
			ensure!(prt_issued.is_some(), Error::<T>::PrtNotIssued);
			let Some((owner, expiry, amount)) = prt_issued; // Guanranteed to be Some()
			ensure!(owner == who, Error::<T>::CallerUnauthorized);
			ensure!(
				T::RelayChainBlockNumber::current_block_number() >= expiry,
				Error::<T>::PrtNotExpired
			);

			// Send the XCM to the relaychain to request thaw.
			T::RelaychainInterface::gilt_thaw(index)?;

			Self::deposit_event(Event::ThawRequested { index, who, amount });
			Ok(())
		}
		*/
		// confirm thaw
	}
}

impl<T: Config> Pallet<T> {
	pub fn encode_prt_metadata(index: ActiveIndex, expiry: T::BlockNumber, amount: Balance) -> Vec<u8> {
		let mut encoded = vec![];
		encoded.append(&mut index.encode());
		encoded.append(&mut expiry.encode());
		encoded.append(&mut amount.encode());

		encoded
	}
}
