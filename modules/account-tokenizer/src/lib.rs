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

//! # Account Tokenized module
//!
//! This module allows Accounts on the Relaychain to be "tokenized" into a
//! Account Token, in the form of a NFT.
//! Authorized oracles can mint NFT into an account on the local chain, when the
//! corresponding account on the relaychain relinquishes its ownership to the parachain account.
//!
//! The owner of the NFT can then "Redeem" the NFT token to get back the control of the account on
//! the Relaychain.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{
	pallet_prelude::*,
	traits::tokens::nonfungibles::{Inspect, Mutate},
	transactional,
};
use frame_system::pallet_prelude::*;

use orml_traits::InspectExtended;

use module_support::ProxyXcm;

// mod mock;
// mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type TokenIdOf<T> = <T as orml_nft::Config>::TokenId;
	pub type ClassIdOf<T> = <T as orml_nft::Config>::ClassId;

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_nft::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		#[pallet::constant]
		type PalletAccount: Get<Self::AccountId>;

		/// The XcmInterface to communicate with the relaychain via XCM.
		type XcmInterface: ProxyXcm<Self::AccountId>;

		/// Origin used by Oracles. Used to relay information from the Relaychain.
		type OracleOrigin: EnsureOrigin<Self::Origin>;

		type NFTInterface: Inspect<Self::AccountId, ClassId = Self::ClassId, InstanceId = Self::TokenId>
			+ Mutate<Self::AccountId>
			+ InspectExtended<Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The AccountToken's NFT class ID as not yet been set.
		NFTClassIdNotSet,
		/// The given account's NFT is already issued.
		AccountTokenAlreadyMinted,
		/// The account's NFT token cannot be found.
		AccountTokenNotFound,
		/// The caller is unauthorized to make this transaction.
		CallerUnauthorized,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// The class ID of Account Tokens has been set
		NFTClassIdSet { class_id: ClassIdOf<T> },
		/// A NFT is minted to the owner of an account on the Relaychain.
		AccountTokenMinted {
			account: T::AccountId,
			owner: T::AccountId,
			token_id: T::TokenId,
		},
		/// The account token is burned, the control of the `account` on the relaychain is
		/// relinquished to `new_owner`.
		AccountTokenBurned {
			account: T::AccountId,
			owner: T::AccountId,
			token_id: T::TokenId,
			new_owner: T::AccountId,
		},
	}

	/// Stores the NFT's class ID. Settable by authorized Oracle. Used to mint and burn PRTs.
	#[pallet::storage]
	#[pallet::getter(fn nft_class_id)]
	type NFTClassId<T: Config> = StorageValue<_, ClassIdOf<T>, OptionQuery>;

	/// Stores accounts that are already minted as an NFT.
	#[pallet::storage]
	#[pallet::getter(fn minted_account)]
	type MintedAccount<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, T::TokenId, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the class ID of the Account Token NFT.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_nft_id(origin: OriginFor<T>, nft_id: ClassIdOf<T>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			NFTClassId::<T>::put(nft_id);
			Self::deposit_event(Event::<T>::NFTClassIdSet { class_id: nft_id });
			Ok(())
		}

		/// Mint an NFT that represents an Account Token.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		#[transactional]
		pub fn mint_account_token(origin: OriginFor<T>, account: T::AccountId, owner: T::AccountId) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			let nft_class_id = Self::nft_class_id().ok_or(Error::<T>::NFTClassIdNotSet)?;

			// Ensure we do not double-issue
			ensure!(
				Self::minted_account(&account).is_none(),
				Error::<T>::AccountTokenAlreadyMinted
			);

			// Mint the Account Token's NFT.
			let token_id = T::NFTInterface::next_token_id(nft_class_id);
			T::NFTInterface::mint_into(&nft_class_id, &token_id, &owner)?;

			// Create a record of the PRT and insert it into storage
			MintedAccount::<T>::insert(account.clone(), token_id);

			Self::deposit_event(Event::AccountTokenMinted {
				account,
				owner,
				token_id,
			});
			Ok(())
		}

		/// Burn the account's token to relinquish the control of the account on the relaychain
		/// to the `new_owner`.
		/// Only callable by the owner of the NFT token.
		#[pallet::weight(0)]
		#[transactional]
		pub fn burn_account_token(
			origin: OriginFor<T>,
			account: T::AccountId,
			new_owner: T::AccountId,
		) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin.clone())?;
			let who = ensure_signed(origin)?;
			let nft_class_id = Self::nft_class_id().ok_or(Error::<T>::NFTClassIdNotSet)?;

			// Ensure we do not double-issue
			let token_id = Self::minted_account(&account).ok_or(Error::<T>::AccountTokenNotFound)?;

			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;

			// Ensure that only the owner of the NFT can burn.
			ensure!(who == owner, Error::<T>::CallerUnauthorized);

			// Find the NFT and burn it
			T::NFTInterface::burn_from(&nft_class_id, &token_id)?;

			// TODO: send an XCM to relaychain to relinquish the control of the `account` to `new_owner`.
			T::XcmInterface::transfer_proxy(account.clone(), new_owner.clone())?;

			Self::deposit_event(Event::AccountTokenBurned {
				account,
				owner,
				token_id,
				new_owner,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
