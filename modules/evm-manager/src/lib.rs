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

//! # Evm Manager Module
//!
//! ## Overview
//!
//! Evm Manager module provide a two way mapping between CurrencyId and
//! ERC20 address so user can use ERC20 address as LP token.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{ensure, pallet_prelude::*, require_transactional};
use module_support::CurrencyIdMapping;
use primitives::{
	evm::{ERC20Info, EvmAddress},
	CurrencyId,
};

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {}

	/// Error for evm accounts module.
	#[pallet::error]
	pub enum Error<T> {
		/// CurrencyId existed
		CurrencyIdExisted,
	}

	#[pallet::storage]
	#[pallet::getter(fn currency_id_map)]
	pub type CurrencyIdMap<T: Config> = StorageMap<_, Twox64Concat, u32, ERC20Info>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {}

pub struct EvmCurrencyIdMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> CurrencyIdMapping for EvmCurrencyIdMapping<T> {
	#[require_transactional]
	fn set_erc20_mapping(address: EvmAddress) -> DispatchResult {
		let currency_id: u32 = CurrencyId::ERC20(address).into();

		CurrencyIdMap::<T>::mutate(currency_id, |maybe_erc20_info| -> DispatchResult {
			if let Some(erc20_info) = maybe_erc20_info.as_mut() {
				ensure!(erc20_info.address == address, Error::<T>::CurrencyIdExisted);
			} else {
				let info = ERC20Info {
					address,
					name: "test".to_string(),   // TODO: get from evm-bridge
					symbol: "test".to_string(), // TODO: get from evm-bridge
					decimals: 10,               // TODO: get from evm-bridge
				};

				*maybe_erc20_info = Some(info);
			}
			Ok(())
		})
	}

	fn get_evm_address(currency_id: u32) -> Option<EvmAddress> {
		CurrencyIdMap::<T>::get(currency_id).map(|v| v.address)
	}
}
