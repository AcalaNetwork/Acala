#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_event, decl_module, decl_storage, Parameter};
use sp_runtime::traits::{AtLeast32Bit, MaybeSerializeDeserialize, Member};
use system::ensure_root;

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type AirDropCurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;
	type Balance: Parameter + Member + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize;
}

decl_storage! {
	trait Store for Module<T: Trait> as AirDrop {
		AirDrops get(fn airdrops): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) T::AirDropCurrencyId => T::Balance;
	}

	add_extra_genesis {
		config(airdrop_accounts): Vec<(T::AccountId, T::AirDropCurrencyId, T::Balance)>;

		build(|config: &GenesisConfig<T>| {
			config.airdrop_accounts.iter().for_each(|(account_id, airdrop_currency_id, initial_balance)| {
				<AirDrops<T>>::mutate(account_id, airdrop_currency_id, | amount | *amount = *initial_balance)
			})
		})
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::AirDropCurrencyId,
		<T as Trait>::Balance,
	{
		Airdrop(AccountId, AirDropCurrencyId, Balance),
		UpdateAirdrop(AccountId, AirDropCurrencyId, Balance),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn airdrop(
			origin,
			to: T::AccountId,
			currency_id: T::AirDropCurrencyId,
			amount: T::Balance,
		) {
			ensure_root(origin)?;
			<AirDrops<T>>::mutate(&to, currency_id, |balance| *balance += amount);
			Self::deposit_event(RawEvent::Airdrop(to, currency_id, amount));
		}

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn update_airdrop(
			origin,
			to: T::AccountId,
			currency_id: T::AirDropCurrencyId,
			amount: T::Balance,
		) {
			ensure_root(origin)?;
			<AirDrops<T>>::insert(&to, currency_id, amount);
			Self::deposit_event(RawEvent::UpdateAirdrop(to, currency_id, amount));
		}
	}
}

impl<T: Trait> Module<T> {}
