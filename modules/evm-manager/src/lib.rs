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
	currency::GetDecimals,
	evm::{Erc20Info, EvmAddress},
	CurrencyId, DexShare,
};
use sp_std::convert::TryInto;

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

	fn decimals(currency_id: CurrencyId) -> Option<u8> {
		match currency_id {
			CurrencyId::Token(_) => currency_id.decimals(),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let decimals_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => {
						CurrencyIdMap::<T>::get(Into::<u32>::into(CurrencyId::Erc20(address))).map(|v| v.decimals)
					}
				};
				let decimals_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => {
						CurrencyIdMap::<T>::get(Into::<u32>::into(CurrencyId::Erc20(address))).map(|v| v.decimals)
					}
				};
				if decimals_0.is_none() || decimals_1.is_none() {
					return None;
				}
				Some(sp_std::cmp::max(decimals_0.unwrap(), decimals_1.unwrap()))
			}
			CurrencyId::Erc20(address) => {
				CurrencyIdMap::<T>::get(Into::<u32>::into(CurrencyId::Erc20(address))).map(|v| v.decimals)
			}
		}
	}

	fn u256_to_currency_id(v: &[u8; 32]) -> Option<CurrencyId> {
		// tag: u8 + u32 + u32 = 1 + 4 + 4
		if !v.starts_with(&[0u8; 23][..]) {
			return None;
		}

		// DEX share
		if v[23] == 1 {
			let left = {
				if v[24..27] == [0u8; 3] {
					// Token
					v[27].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&v[24..28]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			};
			let right = {
				if v[28..31] == [0u8; 3] {
					// Token
					v[31].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&v[28..32]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			};
			if left.is_none() || right.is_none() {
				return None;
			}
			Some(CurrencyId::DexShare(left.unwrap(), right.unwrap()))
		} else {
			(*v).try_into().ok()
		}
	}
}
