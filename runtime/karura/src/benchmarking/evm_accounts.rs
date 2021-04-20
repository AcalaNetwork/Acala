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

use crate::{dollar, AccountId, EvmAccounts, Runtime, KAR};

use super::utils::set_aca_balance;
use codec::Encode;
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_io::hashing::keccak_256;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn alice() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn bob_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&bob());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

runtime_benchmarks! {
	{ Runtime, module_evm_accounts }

	_ {}

	claim_account {
		let caller: AccountId = account("caller", 0, SEED);
		let eth: AccountId = account("eth", 0, SEED);
		set_aca_balance(&bob_account_id(), 1_000 * dollar(KAR));
	}: _(RawOrigin::Signed(caller), EvmAccounts::eth_address(&alice()), EvmAccounts::eth_sign(&alice(), &caller.encode(), &[][..]))

	claim_default_account {
		let caller = whitelisted_caller();
  }: _(RawOrigin::Signed(caller))
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_claim_account() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_claim_account());
		});
	}

	#[test]
	fn test_claim_default_account() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_claim_account());
		});
	}
}
