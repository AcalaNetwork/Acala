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
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{
	pallet_prelude::*,
	require_transactional,
	traits::{
		tokens::nonfungibles::{Inspect, Mutate, Transfer},
		Currency,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		NamedReservableCurrency,
	},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use orml_traits::InspectExtended;
use primitives::{
	nft::{Attributes, ClassProperty, NFTBalance, Properties, CID},
	ReserveIdentifier,
};
use scale_info::TypeInfo;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, Hash, Saturating, StaticLookup, Zero},
	DispatchResult, RuntimeDebug,
};
use sp_std::prelude::*;

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ClassData<Balance> {
	/// Deposit reserved to create token class
	pub deposit: Balance,
	/// Class properties
	pub properties: Properties,
	/// Class attributes
	pub attributes: Attributes,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenData<Balance> {
	/// Deposit reserved to create token
	pub deposit: Balance,
	/// Token attributes
	pub attributes: Attributes,
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

		/// Maximum number of bytes in attributes
		#[pallet::constant]
		type MaxAttributesBytes: Get<u32>;

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
		/// Property of class don't support mint
		NonMintable,
		/// Can not destroy class
		/// Total issuance is not 0
		CannotDestroyClass,
		/// Cannot perform mutable action
		Immutable,
		/// Attributes too large
		AttributesTooLarge,
		/// The given token ID is not correct
		IncorrectTokenId,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Created NFT class.
		CreatedClass {
			owner: T::AccountId,
			class_id: ClassIdOf<T>,
		},
		/// Minted NFT token.
		MintedToken {
			from: T::AccountId,
			to: T::AccountId,
			class_id: ClassIdOf<T>,
			quantity: u32,
		},
		/// Transferred NFT token.
		TransferredToken {
			from: T::AccountId,
			to: T::AccountId,
			class_id: ClassIdOf<T>,
			token_id: TokenIdOf<T>,
		},
		/// Burned NFT token.
		BurnedToken {
			owner: T::AccountId,
			class_id: ClassIdOf<T>,
			token_id: TokenIdOf<T>,
		},
		/// Burned NFT token with remark.
		BurnedTokenWithRemark {
			owner: T::AccountId,
			class_id: ClassIdOf<T>,
			token_id: TokenIdOf<T>,
			remark_hash: T::Hash,
		},
		/// Destroyed NFT class.
		DestroyedClass {
			owner: T::AccountId,
			class_id: ClassIdOf<T>,
		},
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
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
		pub fn create_class(
			origin: OriginFor<T>,
			metadata: CID,
			properties: Properties,
			attributes: Attributes,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let next_id = orml_nft::Pallet::<T>::next_class_id();
			let owner: T::AccountId = T::PalletId::get().into_sub_account_truncating(next_id);
			let class_deposit = T::CreateClassDeposit::get();

			let data_deposit = Self::data_deposit(&metadata, &attributes)?;
			let proxy_deposit = <pallet_proxy::Pallet<T>>::deposit(1u32);
			let deposit = class_deposit.saturating_add(data_deposit);
			let total_deposit = proxy_deposit.saturating_add(deposit);

			// ensure enough token for proxy deposit + class deposit + data deposit
			<T as module::Config>::Currency::transfer(&who, &owner, total_deposit, KeepAlive)?;

			<T as module::Config>::Currency::reserve_named(&RESERVE_ID, &owner, deposit)?;

			// owner add proxy delegate to origin
			<pallet_proxy::Pallet<T>>::add_proxy_delegate(&owner, who, Default::default(), Zero::zero())?;

			let data = ClassData {
				deposit,
				properties,
				attributes,
			};
			orml_nft::Pallet::<T>::create_class(&owner, metadata, data)?;

			Self::deposit_event(Event::CreatedClass {
				owner,
				class_id: next_id,
			});
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
			attributes: Attributes,
			#[pallet::compact] quantity: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			Self::do_mint(&who, &to, class_id, metadata, attributes, quantity)?;
			Ok(())
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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			Self::do_transfer(&who, &to, token)
		}

		/// Burn NFT token
		///
		/// - `token`: (class_id, token_id)
		#[pallet::weight(<T as Config>::WeightInfo::burn())]
		#[transactional]
		pub fn burn(origin: OriginFor<T>, token: (ClassIdOf<T>, TokenIdOf<T>)) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_burn(who, token, None)
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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_burn(who, token, Some(remark))
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

			Self::deposit_event(Event::DestroyedClass { owner: who, class_id });
			Ok(().into())
		}

		/// Update NFT class properties. The current class properties must contains
		/// ClassPropertiesMutable.
		///
		/// - `class_id`: The class ID to update
		/// - `properties`: The new properties
		#[pallet::weight(<T as Config>::WeightInfo::update_class_properties())]
		#[transactional]
		pub fn update_class_properties(
			origin: OriginFor<T>,
			class_id: ClassIdOf<T>,
			properties: Properties,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			orml_nft::Classes::<T>::try_mutate(class_id, |class_info| {
				let class_info = class_info.as_mut().ok_or(Error::<T>::ClassIdNotFound)?;
				ensure!(who == class_info.owner, Error::<T>::NoPermission);

				let mut data = &mut class_info.data;
				ensure!(
					data.properties.0.contains(ClassProperty::ClassPropertiesMutable),
					Error::<T>::Immutable
				);

				data.properties = properties;

				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	#[require_transactional]
	fn do_transfer(from: &T::AccountId, to: &T::AccountId, token: (ClassIdOf<T>, TokenIdOf<T>)) -> DispatchResult {
		let class_info = orml_nft::Pallet::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
		let data = class_info.data;
		ensure!(
			data.properties.0.contains(ClassProperty::Transferable),
			Error::<T>::NonTransferable
		);

		let token_info = orml_nft::Pallet::<T>::tokens(token.0, token.1).ok_or(Error::<T>::TokenIdNotFound)?;

		orml_nft::Pallet::<T>::transfer(from, to, token)?;

		<T as module::Config>::Currency::unreserve_named(&RESERVE_ID, from, token_info.data.deposit);
		<T as module::Config>::Currency::transfer(from, to, token_info.data.deposit, AllowDeath)?;
		<T as module::Config>::Currency::reserve_named(&RESERVE_ID, to, token_info.data.deposit)?;

		Self::deposit_event(Event::TransferredToken {
			from: from.clone(),
			to: to.clone(),
			class_id: token.0,
			token_id: token.1,
		});
		Ok(())
	}

	#[require_transactional]
	fn do_mint(
		who: &T::AccountId,
		to: &T::AccountId,
		class_id: ClassIdOf<T>,
		metadata: CID,
		attributes: Attributes,
		quantity: u32,
	) -> Result<Vec<TokenIdOf<T>>, DispatchError> {
		ensure!(quantity >= 1, Error::<T>::InvalidQuantity);
		let class_info = orml_nft::Pallet::<T>::classes(class_id).ok_or(Error::<T>::ClassIdNotFound)?;
		ensure!(*who == class_info.owner, Error::<T>::NoPermission);

		ensure!(
			class_info.data.properties.0.contains(ClassProperty::Mintable),
			Error::<T>::NonMintable
		);

		let data_deposit = Self::data_deposit(&metadata, &attributes)?;
		let deposit = T::CreateTokenDeposit::get().saturating_add(data_deposit);
		let total_deposit = deposit.saturating_mul(quantity.into());

		// `repatriate_reserved` will check `to` account exist and may return
		// `DeadAccount`.
		<T as module::Config>::Currency::transfer(who, to, total_deposit, KeepAlive)?;
		<T as module::Config>::Currency::reserve_named(&RESERVE_ID, to, total_deposit)?;

		let mut token_ids = Vec::with_capacity(quantity as usize);
		let data = TokenData { deposit, attributes };
		for _ in 0..quantity {
			token_ids.push(orml_nft::Pallet::<T>::mint(
				to,
				class_id,
				metadata.clone(),
				data.clone(),
			)?);
		}

		Self::deposit_event(Event::MintedToken {
			from: who.clone(),
			to: to.clone(),
			class_id,
			quantity,
		});
		Ok(token_ids)
	}

	fn do_burn(who: T::AccountId, token: (ClassIdOf<T>, TokenIdOf<T>), remark: Option<Vec<u8>>) -> DispatchResult {
		let class_info = orml_nft::Pallet::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
		let data = class_info.data;
		ensure!(
			data.properties.0.contains(ClassProperty::Burnable),
			Error::<T>::NonBurnable
		);

		let token_info = orml_nft::Pallet::<T>::tokens(token.0, token.1).ok_or(Error::<T>::TokenIdNotFound)?;
		ensure!(who == token_info.owner, Error::<T>::NoPermission);

		orml_nft::Pallet::<T>::burn(&who, token)?;

		<T as module::Config>::Currency::unreserve_named(&RESERVE_ID, &who, token_info.data.deposit);

		if let Some(remark) = remark {
			let hash = T::Hashing::hash(&remark[..]);
			Self::deposit_event(Event::BurnedTokenWithRemark {
				owner: who,
				class_id: token.0,
				token_id: token.1,
				remark_hash: hash,
			});
		} else {
			Self::deposit_event(Event::BurnedToken {
				owner: who,
				class_id: token.0,
				token_id: token.1,
			});
		}

		Ok(())
	}

	fn data_deposit(metadata: &[u8], attributes: &Attributes) -> Result<BalanceOf<T>, DispatchError> {
		// Addition can't overflow because we will be out of memory before that
		let attributes_len = attributes.iter().fold(0, |acc, (k, v)| {
			acc.saturating_add(v.len().saturating_add(k.len()) as u32)
		});

		ensure!(
			attributes_len <= T::MaxAttributesBytes::get(),
			Error::<T>::AttributesTooLarge
		);

		let total_data_len = attributes_len.saturating_add(metadata.len() as u32);
		Ok(T::DataDepositPerByte::get().saturating_mul(total_data_len.into()))
	}
}

impl<T: Config> InspectExtended<T::AccountId> for Pallet<T> {
	type Balance = NFTBalance;

	fn balance(who: &T::AccountId) -> Self::Balance {
		orml_nft::TokensByOwner::<T>::iter_prefix((who,)).count() as u128
	}

	fn next_token_id(class: Self::CollectionId) -> Self::ItemId {
		orml_nft::Pallet::<T>::next_token_id(class)
	}
}

impl<T: Config> Inspect<T::AccountId> for Pallet<T> {
	type ItemId = TokenIdOf<T>;
	type CollectionId = ClassIdOf<T>;

	fn owner(class: &Self::CollectionId, instance: &Self::ItemId) -> Option<T::AccountId> {
		orml_nft::Pallet::<T>::tokens(class, instance).map(|t| t.owner)
	}

	fn collection_owner(class: &Self::CollectionId) -> Option<T::AccountId> {
		orml_nft::Pallet::<T>::classes(class).map(|c| c.owner)
	}

	fn can_transfer(class: &Self::CollectionId, _: &Self::ItemId) -> bool {
		orml_nft::Pallet::<T>::classes(class).map_or(false, |class_info| {
			class_info.data.properties.0.contains(ClassProperty::Transferable)
		})
	}
}

impl<T: Config> Mutate<T::AccountId> for Pallet<T> {
	/// Mint some asset `instance` of `class` to be owned by `who`.
	fn mint_into(class: &Self::CollectionId, instance: &Self::ItemId, who: &T::AccountId) -> DispatchResult {
		// Ensure the next token ID is correct
		ensure!(
			orml_nft::Pallet::<T>::next_token_id(class) == *instance,
			Error::<T>::IncorrectTokenId
		);

		let class_owner =
			<Self as Inspect<T::AccountId>>::collection_owner(class).ok_or(Error::<T>::ClassIdNotFound)?;
		Self::do_mint(&class_owner, who, *class, Default::default(), Default::default(), 1u32)?;
		Ok(())
	}

	/// Burn some asset `instance` of `class`.
	fn burn(
		class: &Self::CollectionId,
		instance: &Self::ItemId,
		_maybe_check_owner: Option<&T::AccountId>,
	) -> DispatchResult {
		let owner = <Self as Inspect<T::AccountId>>::owner(class, instance).ok_or(Error::<T>::TokenIdNotFound)?;
		Self::do_burn(owner, (*class, *instance), None)
	}
}

impl<T: Config> Transfer<T::AccountId> for Pallet<T> {
	/// Transfer asset `instance` of `class` into `destination` account.
	fn transfer(class: &Self::CollectionId, instance: &Self::ItemId, destination: &T::AccountId) -> DispatchResult {
		let owner = <Self as Inspect<T::AccountId>>::owner(class, instance).ok_or(Error::<T>::TokenIdNotFound)?;
		Self::do_transfer(&owner, destination, (*class, *instance))
	}
}
