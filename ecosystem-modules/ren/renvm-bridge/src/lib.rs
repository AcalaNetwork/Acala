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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::Encode;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Currency, Get},
};
use frame_system::{ensure_none, ensure_signed, pallet_prelude::*};
use orml_traits::BasicCurrency;
use primitives::Balance;
use sp_core::ecdsa;
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::{
	traits::Zero,
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	ArithmeticError, DispatchResult,
};
use sp_std::vec::Vec;
use support::TransactionPayment;

mod mock;
mod tests;

pub use module::*;

pub type EcdsaSignature = ecdsa::Signature;
type PublicKey = [u8; 20];
type DestAddress = Vec<u8>;
const MINT_TX_LENGTH: u32 = 168;

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
		type BridgedTokenCurrency: BasicCurrency<Self::AccountId, Balance = Balance>;
		/// The RenVM Currency identifier
		#[pallet::constant]
		type CurrencyIdentifier: Get<[u8; 32]>;
		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple modules send unsigned transactions.
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;
		/// Charge mint fee.
		type ChargeTransactionPayment: TransactionPayment<Self::AccountId, BalanceOf<Self>, NegativeImbalanceOf<Self>>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The RenVM split public key is invalid.
		InvalidRenVmPublicKey,
		/// The mint signature is invalid.
		InvalidMintSignature,
		/// The mint signature has already been used.
		SignatureAlreadyUsed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Asset minted.
		Minted { owner: T::AccountId, amount: Balance },
		/// Asset burnt in this chain.
		Burnt {
			owner: T::AccountId,
			dest: DestAddress,
			amount: Balance,
		},
		/// Rotated key
		RotatedKey { key: PublicKey },
	}

	/// The RenVM split public key
	#[pallet::storage]
	#[pallet::getter(fn ren_vm_public_key)]
	pub type RenVmPublicKey<T: Config> = StorageValue<_, PublicKey, OptionQuery>;

	/// Signature blacklist. This is required to prevent double claim.
	#[pallet::storage]
	#[pallet::getter(fn signatures)]
	pub type Signatures<T: Config> = StorageMap<_, Twox64Concat, EcdsaSignature, (), OptionQuery>;

	/// Record burn event details
	#[pallet::storage]
	#[pallet::getter(fn burn_events)]
	type BurnEvents<T: Config> = StorageMap<_, Twox64Concat, u32, (T::BlockNumber, DestAddress, Balance), OptionQuery>;

	/// Next burn event ID
	#[pallet::storage]
	#[pallet::getter(fn next_burn_event_id)]
	type NextBurnEventId<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::genesis_config]
	#[cfg_attr(feature = "std", derive(Default))]
	pub struct GenesisConfig {
		pub ren_vm_public_key: PublicKey,
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			RenVmPublicKey::<T>::set(Some(self.ren_vm_public_key));
		}
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Allow a user to mint if they have a valid signature from RenVM.
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// Verify input by `validate_unsigned`
		#[pallet::weight(10_000)]
		pub fn mint(
			origin: OriginFor<T>,
			who: T::AccountId,
			_p_hash: [u8; 32],
			#[pallet::compact] amount: Balance,
			_n_hash: [u8; 32],
			sig: EcdsaSignature,
		) -> DispatchResult {
			ensure_none(origin)?;
			Self::do_mint(&who, amount, &sig)?;

			// TODO: update by benchmarks.
			let weight: Weight = 10_000;

			// charge mint fee. Ignore the result, if it failed, only lost the fee.
			let _ = T::ChargeTransactionPayment::charge_fee(
				&who,
				MINT_TX_LENGTH,
				weight,
				Zero::zero(),
				Pays::Yes,
				DispatchClass::Normal,
			);
			Self::deposit_event(Event::Minted { owner: who, amount });

			Ok(())
		}

		/// Allow a user to burn assets.
		#[pallet::weight(10_000)]
		pub fn burn(origin: OriginFor<T>, to: DestAddress, #[pallet::compact] amount: Balance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			NextBurnEventId::<T>::try_mutate(|id| -> DispatchResult {
				let this_id = *id;
				*id = id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

				T::BridgedTokenCurrency::withdraw(&sender, amount)?;
				BurnEvents::<T>::insert(this_id, (frame_system::Pallet::<T>::block_number(), &to, amount));
				Self::deposit_event(Event::Burnt {
					owner: sender,
					dest: to,
					amount,
				});

				Ok(())
			})?;

			Ok(())
		}

		/// Allow RenVm rotate the public key.
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// Verify input by `validate_unsigned`
		#[pallet::weight(10_000)]
		pub fn rotate_key(origin: OriginFor<T>, new_key: PublicKey, sig: EcdsaSignature) -> DispatchResult {
			ensure_none(origin)?;
			Self::do_rotate_key(new_key, sig);
			Self::deposit_event(Event::RotatedKey { key: new_key });

			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> frame_support::unsigned::ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::mint {
					who,
					p_hash,
					amount,
					n_hash,
					sig,
				} => {
					// check if already exists
					if Signatures::<T>::contains_key(&sig) {
						return InvalidTransaction::Stale.into();
					}

					let verify_result = Encode::using_encoded(&who, |encoded| -> DispatchResult {
						Self::verify_mint_signature(p_hash, *amount, encoded, n_hash, &sig.0)
					});

					// verify signature
					if verify_result.is_err() {
						return InvalidTransaction::BadProof.into();
					}

					ValidTransaction::with_tag_prefix("renvm-bridge")
						.priority(T::UnsignedPriority::get())
						.and_provides(sig)
						.longevity(64_u64)
						.propagate(true)
						.build()
				}
				Call::rotate_key { new_key, sig } => {
					// check if already exists
					if Signatures::<T>::contains_key(&sig) {
						return InvalidTransaction::Stale.into();
					}

					// verify signature
					if Self::verify_rotate_key_signature(new_key, &sig.0).is_err() {
						return InvalidTransaction::BadProof.into();
					}

					ValidTransaction::with_tag_prefix("renvm-bridge")
						.priority(T::UnsignedPriority::get())
						.and_provides(sig)
						.longevity(64_u64)
						.propagate(true)
						.build()
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	fn do_mint(sender: &T::AccountId, amount: Balance, sig: &EcdsaSignature) -> DispatchResult {
		T::BridgedTokenCurrency::deposit(sender, amount)?;
		Signatures::<T>::insert(sig, ());

		Ok(())
	}

	fn do_rotate_key(new_key: PublicKey, sig: EcdsaSignature) {
		RenVmPublicKey::<T>::set(Some(new_key));
		Signatures::<T>::insert(&sig, ());
	}

	// ABI-encode the values for creating the signature hash.
	fn signable_mint_message(
		p_hash: &[u8; 32],
		amount: u128,
		to: &[u8],
		n_hash: &[u8; 32],
		token: &[u8; 32],
	) -> Vec<u8> {
		// p_hash ++ amount ++ token ++ to ++ n_hash
		let length = 32 + 32 + 32 + 32 + 32;
		let mut v = Vec::with_capacity(length);
		v.extend_from_slice(&p_hash[..]);
		v.extend_from_slice(&[0u8; 16][..]);
		v.extend_from_slice(&amount.to_be_bytes()[..]);
		v.extend_from_slice(&token[..]);
		v.extend_from_slice(to);
		v.extend_from_slice(&n_hash[..]);
		v
	}

	// Verify that the signature has been signed by RenVM.
	fn verify_mint_signature(
		p_hash: &[u8; 32],
		amount: Balance,
		to: &[u8],
		n_hash: &[u8; 32],
		sig: &[u8; 65],
	) -> DispatchResult {
		let ren_btc_identifier = T::CurrencyIdentifier::get();

		let signed_message_hash = keccak_256(&Self::signable_mint_message(
			p_hash,
			amount,
			to,
			n_hash,
			&ren_btc_identifier,
		));
		let recoverd =
			secp256k1_ecdsa_recover(sig, &signed_message_hash).map_err(|_| Error::<T>::InvalidMintSignature)?;
		let addr = &keccak_256(&recoverd)[12..];

		let pubkey = RenVmPublicKey::<T>::get().ok_or(Error::<T>::InvalidRenVmPublicKey)?;
		ensure!(addr == pubkey, Error::<T>::InvalidMintSignature);

		Ok(())
	}

	fn signable_rotate_key_message(new_key: &PublicKey) -> Vec<u8> {
		// new_key
		let length = 20;
		let mut v = Vec::with_capacity(length);
		v.extend_from_slice(&new_key[..]);
		v
	}

	// Verify that the signature has been signed by RenVM.
	fn verify_rotate_key_signature(new_key: &PublicKey, sig: &[u8; 65]) -> DispatchResult {
		let signed_message_hash = keccak_256(&Self::signable_rotate_key_message(new_key));
		let recoverd =
			secp256k1_ecdsa_recover(sig, &signed_message_hash).map_err(|_| Error::<T>::InvalidMintSignature)?;
		let addr = &keccak_256(&recoverd)[12..];

		let pubkey = RenVmPublicKey::<T>::get().ok_or(Error::<T>::InvalidRenVmPublicKey)?;
		ensure!(addr == pubkey, Error::<T>::InvalidMintSignature);

		Ok(())
	}
}
