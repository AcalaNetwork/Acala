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

use frame_support::{ensure, pallet_prelude::*, require_transactional, traits::Currency};
use module_support::{CurrencyIdMapping, EVMBridge, InvokeContext};
use primitives::{
	evm::{Erc20Info, EvmAddress},
	CurrencyId,
};

mod mock;
mod tests;

pub use module::*;

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Currency: Currency<Self::AccountId>;
		type EVMBridge: EVMBridge<Self::AccountId, BalanceOf<Self>>;
	}

	/// Error for evm accounts module.
	#[pallet::error]
	pub enum Error<T> {
		/// CurrencyId existed
		CurrencyIdExisted,
	}

	#[pallet::storage]
	#[pallet::getter(fn currency_id_map)]
	pub type CurrencyIdMap<T: Config> = StorageMap<_, Twox64Concat, u32, Erc20Info>;

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
		let currency_id: u32 = CurrencyId::Erc20(address).into();

		CurrencyIdMap::<T>::mutate(currency_id, |maybe_erc20_info| -> DispatchResult {
			if let Some(erc20_info) = maybe_erc20_info.as_mut() {
				ensure!(erc20_info.address == address, Error::<T>::CurrencyIdExisted);
			} else {
				let info = Erc20Info {
					address,
					name: T::EVMBridge::name(InvokeContext {
						contract: address,
						sender: Default::default(),
						origin: Default::default(),
					})?,
					symbol: T::EVMBridge::symbol(InvokeContext {
						contract: address,
						sender: Default::default(),
						origin: Default::default(),
					})?,
					decimals: T::EVMBridge::decimals(InvokeContext {
						contract: address,
						sender: Default::default(),
						origin: Default::default(),
					})?,
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
