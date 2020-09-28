#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, ensure,
	traits::{Get, IsType},
};
use frame_system::ensure_signed;
use orml_non_fungible_token::{self as orml_nft, CID};
use orml_traits::{BasicCurrency, BasicReservableCurrency};
use orml_utilities::with_transaction_result;
use primitives::Balance;
use sp_runtime::{traits::AccountIdConversion, ModuleId, RuntimeDebug};
use sp_std::vec::Vec;

mod mock;
mod tests;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub enum ClassProperty {
	/// Token can be transferred
	Transferable,
	/// Token can be burned
	Burnable,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub struct ClassData {
	/// The minimum balance to create class
	deposit: Balance,
	/// Property of token
	properties: Vec<ClassProperty>,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub struct TokenData {
	/// The minimum balance to create token
	deposit: Balance,
}

decl_event!(
	 pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		<T as orml_nft::Trait>::ClassId,
		<T as orml_nft::Trait>::TokenId,
	{
		 /// Created NFT class. \[owner, class_id\]
		 CreatedClass(AccountId, ClassId),
		 /// Minted NFT token. \[from, to, class_id, quantity\]
		 MintedToken(AccountId, AccountId, ClassId, u32),
		 /// Transfered NFT token. \[from, to, class_id, token_id\]
		 TransferedToken(AccountId, AccountId, ClassId, TokenId),
		 /// Burned NFT token. \[owner, class_id, token_id\]
		 BurnedToken(AccountId, ClassId, TokenId),
		 /// Destroyed NFT class. \[owner, class_id, dest\]
		 DestroyedClass(AccountId, ClassId, AccountId),
	}
);

decl_error! {
	/// Error for module-nft module.
	pub enum Error for Module<T: Trait> {
		/// ClassId not found
		ClassIdNotFound,
		/// The operator is not the owner of the token and has no permission
		NoPermission,
	}
}

pub trait Trait: frame_system::Trait + orml_nft::Trait + pallet_proxy::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// The minimum balance to create class
	type CreateClassDeposit: Get<Balance>;
	/// The minimum balance to create token
	type CreateTokenDeposit: Get<Balance>;
	type ConvertClassData: IsType<<Self as orml_nft::Trait>::ClassData> + IsType<ClassData>;
	type ConvertTokenData: IsType<<Self as orml_nft::Trait>::TokenData> + IsType<TokenData>;
	/// The NFT's module id
	type ModuleId: Get<ModuleId>;
	type Currency: BasicReservableCurrency<Self::AccountId, Balance = Balance>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// The minimum balance to create class
		const CreateClassDeposit: Balance = T::CreateClassDeposit::get();

		/// The minimum balance to create token
		const CreateTokenDeposit: Balance = T::CreateTokenDeposit::get();

		/// The NFT's module id
		const ModuleId: ModuleId = T::ModuleId::get();


		#[weight = 10_000]
		pub fn create_class(origin, metadata: CID, properties: Vec<ClassProperty>) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				ensure!(properties.len() <= std::mem::size_of::<ClassProperty>(), Error::<T>::NoPermission);

				let next_id = orml_nft::Module::<T>::next_class_id();
				let owner: T::AccountId = T::ModuleId::get().into_sub_account(next_id);
				let deposit = T::CreateClassDeposit::get();
				<T as Trait>::Currency::transfer(&who, &owner, deposit)?;
				<T as Trait>::Currency::reserve(&owner, deposit)?;
				//	Depends on https://github.com/paritytech/substrate/issues/7139
				//	For now, use origin as owner and skip the proxy part
				//	pallet_proxy::Module<T>::add_proxy(owner, origin, Default::default(), 0)
				let data = ClassData{deposit, properties};
				//TODO
				orml_nft::Module::<T>::create_class(&owner, metadata, <T as orml_nft::Trait>::ClassData::from(data.into()))?;

				Self::deposit_event(RawEvent::CreatedClass(who, next_id));
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn mint(origin, to: T::AccountId, class_id: <T as orml_nft::Trait>::ClassId, metadata: CID, quantity: u32) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let class_info = orml_nft::Module::<T>::classes(class_id).ok_or(Error::<T>::ClassIdNotFound)?;
				ensure!(who == class_info.owner, Error::<T>::NoPermission);
				let deposit = T::CreateTokenDeposit::get();
				<T as Trait>::Currency::reserve(&who, deposit * (quantity as u128))?;

				for _ in 0..quantity {
					//TODO
					//orml_nft::Module::<T>::mint(&to, class_id, metadata, TokenData { deposit })?;
				}

				Self::deposit_event(RawEvent::MintedToken(who, to, class_id, quantity));
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn transfer(origin, to: T::AccountId, token: (<T as orml_nft::Trait>::ClassId, <T as orml_nft::Trait>::TokenId)) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let class_info = orml_nft::Module::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
				ensure!(who == class_info.owner, Error::<T>::NoPermission);
				//TODO
				//ensure!(<class_info.data as ClassData>.properties.contains(ClassProperty::Transferable), Error::<T>::NoPermission);

				orml_nft::Module::<T>::transfer(&who, &to, token)?;

				Self::deposit_event(RawEvent::TransferedToken(who, to, token.0, token.1));
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn burn(origin, token: (<T as orml_nft::Trait>::ClassId, <T as orml_nft::Trait>::TokenId)) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let class_info = orml_nft::Module::<T>::classes(token.0).ok_or(Error::<T>::ClassIdNotFound)?;
				ensure!(who == class_info.owner, Error::<T>::NoPermission);
				//TODO
				//ensure!(T::ConvertClassData<class_info.data>.properties.contains(ClassProperty::Burnable), Error::<T>::NoPermission);

				orml_nft::Module::<T>::burn(&who, token)?;

				Self::deposit_event(RawEvent::BurnedToken(who, token.0, token.1));
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn destroy_class(origin, class_id: <T as orml_nft::Trait>::ClassId, dest: T::AccountId) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let class_info = orml_nft::Module::<T>::classes(class_id).ok_or(Error::<T>::ClassIdNotFound)?;
				ensure!(who == class_info.owner, Error::<T>::NoPermission);
				ensure!(class_info.total_issuance == 0.into(), Error::<T>::NoPermission);
				<T as Trait>::Currency::unreserve(&who, 0);

				if who == dest {
					return Ok(());
				}
				// Skip two steps until pallet_proxy is accessable
				// pallet_proxy::Module<T>::remove_proxies(owner)
				// transfer all free from origin to dest

				orml_nft::Module::<T>::destroy_class(&who, class_id)?;

				Self::deposit_event(RawEvent::DestroyedClass(who, class_id, dest));
				Ok(())
			})?;
		}
	}
}
