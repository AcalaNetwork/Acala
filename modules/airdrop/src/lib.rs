#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_event, decl_module, decl_storage, transactional};
use frame_system::{self as system, ensure_root};
use primitives::{AirDropCurrencyId, Balance};
use sp_runtime::traits::StaticLookup;

mod mock;
mod tests;

pub trait Config: system::Config {
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as AirDrop {
		AirDrops get(fn airdrops): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) AirDropCurrencyId => Balance;
	}

	add_extra_genesis {
		config(airdrop_accounts): Vec<(T::AccountId, AirDropCurrencyId, Balance)>;

		build(|config: &GenesisConfig<T>| {
			config.airdrop_accounts.iter().for_each(|(account_id, airdrop_currency_id, initial_balance)| {
				<AirDrops<T>>::mutate(account_id, airdrop_currency_id, | amount | *amount += *initial_balance)
			})
		})
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Config>::AccountId,
		AirDropCurrencyId = AirDropCurrencyId,
		Balance = Balance,
	{
		/// \[to, currency_id, amount\]
		Airdrop(AccountId, AirDropCurrencyId, Balance),
		/// \[to, currency_id, amount\]
		UpdateAirdrop(AccountId, AirDropCurrencyId, Balance),
	}
);

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 10_000]
		#[transactional]
		pub fn airdrop(
			origin,
			to: <T::Lookup as StaticLookup>::Source,
			currency_id: AirDropCurrencyId,
			amount: Balance,
		) {
			ensure_root(origin)?;
			let to = T::Lookup::lookup(to)?;
			<AirDrops<T>>::mutate(&to, currency_id, |balance| *balance += amount);
			Self::deposit_event(RawEvent::Airdrop(to, currency_id, amount));
		}

		#[weight = 10_000]
		#[transactional]
		pub fn update_airdrop(
			origin,
			to: <T::Lookup as StaticLookup>::Source,
			currency_id: AirDropCurrencyId,
			amount: Balance,
		) {
			ensure_root(origin)?;
			let to = T::Lookup::lookup(to)?;
			<AirDrops<T>>::insert(&to, currency_id, amount);
			Self::deposit_event(RawEvent::UpdateAirdrop(to, currency_id, amount));
		}
	}
}

impl<T: Config> Module<T> {}
