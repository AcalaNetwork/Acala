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

//! # Starport Module
//!
//! This is the Starport module used to connect with Compound Finance.
//! The following functionalities are supported:
//!
//! * Uploading Assets: User can lock assets native to Acala to "upload" them onto the Compound
//!   chain.
//!
//! * CASH asset management: User can transfer CASH asset freely between Acala and Compound Chain.
//!   While the CASH is on Acala, the yield is identical to those that are on the Compound Chain.
//!
//! * Downloading Assets: User can unlock/download assets back from Compound chain back to Acala.
//!   All asset actions such as transfers on the Compound chain are respected on the Acala chain.
//!
//! * Receive Notices from Compound chain: Receive, verify and execute "Notices", or actionable
//!   requests from the Compound chain.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

mod mock;
mod tests;
use frame_support::{
	pallet_prelude::*, require_transactional, transactional, weights::PostDispatchInfo, BoundedVec, PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::CompoundCashTrait;
use orml_traits::MultiCurrency;
use primitives::{AccountId, Balance, CashYieldIndex, CurrencyId, Moment, TokenSymbol};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, Hash},
	AccountId32, Perbill,
};
use sp_std::prelude::*;

pub use module::*;

pub type CompoundAuthoritySignature = AccountId32;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The pallet handling Compound's Cash tokens
		type Cash: CompoundCashTrait<Balance, Moment>;

		/// The ID for the CASH asset
		#[pallet::constant]
		type CashCurrencyId: Get<CurrencyId>;

		/// The ID for this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The max number authorities that are stored
		#[pallet::constant]
		type MaxGatewayAuthorities: Get<u32>;

		/// The percentage threshold of authorities signatures required for Notices to take effect.
		#[pallet::constant]
		type PercentThresholdForAuthoritySignature: Get<Perbill>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Acala -> Compound Gateway
		/// There are not enough supply on the Compound chain for the lock operation.
		InsufficientAssetSupplyCap,

		/// Notices from Compound
		/// The same notice cannot be invoked more than once.
		NoticeAlreadyInvoked,
		/// Only specific Admin account is able to send Notice to be invoked
		InvalidNoticeInvoker,
		/// The Admin account does not have enough asset for the Unlock operation.
		InsufficientAssetToUnlock,
		/// Not enough authorities have signed this notice for it to be effective.
		InsufficientValidNoticeSignatures,
		/// Too many Authorities.
		ExceededMaxNumberOfAuthorities,
		/// Authorities cannot be empty
		AuthoritiesListCannotBeEmpty,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// User has locked some asset and uploaded them into Compound.
		AssetLockedTo {
			currency_id: CurrencyId,
			amount: Balance,
			user: T::AccountId,
		},

		/// The user has unlocked some asset and downloaded them back into Acala.
		AssetUnlocked {
			currency_id: CurrencyId,
			amount: Balance,
			user: T::AccountId,
		},

		/// The list of authorities has been updated.
		GatewayAuthoritiesChanged,

		/// The supply cap for an asset has been updated.
		SupplyCapSet { currency_id: CurrencyId, new_cap: Balance },

		/// The future yield for CASH is set.
		FutureYieldSet {
			yield_amount: Balance,
			index: CashYieldIndex,
			timestamp: Moment,
		},
	}

	#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
	pub struct GatewayNotice<AccountId> {
		pub id: u64,
		pub payload: GatewayNoticePayload<AccountId>,
	}

	impl<AccountId> GatewayNotice<AccountId> {
		pub fn new(id: u64, payload: GatewayNoticePayload<AccountId>) -> Self {
			Self { id, payload }
		}
	}

	#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
	pub enum GatewayNoticePayload<AccountId> {
		/// Update the current supply cap for an asset. Only assets that have spare supplies.
		/// can be locked or uploaded to the Compound chain.
		SetSupplyCap(CurrencyId, Balance),

		/// Update the current set of authorities who sign Notices.
		ChangeAuthorities(Vec<CompoundAuthoritySignature>),

		/// Unlock or download assets from Compound chain back into Acala chain.
		Unlock {
			currency_id: CurrencyId,
			amount: Balance,
			who: AccountId,
		},

		/// Set the future yield for the Cash asset.
		/// Parameters: uint128 nextCashYield, uint128 nextCashYieldIndex, uint nextCashYieldStart
		SetFutureYield {
			next_cash_yield: Balance,
			next_cash_yield_index: CashYieldIndex,
			next_cash_yield_start: Moment,
		},
	}

	/// Stores the amount of supplies that are still available to be uploaded for each asset type.
	#[pallet::storage]
	#[pallet::getter(fn supply_caps)]
	pub type SupplyCaps<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// Stores the Hash of Notices that have already been invoked. Prevents
	/// double-invocation.
	#[pallet::storage]
	#[pallet::getter(fn invoked_notice_hashes)]
	pub type InvokedNoticeHashes<T: Config> = StorageMap<_, Identity, H256, (), OptionQuery>;

	/// Stores the current authorities on the Compound chain. Used to verify the signatures on a
	/// given Notice.
	#[pallet::storage]
	#[pallet::getter(fn gateway_authorities)]
	pub type GatewayAuthorities<T: Config> =
		StorageValue<_, BoundedVec<CompoundAuthoritySignature, T::MaxGatewayAuthorities>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub initial_authorities: Vec<CompoundAuthoritySignature>,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			GenesisConfig {
				initial_authorities: vec![AccountId::new([0; 32])],
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			// ensure no duplicates exist.
			let unique_authorities = self
				.initial_authorities
				.iter()
				.collect::<std::collections::BTreeSet<_>>();
			assert!(
				unique_authorities.len() == self.initial_authorities.len(),
				"duplicate initial authorities signatures in genesis."
			);
			let bounded_vec = BoundedVec::try_from(self.initial_authorities.clone()).unwrap();
			GatewayAuthorities::<T>::put(bounded_vec);
		}
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock some asset from a user's account on Acala.
		/// Request the same asset be transferred to the Compound chain via its Gateway.
		/// These assets are generally used as collaterals on the Compound Finance network.
		/// This is also known as "Uploading assets"
		///
		/// Parameters:
		/// - `currency_id`: collateral currency id.
		/// - `locked_amount`: The amount of user asset to be "uploaded" onto the Compound chain.
		//#[pallet::weight(< T as Config >::WeightInfo::lock())]
		#[pallet::weight(0)]
		#[transactional]
		pub fn lock(origin: OriginFor<T>, currency_id: CurrencyId, locked_amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_lock_to(who.clone(), who, currency_id, locked_amount)
		}

		/// Lock some asset from a user's account on Acala to another account (on another network).
		/// Request the same asset be transferred to the Compound chain via its Gateway.
		/// These assets are generally used as collaterals on the Compound Finance network.
		/// This is also known as "Uploading assets"
		///
		/// Parameters:
		/// - `to`: The account ID the asset is uploaded to on the Compound chain.
		/// - `currency_id`: collateral currency id.
		/// - `locked_amount`: The amount of user asset to be "uploaded" onto the Compound chain.
		//#[pallet::weight(< T as Config >::WeightInfo::lock_to())]
		#[pallet::weight(0)]
		#[transactional]
		pub fn lock_to(
			origin: OriginFor<T>,
			to: T::AccountId,
			currency_id: CurrencyId,
			locked_amount: Balance,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			Self::do_lock_to(from, to, currency_id, locked_amount)
		}

		/// Invoke a Notice issued from Compound chain via its Gateways.
		///
		/// Parameters:
		/// - `notice`: The Notice issued by Compound Gateway. Contains data to be invoked.
		/// - `signatures`: Represents approvals by given authorities. Used to verify the
		/// authenticity of the notice.
		//#[pallet::weight(< T as Config >::WeightInfo::invoke())]
		#[pallet::weight(0)]
		#[transactional]
		pub fn invoke(
			origin: OriginFor<T>,
			notice: GatewayNotice<T::AccountId>,
			signatures: Vec<CompoundAuthoritySignature>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			// Calculate the hash for this notice, and ensure it is only invoked once.
			let hash = BlakeTwo256::hash(&notice.encode());

			ensure!(
				!InvokedNoticeHashes::<T>::contains_key(&hash),
				Error::<T>::NoticeAlreadyInvoked
			);

			// verify the signatures
			ensure!(
				Self::verify_compound_authority_signature(signatures),
				Error::<T>::InsufficientValidNoticeSignatures
			);

			match notice.payload {
				GatewayNoticePayload::SetSupplyCap(currency_id, amount) => {
					SupplyCaps::<T>::insert(&currency_id, amount);
					Self::deposit_event(Event::<T>::SupplyCapSet {
						currency_id,
						new_cap: amount,
					});
					Ok(())
				}
				GatewayNoticePayload::ChangeAuthorities(new_authorities) => {
					ensure!(
						new_authorities.len() <= (T::MaxGatewayAuthorities::get() as usize),
						Error::<T>::ExceededMaxNumberOfAuthorities
					);
					ensure!(!new_authorities.is_empty(), Error::<T>::AuthoritiesListCannotBeEmpty);

					let bounded_vec = BoundedVec::try_from(new_authorities).unwrap();
					GatewayAuthorities::<T>::put(bounded_vec);
					Self::deposit_event(Event::<T>::GatewayAuthoritiesChanged);
					Ok(())
				}
				GatewayNoticePayload::Unlock {
					currency_id,
					amount,
					who,
				} => Self::do_unlock(currency_id, amount, who),
				GatewayNoticePayload::SetFutureYield {
					next_cash_yield,
					next_cash_yield_index,
					next_cash_yield_start,
				} => {
					T::Cash::set_future_yield(next_cash_yield, next_cash_yield_index, next_cash_yield_start)?;
					Self::deposit_event(Event::<T>::FutureYieldSet {
						yield_amount: next_cash_yield,
						index: next_cash_yield_index,
						timestamp: next_cash_yield_start,
					});
					Ok(())
				}
			}?;

			// After its invocation, store the hash.
			InvokedNoticeHashes::<T>::insert(&hash, ());

			Ok(PostDispatchInfo {
				actual_weight: Some(0),
				pays_fee: Pays::No,
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	#[require_transactional]
	fn do_lock_to(
		from: T::AccountId,
		to: T::AccountId,
		currency_id: CurrencyId,
		locked_amount: Balance,
	) -> DispatchResult {
		// Ensure the user has sufficient balance
		T::Currency::ensure_can_withdraw(currency_id, &from, locked_amount)?;

		let current_supply_cap = Self::supply_caps(currency_id);
		// Ensure there are enough supplies on Compound.
		ensure!(
			current_supply_cap >= locked_amount,
			Error::<T>::InsufficientAssetSupplyCap
		);

		// If the currency is CASH, it is burned
		// All other tokens are transferred to the admin's account.
		match currency_id {
			CurrencyId::Token(TokenSymbol::CASH) => T::Currency::withdraw(currency_id, &from, locked_amount),
			_ => T::Currency::transfer(
				currency_id,
				&from,
				&T::PalletId::get().into_account_truncating(),
				locked_amount,
			),
		}?;

		// Fund locked. Now reduce the supply caps
		SupplyCaps::<T>::insert(&currency_id, current_supply_cap - locked_amount);

		// emit an event
		Self::deposit_event(Event::<T>::AssetLockedTo {
			currency_id,
			amount: locked_amount,
			user: to,
		});

		Ok(())
	}

	#[require_transactional]
	fn do_unlock(currency_id: CurrencyId, unlock_amount: Balance, to: T::AccountId) -> DispatchResult {
		// If the currency is CASH, mint into the user's account
		// All other tokens are transferred from the admin's account.
		match currency_id {
			CurrencyId::Token(TokenSymbol::CASH) => T::Currency::deposit(currency_id, &to, unlock_amount),
			_ => {
				// Ensure the admin has sufficient balance for the transfer
				ensure!(
					T::Currency::ensure_can_withdraw(
						currency_id,
						&T::PalletId::get().into_account_truncating(),
						unlock_amount
					)
					.is_ok(),
					Error::<T>::InsufficientAssetToUnlock
				);
				T::Currency::transfer(
					currency_id,
					&T::PalletId::get().into_account_truncating(),
					&to,
					unlock_amount,
				)
			}
		}?;

		// emit an event
		Self::deposit_event(Event::<T>::AssetUnlocked {
			currency_id,
			amount: unlock_amount,
			user: to,
		});

		Ok(())
	}

	/// Verifies if the given signature is sufficient to prove the authenticity of the Notice.
	fn verify_compound_authority_signature(signatures: Vec<CompoundAuthoritySignature>) -> bool {
		let mut count: u32 = 0;
		for signatory in Self::gateway_authorities() {
			if signatures.iter().position(|x| *x == signatory) != None {
				// TODO: How to verify signature? Are we simply doing a string matching to the addresses?
				count += 1;
			}
		}

		// check if enough signatures has been acquired.
		Perbill::from_rational(count, Self::gateway_authorities().len() as u32)
			>= T::PercentThresholdForAuthoritySignature::get()
	}
}
