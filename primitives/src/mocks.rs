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

use codec::Encode;
use sp_core::{crypto::AccountId32, H160};
use sp_io::hashing::blake2_256;

pub struct MockAddressMapping;

impl crate::evm::AddressMapping<AccountId32> for MockAddressMapping {
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
		H160::from_slice(account_id.as_ref())
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
