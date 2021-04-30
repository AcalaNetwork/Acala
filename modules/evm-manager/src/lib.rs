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
	CurrencyId, DexShare, MIRRORED_LP_TOKENS_ADDRESS_START, SYSTEM_CONTRACT_ADDRESS_PREFIX,
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
		match currency_id {
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
		}
	}

	// Returns the symbol associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>> {
		match currency_id {
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
		}
	}

	// Encode the CurrencyId to [u8; 32].
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn encode_currency_id(val: CurrencyId) -> Option<[u8; 32]> {
		let mut bytes = [0u8; 32];
		match val {
			CurrencyId::Token(token) => {
				bytes[31] = token as u8;
			}
			CurrencyId::DexShare(left, right) => {
				bytes[11] = 1;
				match left {
					DexShare::Token(_) => {
						let id: u32 = left.into();
						bytes[12..16].copy_from_slice(&id.to_be_bytes()[..])
					}
					DexShare::Erc20(address) => {
						let id: u32 = left.into();
						if CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).is_some() {
							bytes[12..16].copy_from_slice(&id.to_be_bytes()[..])
						} else {
							return None;
						}
					}
				}
				match right {
					DexShare::Token(_) => {
						let id: u32 = right.into();
						bytes[16..20].copy_from_slice(&id.to_be_bytes()[..])
					}
					DexShare::Erc20(address) => {
						let id: u32 = right.into();
						if CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).is_some() {
							bytes[16..20].copy_from_slice(&id.to_be_bytes()[..])
						} else {
							return None;
						}
					}
				}
			}
			CurrencyId::Erc20(address) => {
				bytes[11] = 2;
				bytes[12..32].copy_from_slice(&address[..]);
			}
		}
		Some(bytes)
	}

	// Decode the CurrencyId from [u8; 32].
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_currency_id(v: &[u8; 32]) -> Option<CurrencyId> {
		// token/dex/erc20 flag(1 byte) | token(1 byte)
		// token/dex/erc20 flag(1 byte) | dex left(4 byte) | dex right(4 byte)
		// token/dex/erc20 flag(1 byte) | evm address(20 byte)
		//
		// v[11] = 0: token
		// - v[31] = token(1 byte)
		//
		// v[11] = 1: dex share
		// - v[12..16] = dex left(4 byte)
		// - v[16..20] = dex right(4 byte)
		//
		// v[11] = 2: erc20
		// - v[12..32] = evm address(20 byte)

		if !v.starts_with(&[0u8; 11][..]) {
			return None;
		}

		// DEX share
		if v[11] == 1 && v.ends_with(&[0u8; 12][..]) {
			let left = {
				if v[12..15] == [0u8; 3] {
					// Token
					v[15].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&v[12..16]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;
			let right = {
				if v[16..19] == [0u8; 3] {
					// Token
					v[19].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&v[16..20]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;

			return Some(CurrencyId::DexShare(left, right));
		}

		// Token or Erc20
		(*v).try_into().ok()
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

				let mut data = [0u8; 20];
				data[4..20].copy_from_slice(&MIRRORED_LP_TOKENS_ADDRESS_START.to_be_bytes());
				let addr_flag = EvmAddress::from_slice(&data);
				let addr = EvmAddress::from_low_u64_be(u64::from(symbol_0) << 32 | u64::from(symbol_1));

				Some(addr_flag | addr)
			}
			// Token or Erc20
			_ => EvmAddress::try_from(v).ok(),
		}
	}

	// Decode the CurrencyId from EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(v: EvmAddress) -> Option<CurrencyId> {
		let address = v.as_bytes();
		if !address.starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX) {
			return None;
		}

		// Token
		// MIRRORED_TOKENS_ADDRESS_START = 0x1000000
		let mut token_prefix = [0u8; 19];
		token_prefix[16] = 1;
		if address.starts_with(&token_prefix) {
			return address[19].try_into().map(CurrencyId::Token).ok();
		}

		// DexShare
		// MIRRORED_LP_TOKENS_ADDRESS_START = 0x10000000000000000
		let mut lp_token_prefix = [0u8; 12];
		lp_token_prefix[11] = 1;
		if address.starts_with(&lp_token_prefix) {
			let left = {
				if address[12..15] == [0u8; 3] {
					// Token
					address[15].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&address[12..16]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;
			let right = {
				if address[16..19] == [0u8; 3] {
					// Token
					address[19].try_into().map(DexShare::Token).ok()
				} else {
					// Erc20
					let mut id = [0u8; 4];
					id.copy_from_slice(&address[16..20]);
					let id = u32::from_be_bytes(id);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;

			return Some(CurrencyId::DexShare(left, right));
		}

		// Erc20 does not need to be decoded.
		None
	}
}
