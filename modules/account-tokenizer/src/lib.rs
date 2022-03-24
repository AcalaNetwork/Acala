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
//! Account Token, in the form of a NFT. The overall workflow is as follows:
//! 1. User creates an anonymous account using the `Proxy` pallet
//! 2. User transfers the control of the account to our Parachain account
//! 3. `request_mint` is called. Foreign state oracles will confirm the mint request
//! 4. An account token NFT is minted. The token is transferrable.
//! 5. The owner of the NFT can call `request_redeem` to redeem the token. This will cause
//!    the transfer of ownership of the anonymous account from the Parachain's account
//!    to the user's nominated account.
//! 6. Once the transfer is completed and confirmed by the Oracle, the NFT token is burned.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use codec::Decode;
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo},
	log,
	pallet_prelude::*,
	require_transactional,
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate, Transfer},
		BalanceStatus, Currency,
		ExistenceRequirement::AllowDeath,
		ExistenceRequirement::KeepAlive,
		GetStorageVersion, NamedReservableCurrency, StorageVersion, WithdrawReasons,
	},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use orml_traits::{arithmetic::Zero, InspectExtended};
use sp_io::hashing::blake2_256;
use sp_runtime::traits::{AccountIdConversion, TrailingZeroInput};

use module_support::{CreateExtended, ForeignChainStateQuery, ProxyXcm};
use primitives::{
	nft::{ClassId, ClassProperty, Properties, TokenId},
	Balance, ReserveIdentifier,
};

pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::AccountTokenizer;
// Represents `ProxyType::Any` on relaychain.
pub const PROXYTYPE_ANY: [u8; 1] = [0_u8];

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// The currency mechanism.
		type Currency: NamedReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = ReserveIdentifier,
			Balance = Balance,
		>;

		/// Pallet's account - used to mint and burn NFT.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Treasury's account. Fees and penalties are transferred to the treasury.
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// The amount of deposit required to create a mint request.
		/// The fund is confiscated if the request is invalid.
		#[pallet::constant]
		type MintRequestDeposit: Get<Balance>;

		/// Fee for minting an Account Token NFT.
		#[pallet::constant]
		type MintFee: Get<Balance>;

		/// The XcmInterface to communicate with the relaychain via XCM.
		type XcmInterface: ProxyXcm<Self::AccountId>;

		/// The overarching call type.
		type Call: Parameter
			+ Dispatchable<Origin = Self::Origin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ From<Call<Self>>
			+ IsType<<Self as frame_system::Config>::Call>;

		// Used to interface with the Oracle.
		type ForeignStateQuery: ForeignChainStateQuery<
			Self::AccountId,
			<Self as Config>::Call,
			Self::BlockNumber,
			Self::Origin,
		>;

		/// Interface used to communicate with the NFT module.
		type NFTInterface: Inspect<Self::AccountId, ClassId = ClassId, InstanceId = TokenId>
			+ Mutate<Self::AccountId>
			+ InspectExtended<Self::AccountId>
			+ Create<Self::AccountId>
			+ CreateExtended<Self::AccountId, Properties, Balance = Balance>
			+ Transfer<Self::AccountId>;

		type AccountTokenizerGovernance: EnsureOrigin<Self::Origin>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account's NFT token cannot be found.
		AccountTokenNotFound,
		/// The caller is unauthorized to make this transaction.
		CallerUnauthorized,
		/// Cannot decode data from oracle
		InvalidQueryResponse,
		/// Failed to prove account spawned anonymous proxy
		FailedAnonymousProxyCheck,
		/// The account has already had its NFT token minted.
		AccountTokenAlreadyExists,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// The user has requested to mint a Account Token NFT.
		MintRequested { account: T::AccountId, who: T::AccountId },
		/// The mint request is deemed invalid by oracle.
		MintRequestRejected { requester: T::AccountId },
		/// An account Token NFT is minted to an account.
		AccountTokenMinted {
			owner: T::AccountId,
			account: T::AccountId,
			token_id: TokenId,
		},
		/// A request to redeem the account token is submitted. XCM message is sent to the
		/// relaychain.
		RedeemRequested {
			account: T::AccountId,
			owner: T::AccountId,
			token_id: TokenId,
			new_owner: T::AccountId,
		},
		/// The account token is redeemed, the control of the `account` on the relaychain is
		/// relinquished to `new_owner`.
		AccountTokenRedeemed {
			account: T::AccountId,
			token_id: TokenId,
			new_owner: T::AccountId,
		},
		/// The XCM sent to foreign chain to redeem proxy failed, NFT is now in ownership of
		/// `TreasuryAccount`, governance call can return NFT to owner
		AccountTokenRedeemFailed { account: T::AccountId },
		/// Account Tokenizer Governence transfered NFT owned by treasury
		GovernanceNFTTransfer { token_id: TokenId, new_owner: T::AccountId },
	}

	/// Stores the NFT's class ID. Created on RuntimeUpgrade. Used to mint and burn PRTs.
	#[pallet::storage]
	#[pallet::getter(fn nft_class_id)]
	pub type NFTClassId<T: Config> = StorageValue<_, ClassId, ValueQuery>;

	/// Stores proxy accounts that are already minted as an Account Token NFT.
	/// Storage Map: Tokenized Account Id  => NFT Token ID
	#[pallet::storage]
	#[pallet::getter(fn minted_account)]
	pub type MintedAccount<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, TokenId, OptionQuery>;

	#[pallet::genesis_config]
	#[cfg_attr(feature = "std", derive(Default))]
	pub struct GenesisConfig;

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			Pallet::<T>::on_runtime_upgrade();
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Create the NFT class once.
		fn on_runtime_upgrade() -> Weight {
			let on_chain_storage_version = <Self as GetStorageVersion>::on_chain_storage_version();
			if on_chain_storage_version == 0 {
				let create_class_cost = T::NFTInterface::base_create_class_fee();

				// Transfer some fund from the treasury to pay for the class creation.
				let res = T::Currency::transfer(
					&T::TreasuryAccount::get(),
					&Self::account_id(),
					create_class_cost,
					KeepAlive,
				);
				log::debug!(
					"Account Tokenizer: Transferred funds from treasury to create class. result: {:?}",
					res
				);

				// Use storage version to ensure we only register NFT class once.
				let class_id = T::NFTInterface::next_class_id();
				let res = T::NFTInterface::create_class(&class_id, &Self::account_id(), &Self::account_id());
				log::debug!("Account Tokenizer: Created NFT class. result: {:?}", res);

				let res = T::NFTInterface::set_class_properties(
					&class_id,
					Properties(
						ClassProperty::Transferable
							| ClassProperty::Burnable | ClassProperty::Mintable
							| ClassProperty::ClassPropertiesMutable,
					),
				);
				log::debug!("Account Tokenizer: Set NFT class property. result: {:?}", res);

				// Sets NFT class ID storage
				NFTClassId::<T>::put(class_id);

				// Upgrade storage versino so NFT class is only created once.
				StorageVersion::new(1).put::<Self>();
				<T as Config>::WeightInfo::initialize_nft_class()
			} else {
				0
			}
		}

		// ensure that MintFee is >= NFT's mint fee.
		fn integrity_test() {
			sp_std::if_std! {
				sp_io::TestExternalities::new_empty().execute_with(||
					assert!(
						T::MintFee::get() >= T::NFTInterface::base_mint_fee()
					));
			}
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Request to mint an Account Token. Called after the user of the same Account Id has given
		/// the proxy control of an account to the parachain account.
		///
		/// Params:
		/// 	- `account`: The account ID of the anonymous proxy.
		/// 	- `original_owner`: The original owner's account ID. Used to verify anonymous proxy.
		/// 	- `height`: The block number in which the anonymous proxy is generated.
		/// 	- `ext_index`: The index, in the block, of the extrinsics that generated the anonymous
		///    proxy.
		/// 	- `index`: The index of the anonymous proxy.
		#[pallet::weight(< T as Config >::WeightInfo::request_mint())]
		#[transactional]
		pub fn request_mint(
			origin: OriginFor<T>,
			account: T::AccountId,
			original_owner: T::AccountId,
			height: T::BlockNumber,
			ext_index: u32,
			index: u16,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Checks if the account is an anonymous proxy of the origin_owner.
			// hard coded for `ProxyType::Any`. No other proxy type is allowed
			let entropy = (
				b"modlpy/proxy____",
				&original_owner,
				height,
				ext_index,
				&PROXYTYPE_ANY,
				index,
			)
				.using_encoded(blake2_256);
			let derived_account: T::AccountId = Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
				.expect("infinite length input; no invalid inputs for type; qed");
			// ensures account is anonymous proxy
			ensure!(account == derived_account, Error::<T>::FailedAnonymousProxyCheck);

			// Ensure the token hasn't already been minted.
			ensure!(
				!MintedAccount::<T>::contains_key(&account),
				Error::<T>::AccountTokenAlreadyExists
			);

			// Charge the user fee and lock the deposit.
			T::Currency::transfer(&who, &T::TreasuryAccount::get(), T::MintFee::get(), KeepAlive)?;
			T::Currency::reserve_named(&RESERVE_ID, &who, T::MintRequestDeposit::get())?;

			// Submit confiramtion call to be serviced by foreign state oracle
			let call: <T as Config>::Call = Call::<T>::confirm_mint_request {
				owner: who.clone(),
				account: account.clone(),
			}
			.into();
			T::ForeignStateQuery::create_query(&who, call, None)?;

			Self::deposit_event(Event::MintRequested { account, who });
			Ok(())
		}

		/// Confirm the mint request by rejecting or accepting.
		/// On accept - Account Token NFT is minted into the user's account, deposit returned.
		/// On reject - the deposit is confiscated.
		///
		/// Only callable by the Oracle.
		///
		/// Params:
		/// 	- `owner`: The owner of the Account Token to be minted.
		/// 	- `account`: Account ID of the anonymous proxy.
		#[pallet::weight(< T as Config >::WeightInfo::confirm_mint_request())]
		#[transactional]
		pub fn confirm_mint_request(
			origin: OriginFor<T>,
			owner: T::AccountId,
			account: T::AccountId,
		) -> DispatchResult {
			// Extract confirmation info from Origin.
			let data = T::ForeignStateQuery::ensure_origin(origin)?;

			// Checks whether oracle confirms or rejects the mint request
			let success: bool = Decode::decode(&mut &data[..]).map_err(|_| Error::<T>::InvalidQueryResponse)?;

			if success {
				Self::accept_mint_request(owner, account)
			} else {
				Self::reject_mint_request(owner)
			}
		}

		/// Requests to redeem an Account Token. Sends XCM message to the relaychain to transfer the
		/// control of the account.
		/// The NFT is taken into custodial by the module, and is not burned until confirmed by the
		/// Oracle.
		///
		/// Params:
		/// 	- `account`: Account ID of the Account Token
		/// 	- `new_owner`: The owner of the proxy account to be transferred to.
		#[pallet::weight(< T as Config >::WeightInfo::request_redeem())]
		#[transactional]
		pub fn request_redeem(origin: OriginFor<T>, account: T::AccountId, new_owner: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let nft_class_id = Self::nft_class_id();

			// Obtain info about the account token.
			let token_id = Self::minted_account(&account).ok_or(Error::<T>::AccountTokenNotFound)?;
			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;

			// Ensure that only the owner of the NFT can redeem.
			ensure!(who == owner, Error::<T>::CallerUnauthorized);

			// The XCM fee is burned.
			T::Currency::withdraw(
				&who,
				T::XcmInterface::get_transfer_proxy_xcm_fee(),
				WithdrawReasons::FEE,
				KeepAlive,
			)?;

			// Send an XCM to relaychain to relinquish the control of the `account` to `new_owner`.
			T::XcmInterface::transfer_proxy(account.clone(), new_owner.clone())?;

			// Submit confirmation call to be serviced by foreign state oracle
			let call: <T as Config>::Call = Call::<T>::confirm_redeem_account_token {
				account: account.clone(),
				new_owner: new_owner.clone(),
			}
			.into();
			T::ForeignStateQuery::create_query(&who, call, None)?;

			// Take custody of the NFT token.
			T::NFTInterface::transfer(&nft_class_id, &token_id, &T::TreasuryAccount::get())?;

			Self::deposit_event(Event::RedeemRequested {
				account,
				owner,
				token_id,
				new_owner,
			});
			Ok(())
		}

		/// Confirm that the parachain account has relinquished the control of the account on the
		/// relaychain to the `new_owner`. The NFT is burned and storage updated.
		///
		/// Only callable by the Oracle.
		///
		/// Params:
		/// 	- `account`: Account ID of the Account Token
		/// 	- `new_owner`: The owner of the proxy account to be transferred to.
		#[pallet::weight(< T as Config >::WeightInfo::confirm_redeem_account_token())]
		#[transactional]
		pub fn confirm_redeem_account_token(
			origin: OriginFor<T>,
			account: T::AccountId,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let data = T::ForeignStateQuery::ensure_origin(origin)?;
			// Checks whether oracle confirms or rejects the mint request
			let success: bool = Decode::decode(&mut &data[..]).map_err(|_| Error::<T>::InvalidQueryResponse)?;

			if success {
				let nft_class_id = Self::nft_class_id();

				// Obtain info about the account token.
				let token_id = MintedAccount::<T>::take(&account).ok_or(Error::<T>::AccountTokenNotFound)?;
				T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;

				// Find the NFT and burn it
				T::NFTInterface::burn_from(&nft_class_id, &token_id)?;

				Self::deposit_event(Event::AccountTokenRedeemed {
					account,
					token_id,
					new_owner,
				});
			} else {
				Self::deposit_event(Event::AccountTokenRedeemFailed { account });
			}

			Ok(())
		}

		/// Transfers NFT from treasury to user account. This should be used if Xcm sent to transfer
		/// proxy fails.
		///
		/// Params:
		/// 	- `proxy_account`: AccountId of anon proxy that is tokenized.
		/// 	- `new_owner`: AccountId that NFT will be sent to.
		#[pallet::weight(<T as Config>::WeightInfo::transfer_nft())]
		#[transactional]
		pub fn transfer_nft(
			origin: OriginFor<T>,
			proxy_account: T::AccountId,
			new_owner: T::AccountId,
		) -> DispatchResult {
			T::AccountTokenizerGovernance::ensure_origin(origin)?;

			let nft_class_id = Self::nft_class_id();
			let token_id = Self::minted_account(&proxy_account).ok_or(Error::<T>::AccountTokenNotFound)?;
			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;
			// Ensure that NFT is owned by treasury account
			ensure!(T::TreasuryAccount::get() == owner, Error::<T>::CallerUnauthorized);

			T::NFTInterface::transfer(&nft_class_id, &token_id, &new_owner)?;
			Ok(())
		}

		/// Burns nft, useful if oracle failed to respond but XCM was successful
		///
		/// Params:
		/// 	- `token_id`: TokenId representing NFT to be burned
		#[pallet::weight(<T as Config>::WeightInfo::burn_nft())]
		#[transactional]
		pub fn burn_nft(origin: OriginFor<T>, proxy_account: T::AccountId) -> DispatchResult {
			T::AccountTokenizerGovernance::ensure_origin(origin)?;

			let nft_class_id = Self::nft_class_id();
			let token_id = MintedAccount::<T>::take(&proxy_account).ok_or(Error::<T>::AccountTokenNotFound)?;
			let owner = T::NFTInterface::owner(&nft_class_id, &token_id).ok_or(Error::<T>::AccountTokenNotFound)?;
			// Ensure that NFT is owned by treasury account
			ensure!(T::TreasuryAccount::get() == owner, Error::<T>::CallerUnauthorized);

			T::NFTInterface::burn_from(&nft_class_id, &token_id)?;
			Ok(())
		}

		/// Transfers funds from treasury, can be used to reimburse incorrect slashing
		///
		/// Params:
		/// 	- `to`: Account recieving funds
		/// 	- `amount`: Amount of native token sent
		#[pallet::weight(<T as Config>::WeightInfo::transfer_treasury_funds())]
		#[transactional]
		pub fn transfer_treasury_funds(origin: OriginFor<T>, to: T::AccountId, amount: Balance) -> DispatchResult {
			T::AccountTokenizerGovernance::ensure_origin(origin)?;
			T::Currency::transfer(&T::TreasuryAccount::get(), &to, amount, AllowDeath)
		}

		/// Recovers stranded reserved tokens, This occurs when oracle fails to respond to
		/// `request_mint`. Can either slash or unreserve
		///
		/// Params:
		/// 	- `account`: Account that has stranded reserved funds
		/// 	- `amount`: Amount to unreserve/slash
		/// 	- `slash`: Determines whether the funds will be returned or slashed
		#[pallet::weight(<T as Config>::WeightInfo::force_unreserve_funds())]
		#[transactional]
		pub fn force_unreserve_funds(
			origin: OriginFor<T>,
			account: T::AccountId,
			amount: Balance,
			slash: bool,
		) -> DispatchResult {
			T::AccountTokenizerGovernance::ensure_origin(origin)?;
			if slash {
				T::Currency::repatriate_reserved_named(
					&RESERVE_ID,
					&account,
					&T::TreasuryAccount::get(),
					amount,
					BalanceStatus::Free,
				)?;
			} else {
				T::Currency::unreserve_named(&RESERVE_ID, &account, amount);
			}
			Ok(())
		}

		/// This will remint nft if `MintedAccount` storage does not correspond
		/// to a existing nft (This could occur if nft was burned by user)
		///
		/// Params:
		/// 	- `proxy_account`: Anonymous proxy that nft corresponds to
		/// 	- `owner`: Account recieving reminted nft
		#[pallet::weight(<T as Config>::WeightInfo::remint_burned_nft())]
		#[transactional]
		pub fn remint_burned_nft(
			origin: OriginFor<T>,
			proxy_account: T::AccountId,
			owner: T::AccountId,
		) -> DispatchResult {
			T::AccountTokenizerGovernance::ensure_origin(origin)?;
			let nft_class_id = Self::nft_class_id();

			// Check that record of minted account exists
			let token_id = Self::minted_account(&proxy_account).ok_or(Error::<T>::AccountTokenNotFound)?;
			// Checks that corresponding nft does not exist
			if T::NFTInterface::owner(&nft_class_id, &token_id).is_none() {
				// Pay for minting the token
				T::NFTInterface::pay_mint_fee(&owner, &nft_class_id, 1u32)?;

				// Mint the Account Token's NFT.
				let token_id = T::NFTInterface::next_token_id(nft_class_id);
				T::NFTInterface::mint_into(&nft_class_id, &token_id, &owner)?;

				// Create a record of the Mint and insert it into storage
				MintedAccount::<T>::insert(&proxy_account, token_id);
				Ok(())
			} else {
				Err(Error::<T>::AccountTokenAlreadyExists.into())
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Returns the module's account ID.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}

	/// Confirms that the Mint request is valid. Mint a NFT that represents an Account Token.
	/// Only callable by authorized Oracles.
	#[require_transactional]
	pub fn accept_mint_request(owner: T::AccountId, account: T::AccountId) -> DispatchResult {
		// Ensure the token hasn't already been minted.
		if MintedAccount::<T>::contains_key(&account) {
			// NFT is already minted. Confiscate the deposit and reject request
			Self::reject_mint_request(owner)
		} else {
			let nft_class_id = Self::nft_class_id();

			// Pay for minting the token
			T::NFTInterface::pay_mint_fee(&T::TreasuryAccount::get(), &nft_class_id, 1u32)?;

			// Mint the Account Token's NFT.
			let token_id = T::NFTInterface::next_token_id(nft_class_id);
			T::NFTInterface::mint_into(&nft_class_id, &token_id, &owner)?;

			// Create a record of the Mint and insert it into storage
			MintedAccount::<T>::insert(&account, token_id);

			// Release the deposit from the owner
			let remaining = T::Currency::unreserve_named(&RESERVE_ID, &owner, T::MintRequestDeposit::get());
			debug_assert!(remaining.is_zero());

			Self::deposit_event(Event::AccountTokenMinted {
				owner,
				account,
				token_id,
			});
			Ok(())
		}
	}

	/// Reject the Mint request. The deposit by the minter is confiscated.
	/// Only callable by authorized Oracles.
	#[require_transactional]
	pub fn reject_mint_request(requester: T::AccountId) -> DispatchResult {
		// Release the deposit from the requester
		T::Currency::repatriate_reserved_named(
			&RESERVE_ID,
			&requester,
			&T::TreasuryAccount::get(),
			T::MintRequestDeposit::get(),
			BalanceStatus::Free,
		)?;

		Self::deposit_event(Event::MintRequestRejected { requester });
		Ok(())
	}
}
