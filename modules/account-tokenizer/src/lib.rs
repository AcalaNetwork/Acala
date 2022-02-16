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
	traits::{
		tokens::nonfungibles::{Inspect, Mutate},
		BalanceStatus,
	},
	transactional,
};
use frame_system::pallet_prelude::*;
use orml_traits::{InspectExtended, MultiCurrency, MultiReservableCurrency};

use module_support::ProxyXcm;
use primitives::{Balance, CurrencyId};

// mod mock;
// mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_nft::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Multi-currency support for asset management
		type Currency: MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>
			+ MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The native currency's ID.
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// Pallet's account - used to mint and burn NFT.
		#[pallet::constant]
		type PalletAccount: Get<Self::AccountId>;

		/// Treasury's account. Fees and penalties are transferred to the treasury.
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

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
		/// The given account's already been requested to mint.
		AccountAlreadyRequestedMinted,
		/// The mint request for the given account is not found.
		MintRequestNotFound,
		/// The confirmed owner and the mint requester isn't the same.
		MintRequestDifferentFromOwner,
		/// The account's NFT token cannot be found.
		AccountTokenNotFound,
		/// The caller is unauthorized to make this transaction.
		CallerUnauthorized,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// The class ID of Account Tokens has been set
		NFTClassIdSet {
			class_id: T::ClassId,
		},
		/// The mint fee has been set
		MintFeeSet {
			mint_fee: Balance,
		},
		/// The burn fee has been set
		BurnFeeSet {
			burn_fee: Balance,
		},
		/// The deposit amount for request_mint has been set
		RequestMintDepositSet {
			deposit: Balance,
		},
		/// The user has requested to mint a Account Token NFT.
		MintRequested {
			account: T::AccountId,
			who: T::AccountId,
		},
		MintRequestRejected {
			account: T::AccountId,
			who: T::AccountId,
		},
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
	type NFTClassId<T: Config> = StorageValue<_, T::ClassId, OptionQuery>;

	/// Stores accounts that are already minted as an NFT.
	/// Storage Map: Tokenized Account Id  => NFT Token ID
	#[pallet::storage]
	#[pallet::getter(fn minted_account)]
	type MintedAccount<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, T::TokenId, OptionQuery>;

	/// Stores mint requests.
	/// Storage Map: Account to be tokenized  => Requester's Account Id
	#[pallet::storage]
	#[pallet::getter(fn mint_requests)]
	type MintRequests<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, T::AccountId, OptionQuery>;

	/// The amount of fee paid when minting a Account Token
	#[pallet::storage]
	#[pallet::getter(fn mint_fee)]
	type MintFee<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The amount of fee paid when burning a Account Token, in additional to the XCM cost.
	#[pallet::storage]
	#[pallet::getter(fn burn_fee)]
	type BurnFee<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Deposit locked when requesting to mint. If the mint request is successful, it is returned.
	/// Otherwise the deposit is confiscated as penalty.
	#[pallet::storage]
	#[pallet::getter(fn request_mint_deposit)]
	type RequestMintDeposit<T: Config> = StorageValue<_, Balance, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the class ID of the Account Token NFT.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_nft_id(origin: OriginFor<T>, nft_id: T::ClassId) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			NFTClassId::<T>::put(nft_id);
			Self::deposit_event(Event::<T>::NFTClassIdSet { class_id: nft_id });
			Ok(())
		}

		/// Sets the Mint Fee.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_mint_fee(origin: OriginFor<T>, mint_fee: Balance) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			MintFee::<T>::put(mint_fee);
			Self::deposit_event(Event::<T>::MintFeeSet { mint_fee });
			Ok(())
		}

		/// Sets the Burn Fee.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_burn_fee(origin: OriginFor<T>, burn_fee: Balance) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			BurnFee::<T>::put(burn_fee);
			Self::deposit_event(Event::<T>::BurnFeeSet { burn_fee });
			Ok(())
		}

		/// Sets the Deposit amount for request_mint.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		pub fn set_request_mint_deposit(origin: OriginFor<T>, deposit: Balance) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;
			RequestMintDeposit::<T>::put(deposit);
			Self::deposit_event(Event::<T>::RequestMintDepositSet { deposit });
			Ok(())
		}

		/// Request to mint an Account Token. Called after the user of the same Account Id has given
		/// the proxy control of an account to the parachain account.
		#[pallet::weight(0)]
		#[transactional]
		pub fn request_mint(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::nft_class_id().ok_or(Error::<T>::NFTClassIdNotSet)?;

			// An account can only have a single "requester".
			ensure!(
				Self::mint_requests(&account).is_none(),
				Error::<T>::AccountAlreadyRequestedMinted
			);

			// Ensure the account token hasn't already been minted
			ensure!(
				Self::minted_account(&account).is_none(),
				Error::<T>::AccountTokenAlreadyMinted
			);

			// Charge the user fee and lock the deposit.
			T::Currency::transfer(
				T::NativeCurrencyId::get(),
				&who,
				&T::TreasuryAccount::get(),
				Self::mint_fee(),
			)?;
			T::Currency::reserve(T::NativeCurrencyId::get(), &who, Self::request_mint_deposit())?;

			// Add a record of the request.
			MintRequests::<T>::insert(account.clone(), who.clone());

			Self::deposit_event(Event::MintRequested { account, who });
			Ok(())
		}

		/// Confirms that the Mint request is valid. Mint a NFT that represents an Account Token.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		#[transactional]
		pub fn confirm_mint_request(
			origin: OriginFor<T>,
			account: T::AccountId,
			owner: T::AccountId,
		) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			let nft_class_id = Self::nft_class_id().ok_or(Error::<T>::NFTClassIdNotSet)?;

			// The confirmed owner and the mint requester is the same.
			let requester = MintRequests::<T>::take(account.clone()).ok_or(Error::<T>::MintRequestNotFound)?;
			ensure!(requester == owner, Error::<T>::MintRequestDifferentFromOwner);

			// Ensure we do not double-mint
			ensure!(
				Self::minted_account(&account).is_none(),
				Error::<T>::AccountTokenAlreadyMinted
			);

			// Mint the Account Token's NFT.
			let token_id = T::NFTInterface::next_token_id(nft_class_id);
			T::NFTInterface::mint_into(&nft_class_id, &token_id, &owner)?;

			// Create a record of the Mint and insert it into storage
			MintedAccount::<T>::insert(account.clone(), token_id);

			// Release the deposit from the requester
			T::Currency::unreserve(T::NativeCurrencyId::get(), &owner, Self::request_mint_deposit());

			Self::deposit_event(Event::AccountTokenMinted {
				account,
				owner,
				token_id,
			});
			Ok(())
		}

		/// Reject the Mint request. The deposit by the minter is confiscated.
		/// Only callable by authorized Oracles.
		#[pallet::weight(0)]
		#[transactional]
		pub fn reject_mint_request(origin: OriginFor<T>, account: T::AccountId, owner: T::AccountId) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			// The confirmed owner and the mint requester is the same.
			let requester = MintRequests::<T>::take(&account).ok_or(Error::<T>::MintRequestNotFound)?;
			ensure!(requester == owner, Error::<T>::MintRequestDifferentFromOwner);

			// Release the deposit from the requester
			T::Currency::repatriate_reserved(
				T::NativeCurrencyId::get(),
				&requester,
				&T::TreasuryAccount::get(),
				Self::request_mint_deposit(),
				BalanceStatus::Free,
			)?;

			Self::deposit_event(Event::MintRequestRejected {
				account,
				who: requester,
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
			let who = ensure_signed(origin)?;
			let nft_class_id = Self::nft_class_id().ok_or(Error::<T>::NFTClassIdNotSet)?;

			// Obtain info about the account token.
			let token_id = Self::minted_account(&account).ok_or(Error::<T>::AccountTokenNotFound)?;
			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;

			// Ensure that only the owner of the NFT can burn.
			ensure!(who == owner, Error::<T>::CallerUnauthorized);

			// Burn fee goes to the treasury, and the XCM fee is burned.
			T::Currency::transfer(
				T::NativeCurrencyId::get(),
				&who,
				&T::TreasuryAccount::get(),
				Self::burn_fee(),
			)?;
			T::Currency::withdraw(
				T::NativeCurrencyId::get(),
				&who,
				T::XcmInterface::get_transfer_proxy_xcm_fee(),
			)?;

			// Find the NFT and burn it
			T::NFTInterface::burn_from(&nft_class_id, &token_id)?;

			// Send an XCM to relaychain to relinquish the control of the `account` to `new_owner`.
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
