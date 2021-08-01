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

//! Unit tests for the genesis resources data.

#![cfg(test)]

use acala_primitives::{AccountId, Balance, BlockNumber};

#[test]
#[cfg(feature = "with-karura-runtime")]
fn karura_foundation_accounts_config_is_correct() {
	use sp_core::crypto::Ss58Codec;

	let karura_foundation_accounts = karura_runtime::KaruraFoundationAccounts::get();
	assert!(karura_foundation_accounts
		.contains(&AccountId::from_string("tij5W2NzmtxxAbwudwiZpif9ScmZfgFYdzrJWKYq6oNbSNH").unwrap()),);
	assert!(karura_foundation_accounts
		.contains(&AccountId::from_string("pndshZqDAC9GutDvv7LzhGhgWeGv5YX9puFA8xDidHXCyjd").unwrap()),);
}

#[test]
fn check_karura_vesting() {
	let vesting_json = &include_bytes!("../../../../resources/karura-vesting-KAR.json")[..];
	let vesting: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)> =
		serde_json::from_slice(vesting_json).unwrap();

	// ensure no duplicates exist.
	let unique_vesting_accounts = vesting
		.iter()
		.map(|(x, _, _, _, _)| x)
		.cloned()
		.collect::<std::collections::BTreeSet<_>>();
	assert_eq!(unique_vesting_accounts.len(), vesting.len());
}

#[test]
fn check_acala_vesting() {
	let vesting_json = &include_bytes!("../../../../resources/acala-vesting-ACA.json")[..];
	let vesting: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)> =
		serde_json::from_slice(vesting_json).unwrap();

	// ensure no duplicates exist.
	let unique_vesting_accounts = vesting
		.iter()
		.map(|(x, _, _, _, _)| x)
		.cloned()
		.collect::<std::collections::BTreeSet<_>>();
	assert_eq!(unique_vesting_accounts.len(), vesting.len());
}

#[test]
fn check_karura_allocation() {
	let allocation_json = &include_bytes!("../../../../resources/karura-allocation-KAR.json")[..];
	let _: Vec<(AccountId, Balance)> = serde_json::from_slice(allocation_json).unwrap();
}

#[test]
fn check_acala_allocation() {
	let allocation_json = &include_bytes!("../../../../resources/acala-allocation-ACA.json")[..];
	let _: Vec<(AccountId, Balance)> = serde_json::from_slice(allocation_json).unwrap();
}

#[test]
fn check_acala_airdrop() {
	let airdrop_json = &include_bytes!("../../../../resources/mandala-airdrop-ACA.json")[..];
	let _: Vec<(AccountId, Balance)> = serde_json::from_slice(airdrop_json).unwrap();
}

#[test]
fn check_nfts() {
	let nfts_json = &include_bytes!("../../../../resources/mandala-airdrop-NFT.json")[..];
	let _: Vec<(
		AccountId,
		Vec<u8>,
		module_nft::ClassData<Balance>,
		Vec<(Vec<u8>, module_nft::TokenData<Balance>, Vec<AccountId>)>,
	)> = serde_json::from_slice(nfts_json).unwrap();
}
