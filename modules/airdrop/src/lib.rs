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
#![allow(clippy::unused_unit)]

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

impl<T: Config> Pallet<T> {}
