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
use sp_runtime::traits::BlockNumberProvider;

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

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_nft::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency ID used to buy Gilts on the Relaychain.
		#[pallet::constant]
		type RelaychainCurrency: Get<CurrencyId>;

		/// The NFT's module id
		#[pallet::constant]
		type PalletAccount: Get<Self::AccountId>;

		/// Multi-currency support for asset management.
		type Currency: MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>
			+ MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The RelaychainInterface to communicate with the relaychain via XCM.
		type RelaychainInterface: GiltXcm<Balance>;

		/// Block number provider for the relaychain.
		type RelayChainBlockNumber: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// Origin used by Oracles. Used to confirm operations on the Relaychain.
		type OracleOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		type NFTInterface: ManageNFT<Self::AccountId, CID, Attributes, ClassId = Self::ClassId> + NFT<Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The user does not have enough Relaychain Currency.
		InsufficientBalance,
		/// The Prt's NFT class ID as not yet been set.
		PrtClassIdNotSet,
		/// The PRT is already issued to the user.
		PrtAlreadyIssued,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// The class ID of the PRT has been set
		PrtClassIdSet { class_id: ClassIdOf<T> },
		/// A bid to mint PRT is placed. Duration is in number of Periods.
		BidPlaced {
			who: T::AccountId,
			amount: Balance,
			duration: u32,
		},
		/// User requested to retract the Gilt bid.
		RetractBidRequested {
			who: T::AccountId,
			amount: Balance,
			duration: u32,
		},
		/// a bid to mint PRT is retracted.
		BidRetracted {
			who: T::AccountId,
			amount: Balance,
			duration: u32,
		},
		/// PRT is issued
		PrtIssued {
			who: T::AccountId,
			relaychain_currency_amount: Balance,
			prt_amount: Balance,
			expiry: T::BlockNumber,
			active_index: ActiveIndex,
		},
		/// Request to thaw PRT
		ThawRequested {
			who: T::AccountId,
			amount: Balance,
			duration: u32,
		},
		/// PRT is traded in and Relaychain currency thawed.
		PrtThawed {
			who: T::AccountId,
			relaychain_currency_amount: Balance,
			prt_amount: Balance,
		},
	}

	/// Stores the NFT's class ID. Settable by authorized Oracle. Used to mint and burn PRTs.
	#[pallet::storage]
	#[pallet::getter(fn prt_class_id)]
	type PrtClassId<T: Config> = StorageValue<_, ClassIdOf<T>, OptionQuery>;

	/// Stores confirmed Gilt tokens that are issued on the Relaychain.
	#[pallet::storage]
	#[pallet::getter(fn issued_gilt)]
	type IssuedGilt<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		ActiveIndex,
		(T::BlockNumber, Balance),
		OptionQuery,
	>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn set_nft_id(origin: OriginFor<T>, nft_id: ClassIdOf<T>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			PrtClassId::<T>::put(nft_id.clone());
			Self::deposit_event(Event::<T>::PrtClassIdSet { class_id: nft_id });
			Ok(())
		}

		/// Sends a request to the relaychain to place a bid to freeze some Relaychain currency to
		/// mint some Gilts. The relaychain tokens are reserved, but no PRT will be minted until the
		/// relaychian confirms that the bid is accepted and Gilts issued.
		#[pallet::weight(0)]
		#[transactional]
		pub fn place_bid(origin: OriginFor<T>, #[pallet::compact] amount: Balance, duration: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Ensure PRT's class ID has been set.
			ensure!(Self::prt_class_id().is_some(), Error::<T>::PrtClassIdNotSet);

			// Ensure user has enough funds.
			ensure!(
				T::Currency::can_reserve(T::RelaychainCurrency::get(), &who, amount),
				Error::<T>::InsufficientBalance
			);

			// Reserve DOT & reserve this bid
			T::Currency::reserve(T::RelaychainCurrency::get(), &who, amount)?;

			// Place this bid on relaychain via XCM
			T::RelaychainInterface::gilt_place_bid(amount, duration)?;

			Self::deposit_event(Event::BidPlaced { who, amount, duration });

			Ok(())
		}

		/// This should be called only by oracles to confirm that a specific user's Gilt has been
		/// successfully minted on the relaychain.
		/// This is the get around the async nature of cross-chain communications.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_gilt_issued(
			origin: OriginFor<T>,
			user: T::AccountId,
			index: ActiveIndex,
			expiry: T::BlockNumber,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			let prt_class_id = Self::prt_class_id();
			// Ensure PRT's class ID has been set.
			ensure!(prt_class_id.is_some(), Error::<T>::PrtClassIdNotSet);

			// Put the Gilt record into storage to prevent double-minting
			ensure!(
				Self::issued_gilt(user.clone(), index).is_none(),
				Error::<T>::PrtAlreadyIssued
			);
			IssuedGilt::<T>::insert(user.clone(), index, (expiry.clone(), amount));

			// Mint PRT into the user's account.
			let metadata = Self::encode_prt_metadata(index, expiry.clone(), amount);
			T::NFTInterface::mint(
				T::PalletAccount::get(),
				user.clone(),
				prt_class_id.unwrap(),
				metadata,
				Default::default(),
				1u32,
			)?;

			Ok(())
		}

		/// Sends a request to the relaychain to retract the bid for Gilts. The relaychain tokens
		/// stays reserved until the relaychain confirms that the bid is successfully retracted.
		#[pallet::weight(0)]
		#[transactional]
		pub fn retract_bid(origin: OriginFor<T>, #[pallet::compact] amount: Balance, duration: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Place this bid on relaychain via XCM
			T::RelaychainInterface::gilt_retract_bid(amount, duration)?;

			Self::deposit_event(Event::RetractBidRequested { who, amount, duration });

			Ok(())
		}

		// confirm retraction

		// confirm thaw

		// thaw
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
