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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use enumflags2::BitFlags;
use frame_support::{
	pallet_prelude::*,
	traits::{
		Currency,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		NamedReservableCurrency,
	},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use orml_traits::NFT;
use primitives::{NFTBalance, ReserveIdentifier};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, Hash, Saturating, StaticLookup, Zero},
	DispatchResult, RuntimeDebug,
};
use sp_std::vec::Vec;

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

pub type CID = Vec<u8>;

#[repr(u8)]
#[derive(Encode, Decode, Clone, Copy, BitFlags, RuntimeDebug, PartialEq, Eq)]
pub enum ClassProperty {
	/// Token can be transferred
	Transferable = 0b00000001,
	/// Token can be burned
	Burnable = 0b00000010,
}

#[derive(Clone, Copy, PartialEq, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Properties(pub BitFlags<ClassProperty>);

impl Eq for Properties {}
impl Encode for Properties {
	fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
		self.0.bits().using_encoded(f)
	}
}
impl Decode for Properties {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let field = u8::decode(input)?;
		Ok(Self(
			<BitFlags<ClassProperty>>::from_bits(field as u8).map_err(|_| "invalid value")?,
		))
	}
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ClassData<Balance> {
	/// The minimum balance to create class
	pub deposit: Balance,
	/// Property of token
	pub properties: Properties,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenData<Balance> {
	/// The minimum balance to create token
	pub deposit: Balance,
}

pub type TokenIdOf<T> = <T as orml_nft::Config>::TokenId;
pub type ClassIdOf<T> = <T as orml_nft::Config>::ClassId;
pub type BalanceOf<T> =
	<<T as pallet_proxy::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::Nft;

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ orml_nft::Config<ClassData = ClassData<BalanceOf<Self>>, TokenData = TokenData<BalanceOf<Self>>>
		+ pallet_proxy::Config
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Currency type for reserve balance.
		type Currency: NamedReservableCurrency<
			Self::AccountId,
			Balance = BalanceOf<Self>,
			ReserveIdentifier = ReserveIdentifier,
		>;

		/// The minimum balance to create class
		#[pallet::constant]
		type CreateClassDeposit: Get<BalanceOf<Self>>;

		/// The minimum balance to create token
		#[pallet::constant]
		type CreateTokenDeposit: Get<BalanceOf<Self>>;

		/// Deposit required for per byte.
		#[pallet::constant]
		type DataDepositPerByte: Get<BalanceOf<Self>>;

		/// The NFT's module id
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// ClassId not found
		ClassIdNotFound,
		/// TokenId not found
		TokenIdNotFound,
		/// The operator is not the owner of the token and has no permission
		NoPermission,
		/// Quantity is invalid. need >= 1
		InvalidQuantity,
		/// Property of class don't support transfer
		NonTransferable,
		/// Property of class don't support burn
		NonBurnable,
		/// Can not destroy class
		/// Total issuance is not 0
		CannotDestroyClass,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Created NFT class. \[owner, class_id\]
		CreatedClass(T::AccountId, ClassIdOf<T>),
		/// Minted NFT token. \[from, to, class_id, quantity\]
		MintedToken(T::AccountId, T::AccountId, ClassIdOf<T>, u32),
		/// Transferred NFT token. \[from, to, class_id, token_id\]
		TransferredToken(T::AccountId, T::AccountId, ClassIdOf<T>, TokenIdOf<T>),
		/// Burned NFT token. \[owner, class_id, token_id\]
		BurnedToken(T::AccountId, ClassIdOf<T>, TokenIdOf<T>),
		/// Burned NFT token with remark. \[owner, class_id, token_id, remark_hash\]
		BurnedTokenWithRemark(T::AccountId, ClassIdOf<T>, TokenIdOf<T>, T::Hash),
		/// Destroyed NFT class. \[owner, class_id\]
		DestroyedClass(T::AccountId, ClassIdOf<T>),
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create NFT class, tokens belong to the class.
		///
		/// - `metadata`: external metadata
		/// - `properties`: class property, include `Transferable` `Burnable`
		#[pallet::weight(<T as Config>::WeightInfo::create_class())]
		#[transactional]
		pub fn create_class(origin: OriginFor<T>, metadata: CID, properties: Properties) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let next_id = orml_nft::Pallet::<T>::next_class_id();
			let owner: T::AccountId = T::PalletId::get().into_sub_account(next_id);
			let class_deposit = T::CreateClassDeposit::get();

			let data_deposit = T::DataDepositPerByte::get().saturating_mul((metadata.len() as u32).into());
			let proxy_deposit = <pallet_proxy::Pallet<T>>::deposit(1u32);
			let deposit = class_deposit.saturating_add(data_deposit);
			let total_deposit = proxy_deposit.saturating_add(deposit);

			// ensure enough token for proxy deposit + class deposit + data deposit
			<T as module::Config>::Currency::transfer(&who, &owner, total_deposit, KeepAlive)?;

			<T as module::Config>::Currency::reserve_named(&RESERVE_ID, &owner, deposit)?;

			// owner add proxy delegate to origin
			<pallet_proxy::Pallet<T>>::add_proxy_delegate(&owner, who, Default::default(), Zero::zero())?;

			let data = ClassData { deposit, properties };
			orml_nft::Pallet::<T>::create_class(&owner, metadata, data)?;

			Self::deposit_event(Event::CreatedClass(owner, next_id));
			Ok(().into())
		}

		/// Mint NFT token
		///
		/// - `to`: the token owner's account
		/// - `class_id`: token belong to the class id
		/// - `metadata`: external metadata
		/// - `quantity`: token quantity
		#[pallet::weight(<T as Config>::WeightInfo::mint(*quantity))]
		#[transactional]
		pub fn mint(
			origin: OriginFor<T>,
			to: <T::Lookup as StaticLookup>::Source,
			class_id: ClassIdOf<T>,
			metadata: CID,
			quantity: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			ensure!(quantity >= 1, Error::<T>::InvalidQuantity);
			let class_info = orml_nft::Pallet::<T>::classes(class_id).ok_or(Error::<T>::ClassIdNotFound)?;
			ensure!(who == class_info.owner, Error::<T>::NoPermission);
			let deposit = T::CreateTokenDeposit::get();
			let total_deposit = deposit.saturating_mul(quantity.into());

			// `repatriate_reserved` will check `to` account exist and may return
			// `DeadAccount`.
			<T as module::Config>::Currency::transfer(&who, &to, total_deposit, KeepAlive)?;
			<T as module::Config>::Currency::reserve_named(&RESERVE_ID, &to, total_deposit)?;

			let data = TokenData { deposit };
			for _ in 0..quantity {
				orml_nft::Pallet::<T>::mint(&to, class_id, metadata.clone(), data.clone())?;
			}

			Self::deposit_event(Event::MintedToken(who, to, class_id, quantity));
			Ok(().into())
		}

		/// Transfer NFT token to another account
		///
		/// - `to`: the token owner's account
		/// - `token`: (class_id, token_id)
		#[pallet::weight(<T as Config>::WeightInfo::transfer())]
		#[transactional]
		pub fn transfer(
			origin: OriginFor<T>,
			to: <T::Lookup as StaticLookup>::Source,
			token: (ClassIdOf<T>, TokenIdOf<T>),
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			Self::do_transfer(&who, &to, token)?;
			Ok(().into())
		}

		/// Burn NFT token
		///
		/// - `token`: (class_id, token_id)
		#[pallet::weight(<T as Config>::WeightInfo::burn())]
		#[transactional]
		pub fn burn(origin: OriginFor<T>, token: (ClassIdOf<T>, TokenIdOf<T>)) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_burn(&who, token)?;
			Self::deposit_event(Event::BurnedToken(who, token.0, token.1));
			Ok(().into())
		}

		/// Burn NFT token
		///
		/// - `token`: (class_id, token_id)
		/// - `remark`: Vec<u8>
		#[pallet::weight(<T as Config>::WeightInfo::burn_with_remark(remark.len() as u32))]
		#[transactional]
		pub fn burn_with_remark(
			origin: OriginFor<T>,
			token: (ClassIdOf<T>, TokenIdOf<T>),
			remark: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_burn(&who, token)?;
			let hash = T::Hashing::hash(&remark[..]);
			Self::deposit_event(Event::BurnedTokenWithRemark(who, token.0, token.1, hash));
			Ok(().into())
		}

		/// Destroy NFT class, remove dest from proxy, and send all the free
		/// balance to dest
		///
		/// - `class_id`: The class ID to destroy
		/// - `dest`: The proxy account that will receive free balance
		#[pallet::weight(<T as Config>::WeightInfo::destroy_class())]
		#[transactional]
		pub fn destroy_class(
			origin: OriginFor<T>,
			class_id: ClassIdOf<T>,
			dest: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			let class_info = orml_nft::Pallet::<T>::classes(class_id).ok_or(Error::<T>::ClassIdNotFound)?;
			ensure!(who == class_info.owner, Error::<T>::NoPermission);
			ensure!(
				class_info.total_issuance == Zero::zero(),
				Error::<T>::CannotDestroyClass
			);

			let data = class_info.data;

			<T as module::Config>::Currency::unreserve_named(&RESERVE_ID, &who, data.deposit);

			orml_nft::Pallet::<T>::destroy_class(&who, class_id)?;

			// this should unresere proxy deposit
			pallet_proxy::Pallet::<T>::remove_proxy_delegate(&who, dest.clone(), Default::default(), Zero::zero())?;

			<T as module::Config>::Currency::transfer(
				&who,
				&dest,
				<T as module::Config>::Currency::free_balance(&who),
				AllowDeath,
			)?;

			Self::deposit_event(Event::DestroyedClass(who, class_id));
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Ensured atomic.
	#[transactional]
	fn do_transfer(from: &T::AccountId, to: &T::AccountId, token: (ClassIdOf<T>, TokenIdOf<T>)) -> DispatchResult {
		let class_info = orml_nft::Pallet::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
		let data = class_info.data;
		ensure!(
			data.properties.0.contains(ClassProperty::Transferable),
			Error::<T>::NonTransferable
		);

		let token_info = orml_nft::Pallet::<T>::tokens(token.0, token.1).ok_or(Error::<T>::TokenIdNotFound)?;

		orml_nft::Pallet::<T>::transfer(from, to, token)?;

		<T as module::Config>::Currency::unreserve_named(&RESERVE_ID, &from, token_info.data.deposit);
		<T as module::Config>::Currency::transfer(&from, &to, token_info.data.deposit, AllowDeath)?;
		<T as module::Config>::Currency::reserve_named(&RESERVE_ID, &to, token_info.data.deposit)?;

		Self::deposit_event(Event::TransferredToken(from.clone(), to.clone(), token.0, token.1));
		Ok(())
	}

	/// Ensured atomic.
	#[transactional]
	fn do_burn(who: &T::AccountId, token: (ClassIdOf<T>, TokenIdOf<T>)) -> DispatchResult {
		let class_info = orml_nft::Pallet::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
		let data = class_info.data;
		ensure!(
			data.properties.0.contains(ClassProperty::Burnable),
			Error::<T>::NonBurnable
		);

		let token_info = orml_nft::Pallet::<T>::tokens(token.0, token.1).ok_or(Error::<T>::TokenIdNotFound)?;
		ensure!(*who == token_info.owner, Error::<T>::NoPermission);

		orml_nft::Pallet::<T>::burn(&who, token)?;

		<T as module::Config>::Currency::unreserve_named(&RESERVE_ID, &who, token_info.data.deposit);
		Ok(())
	}
}

impl<T: Config> NFT<T::AccountId> for Pallet<T> {
	type ClassId = ClassIdOf<T>;
	type TokenId = TokenIdOf<T>;
	type Balance = NFTBalance;

	fn balance(who: &T::AccountId) -> Self::Balance {
		orml_nft::TokensByOwner::<T>::iter_prefix(who).count() as u128
	}

	fn owner(token: (Self::ClassId, Self::TokenId)) -> Option<T::AccountId> {
		orml_nft::Pallet::<T>::tokens(token.0, token.1).map(|t| t.owner)
	}

	fn transfer(from: &T::AccountId, to: &T::AccountId, token: (Self::ClassId, Self::TokenId)) -> DispatchResult {
		Self::do_transfer(from, to, token)
	}
}
