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

use crate::{AddressMapping, CurrencyId, CurrencyIdMapping};
use codec::Encode;
use frame_support::pallet_prelude::DispatchResult;
use primitives::{currency::TokenInfo, evm::EvmAddress, H160_POSITION_TOKEN, H160_PREFIX_TOKEN};
use sp_core::{crypto::AccountId32, H160};
use sp_io::hashing::blake2_256;
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};

pub struct MockAddressMapping;

impl AddressMapping<AccountId32> for MockAddressMapping {
	fn get_account_id(address: &H160) -> AccountId32 {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		AccountId32::from(data)
	}

	fn get_evm_address(account_id: &AccountId32) -> Option<H160> {
		let data: [u8; 32] = account_id.clone().into();
		if data.starts_with(b"evm:") {
			Some(H160::from_slice(&data[4..24]))
		} else {
			None
		}
	}

	fn get_default_evm_address(account_id: &AccountId32) -> H160 {
		let slice: &[u8] = account_id.as_ref();
		H160::from_slice(&slice[0..20])
	}

	fn get_or_create_evm_address(account_id: &AccountId32) -> H160 {
		Self::get_evm_address(account_id).unwrap_or({
			let payload = (b"evm:", account_id);
			H160::from_slice(&payload.using_encoded(blake2_256)[0..20])
		})
	}

	fn is_linked(account_id: &AccountId32, evm: &H160) -> bool {
		Self::get_or_create_evm_address(account_id) == *evm
	}
}

pub struct MockCurrencyIdMapping;

impl CurrencyIdMapping for MockCurrencyIdMapping {
	fn set_erc20_mapping(_address: EvmAddress) -> DispatchResult {
		Ok(())
	}

	fn get_evm_address(_currency_id: u32) -> Option<EvmAddress> {
		Some(EvmAddress::default())
	}

	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		currency_id.name().map(|v| v.as_bytes().to_vec())
	}

	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>> {
		currency_id.symbol().map(|v| v.as_bytes().to_vec())
	}

	fn decimals(currency_id: CurrencyId) -> Option<u8> {
		currency_id.decimals()
	}

	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress> {
		EvmAddress::try_from(v).ok()
	}

	fn decode_evm_address(v: EvmAddress) -> Option<CurrencyId> {
		let address = v.as_bytes();
		if address.starts_with(&H160_PREFIX_TOKEN) {
			address[H160_POSITION_TOKEN].try_into().map(CurrencyId::Token).ok()
		} else {
			None
		}
	}
}
