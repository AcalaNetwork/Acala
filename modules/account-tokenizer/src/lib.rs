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
	dispatch::{Dispatchable, GetDispatchInfo},
	log,
	pallet_prelude::*,
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate},
		BalanceStatus, Currency,
		ExistenceRequirement::KeepAlive,
		GetStorageVersion, NamedReservableCurrency, StorageVersion, WithdrawReasons,
	},
	transactional,
};

use frame_system::pallet_prelude::*;
use orml_traits::{arithmetic::Zero, CreateExtended, InspectExtended};
use sp_std::vec::Vec;

use module_support::{ForeignChainStateQuery, ProxyXcm};
use primitives::{
	nft::{ClassProperty, Properties},
	Balance, ReserveIdentifier,
};

pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::AccountTokenizer;

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_nft::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism.
		type Currency: NamedReservableCurrency<Self::AccountId, ReserveIdentifier = ReserveIdentifier>
			+ Currency<Self::AccountId, Balance = Balance>;

		/// Pallet's account - used to mint and burn NFT.
		#[pallet::constant]
		type PalletAccount: Get<Self::AccountId>;

		/// Treasury's account. Fees and penalties are transferred to the treasury.
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// The amount of deposit required to create a mint request.
		/// The fund is confiscated if the request is invalid.
		#[pallet::constant]
		type MintRequestDeposit: Get<Balance>;

		/// Fee for minting an account Token NFT.
		#[pallet::constant]
		type MintFee: Get<Balance>;

		/// The XcmInterface to communicate with the relaychain via XCM.
		type XcmInterface: ProxyXcm<Self::AccountId>;

		/// Origin used by Oracles. Used to relay information from the Relaychain.
		type OracleOrigin: EnsureOrigin<Self::Origin, Success = Vec<u8>>;

		/// The overarching call type.
		type Call: Parameter
			+ Dispatchable<Origin = Self::Origin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ From<Call<Self>>
			+ IsType<<Self as frame_system::Config>::Call>;

		type ForeignStateQuery: ForeignChainStateQuery<Self::AccountId, <Self as Config>::Call>;

		/// Interface used to communicate with the NFT module.
		type NFTInterface: Inspect<Self::AccountId, ClassId = Self::ClassId, InstanceId = Self::TokenId>
			+ Mutate<Self::AccountId>
			+ InspectExtended<Self::AccountId>
			+ Create<Self::AccountId>
			+ CreateExtended<Self::AccountId, Properties>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account's NFT token cannot be found.
		AccountTokenNotFound,
		/// The caller is unauthorized to make this transaction.
		CallerUnauthorized,
		/// The owner of the NFT has insufficient reserve balance.
		InsufficientReservedBalance,
		/// Cannot decode data from oracle
		BadOracleData,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// The user has requested to mint a Account Token NFT.
		MintRequested {
			account: T::AccountId,
			who: T::AccountId,
		},
		MintRequestRejected {
			requester: T::AccountId,
		},
		/// A NFT is minted to the owner of an account on the Relaychain.
		AccountTokenMinted {
			owner: T::AccountId,
			account: T::AccountId,
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
	pub type NFTClassId<T: Config> = StorageValue<_, T::ClassId, ValueQuery>;

	/// Stores accounts that are already minted as an NFT.
	/// Storage Map: Tokenized Account Id  => NFT Token ID
	#[pallet::storage]
	#[pallet::getter(fn minted_account)]
	pub type MintedAccount<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, T::TokenId, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_runtime_upgrade() -> Weight {
			let on_chain_storage_version = <Self as GetStorageVersion>::on_chain_storage_version();
			if on_chain_storage_version == 0 {
				// Use storage version to ensure we only register NFT class once.
				let class_id = T::NFTInterface::next_class_id();
				let res = T::NFTInterface::create_class(&class_id, &T::PalletAccount::get(), &T::PalletAccount::get());
				log::debug!("Account Tokenizer: Created NFT class. result: {:?}", res);

				let res = T::NFTInterface::set_class_properties(
					&class_id,
					Properties(
						ClassProperty::Transferable
							| ClassProperty::Burnable | ClassProperty::Mintable
							| ClassProperty::ClassPropertiesMutable,
					)
					.into(),
				);
				log::debug!("Account Tokenizer: Set NFT class property. result: {:?}", res);

				NFTClassId::<T>::put(class_id);
				StorageVersion::new(1).put::<Self>();
			}
			0
		}
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Request to mint an Account Token. Called after the user of the same Account Id has given
		/// the proxy control of an account to the parachain account.
		#[pallet::weight(0)]
		#[transactional]
		pub fn request_mint(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Charge the user fee and lock the deposit.
			T::Currency::transfer(&who, &T::TreasuryAccount::get(), T::MintFee::get(), KeepAlive)?;
			T::Currency::reserve_named(&RESERVE_ID, &who, T::MintRequestDeposit::get())?;

			// Submit confiramtion call to be serviced by foreign state oracle
			let call: <T as Config>::Call = Call::<T>::confirm_mint_request {
				owner: who.clone(),
				account: account.clone(),
			}
			.into();
			T::ForeignStateQuery::query_task(who.clone(), call.using_encoded(|x| x.len()), call)?;

			Self::deposit_event(Event::MintRequested { account, who });
			Ok(())
		}

		#[pallet::weight(0)]
		pub fn confirm_mint_request(
			origin: OriginFor<T>,
			owner: T::AccountId,
			account: T::AccountId,
		) -> DispatchResult {
			// Extract confirmation info from Origin.
			let data = T::OracleOrigin::ensure_origin(origin)?;
			let rejected = data.get(0).ok_or(Error::<T>::BadOracleData)?.is_zero();

			// Accept or reject the mint request.
			if rejected {
				Self::reject_mint_request(owner)
			} else {
				Self::accept_mint_request(owner, account)
			}
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
			let nft_class_id = Self::nft_class_id();

			// Obtain info about the account token.
			let token_id = MintedAccount::<T>::take(&account).ok_or(Error::<T>::AccountTokenNotFound)?;
			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;

			// Ensure that only the owner of the NFT can burn.
			ensure!(who == owner, Error::<T>::CallerUnauthorized);

			// The XCM fee is burned.
			T::Currency::withdraw(
				&who,
				T::XcmInterface::get_transfer_proxy_xcm_fee(),
				WithdrawReasons::FEE,
				KeepAlive,
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

impl<T: Config> Pallet<T> {
	/// Confirms that the Mint request is valid. Mint a NFT that represents an Account Token.
	/// Only callable by authorized Oracles.
	#[transactional]
	pub fn accept_mint_request(owner: T::AccountId, account: T::AccountId) -> DispatchResult {
		let nft_class_id = Self::nft_class_id();

		// Mint the Account Token's NFT.
		let token_id = T::NFTInterface::next_token_id(nft_class_id);
		T::NFTInterface::mint_into(&nft_class_id, &token_id, &owner)?;

		// Create a record of the Mint and insert it into storage
		MintedAccount::<T>::insert(account.clone(), token_id);

		// Release the deposit from the owner
		let remaining = T::Currency::unreserve_named(&RESERVE_ID, &owner, T::MintRequestDeposit::get());
		ensure!(remaining.is_zero(), Error::<T>::InsufficientReservedBalance);

		Self::deposit_event(Event::AccountTokenMinted {
			owner,
			account,
			token_id,
		});
		Ok(())
	}

	/// Reject the Mint request. The deposit by the minter is confiscated.
	/// Only callable by authorized Oracles.
	#[transactional]
	pub fn reject_mint_request(requester: T::AccountId) -> DispatchResult {
		// Release the deposit from the requester
		T::Currency::repatriate_reserved_named(
			&RESERVE_ID,
			&requester,
			&T::TreasuryAccount::get(),
			T::MintRequestDeposit::get(),
			BalanceStatus::Free,
		)?;

		Self::deposit_event(Event::MintRequestRejected { requester: requester });
		Ok(())
	}
}
