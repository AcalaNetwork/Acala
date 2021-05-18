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
//! Evm manager module provides common support features for Evm, including:
//! - A two way mapping between `u32` and `Erc20 address` so user can use Erc20 address as LP token.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{ensure, pallet_prelude::*, require_transactional, traits::Currency};
use module_support::{CurrencyIdMapping, EVMBridge, InvokeContext};
use primitives::{
	currency::TokenInfo,
	evm::{Erc20Info, EvmAddress},
	*,
};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
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

	/// Mapping between u32 and Erc20 address.
	/// Erc20 address is 20 byte, take the first 4 non-zero bytes, if it is less
	/// than 4, add 0 to the left.
	///
	/// map u32 => Option<Erc20Info>
	#[pallet::storage]
	#[pallet::getter(fn currency_id_map)]
	pub type CurrencyIdMap<T: Config> = StorageMap<_, Twox64Concat, u32, Erc20Info, OptionQuery>;

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
	// Use first 4 non-zero bytes as u32 to the mapping between u32 and evm address.
	// Take the first 4 non-zero bytes, if it is less than 4, add 0 to the left.
	#[require_transactional]
	fn set_erc20_mapping(address: EvmAddress) -> DispatchResult {
		CurrencyIdMap::<T>::mutate(
			Into::<u32>::into(DexShare::Erc20(address)),
			|maybe_erc20_info| -> DispatchResult {
				if let Some(erc20_info) = maybe_erc20_info.as_mut() {
					ensure!(erc20_info.address == address, Error::<T>::CurrencyIdExisted);
				} else {
					let invoke_context = InvokeContext {
						contract: address,
						sender: Default::default(),
						origin: Default::default(),
					};

					let info = Erc20Info {
						address,
						name: T::EVMBridge::name(invoke_context)?,
						symbol: T::EVMBridge::symbol(invoke_context)?,
						decimals: T::EVMBridge::decimals(invoke_context)?,
					};

					*maybe_erc20_info = Some(info);
				}
				Ok(())
			},
		)
	}

	// Returns the EvmAddress associated with a given u32.
	fn get_evm_address(currency_id: u32) -> Option<EvmAddress> {
		CurrencyIdMap::<T>::get(currency_id).map(|v| v.address)
	}

	// Returns the name associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let name = match currency_id {
			CurrencyId::Token(_) => currency_id.name().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let name_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.name),
				}?;
				let name_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_1))
						.filter(|v| v.address == address)
						.map(|v| v.name),
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP "[..]);
				vec.extend_from_slice(&name_0);
				vec.extend_from_slice(&b" - ".to_vec());
				vec.extend_from_slice(&name_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.name),
			CurrencyId::ChainSafe(_) => None,
		}?;

		// More than 32 bytes will be truncated.
		if name.len() > 32 {
			Some(name[..32].to_vec())
		} else {
			Some(name)
		}
	}

	// Returns the symbol associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let symbol = match currency_id {
			CurrencyId::Token(_) => currency_id.symbol().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let token_symbol_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.symbol),
				}?;
				let token_symbol_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_1))
						.filter(|v| v.address == address)
						.map(|v| v.symbol),
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP_"[..]);
				vec.extend_from_slice(&token_symbol_0);
				vec.extend_from_slice(&b"_".to_vec());
				vec.extend_from_slice(&token_symbol_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.symbol),
			CurrencyId::ChainSafe(_) => None,
		}?;

		// More than 32 bytes will be truncated.
		if symbol.len() > 32 {
			Some(symbol[..32].to_vec())
		} else {
			Some(symbol)
		}
	}

	// Returns the decimals associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn decimals(currency_id: CurrencyId) -> Option<u8> {
		match currency_id {
			CurrencyId::Token(_) => currency_id.decimals(),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let decimals_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.decimals),
				}?;
				let decimals_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_1))
						.filter(|v| v.address == address)
						.map(|v| v.decimals),
				}?;

				Some(sp_std::cmp::max(decimals_0, decimals_1))
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.decimals),
			CurrencyId::ChainSafe(_) => None,
		}
	}

	// Encode the CurrencyId to EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress> {
		match v {
			CurrencyId::DexShare(left, right) => {
				let symbol_0 = match left {
					DexShare::Token(_) => Some(left.into()),
					DexShare::Erc20(address) => {
						let id: u32 = left.into();
						CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).map(|_| id)
					}
				}?;
				let symbol_1 = match right {
					DexShare::Token(_) => Some(right.into()),
					DexShare::Erc20(address) => {
						let id: u32 = right.into();
						CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).map(|_| id)
					}
				}?;

				let mut prefix = EvmAddress::default();
				prefix[0..H160_PREFIX_DEXSHARE.len()].copy_from_slice(&H160_PREFIX_DEXSHARE);
				Some(prefix | EvmAddress::from_low_u64_be(u64::from(symbol_0) << 32 | u64::from(symbol_1)))
			}

			// Token or Erc20 or ChainSafe
			_ => EvmAddress::try_from(v).ok(),
		}
	}

	// Decode the CurrencyId from EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(addr: EvmAddress) -> Option<CurrencyId> {
		let address = addr.as_bytes();

		// Token
		if address.starts_with(&H160_PREFIX_TOKEN) {
			return address[H160_POSITION_TOKEN].try_into().map(CurrencyId::Token).ok();
		}

		// DexShare
		if address.starts_with(&H160_PREFIX_DEXSHARE) {
			let left = {
				if address[H160_POSITION_DEXSHARE_LEFT].starts_with(&[0u8; 3]) {
					// Token
					address[H160_POSITION_DEXSHARE_LEFT][3]
						.try_into()
						.map(DexShare::Token)
						.ok()
				} else {
					// Erc20
					let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_LEFT].try_into().ok()?);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;
			let right = {
				if address[H160_POSITION_DEXSHARE_RIGHT].starts_with(&[0u8; 3]) {
					// Token
					address[H160_POSITION_DEXSHARE_RIGHT][3]
						.try_into()
						.map(DexShare::Token)
						.ok()
				} else {
					// Erc20
					let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_RIGHT].try_into().ok()?);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;

			return Some(CurrencyId::DexShare(left, right));
		}

		// Erc20
		let id = Into::<u32>::into(DexShare::Erc20(addr));
		CurrencyIdMap::<T>::get(id).map(|v| CurrencyId::Erc20(v.address))
	}
}
